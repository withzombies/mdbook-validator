//! End-to-end integration tests for mdbook-validator
//!
//! These tests run `mdbook build` against a real test book and verify:
//! 1. Build succeeds with valid examples
//! 2. Markers are stripped from output
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

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Returns the path to the E2E test book
fn e2e_book_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/e2e-book")
}

/// Ensure the book is built exactly once before any tests check output.
/// Uses `OnceLock` for thread-safe, zero-cost synchronization.
static BUILD_RESULT: OnceLock<bool> = OnceLock::new();

fn ensure_book_built() {
    let _ = BUILD_RESULT.get_or_init(|| {
        let book_path = e2e_book_path();

        let output = Command::new("mdbook")
            .args(["build", book_path.to_str().expect("valid path")])
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
