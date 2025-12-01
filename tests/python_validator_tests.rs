//! Python validator integration tests
//!
//! Tests for validate-python.sh running as host-based validator.
//! Container runs `py_compile` on scripts, host validates output for errors.
//!
//! Key differences from SQLite/osquery validators:
//! - `py_compile` writes errors to STDERR (not stdout)
//! - Success = `py_compile` found NO issues (exit 0)
//! - Failure = `py_compile` found issues (exit non-0, stderr has error)
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

const PYTHON_IMAGE: &str = "python:3.12-slim";
const VALIDATOR_SCRIPT: &str = "validators/validate-python.sh";

/// Helper to run python validator with host-based validation.
///
/// 1. Starts python container
/// 2. Writes script to temp file and runs `py_compile` (output to stderr)
/// 3. Validates output on host using validator script
///
/// Returns (exit code, stdout, stderr) where:
/// - exit code: 0 = valid script, non-0 = issues found
/// - stdout: typically empty (`py_compile` uses stderr)
/// - stderr: `py_compile` errors or validation errors
async fn run_python_validator(script: &str, assertions: Option<&str>) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(PYTHON_IMAGE)
        .await
        .expect("python container should start");

    // Write script to temp file and run py_compile
    // py_compile output goes to stderr
    let escaped_script = script.replace('\'', "'\\''");
    let cmd = format!(
        "printf '%s' '{}' > /tmp/script.py && python3 -m py_compile /tmp/script.py 2>&1",
        escaped_script
    );

    let result = container
        .exec_raw(&["sh", "-c", &cmd])
        .await
        .expect("python exec should succeed");

    println!("Container exit code: {}", result.exit_code);
    println!("Container stdout: {}", result.stdout);
    println!("Container stderr: {}", result.stderr);

    // For py_compile, errors appear in stdout (due to 2>&1 redirect)
    // Pass it as container_stderr to the validator
    let container_stderr = if result.stdout.is_empty() {
        &result.stderr
    } else {
        &result.stdout
    };

    // Validate on host - pass container output for error detection
    let runner = RealCommandRunner;
    let validation_result = host_validator::run_validator(
        &runner,
        VALIDATOR_SCRIPT,
        "",
        assertions,
        None,
        Some(container_stderr),
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

/// Test: Simple valid Python script passes validation
#[tokio::test]
async fn test_python_valid_script_passes() {
    let script = r#"# Valid Python script
print("Hello, world")
x = 42
"#;
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "valid script should pass py_compile");
}

/// Test: Script with function definition passes
#[tokio::test]
async fn test_python_function_def_passes() {
    let script = r#"def greet(name):
    """Greet someone by name."""
    return f"Hello, {name}!"

result = greet("World")
print(result)
"#;
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "script with function should pass");
}

/// Test: Empty script passes (valid Python)
#[tokio::test]
async fn test_python_empty_script_passes() {
    let script = "";
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "empty script should pass py_compile");
}

// ============================================================================
// Invalid script tests (should fail - exit non-0)
// ============================================================================

/// Test: Missing colon after function def triggers `SyntaxError`
#[tokio::test]
async fn test_python_syntax_error_fails() {
    let script = r#"def greet(name)
    return f"Hello, {name}!"
"#;
    let (exit_code, _, stderr) = run_python_validator(script, None).await;
    assert_ne!(exit_code, 0, "syntax error should fail");
    assert!(
        stderr.contains("SyntaxError") || stderr.contains("validation failed"),
        "stderr should mention SyntaxError or validation failed: {}",
        stderr
    );
}

/// Test: Wrong indentation triggers `IndentationError`
#[tokio::test]
async fn test_python_indentation_error_fails() {
    let script = r#"def greet():
    print("hello")
  print("wrong indent")
"#;
    let (exit_code, _, stderr) = run_python_validator(script, None).await;
    assert_ne!(exit_code, 0, "indentation error should fail");
    assert!(
        stderr.contains("IndentationError") || stderr.contains("validation failed"),
        "stderr should mention IndentationError or validation failed: {}",
        stderr
    );
}

/// Test: Mixed tabs and spaces triggers `TabError`
#[tokio::test]
async fn test_python_tab_error_fails() {
    // Python 3 requires consistent indentation within a block
    // Mix tabs and spaces in same function to trigger TabError
    let script = "def foo():\n    print('spaces')\n\tprint('tab')\n";
    let (exit_code, _, stderr) = run_python_validator(script, None).await;
    assert_ne!(exit_code, 0, "tab error should fail");
    assert!(
        stderr.contains("TabError")
            || stderr.contains("IndentationError")
            || stderr.contains("validation failed"),
        "stderr should mention TabError/IndentationError or validation failed: {}",
        stderr
    );
}

/// Test: Unclosed parenthesis triggers `SyntaxError`
#[tokio::test]
async fn test_python_unclosed_paren_fails() {
    let script = r#"print("hello"
x = 1
"#;
    let (exit_code, _, stderr) = run_python_validator(script, None).await;
    assert_ne!(exit_code, 0, "unclosed paren should fail");
    assert!(
        stderr.contains("SyntaxError") || stderr.contains("validation failed"),
        "stderr should mention SyntaxError or validation failed: {}",
        stderr
    );
}

// ============================================================================
// Assertion tests
// ============================================================================

/// Test: contains assertion passes when error type is present in output
#[tokio::test]
async fn test_python_contains_assertion_passes() {
    // This script has a SyntaxError - missing colon
    let script = r"def foo()
    pass
";
    // Assert that SyntaxError is in the output
    let (exit_code, _, stderr) =
        run_python_validator(script, Some("contains \"SyntaxError\"")).await;
    // py_compile will fail, and assertion should pass since SyntaxError IS present
    assert_ne!(exit_code, 0, "py_compile should fail on syntax error");
    assert!(
        stderr.contains("SyntaxError"),
        "stderr should contain SyntaxError: {}",
        stderr
    );
}

/// Test: contains assertion fails when expected error is NOT present
#[tokio::test]
async fn test_python_contains_assertion_fails() {
    // Valid script - no errors in output
    let script = r#"print("Hello")
"#;
    // Assert that SyntaxError is in the output (it won't be - script is valid)
    let (exit_code, _, stderr) =
        run_python_validator(script, Some("contains \"SyntaxError\"")).await;
    assert_ne!(exit_code, 0, "assertion for missing error should fail");
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
async fn test_python_comments_only_passes() {
    let script = r"# This is a comment
# Another comment
# Nothing but comments
";
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "comments-only script should pass");
}

/// Test: Script with docstring passes
#[tokio::test]
async fn test_python_docstring_passes() {
    let script = r#""""
This is a module-level docstring.
It can span multiple lines.
"""

def example():
    """Function docstring."""
    pass
"#;
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "docstring script should pass");
}

/// Test: Script with multiline string passes
#[tokio::test]
async fn test_python_multiline_string_passes() {
    let script = r#"text = """
This is a multiline
string that spans
several lines.
"""

print(text)
"#;
    let (exit_code, _, _) = run_python_validator(script, None).await;
    assert_eq!(exit_code, 0, "multiline string script should pass");
}
