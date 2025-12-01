//! Container lifecycle management using testcontainers + bollard
//!
//! Uses testcontainers async API to start containers and bollard
//! for exec with environment variables.

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::error::ValidatorError;
use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use futures_util::StreamExt;
use testcontainers::core::client::docker_client_instance;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

use crate::docker::{BollardDocker, DockerOperations};

/// Collect stdout/stderr from an exec output stream and get the exit code.
///
/// This is an internal helper used by both `exec_with_env` and `exec_raw` to avoid
/// code duplication in output collection logic.
async fn collect_exec_output(
    docker: &dyn DockerOperations,
    exec_id: &str,
    mut output: impl futures_util::Stream<Item = Result<LogOutput, bollard::errors::Error>> + Unpin,
) -> Result<ValidationResult> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    while let Some(result) = output.next().await {
        match result {
            Ok(LogOutput::StdOut { message }) => {
                stdout.extend_from_slice(&message);
            }
            Ok(LogOutput::StdErr { message }) => {
                stderr.extend_from_slice(&message);
            }
            Ok(_) => {}
            Err(e) => {
                return Err(ValidatorError::ContainerExec {
                    message: format!("Output stream error: {e}"),
                }
                .into());
            }
        }
    }

    // Get exit code
    let inspect = docker.inspect_exec(exec_id).await?;
    let exit_code = inspect.exit_code.unwrap_or(-1);

    Ok(ValidationResult {
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    })
}

/// Result of executing a validator
#[derive(Debug)]
#[must_use]
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
    /// Docker operations for exec calls (injected for testability)
    docker: Arc<dyn DockerOperations>,
}

impl ValidatorContainer {
    /// Create a `ValidatorContainer` with a custom Docker operations implementation.
    ///
    /// This constructor is primarily for testing error paths by injecting mock
    /// Docker implementations. Production code should use `start_with_image`
    /// or `start_raw` instead.
    ///
    /// # Arguments
    ///
    /// * `container` - The testcontainers async container
    /// * `docker` - Docker operations implementation (use `BollardDocker` for production)
    pub fn with_docker(
        container: ContainerAsync<GenericImage>,
        docker: Arc<dyn DockerOperations>,
    ) -> Self {
        let container_id = container.id().to_owned();
        Self {
            _container: container,
            container_id,
            docker,
        }
    }

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

        // Get Docker client and wrap it
        let docker_client = docker_client_instance()
            .await
            .context("Failed to get Docker client")?;
        let docker: Arc<dyn DockerOperations> = Arc::new(BollardDocker::new(docker_client));

        Ok(Self {
            _container: container,
            container_id,
            docker,
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

        let exec = self
            .docker
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
            .await?;

        let exec_id = exec.id;

        let start_result = self
            .docker
            .start_exec(&exec_id, Some(StartExecOptions::default()))
            .await?;

        let StartExecResults::Attached { output, .. } = start_result else {
            return Err(ValidatorError::ContainerExec {
                message: "Exec should be attached but wasn't".into(),
            }
            .into());
        };

        collect_exec_output(self.docker.as_ref(), &exec_id, output).await
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
        let cmd_owned: Vec<String> = cmd.iter().map(|s| (*s).to_owned()).collect();

        let exec = self
            .docker
            .create_exec(
                &self.container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd_owned),
                    ..Default::default()
                },
            )
            .await?;

        let exec_id = exec.id;

        let start_result = self
            .docker
            .start_exec(&exec_id, Some(StartExecOptions::default()))
            .await?;

        let StartExecResults::Attached { output, .. } = start_result else {
            return Err(ValidatorError::ContainerExec {
                message: "Exec should be attached but wasn't".into(),
            }
            .into());
        };

        collect_exec_output(self.docker.as_ref(), &exec_id, output).await
    }

    /// Execute a command in the container with stdin content.
    ///
    /// This passes content via stdin instead of shell interpolation, eliminating
    /// shell injection risks from special characters in the content.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Command and arguments to execute (e.g., `&["cat"]`)
    /// * `stdin_content` - Content to pass via stdin
    ///
    /// # Errors
    ///
    /// Returns error if exec creation, stdin write, or execution fails.
    pub async fn exec_with_stdin(
        &self,
        cmd: &[&str],
        stdin_content: &str,
    ) -> Result<ValidationResult> {
        use tokio::io::AsyncWriteExt;

        let cmd_owned: Vec<String> = cmd.iter().map(|s| (*s).to_owned()).collect();

        let exec = self
            .docker
            .create_exec(
                &self.container_id,
                CreateExecOptions {
                    attach_stdin: Some(true),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd_owned),
                    ..Default::default()
                },
            )
            .await?;

        let exec_id = exec.id;

        let start_result = self
            .docker
            .start_exec(&exec_id, Some(StartExecOptions::default()))
            .await?;

        let StartExecResults::Attached { output, mut input } = start_result else {
            return Err(ValidatorError::ContainerExec {
                message: "Exec should be attached but wasn't".into(),
            }
            .into());
        };

        // Write stdin content and close to signal EOF
        input
            .write_all(stdin_content.as_bytes())
            .await
            .context("Failed to write to stdin")?;
        input.shutdown().await.context("Failed to close stdin")?;

        collect_exec_output(self.docker.as_ref(), &exec_id, output).await
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
        Self::start_raw_with_mount(image, None).await
    }

    /// Start a container with an optional host directory mounted.
    ///
    /// This is for the new architecture where validators run on the host,
    /// and containers only provide the tool (sqlite3, osquery, etc.).
    ///
    /// # Arguments
    ///
    /// * `image` - Docker image in "name:tag" format
    /// * `mount` - Optional (`host_path`, `container_path`) tuple for bind mount
    ///
    /// # Errors
    ///
    /// Returns error if Docker is not running or container fails to start.
    pub async fn start_raw_with_mount(
        image: &str,
        mount: Option<(&std::path::Path, &str)>,
    ) -> Result<Self> {
        use testcontainers::core::Mount;

        let (name, tag) = image.rsplit_once(':').unwrap_or((image, "latest"));

        let base_image = GenericImage::new(name, tag).with_cmd(["sleep", "infinity"]);

        let container = if let Some((host_path, container_path)) = mount {
            let host_str = host_path.to_string_lossy().to_string();
            base_image
                .with_mount(Mount::bind_mount(host_str, container_path))
                .start()
                .await
                .context("Failed to start container with mount. Is Docker running?")?
        } else {
            base_image
                .start()
                .await
                .context("Failed to start container. Is Docker running?")?
        };

        let container_id = container.id().to_owned();

        // Get Docker client and wrap it
        let docker_client = docker_client_instance()
            .await
            .context("Failed to get Docker client")?;
        let docker: Arc<dyn DockerOperations> = Arc::new(BollardDocker::new(docker_client));

        Ok(Self {
            _container: container,
            container_id,
            docker,
        })
    }
}
