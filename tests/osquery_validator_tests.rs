//! osquery validator integration tests
//!
//! Tests for validate-osquery.sh running in osquery container.
//! All tests use real osquery container - no mocking.
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

const OSQUERY_IMAGE: &str = "osquery/osquery:5.17.0-ubuntu22.04";

/// Helper to run osquery validator with given SQL and optional assertions
async fn run_osquery_validator(
    sql: &str,
    assertions: Option<&str>,
    expect: Option<&str>,
) -> (i64, String, String) {
    let script = std::fs::read("validators/validate-osquery.sh")
        .expect("validator script must exist at validators/validate-osquery.sh");

    let container = ValidatorContainer::start_with_image(OSQUERY_IMAGE, &script)
        .await
        .expect("osquery container should start");

    let result = container
        .exec_with_env(None, sql, assertions, expect)
        .await
        .expect("exec should succeed");

    println!("Exit code: {}", result.exit_code);
    println!("Stdout: {}", result.stdout);
    println!("Stderr: {}", result.stderr);

    (result.exit_code, result.stdout, result.stderr)
}

/// Test: Valid SQL query passes validation
#[tokio::test]
async fn test_osquery_valid_query_passes() {
    let (exit_code, _, _) =
        run_osquery_validator("SELECT uid, username FROM users LIMIT 1;", None, None).await;
    assert_eq!(exit_code, 0, "valid query should pass");
}

/// Test: Invalid table name fails validation
#[tokio::test]
async fn test_osquery_invalid_table_fails() {
    let (exit_code, _, stderr) =
        run_osquery_validator("SELECT * FROM nonexistent_table_xyz;", None, None).await;
    assert_ne!(exit_code, 0, "invalid table should fail");
    assert!(
        stderr.to_lowercase().contains("no such table")
            || stderr.to_lowercase().contains("error")
            || stderr.contains("Query failed"),
        "stderr should contain error message: {}",
        stderr
    );
}

/// Test: rows >= N assertion passes when query returns enough rows
#[tokio::test]
async fn test_osquery_rows_assertion_passes() {
    // Root user (uid=0) should always exist
    let (exit_code, _, _) = run_osquery_validator(
        "SELECT uid FROM users WHERE uid = 0;",
        Some("rows >= 1"),
        None,
    )
    .await;
    assert_eq!(exit_code, 0, "should find root user");
}

/// Test: rows >= N assertion fails when query returns too few rows
#[tokio::test]
async fn test_osquery_rows_assertion_fails() {
    // uid 99999 should not exist
    let (exit_code, _, stderr) = run_osquery_validator(
        "SELECT uid FROM users WHERE uid = 99999;",
        Some("rows >= 1"),
        None,
    )
    .await;
    assert_ne!(exit_code, 0, "should fail - no such user");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
}

/// Test: contains assertion passes when output contains string
#[tokio::test]
async fn test_osquery_contains_assertion_passes() {
    let (exit_code, _, _) = run_osquery_validator(
        "SELECT username FROM users WHERE uid = 0;",
        Some("contains \"root\""),
        None,
    )
    .await;
    assert_eq!(exit_code, 0, "should find root in output");
}

/// Test: contains assertion fails when output doesn't contain string
#[tokio::test]
async fn test_osquery_contains_assertion_fails() {
    let (exit_code, _, stderr) = run_osquery_validator(
        "SELECT username FROM users WHERE uid = 0;",
        Some("contains \"nonexistent_user_xyz_12345\""),
        None,
    )
    .await;
    assert_ne!(exit_code, 0, "should fail - string not in output");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
}

/// Test: Empty content fails with clear error
#[tokio::test]
async fn test_osquery_empty_content_fails() {
    let (exit_code, _, stderr) = run_osquery_validator("", None, None).await;
    assert_ne!(exit_code, 0, "empty content should fail");
    assert!(
        stderr.to_lowercase().contains("empty")
            || stderr.to_lowercase().contains("required")
            || stderr.contains("Query failed"),
        "stderr should indicate empty content error: {}",
        stderr
    );
}

/// Test: SQL syntax error fails with clear error message
#[tokio::test]
async fn test_osquery_syntax_error_fails() {
    // "SELEC" is a typo - should be "SELECT"
    let (exit_code, _, stderr) = run_osquery_validator("SELEC * FROM users;", None, None).await;
    assert_ne!(exit_code, 0, "syntax error should fail");
    assert!(
        stderr.to_lowercase().contains("error")
            || stderr.to_lowercase().contains("syntax")
            || stderr.contains("Query failed"),
        "stderr should contain error message: {}",
        stderr
    );
}

/// Test: rows = N assertion fails when count doesn't match exactly
#[tokio::test]
async fn test_osquery_rows_equals_assertion_fails() {
    // Query returns 1 row (root user), but we assert rows = 5
    let (exit_code, _, stderr) = run_osquery_validator(
        "SELECT uid FROM users WHERE uid = 0;",
        Some("rows = 5"),
        None,
    )
    .await;
    assert_ne!(exit_code, 0, "should fail - got 1 row, expected 5");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
    assert!(
        stderr.contains("rows = 5"),
        "stderr should show expected value: {}",
        stderr
    );
}

/// Test: rows > N assertion fails when count is not greater
#[tokio::test]
async fn test_osquery_rows_greater_than_assertion_fails() {
    // Query returns 1 row (root user), but we assert rows > 5
    let (exit_code, _, stderr) = run_osquery_validator(
        "SELECT uid FROM users WHERE uid = 0;",
        Some("rows > 5"),
        None,
    )
    .await;
    assert_ne!(exit_code, 0, "should fail - got 1 row, need more than 5");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
    assert!(
        stderr.contains("rows > 5"),
        "stderr should show expected value: {}",
        stderr
    );
}
