//! End-to-end integration tests for mdbook-validator
//!
//! These tests run `mdbook build` against a real test book and verify:
//! 1. Build succeeds with valid examples
//! 2. Markers are stripped from output
//! 3. Invalid examples fail with expected errors
//!
//! Requires: mdbook CLI installed, Docker running
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_fun_call
)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Returns the path to the E2E test book
fn e2e_book_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/e2e-book")
}

/// Returns the path to the mdbook-validator binary
fn validator_binary_path() -> PathBuf {
    // Use CARGO_BIN_EXE_mdbook-validator which cargo sets during test compilation
    // This points to the actual binary that cargo will build for the test
    PathBuf::from(env!("CARGO_BIN_EXE_mdbook-validator"))
}

/// Rewrite book.toml to use absolute binary path instead of cargo run
fn rewrite_book_toml_for_temp(content: &str) -> String {
    let binary_path = validator_binary_path();
    content.replace(
        "command = \"cargo run --quiet --\"",
        &format!("command = \"{}\"", binary_path.display()),
    )
}

/// Stores the temp directory path so tests can access the output
static TEMP_BOOK_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Ensure the book is built exactly once before any tests check output.
/// Uses `OnceLock` for thread-safe, zero-cost synchronization.
/// Builds in a temp directory to avoid modifying source files.
static BUILD_RESULT: OnceLock<bool> = OnceLock::new();

