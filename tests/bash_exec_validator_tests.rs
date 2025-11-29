//! Bash-exec validator integration tests
//!
//! Tests for validate-bash-exec.sh running as host-based validator.
//! Container runs bash scripts, outputs JSON, host validates with assertions.
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    clippy::cast_possible_truncation,
    clippy::doc_markdown
)]

use mdbook_validator::container::ValidatorContainer;
use mdbook_validator::host_validator;

const UBUNTU_IMAGE: &str = "ubuntu:22.04";
const VALIDATOR_SCRIPT: &str = "validators/validate-bash-exec.sh";

/// Extract file paths from assertion string for file_exists, dir_exists, file_contains.
fn extract_file_paths_from_assertions(assertions: Option<&str>) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(assertions) = assertions {
        for line in assertions.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("file_exists ") {
                paths.push(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix("dir_exists ") {
                paths.push(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix("file_contains ") {
                // Extract path before the quoted string: "file_contains /path \"string\""
                if let Some(path) = rest.split_whitespace().next() {
                    paths.push(path.to_owned());
                }
            }
        }
    }
    paths
}

/// Build container command that executes bash script and outputs JSON.
///
/// Output JSON format: {"exit_code": N, "stdout": "...", "stderr": "...", "files": {...}}
/// Files object contains: {"path": {"exists": bool, "is_dir": bool, "content": "..."}}
fn build_bash_exec_command(script: &str, setup: Option<&str>, file_paths: &[String]) -> String {
    // Escape single quotes in script content for shell
    let escaped_script = script.replace('\'', "'\\''");
    let setup_cmd = setup
        .map(|s| {
            let escaped = s.replace('\'', "'\\''");
            format!("eval '{}' 2>/dev/null; ", escaped)
        })
        .unwrap_or_default();

    // Build the file paths list for checking
    let file_paths_str = file_paths.join(" ");

    format!(
        r#"
{setup_cmd}
SCRIPT_FILE=$(mktemp)
printf '%s' '{escaped_script}' > "$SCRIPT_FILE"
chmod +x "$SCRIPT_FILE"

STDOUT_FILE=$(mktemp)
STDERR_FILE=$(mktemp)
set +e
bash "$SCRIPT_FILE" > "$STDOUT_FILE" 2> "$STDERR_FILE"
EXIT_CODE=$?
set -e

# Read output and escape for JSON
STDOUT_CONTENT=$(cat "$STDOUT_FILE" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | tr '\n' ' ')
STDERR_CONTENT=$(cat "$STDERR_FILE" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | tr '\n' ' ')

# Check files from assertions
FILES_JSON=""
FILE_PATHS="{file_paths_str}"
FIRST_FILE=true
for path in $FILE_PATHS; do
    if [ "$FIRST_FILE" = true ]; then
        FIRST_FILE=false
    else
        FILES_JSON="$FILES_JSON, "
    fi
    if [ -e "$path" ]; then
        IS_DIR=$([ -d "$path" ] && echo "true" || echo "false")
        IS_FILE=$([ -f "$path" ] && echo "true" || echo "false")
        CONTENT=""
        if [ -f "$path" ]; then
            CONTENT=$(cat "$path" 2>/dev/null | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | tr '\n' ' ')
        fi
        FILES_JSON="$FILES_JSON\"$path\": {{\"exists\": true, \"is_dir\": $IS_DIR, \"content\": \"$CONTENT\"}}"
    else
        FILES_JSON="$FILES_JSON\"$path\": {{\"exists\": false, \"is_dir\": false, \"content\": \"\"}}"
    fi
done

# Output JSON with files
printf '{{"exit_code": %d, "stdout": "%s", "stderr": "%s", "files": {{%s}}}}' "$EXIT_CODE" "$STDOUT_CONTENT" "$STDERR_CONTENT" "$FILES_JSON"

rm -f "$SCRIPT_FILE" "$STDOUT_FILE" "$STDERR_FILE"
"#
    )
}

/// Helper to run bash-exec validator with host-based validation.
///
/// 1. Starts ubuntu container
/// 2. Runs script, captures exit_code/stdout/stderr/files as JSON
/// 3. Validates JSON output on host using validator script
async fn run_bash_exec_validator(
    script: &str,
    setup: Option<&str>,
    assertions: Option<&str>,
) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(UBUNTU_IMAGE)
        .await
        .expect("ubuntu container should start");

    // Extract file paths from assertions for checking in container
    let file_paths = extract_file_paths_from_assertions(assertions);

    // Build command that outputs JSON with file state
    let cmd = build_bash_exec_command(script, setup, &file_paths);
    let result = container
        .exec_raw(&["sh", "-c", &cmd])
        .await
        .expect("bash exec should succeed");

    println!("Container exit code: {}", result.exit_code);
    println!("Container stdout: {}", result.stdout);
    println!("Container stderr: {}", result.stderr);

    // Validate JSON output on host
    let validation_result = host_validator::run_validator(
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

// =============================================================================
// Basic Script Execution Tests
// =============================================================================

/// Test: Script with exit 0 passes validation (default behavior)
#[tokio::test]
async fn test_bash_exec_valid_script_passes() {
    let script = r#"echo "hello world" && exit 0"#;
    let (exit_code, stdout, stderr) = run_bash_exec_validator(script, None, None).await;

    assert_eq!(exit_code, 0, "Valid script should pass. stderr: {}", stderr);
    assert!(
        stdout.contains("exit_code"),
        "Should have JSON output: {}",
        stdout
    );
}

/// Test: Script with non-zero exit fails validation (default requires exit 0)
#[tokio::test]
async fn test_bash_exec_invalid_script_fails() {
    let script = "exit 1";
    let (exit_code, _, stderr) = run_bash_exec_validator(script, None, None).await;

    assert_ne!(
        exit_code, 0,
        "Script with exit 1 should fail validation (default requires exit 0)"
    );
    assert!(
        stderr.contains("exit code") || stderr.contains("exit_code"),
        "Should mention exit code in error: {}",
        stderr
    );
}

// =============================================================================
// exit_code Assertion Tests
// =============================================================================

/// Test: exit_code = 0 assertion passes when script exits 0
#[tokio::test]
async fn test_bash_exec_exit_code_assertion_passes() {
    let script = "exit 0";
    let (exit_code, _, stderr) = run_bash_exec_validator(script, None, Some("exit_code = 0")).await;

    assert_eq!(
        exit_code, 0,
        "exit_code = 0 assertion should pass. stderr: {}",
        stderr
    );
}

/// Test: exit_code = 0 assertion fails when script exits 1
#[tokio::test]
async fn test_bash_exec_exit_code_assertion_fails() {
    let script = "exit 1";
    let (exit_code, _, stderr) = run_bash_exec_validator(script, None, Some("exit_code = 0")).await;

    assert_ne!(
        exit_code, 0,
        "exit_code = 0 assertion should fail when script exits 1"
    );
    assert!(
        stderr.contains("Assertion failed") || stderr.contains("exit_code"),
        "Should mention assertion failure: {}",
        stderr
    );
}

/// Test: Non-zero exit code allowed when explicitly asserted
#[tokio::test]
async fn test_bash_exec_nonzero_exit_allowed_with_assertion() {
    let script = "exit 42";
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, None, Some("exit_code = 42")).await;

    assert_eq!(
        exit_code, 0,
        "exit_code = 42 assertion should pass when script exits 42. stderr: {}",
        stderr
    );
}

// =============================================================================
// stdout_contains Assertion Tests
// =============================================================================

/// Test: stdout_contains assertion passes when output contains string
#[tokio::test]
async fn test_bash_exec_stdout_contains_passes() {
    let script = r#"echo "hello world""#;
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, None, Some("stdout_contains \"hello\"")).await;

    assert_eq!(
        exit_code, 0,
        "stdout_contains should pass when output contains string. stderr: {}",
        stderr
    );
}

/// Test: stdout_contains assertion fails when output doesn't contain string
#[tokio::test]
async fn test_bash_exec_stdout_contains_fails() {
    let script = r#"echo "goodbye""#;
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, None, Some("stdout_contains \"hello\"")).await;

    assert_ne!(
        exit_code, 0,
        "stdout_contains should fail when output doesn't contain string"
    );
    assert!(
        stderr.contains("Assertion failed") || stderr.contains("not found"),
        "Should mention assertion failure: {}",
        stderr
    );
}

