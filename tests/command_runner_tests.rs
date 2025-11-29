// Tests are allowed to panic for assertions and test failure
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]

//! Tests for `CommandRunner` trait error paths using mocks.
//!
//! These tests verify that `run_validator` properly handles and propagates
//! errors from the underlying command execution (spawn, stdin, wait failures).

use anyhow::{anyhow, Result};
use mdbook_validator::command::CommandRunner;
use mdbook_validator::host_validator::run_validator;
use std::process::{ExitStatus, Output};

/// Mock command runner that returns a configurable error.
struct FailingCommandRunner {
    error_message: &'static str,
}

impl CommandRunner for FailingCommandRunner {
    fn run_script(
        &self,
        _script_path: &str,
        _stdin_content: &str,
        _env_vars: &[(&str, &str)],
    ) -> Result<Output> {
        Err(anyhow!("{}", self.error_message))
    }
}

/// Mock command runner that returns a successful output with configurable content.
struct SuccessCommandRunner {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
}

impl SuccessCommandRunner {
    fn with_exit_code(exit_code: i32) -> Self {
        Self {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code,
        }
    }

    fn with_stdout(mut self, stdout: &str) -> Self {
        self.stdout = stdout.as_bytes().to_vec();
        self
    }

    fn with_stderr(mut self, stderr: &str) -> Self {
        self.stderr = stderr.as_bytes().to_vec();
        self
    }
}

impl CommandRunner for SuccessCommandRunner {
    fn run_script(
        &self,
        _script_path: &str,
        _stdin_content: &str,
        _env_vars: &[(&str, &str)],
    ) -> Result<Output> {
        // Create an Output with the configured values
        // We need to create an ExitStatus, which requires platform-specific handling
        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(self.exit_code << 8) // Exit codes are in upper bits on Unix
        };
        #[cfg(not(unix))]
        let status = {
            // On non-Unix, we can't easily create custom ExitStatus
            // This is a limitation but tests run on Unix in CI
            panic!("Mock exit status not supported on this platform");
        };

        Ok(Output {
            status,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        })
    }
}

// === Error path tests ===

#[test]
fn test_spawn_failure_returns_error() {
    // Simulate a spawn failure (e.g., shell not found)
    let runner = FailingCommandRunner {
        error_message: "Failed to spawn validator: /nonexistent/script.sh",
    };

    let result = run_validator(&runner, "/nonexistent/script.sh", "{}", None, None, None);

    assert!(result.is_err(), "Expected error on spawn failure");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("spawn"),
        "Error should mention spawn: {}",
        err
    );
}

#[test]
fn test_stdin_write_failure_returns_error() {
    // Simulate a stdin write failure
    let runner = FailingCommandRunner {
        error_message: "Failed to write to validator stdin",
    };

    let result = run_validator(
        &runner,
        "/some/script.sh",
        "large json content",
        None,
        None,
        None,
    );

    assert!(result.is_err(), "Expected error on stdin write failure");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("stdin"),
        "Error should mention stdin: {}",
        err
    );
}

#[test]
fn test_wait_failure_returns_error() {
    // Simulate a wait_with_output failure
    let runner = FailingCommandRunner {
        error_message: "Failed to wait for validator",
    };

    let result = run_validator(&runner, "/some/script.sh", "{}", None, None, None);

    assert!(result.is_err(), "Expected error on wait failure");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("wait"),
        "Error should mention wait: {}",
        err
    );
}

// === Success path tests with mock ===

#[test]
fn test_mock_runner_success_exit_code_zero() {
    let runner = SuccessCommandRunner::with_exit_code(0)
        .with_stdout("OK")
        .with_stderr("");

    let result = run_validator(&runner, "/test.sh", "{}", None, None, None);

    assert!(result.is_ok(), "Expected success");
    let validation = result.unwrap();
    assert_eq!(validation.exit_code, 0);
    assert_eq!(validation.stdout, "OK");
    assert!(validation.stderr.is_empty());
}

#[test]
fn test_mock_runner_success_exit_code_nonzero() {
    let runner = SuccessCommandRunner::with_exit_code(1)
        .with_stdout("")
        .with_stderr("Validation failed: rows < 1");

    let result = run_validator(&runner, "/test.sh", "{}", None, None, None);

    assert!(
        result.is_ok(),
        "run_validator should succeed even with nonzero exit"
    );
    let validation = result.unwrap();
    assert_eq!(validation.exit_code, 1);
    assert!(validation.stderr.contains("Validation failed"));
}

#[test]
fn test_mock_runner_captures_stdout_and_stderr() {
    let runner = SuccessCommandRunner::with_exit_code(0)
        .with_stdout("stdout content here")
        .with_stderr("stderr content here");

    let result = run_validator(&runner, "/test.sh", "{}", None, None, None);

    assert!(result.is_ok());
    let validation = result.unwrap();
    assert_eq!(validation.stdout, "stdout content here");
    assert_eq!(validation.stderr, "stderr content here");
}

#[test]
fn test_mock_runner_with_assertions_and_expect() {
    // Verify that assertions and expect don't affect the mock (they're just env vars)
    let runner = SuccessCommandRunner::with_exit_code(0);

    let result = run_validator(
        &runner,
        "/test.sh",
        r#"[{"id": 1}]"#,
        Some("rows >= 1"),
        Some(r#"[{"id": 1}]"#),
        Some("container stderr"),
    );

    assert!(result.is_ok());
    let validation = result.unwrap();
    assert_eq!(validation.exit_code, 0);
}

#[test]
fn test_mock_runner_negative_exit_code_handling() {
    // When exit code can't be determined, run_validator returns -1
    // This tests the unwrap_or(-1) path in host_validator.rs
    // We simulate this by using a signal termination code on Unix
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        struct SignalKilledRunner;

        impl CommandRunner for SignalKilledRunner {
            fn run_script(
                &self,
                _script_path: &str,
                _stdin_content: &str,
                _env_vars: &[(&str, &str)],
            ) -> Result<Output> {
                // Simulate process killed by signal (no exit code)
                let status = ExitStatus::from_raw(9); // SIGKILL signal, no exit code
                Ok(Output {
                    status,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        }

        let runner = SignalKilledRunner;
        let result = run_validator(&runner, "/test.sh", "{}", None, None, None);

        assert!(result.is_ok());
        let validation = result.unwrap();
        // Signal-killed processes return -1 from our code (unwrap_or(-1))
        assert_eq!(validation.exit_code, -1);
    }
}
