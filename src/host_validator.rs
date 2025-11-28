//! Host-side validator execution
//!
//! Runs validator scripts on the host machine, enabling use of jq
//! and other host tools for JSON parsing.

use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Result of running a host validator
#[derive(Debug)]
pub struct HostValidationResult {
    /// Exit code from the validator (0 = success)
    pub exit_code: i32,
    /// Standard output from the validator
    pub stdout: String,
    /// Standard error from the validator
    pub stderr: String,
}

/// Run a validator script on the host with JSON input.
///
/// # Arguments
///
/// * `script_path` - Path to validator script (e.g., "validators/validate-sqlite.sh")
/// * `json_input` - JSON output from container to validate
/// * `assertions` - Optional assertion rules
/// * `expect` - Optional expected output
/// * `container_stderr` - Optional stderr output from container (for warning detection)
///
/// # Errors
///
/// Returns error if the validator script cannot be spawned or if stdin write fails.
pub fn run_validator(
    script_path: &str,
    json_input: &str,
    assertions: Option<&str>,
    expect: Option<&str>,
    container_stderr: Option<&str>,
) -> Result<HostValidationResult> {
    let mut cmd = Command::new("sh");
    cmd.arg(script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Set env vars for assertions
    if let Some(a) = assertions {
        cmd.env("VALIDATOR_ASSERTIONS", a);
    }
    if let Some(e) = expect {
        cmd.env("VALIDATOR_EXPECT", e);
    }
    if let Some(stderr) = container_stderr {
        cmd.env("VALIDATOR_CONTAINER_STDERR", stderr);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn validator: {script_path}"))?;

    // Write JSON to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(json_input.as_bytes())
            .context("Failed to write JSON to validator stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for validator")?;

    Ok(HostValidationResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}
