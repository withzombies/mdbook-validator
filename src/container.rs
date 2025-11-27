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
    /// Start a new validator container with the given image and script.
    ///
    /// The script is copied to `/validate.sh` inside the container.
    /// Container uses `sleep infinity` to stay running for exec calls.
    ///
    /// # Arguments
    ///
    /// * `image` - Docker image in "name:tag" format (e.g., "osquery/osquery:5.17.0-ubuntu22.04")
    /// * `validator_script` - Script content to copy to `/validate.sh`
    ///
    /// # Errors
    ///
    /// Returns error if Docker is not running or container fails to start.
    pub async fn start_with_image(image: &str, validator_script: &[u8]) -> Result<Self> {
        let (name, tag) = image.rsplit_once(':').unwrap_or((image, "latest"));

        let container = GenericImage::new(name, tag)
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

    /// Start a new validator container with the default Alpine image.
    ///
    /// The script is copied to `/validate.sh` inside the container.
    /// Container uses `sleep infinity` to stay running for exec calls.
    ///
    /// # Errors
    ///
    /// Returns error if Docker is not running or container fails to start.
    pub async fn start(validator_script: &[u8]) -> Result<Self> {
        Self::start_with_image("alpine:3", validator_script).await
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

    /// Execute a raw command in the container and return output.
    ///
    /// This is a lower-level method than `exec_with_env` that runs arbitrary
    /// commands without environment variables or script injection.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Command and arguments to execute (e.g., `&["sqlite3", "-json", "/tmp/db", "SELECT 1"]`)
    ///
    /// # Errors
    ///
    /// Returns error if exec creation or execution fails.
    pub async fn exec_raw(&self, cmd: &[&str]) -> Result<ValidationResult> {
        let docker = docker_client_instance()
            .await
            .context("Failed to get Docker client")?;

        let cmd_owned: Vec<String> = cmd.iter().map(|s| (*s).to_owned()).collect();

        let exec = docker
            .create_exec(
                &self.container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd_owned),
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

    /// Start a container without copying a validator script.
    ///
    /// This is for the new architecture where validators run on the host,
    /// and containers only provide the tool (sqlite3, osquery, etc.).
    ///
    /// # Arguments
    ///
    /// * `image` - Docker image in "name:tag" format
    ///
    /// # Errors
    ///
    /// Returns error if Docker is not running or container fails to start.
    pub async fn start_raw(image: &str) -> Result<Self> {
        let (name, tag) = image.rsplit_once(':').unwrap_or((image, "latest"));

        let container = GenericImage::new(name, tag)
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
}
