//! Shellcheck validator integration tests
//!
//! Tests for validate-shellcheck.sh running as host-based validator.
//! Container runs shellcheck on scripts, host validates output for SC codes.
//!
//! Key differences from SQLite/osquery validators:
//! - Shellcheck writes findings to STDERR (not stdout)
//! - Success = shellcheck found NO issues (exit 0)
//! - Failure = shellcheck found issues (exit non-0, stderr has SC codes)
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

const SHELLCHECK_IMAGE: &str = "koalaman/shellcheck-alpine:stable";
const VALIDATOR_SCRIPT: &str = "validators/validate-shellcheck.sh";

/// Helper to run shellcheck validator with host-based validation.
///
/// 1. Starts shellcheck container
/// 2. Writes script to temp file and runs shellcheck (output to stderr)
/// 3. Validates output on host using validator script
///
/// Returns (exit code, stdout, stderr) where:
/// - exit code: 0 = valid script, non-0 = issues found
/// - stdout: typically empty (shellcheck uses stderr)
/// - stderr: shellcheck findings or validation errors
async fn run_shellcheck_validator(script: &str, assertions: Option<&str>) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(SHELLCHECK_IMAGE)
        .await
        .expect("shellcheck container should start");

    // Write script to temp file and run shellcheck
    // Shellcheck output goes to stderr (redirected via >&2)
    let escaped_script = script.replace('\'', "'\\''");
    let cmd = format!(
        "printf '%s' '{}' > /tmp/script.sh && shellcheck /tmp/script.sh >&2",
        escaped_script
    );

    let result = container
        .exec_raw(&["sh", "-c", &cmd])
        .await
        .expect("shellcheck exec should succeed");

    println!("Container exit code: {}", result.exit_code);
    println!("Container stdout: {}", result.stdout);
    println!("Container stderr: {}", result.stderr);

    // Validate on host - pass container stderr for SC code detection
    let runner = RealCommandRunner;
    let validation_result = host_validator::run_validator(
        &runner,
        VALIDATOR_SCRIPT,
        &result.stdout,
        assertions,
        None,
        Some(&result.stderr),
    )
    .expect("host validator should run");

    println!("Validation exit code: {}", validation_result.exit_code);
    println!("Validation stdout: {}", validation_result.stdout);
    println!("Validation stderr: {}", validation_result.stderr);

    (
        validation_result.exit_code,
        result.stdout,
        validation_result.stderr,
    )
}

// ============================================================================
// Valid script tests (should pass - exit 0)
// ============================================================================

/// Test: Clean bash script with shebang passes validation
#[tokio::test]
async fn test_shellcheck_valid_script_passes() {
    let script = r#"#!/bin/bash
# Valid script - no issues
echo "Hello, world"
exit 0
"#;
    let (exit_code, _, _) = run_shellcheck_validator(script, None).await;
    assert_eq!(exit_code, 0, "valid script should pass shellcheck");
}

/// Test: Script with properly quoted variables passes
#[tokio::test]
async fn test_shellcheck_properly_quoted_passes() {
    let script = r#"#!/bin/bash
# Variables are properly quoted - no SC2086
name="world"
file="/path/with spaces/file.txt"
echo "Hello, $name"
cat "$file"
"#;
    let (exit_code, _, _) = run_shellcheck_validator(script, None).await;
    assert_eq!(exit_code, 0, "properly quoted script should pass");
}

/// Test: Empty script (just shebang) passes
#[tokio::test]
async fn test_shellcheck_empty_script_passes() {
    let script = "#!/bin/bash\n";
    let (exit_code, _, _) = run_shellcheck_validator(script, None).await;
    assert_eq!(exit_code, 0, "empty script with shebang should pass");
}

// ============================================================================
// Invalid script tests (should fail - exit non-0)
// ============================================================================

/// Test: Unquoted variable triggers SC2086
#[tokio::test]
async fn test_shellcheck_unquoted_variable_fails() {
    let script = r#"#!/bin/bash
file="test file.txt"
cat $file
"#;
    let (exit_code, _, stderr) = run_shellcheck_validator(script, None).await;
    assert_ne!(exit_code, 0, "unquoted variable should fail");
    assert!(
        stderr.contains("SC2086") || stderr.contains("found issues"),
        "stderr should mention SC2086 or issues: {}",
        stderr
    );
}