// =============================================================================
// file_exists Assertion Tests
// =============================================================================

/// Test: file_exists assertion passes when script creates the file
#[tokio::test]
async fn test_bash_exec_file_exists_passes() {
    let script = "touch /tmp/testfile";
    let (exit_code, stdout, stderr) =
        run_bash_exec_validator(script, None, Some("file_exists /tmp/testfile")).await;

    println!("stdout: {stdout}");
    assert_eq!(
        exit_code, 0,
        "file_exists should pass when file is created. stderr: {}",
        stderr
    );
}

/// Test: file_exists assertion fails when file doesn't exist
#[tokio::test]
async fn test_bash_exec_file_exists_fails() {
    let script = "echo 'no file created'";
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, None, Some("file_exists /tmp/nonexistent_file")).await;

    assert_ne!(
        exit_code, 0,
        "file_exists should fail when file doesn't exist"
    );
    assert!(
        stderr.contains("Assertion failed") || stderr.contains("not found"),
        "Should mention assertion failure: {}",
        stderr
    );
}

// =============================================================================
// dir_exists Assertion Tests
// =============================================================================

/// Test: dir_exists assertion passes when script creates the directory
#[tokio::test]
async fn test_bash_exec_dir_exists_passes() {
    let script = "mkdir -p /tmp/testdir";
    let (exit_code, stdout, stderr) =
        run_bash_exec_validator(script, None, Some("dir_exists /tmp/testdir")).await;

    println!("stdout: {stdout}");
    assert_eq!(
        exit_code, 0,
        "dir_exists should pass when directory is created. stderr: {}",
        stderr
    );
}

