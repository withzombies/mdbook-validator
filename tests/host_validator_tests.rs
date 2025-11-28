// Tests are allowed to panic for assertions and test failure
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

//! Tests for `host_validator` module

use mdbook_validator::host_validator::run_validator;

const ECHO_VALIDATOR: &str = "tests/fixtures/echo_validator.sh";
const EXIT_CODE_VALIDATOR: &str = "tests/fixtures/exit_code_validator.sh";

#[test]
fn test_host_validator_runs_script() {
    // Test that run_validator can spawn and run a script
    let result =
        run_validator(ECHO_VALIDATOR, "{}", None, None, None).expect("validator should run");

    assert_eq!(result.exit_code, 0, "exit code should be 0");
    assert!(
        result.stdout.contains("JSON_INPUT"),
        "stdout should contain output from script"
    );
}

#[test]
fn test_host_validator_passes_json_stdin() {
    // Test that JSON input is passed via stdin
    let json_input = r#"[{"id": 1}, {"id": 2}]"#;
    let result =
        run_validator(ECHO_VALIDATOR, json_input, None, None, None).expect("validator should run");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains(json_input),
        "stdout should contain the JSON input: {}",
        result.stdout
    );
}

#[test]
fn test_host_validator_sets_env_vars() {
    // Test that assertions and expect are passed as env vars
    let result = run_validator(
        ECHO_VALIDATOR,
        "{}",
        Some("rows >= 1"),
        Some(r#"[{"count": 5}]"#),
        None,
    )
    .expect("validator should run");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("rows >= 1"),
        "stdout should contain assertions: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains(r#"[{"count": 5}]"#),
        "stdout should contain expect: {}",
        result.stdout
    );
}

#[test]
fn test_host_validator_captures_exit_code() {
    // Test that non-zero exit codes are captured
    let result =
        run_validator(EXIT_CODE_VALIDATOR, "{}", None, None, None).expect("validator should run");

    assert_eq!(result.exit_code, 42, "exit code should be 42");
}

#[test]
fn test_host_validator_passes_container_stderr() {
    // Test that container stderr is passed as env var
    let container_stderr = "W1128 options.cpp:101] Cannot set unknown flag: fake_option";
    let result = run_validator(ECHO_VALIDATOR, "{}", None, None, Some(container_stderr))
        .expect("validator should run");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains(container_stderr),
        "stdout should contain container stderr: {}",
        result.stdout
    );
}

#[test]
fn test_host_validator_nonexistent_script_returns_error_exit() {
    // When script doesn't exist, sh spawns successfully but returns exit 127
    // This is the expected behavior (we use `sh script_path` to avoid needing +x)
    let result = run_validator("nonexistent_script_xyz_123.sh", "{}", None, None, None)
        .expect("sh should spawn, script failure is exit code");

    assert_eq!(
        result.exit_code, 127,
        "sh returns 127 for nonexistent script"
    );
    assert!(
        result.stderr.contains("No such file or directory") || result.stderr.contains("not found"),
        "stderr should indicate script not found: {}",
        result.stderr
    );
}
