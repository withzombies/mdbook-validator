//! `SQLite` validator integration tests
//!
//! Tests for validate-sqlite.sh running in keinos/sqlite3 container.
//! All tests use real `SQLite` container - no mocking.
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

use mdbook_validator::container::ValidatorContainer;

const SQLITE_IMAGE: &str = "keinos/sqlite3:3.47.2";

/// Helper to run `SQLite` validator with given SQL, optional setup, and optional assertions
async fn run_sqlite_validator(
    sql: &str,
    setup: Option<&str>,
    assertions: Option<&str>,
    expect: Option<&str>,
) -> (i64, String, String) {
    let script = std::fs::read("validators/validate-sqlite.sh")
        .expect("validator script must exist at validators/validate-sqlite.sh");

    let container = ValidatorContainer::start_with_image(SQLITE_IMAGE, &script)
        .await
        .expect("sqlite container should start");

    let result = container
        .exec_with_env(setup, sql, assertions, expect)
        .await
        .expect("exec should succeed");

    println!("Exit code: {}", result.exit_code);
    println!("Stdout: {}", result.stdout);
    println!("Stderr: {}", result.stderr);

    (result.exit_code, result.stdout, result.stderr)
}

/// Test: Valid SQL query passes validation (SELECT 1)
#[tokio::test]
async fn test_sqlite_valid_query_passes() {
    let (exit_code, stdout, _) = run_sqlite_validator("SELECT 1;", None, None, None).await;
    assert_eq!(exit_code, 0, "valid query should pass");
    assert!(
        stdout.contains('1'),
        "output should contain the value 1: {}",
        stdout
    );
}

/// Test: SETUP SQL runs before CONTENT (CREATE TABLE + INSERT + SELECT)
#[tokio::test]
async fn test_sqlite_setup_and_query() {
    let setup = "CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(42);";
    let (exit_code, stdout, _) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), None, None).await;
    assert_eq!(exit_code, 0, "query with setup should pass");
    assert!(
        stdout.contains("42"),
        "output should contain the inserted value 42: {}",
        stdout
    );
}

/// Test: Invalid table name fails validation
#[tokio::test]
async fn test_sqlite_invalid_table_fails() {
    let (exit_code, _, stderr) =
        run_sqlite_validator("SELECT * FROM nonexistent_table_xyz;", None, None, None).await;
    assert_ne!(exit_code, 0, "invalid table should fail");
    assert!(
        stderr.to_lowercase().contains("no such table") || stderr.to_lowercase().contains("error"),
        "stderr should contain error message: {}",
        stderr
    );
}

/// Test: Empty content fails with clear error
#[tokio::test]
async fn test_sqlite_empty_content_fails() {
    let (exit_code, _, stderr) = run_sqlite_validator("", None, None, None).await;
    assert_ne!(exit_code, 0, "empty content should fail");
    assert!(
        stderr.to_lowercase().contains("empty"),
        "stderr should indicate empty content error: {}",
        stderr
    );
}

/// Test: SQL syntax error fails with clear error message
#[tokio::test]
async fn test_sqlite_syntax_error_fails() {
    // "SELEC" is a typo - should be "SELECT"
    let (exit_code, _, stderr) =
        run_sqlite_validator("SELEC * FROM users;", None, None, None).await;
    assert_ne!(exit_code, 0, "syntax error should fail");
    assert!(
        stderr.to_lowercase().contains("error")
            || stderr.to_lowercase().contains("syntax")
            || stderr.contains("Query failed"),
        "stderr should contain error message: {}",
        stderr
    );
}
