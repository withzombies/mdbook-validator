//! Tests for `ValidatorError` enum
//!
//! Verifies:
//! - Display impl shows error codes (E001-E010)
//! - `code()` method returns correct codes
//! - `matches!()` macro works for pattern matching
//! - Edge cases (empty messages, negative exit codes, special chars)

#![allow(clippy::panic, clippy::expect_used)]

use mdbook_validator::ValidatorError;

// === Display tests (verify error codes in message) ===

#[test]
fn test_config_error_displays_with_code() {
    let err = ValidatorError::Config {
        message: "test message".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E001]"), "Should contain E001: {display}");
    assert!(
        display.contains("test message"),
        "Should contain message: {display}"
    );
}

#[test]
fn test_container_startup_displays_with_code() {
    let err = ValidatorError::ContainerStartup {
        message: "docker not running".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E002]"), "Should contain E002: {display}");
}

#[test]
fn test_container_exec_displays_with_code() {
    let err = ValidatorError::ContainerExec {
        message: "exec failed".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E003]"), "Should contain E003: {display}");
}

#[test]
fn test_setup_failed_displays_exit_code() {
    let err = ValidatorError::SetupFailed {
        exit_code: 1,
        message: "invalid SQL".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E004]"), "Should contain E004: {display}");
    assert!(
        display.contains("exit 1"),
        "Should contain exit code: {display}"
    );
}

#[test]
fn test_validation_failed_displays_exit_code() {
    let err = ValidatorError::ValidationFailed {
        exit_code: 42,
        message: "assertion failed".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E006]"), "Should contain E006: {display}");
    assert!(
        display.contains("exit 42"),
        "Should contain exit code: {display}"
    );
}

#[test]
fn test_unknown_validator_displays_name() {
    let err = ValidatorError::UnknownValidator {
        name: "nonexistent".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E007]"), "Should contain E007: {display}");
    assert!(
        display.contains("nonexistent"),
        "Should contain name: {display}"
    );
}

#[test]
fn test_invalid_config_displays_name_and_reason() {
    let err = ValidatorError::InvalidConfig {
        name: "sqlite".into(),
        reason: "container empty".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E008]"), "Should contain E008: {display}");
    assert!(display.contains("sqlite"), "Should contain name: {display}");
    assert!(
        display.contains("container empty"),
        "Should contain reason: {display}"
    );
}

#[test]
fn test_script_not_found_displays_path() {
    let err = ValidatorError::ScriptNotFound {
        path: "/missing/script.sh".into(),
    };
    let display = err.to_string();
    assert!(display.contains("[E010]"), "Should contain E010: {display}");
    assert!(
        display.contains("/missing/script.sh"),
        "Should contain path: {display}"
    );
}

// === code() method tests ===

#[test]
fn test_code_returns_correct_codes() {
    assert_eq!(
        ValidatorError::Config {
            message: String::new()
        }
        .code(),
        "E001"
    );
    assert_eq!(
        ValidatorError::ContainerStartup {
            message: String::new()
        }
        .code(),
        "E002"
    );
    assert_eq!(
        ValidatorError::ContainerExec {
            message: String::new()
        }
        .code(),
        "E003"
    );
    assert_eq!(
        ValidatorError::SetupFailed {
            exit_code: 0,
            message: String::new()
        }
        .code(),
        "E004"
    );
    assert_eq!(
        ValidatorError::QueryFailed {
            exit_code: 0,
            message: String::new()
        }
        .code(),
        "E005"
    );
    assert_eq!(
        ValidatorError::ValidationFailed {
            exit_code: 0,
            message: String::new()
        }
        .code(),
        "E006"
    );
    assert_eq!(
        ValidatorError::UnknownValidator {
            name: String::new()
        }
        .code(),
        "E007"
    );
    assert_eq!(
        ValidatorError::InvalidConfig {
            name: String::new(),
            reason: String::new()
        }
        .code(),
        "E008"
    );
    assert_eq!(
        ValidatorError::FixturesError {
            message: String::new()
        }
        .code(),
        "E009"
    );
    assert_eq!(
        ValidatorError::ScriptNotFound {
            path: String::new()
        }
        .code(),
        "E010"
    );
}

// === matches!() macro tests ===

#[test]
fn test_matches_on_config() {
    let err = ValidatorError::Config {
        message: "test".into(),
    };
    assert!(matches!(err, ValidatorError::Config { .. }));
    assert!(!matches!(err, ValidatorError::ContainerStartup { .. }));
}

#[test]
fn test_matches_on_validation_failed_with_exit_code() {
    let err = ValidatorError::ValidationFailed {
        exit_code: 1,
        message: "fail".into(),
    };
    assert!(matches!(
        err,
        ValidatorError::ValidationFailed { exit_code: 1, .. }
    ));
    assert!(!matches!(
        err,
        ValidatorError::ValidationFailed { exit_code: 0, .. }
    ));
}

// === Edge case tests ===

#[test]
fn test_empty_message_displays_correctly() {
    let err = ValidatorError::Config {
        message: String::new(),
    };
    let display = err.to_string();
    assert!(
        display.contains("[E001]"),
        "Should still contain code: {display}"
    );
}

#[test]
fn test_negative_exit_code_displays() {
    let err = ValidatorError::SetupFailed {
        exit_code: -1,
        message: "signal".into(),
    };
    let display = err.to_string();
    assert!(
        display.contains("-1"),
        "Should handle negative exit code: {display}"
    );
}

#[test]
fn test_special_chars_in_message() {
    let err = ValidatorError::Config {
        message: "error: \"quoted\" & <special>".into(),
    };
    let display = err.to_string();
    assert!(
        display.contains("\"quoted\""),
        "Should preserve quotes: {display}"
    );
    assert!(
        display.contains("<special>"),
        "Should preserve special chars: {display}"
    );
}