/// Test: dir_exists assertion fails when directory doesn't exist
#[tokio::test]
async fn test_bash_exec_dir_exists_fails() {
    let script = "echo 'no dir created'";
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, None, Some("dir_exists /tmp/nonexistent_dir")).await;

    assert_ne!(
        exit_code, 0,
        "dir_exists should fail when directory doesn't exist"
    );
    assert!(
        stderr.contains("Assertion failed") || stderr.contains("not found"),
        "Should mention assertion failure: {}",
        stderr
    );
}

// =============================================================================
// file_contains Assertion Tests
// =============================================================================

/// Test: file_contains assertion passes when file contains the pattern
#[tokio::test]
async fn test_bash_exec_file_contains_passes() {
    let script = r"echo 'config=value' > /tmp/config.txt";
    let (exit_code, stdout, stderr) = run_bash_exec_validator(
        script,
        None,
        Some("file_contains /tmp/config.txt \"config=value\""),
    )
    .await;

    println!("stdout: {stdout}");
    assert_eq!(
        exit_code, 0,
        "file_contains should pass when file contains pattern. stderr: {}",
        stderr
    );
}

/// Test: file_contains assertion fails when file doesn't contain the pattern
#[tokio::test]
async fn test_bash_exec_file_contains_fails() {
    let script = r"echo 'wrong content' > /tmp/config.txt";
    let (exit_code, _, stderr) = run_bash_exec_validator(
        script,
        None,
        Some("file_contains /tmp/config.txt \"expected\""),
    )
    .await;

    assert_ne!(
        exit_code, 0,
        "file_contains should fail when file doesn't contain pattern"
    );
    assert!(
        stderr.contains("Assertion failed") || stderr.contains("not found"),
        "Should mention assertion failure: {}",
        stderr
    );
}

// =============================================================================
// SETUP Block Tests
// =============================================================================

/// Test: SETUP block runs before main script
#[tokio::test]
async fn test_bash_exec_setup_runs_first() {
    let script = "cat /tmp/setup_file";
    let setup = Some("echo 'setup content' > /tmp/setup_file");
    let (exit_code, _, stderr) =
        run_bash_exec_validator(script, setup, Some("stdout_contains \"setup content\"")).await;

    assert_eq!(
        exit_code, 0,
        "SETUP should create file before script runs. stderr: {}",
        stderr
    );
}
