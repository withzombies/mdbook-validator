//! `SQLite` validator integration tests
//!
//! Tests for validate-sqlite.sh running as host-based validator.
//! Container runs sqlite3, host validates JSON output with jq.
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

use mdbook_validator::container::ValidatorContainer;
use mdbook_validator::host_validator;

const SQLITE_IMAGE: &str = "keinos/sqlite3:3.47.2";
const VALIDATOR_SCRIPT: &str = "validators/validate-sqlite.sh";

/// Helper to run `SQLite` query with host-based validation.
///
/// 1. Starts container with sqlite3 (no script injection)
/// 2. Runs setup script in container (if any) - setup IS the shell command
/// 3. Runs query SQL in container with -json flag
/// 4. Validates JSON output on host using validator script
async fn run_sqlite_validator(
    sql: &str,
    setup: Option<&str>,
    assertions: Option<&str>,
    expect: Option<&str>,
) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(SQLITE_IMAGE)
        .await
        .expect("sqlite container should start");

    // Run setup script in container (if any)
    // Setup content IS the shell command - run directly via sh -c
    if let Some(setup_script) = setup {
        let setup_script = setup_script.trim();
        if !setup_script.is_empty() {
            let setup_result = container
                .exec_raw(&["sh", "-c", setup_script])
                .await
                .expect("setup exec should succeed");

            if setup_result.exit_code != 0 {
                println!("Setup failed - Exit code: {}", setup_result.exit_code);
                println!("Setup stderr: {}", setup_result.stderr);
                return (
                    setup_result.exit_code as i32,
                    setup_result.stdout,
                    format!("Setup script failed: {}", setup_result.stderr),
                );
            }
        }
    }

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
    let cmd = format!("sqlite3 -json /tmp/test.db \"{}\"", sql);
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
    let validation_result =
        host_validator::run_validator(VALIDATOR_SCRIPT, &query_result.stdout, assertions, expect)
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

/// Test: Valid SQL query passes validation (SELECT 1)
#[tokio::test]
async fn test_sqlite_valid_query_passes() {
    let (exit_code, stdout, _) = run_sqlite_validator("SELECT 1 as value;", None, None, None).await;
    assert_eq!(exit_code, 0, "valid query should pass");
    assert!(
        stdout.contains('1'),
        "output should contain the value 1: {}",
        stdout
    );
}

/// Test: SETUP script runs before CONTENT (CREATE TABLE + INSERT + SELECT)
#[tokio::test]
async fn test_sqlite_setup_and_query() {
    // SETUP is now a full shell command
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(42);'";
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
        stderr.to_lowercase().contains("no such table")
            || stderr.to_lowercase().contains("error")
            || stderr.contains("Query failed"),
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

// ============================================================================
// Assertion tests
// ============================================================================

/// Test: rows = N assertion passes when row count matches exactly
#[tokio::test]
async fn test_sqlite_rows_equals_assertion_passes() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, _) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows = 2"), None).await;
    assert_eq!(exit_code, 0, "rows = 2 should pass when 2 rows returned");
}

/// Test: rows = N assertion fails when row count doesn't match
#[tokio::test]
async fn test_sqlite_rows_equals_assertion_fails() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, stderr) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows = 5"), None).await;
    assert_ne!(exit_code, 0, "rows = 5 should fail when 2 rows returned");
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

/// Test: rows >= N assertion passes when row count is at least N
#[tokio::test]
async fn test_sqlite_rows_gte_assertion_passes() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, _) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows >= 1"), None).await;
    assert_eq!(exit_code, 0, "rows >= 1 should pass when 2 rows returned");
}

/// Test: rows >= N assertion fails when row count is less than N
#[tokio::test]
async fn test_sqlite_rows_gte_assertion_fails() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, stderr) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows >= 10"), None).await;
    assert_ne!(exit_code, 0, "rows >= 10 should fail when 2 rows returned");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
}

/// Test: rows > N assertion passes when row count is greater than N
#[tokio::test]
async fn test_sqlite_rows_gt_assertion_passes() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, _) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows > 1"), None).await;
    assert_eq!(exit_code, 0, "rows > 1 should pass when 2 rows returned");
}

