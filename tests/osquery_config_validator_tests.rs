//! osquery config validator integration tests
//!
//! Tests for validate-osquery-config.sh running as host-based validator.
//! Container runs osqueryi `--config_check`, host validates with assertions.
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

const OSQUERY_IMAGE: &str = "osquery/osquery:5.17.0-ubuntu22.04";
const VALIDATOR_SCRIPT: &str = "validators/validate-osquery-config.sh";

/// Helper to run osquery config validation with host-based assertion checking.
///
/// Flow:
/// 1. Starts container with osqueryi
/// 2. Writes config JSON to /tmp/config.json
/// 3. Runs osqueryi `--config_check` (validates config)
/// 4. On success, echoes config back to stdout
/// 5. Host validator checks assertions against config JSON
async fn run_osquery_config_validator(
    config_json: &str,
    assertions: Option<&str>,
    expect: Option<&str>,
) -> (i32, String, String) {
    let container = ValidatorContainer::start_raw(OSQUERY_IMAGE)
        .await
        .expect("osquery container should start");

    // Handle empty config
    let config_json = config_json.trim();
    if config_json.is_empty() {
        return (
            1,
            String::new(),
            "Config failed: config JSON is empty".to_owned(),
        );
    }

    // Build exec_command: write config to file, validate with --config_check, echo back on success
    // The config JSON is passed as $0 argument to sh -c
    // We need to escape single quotes in the JSON for the shell command
    let escaped_config = config_json.replace('\'', "'\\''");

    // This mirrors the exec_command from book.toml:
    // sh -c 'printf "%s" "$0" > /tmp/config.json && osqueryi --config_path=/tmp/config.json --config_check >&2 && cat /tmp/config.json'
    let cmd = format!(
        "printf '%s' '{}' > /tmp/config.json && osqueryi --config_path=/tmp/config.json --config_check >&2 && cat /tmp/config.json",
        escaped_config
    );

    let result = container
        .exec_raw(&["sh", "-c", &cmd])
        .await
        .expect("config validation exec should succeed");

    println!("Config check exit code: {}", result.exit_code);
    println!("Config check stdout: {}", result.stdout);
    println!("Config check stderr: {}", result.stderr);

    if result.exit_code != 0 {
        return (
            result.exit_code as i32,
            result.stdout,
            format!("Config check failed: {}", result.stderr),
        );
    }

    // Validate config with host validator (checks assertions and container warnings)
    // Pass container stderr so validator can detect osquery warnings like "Cannot set unknown"
    let validation_result = host_validator::run_validator(
        VALIDATOR_SCRIPT,
        &result.stdout,
        assertions,
        expect,
        Some(&result.stderr),
    )
    .expect("host validator should run");

    println!("Validation exit code: {}", validation_result.exit_code);
    println!("Validation stdout: {}", validation_result.stdout);
    println!("Validation stderr: {}", validation_result.stderr);

    // Combine container stderr (osquery warnings) with validator stderr (assertion failures)
    let combined_stderr = if validation_result.stderr.is_empty() {
        result.stderr.clone()
    } else if result.stderr.is_empty() {
        validation_result.stderr.clone()
    } else {
        format!("{}\n{}", result.stderr, validation_result.stderr)
    };

    (
        validation_result.exit_code,
        result.stdout, // Return config JSON from container
        combined_stderr,
    )
}

/// Test: Valid minimal osquery config passes validation
#[tokio::test]
async fn test_osquery_config_valid_passes() {
    // Minimal valid osquery config - empty options object
    let config = r#"{"options": {}}"#;
    let (exit_code, _, _) = run_osquery_config_validator(config, None, None).await;
    assert_eq!(exit_code, 0, "valid config should pass");
}

/// Test: Invalid JSON fails with clear error
#[tokio::test]
async fn test_osquery_config_invalid_json_fails() {
    // Malformed JSON - missing closing brace
    let config = r#"{"options": {"logger_path": "/var/log"}"#;
    let (exit_code, _, stderr) = run_osquery_config_validator(config, None, None).await;
    assert_ne!(exit_code, 0, "invalid JSON should fail");
    assert!(
        stderr.to_lowercase().contains("error")
            || stderr.to_lowercase().contains("json")
            || stderr.contains("Config check failed"),
        "stderr should indicate JSON error: {}",
        stderr
    );
}

/// Test: Valid JSON but unknown osquery option fails validation
///
/// osquery `--config_check` is lenient with unknown options (warns but exits 0),
/// but our validator makes warnings into errors for stricter validation.
/// This catches typos like `loger_path` instead of `logger_path`.
#[tokio::test]
async fn test_osquery_config_unknown_option_fails() {
    // Valid JSON but completely fake osquery option
    let config = r#"{"options": {"completely_fake_nonexistent_option_xyz_12345": "value"}}"#;
    let (exit_code, _, stderr) = run_osquery_config_validator(config, None, None).await;

    // Our validator makes osquery warnings into errors
    assert_ne!(exit_code, 0, "unknown option should fail validation");

    // Stderr should indicate the failure
    assert!(
        stderr.contains("Cannot set unknown") || stderr.contains("unknown option"),
        "stderr should mention unknown option: {}",
        stderr
    );
}

/// Test: contains assertion passes when config contains the string
#[tokio::test]
async fn test_osquery_config_contains_assertion_passes() {
    let config = r#"{"schedule": {"query1": {"query": "SELECT 1", "interval": 60}}}"#;
    let (exit_code, _, _) =
        run_osquery_config_validator(config, Some("contains \"schedule\""), None).await;
    assert_eq!(exit_code, 0, "should find 'schedule' in config");
}

/// Test: contains assertion fails when config doesn't contain the string
#[tokio::test]
async fn test_osquery_config_contains_assertion_fails() {
    let config = r#"{"options": {}}"#;
    let (exit_code, _, stderr) =
        run_osquery_config_validator(config, Some("contains \"nonexistent_key_xyz_12345\""), None)
            .await;
    assert_ne!(exit_code, 0, "should fail - key not in config");
    assert!(
        stderr.contains("Assertion failed"),
        "stderr should mention assertion failure: {}",
        stderr
    );
}
