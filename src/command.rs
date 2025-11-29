//! Command execution abstraction for testing.
//!
//! Provides a trait for running shell commands, enabling mocking in tests
//! to cover error paths (spawn failure, stdin failure, wait failure).

use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Trait for running shell commands.
///
/// Enables mocking in tests to verify error handling without actual failures.
/// Uses generics for zero-cost abstraction in production code.
pub trait CommandRunner: Send + Sync {
    /// Run a validator script with the given stdin content and environment variables.
    ///
    /// # Arguments
    ///
    /// * `script_path` - Path to the script to execute (run via `sh`)
    /// * `stdin_content` - Content to write to the script's stdin
    /// * `env_vars` - Environment variables to set for the script
    ///
    /// # Errors
    ///
    /// Returns error if spawning the process, writing stdin, or waiting for output fails.
    fn run_script(
        &self,
        script_path: &str,
        stdin_content: &str,
        env_vars: &[(&str, &str)],
    ) -> Result<Output>;
}

/// Real implementation using [`std::process::Command`].
///
/// This is the default implementation used in production.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run_script(
        &self,
        script_path: &str,
        stdin_content: &str,
        env_vars: &[(&str, &str)],
    ) -> Result<Output> {
        let mut cmd = Command::new("sh");
        cmd.arg(script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(*key, *value);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn validator: {script_path}"))?;

        // Write content to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_content.as_bytes())
                .context("Failed to write to validator stdin")?;
        }

        child
            .wait_with_output()
            .context("Failed to wait for validator")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_real_command_runner_default() {
        let runner = RealCommandRunner;
        // Just verify it can be created
        let _ = runner;
    }

    #[test]
    fn test_real_command_runner_clone() {
        let runner = RealCommandRunner;
        let cloned = runner;
        let _ = cloned;
    }

    #[test]
    fn test_run_script_success() {
        let runner = RealCommandRunner;
        let result = runner.run_script("/bin/sh", "", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_script_with_stdin() {
        let runner = RealCommandRunner;
        // Use -c to run a command that reads stdin
        let result = runner.run_script("-c", "cat", &[]);
        // This will run `sh -c cat` which reads from stdin (empty in this case)
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_script_with_env_vars() {
        let runner = RealCommandRunner;
        // Use echo_validator.sh which echoes VALIDATOR_ASSERTIONS env var
        let result = runner.run_script(
            "tests/fixtures/echo_validator.sh",
            "{}",
            &[("VALIDATOR_ASSERTIONS", "rows >= 1")],
        );
        assert!(result.is_ok());
        let output = result.expect("run_script should succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("rows >= 1"),
            "Expected 'rows >= 1' in stdout: {stdout}"
        );
    }

    #[test]
    fn test_run_script_nonexistent_script() {
        let runner = RealCommandRunner;
        // sh will run successfully but exit with error for non-existent script
        let result = runner.run_script("/nonexistent/script.sh", "", &[]);
        assert!(result.is_ok()); // sh spawns successfully
        let output = result.expect("run_script should succeed");
        assert!(!output.status.success()); // but the script fails
    }
}
