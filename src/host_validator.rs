//! Host-side validator execution
//!
//! Runs validator scripts on the host machine, enabling use of jq
//! and other host tools for JSON parsing.

use anyhow::Result;

use crate::command::CommandRunner;

/// Result of running a host validator
#[derive(Debug)]
#[must_use]
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
/// * `runner` - Command runner for executing scripts (enables mocking)
/// * `script_path` - Path to validator script (e.g., "validators/validate-sqlite.sh")
/// * `json_input` - JSON output from container to validate
/// * `assertions` - Optional assertion rules
/// * `expect` - Optional expected output
/// * `container_stderr` - Optional stderr output from container (for warning detection)
///
/// # Errors
///
/// Returns error if the validator script cannot be spawned or if stdin write fails.
pub fn run_validator<R: CommandRunner>(
    runner: &R,
    script_path: &str,
    json_input: &str,
    assertions: Option<&str>,
    expect: Option<&str>,
    container_stderr: Option<&str>,
) -> Result<HostValidationResult> {
    // Build environment variables
    let mut env_vars: Vec<(&str, &str)> = Vec::new();

    if let Some(a) = assertions {
        env_vars.push(("VALIDATOR_ASSERTIONS", a));
    }
    if let Some(e) = expect {
        env_vars.push(("VALIDATOR_EXPECT", e));
    }
    if let Some(stderr) = container_stderr {
        env_vars.push(("VALIDATOR_CONTAINER_STDERR", stderr));
    }

    let output = runner.run_script(script_path, json_input, &env_vars)?;

    Ok(HostValidationResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}
