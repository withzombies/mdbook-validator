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

/// Ensure the book is built exactly once before any tests check output.
/// Uses `OnceLock` for thread-safe, zero-cost synchronization.
static BUILD_RESULT: OnceLock<bool> = OnceLock::new();

fn ensure_book_built() {
    let _ = BUILD_RESULT.get_or_init(|| {
        let book_path = e2e_book_path();

        // Create a modified book.toml with absolute binary path
        let book_toml_path = book_path.join("book.toml");
        let original_toml = fs::read_to_string(&book_toml_path).expect("Read book.toml");
        let modified_toml = rewrite_book_toml_for_temp(&original_toml);

        // Debug: print binary path and modified toml
        let binary_path = validator_binary_path();
        println!("DEBUG: Binary path = {}", binary_path.display());
        println!("DEBUG: Binary exists = {}", binary_path.exists());
        println!(
            "DEBUG: Modified book.toml command line:\n{}",
            modified_toml
                .lines()
                .find(|l| l.contains("command ="))
                .unwrap_or("NOT FOUND")
        );

        fs::write(&book_toml_path, &modified_toml).expect("Write modified book.toml");

        let output = Command::new("mdbook")
            .args(["build", book_path.to_str().expect("valid path")])
            .output()
            .expect("Failed to execute mdbook - is mdbook installed?");

        // Restore original book.toml
        fs::write(&book_toml_path, &original_toml).expect("Restore original book.toml");

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

    let book_path = e2e_book_path();

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
