//! Container lifecycle management using testcontainers + bollard
//!
//! Uses testcontainers async API to start containers and bollard
//! for exec with environment variables.

use anyhow::{Context, Result};
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use futures_util::StreamExt;
use testcontainers::core::client::docker_client_instance;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

/// Result of executing a validator
#[derive(Debug)]
pub struct ValidationResult {
    /// Exit code from the validator (0 = success)
    pub exit_code: i64,
    /// Standard output from the validator
    pub stdout: String,
    /// Standard error from the validator
    pub stderr: String,
}

/// Manages validator container lifecycle
///
/// Starts an Alpine container with a validator script copied in,
/// then executes the script with environment variables for validation data.
pub struct ValidatorContainer {
    /// Kept alive to prevent container cleanup (testcontainers drops on Drop)
    _container: ContainerAsync<GenericImage>,
    container_id: String,
}

impl ValidatorContainer {
    /// Start a new validator container with the given script.
    ///
    /// The script is copied to `/validate.sh` inside the container.
    /// Container uses `sleep infinity` to stay running for exec calls.
    ///
    /// # Errors
    ///
    /// Returns error if Docker is not running or container fails to start.
    pub async fn start(validator_script: &[u8]) -> Result<Self> {
        let container = GenericImage::new("alpine", "3")
            .with_copy_to("/validate.sh", validator_script.to_vec())
            .with_cmd(["sleep", "infinity"])
            .start()
            .await
            .context("Failed to start container. Is Docker running?")?;

        let container_id = container.id().to_owned();
        Ok(Self {
            _container: container,
            container_id,
        })
    }

    /// Execute validator with environment variables.
    ///
    /// Environment variables:
    /// - `VALIDATOR_CONTENT`: The visible code content (always set)
    /// - `VALIDATOR_SETUP`: Setup content (if present)
    /// - `VALIDATOR_ASSERTIONS`: Assertion rules (if present)
    /// - `VALIDATOR_EXPECT`: Expected output (if present)
    ///
    /// # Errors
    ///
    /// Returns error if exec creation or execution fails.
    pub async fn exec_with_env(
        &self,
        setup: Option<&str>,
        content: &str,
        assertions: Option<&str>,
        expect: Option<&str>,
    ) -> Result<ValidationResult> {
        let docker = docker_client_instance()
            .await
            .context("Failed to get Docker client")?;

        let mut env_vars = vec![format!("VALIDATOR_CONTENT={content}")];
        if let Some(s) = setup {
            env_vars.push(format!("VALIDATOR_SETUP={s}"));
        }
        if let Some(a) = assertions {
            env_vars.push(format!("VALIDATOR_ASSERTIONS={a}"));
        }
        if let Some(e) = expect {
            env_vars.push(format!("VALIDATOR_EXPECT={e}"));
        }

        let exec = docker
            .create_exec(
                &self.container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    env: Some(env_vars),
                    cmd: Some(vec!["sh".to_owned(), "/validate.sh".to_owned()]),
                    ..Default::default()
                },
            )
            .await
            .context("Failed to create exec")?;

        let exec_id = exec.id;

        let start_result = docker
            .start_exec(&exec_id, Some(StartExecOptions::default()))
            .await
            .context("Failed to start exec")?;

        let StartExecResults::Attached { mut output, .. } = start_result else {
            anyhow::bail!("Exec should be attached but wasn't");
        };

        // Collect stdout and stderr
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        while let Some(result) = output.next().await {
            match result {
                Ok(bollard::container::LogOutput::StdOut { message }) => {
                    stdout.extend_from_slice(&message);
                }
                Ok(bollard::container::LogOutput::StdErr { message }) => {
                    stderr.extend_from_slice(&message);
                }
                Ok(_) => {}
                Err(e) => {
                    anyhow::bail!("Output stream error: {e}");
                }
            }
        }

        // Get exit code
        let inspect = docker
            .inspect_exec(&exec_id)
            .await
            .context("Failed to inspect exec")?;
        let exit_code = inspect.exit_code.unwrap_or(-1);

        Ok(ValidationResult {
            exit_code,
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
        })
    }

    /// Get the container ID
    #[must_use]
    pub fn id(&self) -> &str {
        &self.container_id
    }
}