fn ensure_book_built() {
    let _ = BUILD_RESULT.get_or_init(|| {
        use std::env::temp_dir;

        let book_path = e2e_book_path();

        // Create a temporary book directory to avoid modifying source files
        let temp_book = temp_dir().join(format!("e2e-valid-book-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_book); // Clean up from previous runs

        // Copy the entire e2e-book to temp directory
        copy_dir_recursive(&book_path, &temp_book).expect("Copy e2e-book to temp");

        // Rewrite book.toml in temp directory to use absolute binary path
        let book_toml_path = temp_book.join("book.toml");
        let original_toml = fs::read_to_string(&book_toml_path).expect("Read book.toml");
        let modified_toml = rewrite_book_toml_for_temp(&original_toml);
        fs::write(&book_toml_path, &modified_toml).expect("Write modified book.toml");

        // Symlink validators directory (needed for validator scripts)
        #[cfg(unix)]
        {
            let validators_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("validators");
            let validators_dst = temp_book.join("validators");
            // Remove the copied validators dir and replace with symlink
            let _ = fs::remove_dir_all(&validators_dst);
            std::os::unix::fs::symlink(&validators_src, &validators_dst)
                .expect("Symlink validators");
        }

        // Store temp path for other tests to access output
        let _ = TEMP_BOOK_PATH.set(temp_book.clone());

        let output = Command::new("mdbook")
            .args(["build", temp_book.to_str().expect("valid path")])
            .output()
            .expect("Failed to execute mdbook - is mdbook installed?");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        println!("mdbook build stdout:\n{stdout}");
        println!("mdbook build stderr:\n{stderr}");

        let success = output.status.success();

        assert!(
            success,
            "mdbook build failed with exit code {:?}:\nstdout: {}\nstderr: {}",
            output.status.code(),
            stdout,
            stderr
        );

        success
    });
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}

/// Test: Valid book builds successfully with real mdbook command
///
/// This is the primary E2E test that verifies:
/// - mdbook preprocessor integration works
/// - All validators (osquery, sqlite, osquery-config) execute correctly
/// - Build completes without errors
#[test]
fn e2e_valid_book_builds_successfully() {
    ensure_book_built();

    let build_success = BUILD_RESULT
        .get()
        .expect("Build should have been initialized");
    assert!(build_success, "mdbook build should have succeeded");
}

/// Test: Output HTML has no validation markers
///
/// Verifies that SETUP, ASSERT, EXPECT markers and @@ hidden lines
/// are properly stripped from the final HTML output.
#[test]
fn e2e_output_has_no_markers() {
    ensure_book_built();

    let book_path = TEMP_BOOK_PATH.get().expect("Temp book path should be set");

    // Read the generated HTML
    let html_path = book_path.join("book/valid-examples.html");
    let content = std::fs::read_to_string(&html_path).expect(&format!(
        "Failed to read output HTML at {}",
        html_path.display()
    ));

    // Check for marker absence
    assert!(
        !content.contains("<!--SETUP"),
        "Output should not contain SETUP marker.\nFound in: {}",
        html_path.display()
    );
    assert!(
        !content.contains("<!--ASSERT"),
        "Output should not contain ASSERT marker.\nFound in: {}",
        html_path.display()
    );
    assert!(
        !content.contains("<!--EXPECT"),
        "Output should not contain EXPECT marker.\nFound in: {}",
        html_path.display()
    );

    // Check that visible content is preserved
    assert!(
        content.contains("SELECT uid, username FROM users"),
        "Output should contain visible SQL query.\nContent:\n{}",
        &content[..content.len().min(2000)]
    );
    assert!(
        content.contains("SELECT * FROM items"),
        "Output should contain sqlite query.\nContent:\n{}",
        &content[..content.len().min(2000)]
    );
    assert!(
        content.contains("logger_path"),
        "Output should contain osquery config content.\nContent:\n{}",
        &content[..content.len().min(2000)]
    );

    println!("E2E marker stripping test passed!");
}

/// Test: Hidden blocks are validated but removed from output
///
/// Verifies that code blocks with the `hidden` attribute:
/// 1. Are validated (build succeeds, so validation passed)
/// 2. Are completely removed from the HTML output
/// 3. Can set up state for subsequent visible blocks
#[test]
fn e2e_hidden_blocks_not_in_output() {
    ensure_book_built();

    let book_path = TEMP_BOOK_PATH.get().expect("Temp book path should be set");

    // Read the generated HTML
    let html_path = book_path.join("book/valid-examples.html");
    let content = std::fs::read_to_string(&html_path).expect(&format!(
        "Failed to read output HTML at {}",
        html_path.display()
    ));

    // The hidden block contains "XYZ_HIDDEN_BLOCK_CONTENT_789" - this should NOT appear in output
    // (This unique marker only appears inside the hidden code block, not in surrounding text)
    assert!(
        !content.contains("XYZ_HIDDEN_BLOCK_CONTENT_789"),
        "Hidden block content should NOT appear in output.\n\
         The marker 'XYZ_HIDDEN_BLOCK_CONTENT_789' was found but should have been removed.\n\
         File: {}",
        html_path.display()
    );

    // The visible block that follows the hidden block should still be present
    // It queries the table created by the hidden block
    assert!(
        content.contains("SELECT COUNT(*) as count FROM hidden_test"),
        "Visible block following hidden block should be in output.\n\
         File: {}",
        html_path.display()
    );

    // The section heading should still be there
    assert!(
        content.contains("Hidden Block (Validated but Not Shown)"),
        "Section heading should still be in output.\n\
         File: {}",
        html_path.display()
    );

    println!("E2E hidden block test passed - hidden content removed, visible content preserved!");
}

/// Test: Invalid shellcheck script fails with SC2086 error
///
/// Verifies that:
/// - mdbook build fails when shellcheck finds issues
/// - Error output contains SC2086 (unquoted variable)
/// - The failure is clear and actionable
///
/// Uses a temporary book directory to avoid race conditions with other tests.
#[test]
fn e2e_invalid_shellcheck_fails_with_sc2086() {
    use std::env::temp_dir;

    let book_path = e2e_book_path();
    let invalid_md_path = book_path.join("src/invalid-shellcheck.md");

    // Verify the invalid example file exists
    assert!(
        invalid_md_path.exists(),
        "Invalid shellcheck fixture should exist at {}",
        invalid_md_path.display()
    );

    // Create a temporary book directory to avoid race conditions
    let temp_book = temp_dir().join(format!("e2e-invalid-shellcheck-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_book); // Clean up from previous runs
    fs::create_dir_all(temp_book.join("src")).expect("Should create temp book src dir");

    // Copy book.toml and rewrite to use absolute binary path
    let book_toml = fs::read_to_string(book_path.join("book.toml")).expect("Read book.toml");
    let book_toml = rewrite_book_toml_for_temp(&book_toml);
    fs::write(temp_book.join("book.toml"), &book_toml).expect("Write temp book.toml");

    // Create SUMMARY.md with only the invalid file
    fs::write(
        temp_book.join("src/SUMMARY.md"),
        "# Summary\n\n- [Invalid Shellcheck](./invalid-shellcheck.md)\n",
    )
    .expect("Write temp SUMMARY.md");

    // Copy invalid-shellcheck.md to temp book
    fs::copy(
        &invalid_md_path,
        temp_book.join("src/invalid-shellcheck.md"),
    )
    .expect("Copy invalid-shellcheck.md");

    // Symlink validators directory (needed for validator scripts)
    #[cfg(unix)]
    {
        let validators_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("validators");
        let validators_dst = temp_book.join("validators");
        std::os::unix::fs::symlink(&validators_src, &validators_dst).expect("Symlink validators");
    }

    // Run mdbook build - should fail
    let output = Command::new("mdbook")
        .args(["build", temp_book.to_str().expect("valid path")])
        .output()
        .expect("Failed to execute mdbook");

    // Clean up temp directory
    let _ = fs::remove_dir_all(&temp_book);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Invalid shellcheck test stdout:\n{stdout}");
    println!("Invalid shellcheck test stderr:\n{stderr}");

    // Build should fail
    assert!(
        !output.status.success(),
        "mdbook build should fail with invalid shellcheck example.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Error should mention SC2086 (unquoted variable)
    let combined_output = format!("{stdout}{stderr}");
    assert!(
        combined_output.contains("SC2086"),
        "Error output should contain SC2086 (unquoted variable).\nstdout: {stdout}\nstderr: {stderr}"
    );

    println!("E2E invalid shellcheck test passed - correctly detected SC2086!");
}

/// Test: Invalid Python script fails with `SyntaxError`
///
/// Verifies that:
/// - mdbook build fails when Python has syntax errors
/// - Error output contains `SyntaxError`
/// - The failure is clear and actionable
///
/// Uses a temporary book directory to avoid race conditions with other tests.
#[test]
fn e2e_invalid_python_fails_with_syntax_error() {
    use std::env::temp_dir;

    let book_path = e2e_book_path();
    let invalid_md_path = book_path.join("src/invalid-python.md");

    // Verify the invalid example file exists
    assert!(
        invalid_md_path.exists(),
        "Invalid python fixture should exist at {}",
        invalid_md_path.display()
    );

    // Create a temporary book directory to avoid race conditions
    let temp_book = temp_dir().join(format!("e2e-invalid-python-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_book);
    fs::create_dir_all(temp_book.join("src")).expect("Should create temp book src dir");

    // Copy book.toml and rewrite to use absolute binary path
    let book_toml = fs::read_to_string(book_path.join("book.toml")).expect("Read book.toml");
    let book_toml = rewrite_book_toml_for_temp(&book_toml);
    fs::write(temp_book.join("book.toml"), &book_toml).expect("Write temp book.toml");

    // Create SUMMARY.md with only the invalid file
    fs::write(
        temp_book.join("src/SUMMARY.md"),
        "# Summary\n\n- [Invalid Python](./invalid-python.md)\n",
    )
    .expect("Write temp SUMMARY.md");

    // Copy invalid-python.md to temp book
    fs::copy(&invalid_md_path, temp_book.join("src/invalid-python.md"))
        .expect("Copy invalid-python.md");

    // Symlink validators directory
    #[cfg(unix)]
    {
        let validators_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("validators");
        let validators_dst = temp_book.join("validators");
        std::os::unix::fs::symlink(&validators_src, &validators_dst).expect("Symlink validators");
    }

    // Run mdbook build - should fail
    let output = Command::new("mdbook")
        .args(["build", temp_book.to_str().expect("valid path")])
        .output()
        .expect("Failed to execute mdbook");

    // Clean up temp directory
    let _ = fs::remove_dir_all(&temp_book);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Invalid python test stdout:\n{stdout}");
    println!("Invalid python test stderr:\n{stderr}");

    // Build should fail
    assert!(
        !output.status.success(),
        "mdbook build should fail with invalid python example.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Error should mention SyntaxError
    let combined_output = format!("{stdout}{stderr}");
    assert!(
        combined_output.contains("SyntaxError"),
        "Error output should contain SyntaxError.\nstdout: {stdout}\nstderr: {stderr}"
    );

    println!("E2E invalid python test passed - correctly detected SyntaxError!");
}