/// Test: rows > N assertion fails when row count is not greater than N
#[tokio::test]
async fn test_sqlite_rows_gt_assertion_fails() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, stderr) =
        run_sqlite_validator("SELECT * FROM t;", Some(setup), Some("rows > 5"), None).await;
    assert_ne!(exit_code, 0, "rows > 5 should fail when 2 rows returned");
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

/// Test: contains "string" assertion passes when string is in output
#[tokio::test]
async fn test_sqlite_contains_assertion_passes() {
    let setup = r#"sqlite3 /tmp/test.db "CREATE TABLE users(name TEXT); INSERT INTO users VALUES('alice'), ('bob');""#;
    let (exit_code, _, _) = run_sqlite_validator(
        "SELECT * FROM users;",
        Some(setup),
        Some("contains \"alice\""),
        None,
    )
    .await;
    assert_eq!(exit_code, 0, "contains alice should pass");
}

/// Test: contains "string" assertion fails when string is not in output
#[tokio::test]
async fn test_sqlite_contains_assertion_fails() {
    let setup = r#"sqlite3 /tmp/test.db "CREATE TABLE users(name TEXT); INSERT INTO users VALUES('alice'), ('bob');""#;
    let (exit_code, _, stderr) = run_sqlite_validator(
        "SELECT * FROM users;",
        Some(setup),
        Some("contains \"nonexistent\""),
        None,
    )
    .await;
    assert_ne!(exit_code, 0, "contains nonexistent should fail");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
    assert!(
        stderr.contains("not found"),
        "stderr should mention not found: {}",
        stderr
    );
}

/// Test: `VALIDATOR_EXPECT` passes when output matches exactly
#[tokio::test]
async fn test_sqlite_expected_output_passes() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(id INTEGER); INSERT INTO t VALUES(1), (2);'";
    // SQLite JSON output format: [{"id":1},{"id":2}]
    let (exit_code, _, _) = run_sqlite_validator(
        "SELECT id FROM t ORDER BY id;",
        Some(setup),
        None,
        Some("[{\"id\":1},{\"id\":2}]"),
    )
    .await;
    assert_eq!(exit_code, 0, "expected output should match");
}

/// Test: `VALIDATOR_EXPECT` fails when output doesn't match
#[tokio::test]
async fn test_sqlite_expected_output_fails() {
    let setup = "sqlite3 /tmp/test.db 'CREATE TABLE t(id INTEGER); INSERT INTO t VALUES(1), (2);'";
    let (exit_code, _, stderr) = run_sqlite_validator(
        "SELECT id FROM t ORDER BY id;",
        Some(setup),
        None,
        Some("[{\"id\":99}]"),
    )
    .await;
    assert_ne!(exit_code, 0, "expected output should not match");
    assert!(
        stderr.contains("mismatch") || stderr.contains("Mismatch"),
        "stderr should mention mismatch: {}",
        stderr
    );
}

/// Test: Multi-line SETUP with heredoc syntax works
#[tokio::test]
async fn test_sqlite_multiline_setup_heredoc() {
    // Heredoc-style setup for multi-statement SQL
    let setup = r"sqlite3 /tmp/test.db << 'EOF'
CREATE TABLE products(id INTEGER, name TEXT, price REAL);
INSERT INTO products VALUES(1, 'Widget', 9.99);
INSERT INTO products VALUES(2, 'Gadget', 19.99);
INSERT INTO products VALUES(3, 'Gizmo', 29.99);
EOF";
    let (exit_code, stdout, stderr) = run_sqlite_validator(
        "SELECT * FROM products ORDER BY id;",
        Some(setup),
        Some("rows = 3"),
        None,
    )
    .await;
    assert_eq!(
        exit_code, 0,
        "multiline heredoc setup should pass: {}",
        stderr
    );
    assert!(
        stdout.contains("Widget") && stdout.contains("Gadget") && stdout.contains("Gizmo"),
        "output should contain all products: {}",
        stdout
    );
}