/// Test: Using backticks instead of $() triggers SC2006
#[tokio::test]
async fn test_shellcheck_backticks_fails() {
    let script = r#"#!/bin/bash
result=`date`
echo "$result"
"#;
    let (exit_code, _, stderr) = run_shellcheck_validator(script, None).await;
    assert_ne!(exit_code, 0, "backticks should fail");
    assert!(
        stderr.contains("SC2006") || stderr.contains("found issues"),
        "stderr should mention SC2006 or issues: {}",
        stderr
    );
}

/// Test: Unquoted command substitution triggers SC2046
#[tokio::test]
async fn test_shellcheck_unquoted_subshell_fails() {
    let script = r"#!/bin/bash
# SC2046: Quote to prevent word splitting
rm $(cat /tmp/files.txt)
";
    let (exit_code, _, stderr) = run_shellcheck_validator(script, None).await;
    assert_ne!(exit_code, 0, "unquoted subshell should fail");
    assert!(
        stderr.contains("SC2046") || stderr.contains("found issues"),
        "stderr should mention SC2046 or issues: {}",
        stderr
    );
}

/// Test: Script with multiple errors fails
#[tokio::test]
async fn test_shellcheck_multiple_errors_fails() {
    let script = r#"#!/bin/bash
# Multiple shellcheck issues
file="test file.txt"
cat $file
result=`pwd`
echo $result
"#;
    let (exit_code, _, stderr) = run_shellcheck_validator(script, None).await;
    assert_ne!(exit_code, 0, "script with multiple errors should fail");
    assert!(
        stderr.contains("found issues") || stderr.contains("SC"),
        "stderr should mention shellcheck found issues: {}",
        stderr
    );
}

// ============================================================================
// Assertion tests
// ============================================================================

/// Test: contains assertion passes when SC code is present in output
#[tokio::test]
async fn test_shellcheck_contains_assertion_passes() {
    // This script has SC2086 - unquoted variable
    let script = r"#!/bin/bash
cat $file
";
    // Assert that SC2086 is in the output
    let (exit_code, _, stderr) =
        run_shellcheck_validator(script, Some("contains \"SC2086\"")).await;
    // Shellcheck will fail, but assertion should pass since SC2086 IS present
    // The validator should fail due to SC2086 being found, exit != 0
    assert_ne!(exit_code, 0, "shellcheck should fail on SC2086");
    assert!(
        stderr.contains("SC2086"),
        "stderr should contain SC2086: {}",
        stderr
    );
}

/// Test: contains assertion fails when expected SC code is NOT present
#[tokio::test]
async fn test_shellcheck_contains_assertion_fails() {
    // Valid script - no SC codes in output
    let script = r#"#!/bin/bash
echo "Hello"
"#;
    // Assert that SC9999 is in the output (it won't be - script is valid)
    let (exit_code, _, stderr) =
        run_shellcheck_validator(script, Some("contains \"SC9999\"")).await;
    assert_ne!(exit_code, 0, "assertion for missing SC code should fail");
    assert!(
        stderr.contains("not found") || stderr.contains("Assertion failed"),
        "stderr should indicate assertion failed: {}",
        stderr
    );
}

// ============================================================================
// Edge case tests
// ============================================================================

/// Test: Script with only comments passes
#[tokio::test]
async fn test_shellcheck_comments_only_passes() {
    let script = r"#!/bin/bash
# This is a comment
# Another comment
# Nothing but comments
";
    let (exit_code, _, _) = run_shellcheck_validator(script, None).await;
    assert_eq!(exit_code, 0, "comments-only script should pass");
}

/// Test: Script without shebang gets SC2148
#[tokio::test]
async fn test_shellcheck_no_shebang_fails() {
    // SC2148: Tips depend on target shell and target is not specified
    let script = r#"echo "no shebang"
"#;
    let (exit_code, _, stderr) = run_shellcheck_validator(script, None).await;
    assert_ne!(exit_code, 0, "script without shebang should fail");
    assert!(
        stderr.contains("SC2148") || stderr.contains("found issues"),
        "stderr should mention SC2148 or issues: {}",
        stderr
    );
}

/// Test: POSIX sh script passes with proper shebang
#[tokio::test]
async fn test_shellcheck_posix_sh_passes() {
    let script = r#"#!/bin/sh
# POSIX sh script
VAR="value"
echo "$VAR"
"#;
    let (exit_code, _, _) = run_shellcheck_validator(script, None).await;
    assert_eq!(exit_code, 0, "valid POSIX sh script should pass");
}
