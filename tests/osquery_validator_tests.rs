//! osquery validator integration tests
//!
//! Tests for validate-osquery.sh running as host-based validator.
//! Container runs osqueryi, host validates JSON output with jq.
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    clippy::cast_possible_truncation
)]

use mdbook_validator::command::RealCommandRunner;
use mdbook_validator::container::ValidatorContainer;
use mdbook_validator::host_validator;

const OSQUERY_IMAGE: &str = "osquery/osquery:5.17.0-ubuntu22.04";
const VALIDATOR_SCRIPT: &str = "validators/validate-osquery.sh";

/// Helper to run osquery query with host-based validation.
///
/// 1. Starts container with osqueryi (no script injection)
/// 2. Runs query SQL in container with --json flag
/// 3. Validates JSON output on host using validator script
async fn run_osquery_validator(
    sql: &str,
    assertions: Option<&str>,
    expect: Option<&str>,
) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(OSQUERY_IMAGE)
        .await
        .expect("osquery container should start");

    // Handle empty SQL
    let sql = sql.trim();
    if sql.is_empty() {
        return (
            1,
            String::new(),
            "Query failed: VALIDATOR_CONTENT is empty".to_owned(),
        );
    }

    // Run query with JSON output
    let cmd = format!("osqueryi --json \"{}\"", sql);
    let query_result = container
        .exec_raw(&["sh", "-c", &cmd])
        .await
        .expect("query exec should succeed");

    println!("Query exit code: {}", query_result.exit_code);
    println!("Query stdout: {}", query_result.stdout);
    println!("Query stderr: {}", query_result.stderr);

    if query_result.exit_code != 0 {
        return (
            query_result.exit_code as i32,
            query_result.stdout,
            format!("Query failed: {}", query_result.stderr),
        );
    }

    // Validate JSON output on host
    let runner = RealCommandRunner;
    let validation_result = host_validator::run_validator(
        &runner,
        VALIDATOR_SCRIPT,
        &query_result.stdout,
        assertions,
        expect,
        None,
    )
    .expect("host validator should run");

    println!("Validation exit code: {}", validation_result.exit_code);
    println!("Validation stdout: {}", validation_result.stdout);
    println!("Validation stderr: {}", validation_result.stderr);

    (
        validation_result.exit_code,
        query_result.stdout, // Return JSON output from query
        validation_result.stderr,
    )
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
