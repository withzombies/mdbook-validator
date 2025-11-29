//! Docker operations abstraction for testing.
//!
//! Provides a trait for Docker exec operations, enabling mocking in tests
//! to cover error paths (`create_exec` failure, `start_exec` failure, `inspect_exec` failure).

use anyhow::Result;
use async_trait::async_trait;
use bollard::exec::{CreateExecOptions, CreateExecResults, StartExecOptions, StartExecResults};
use bollard::service::ExecInspectResponse;
use bollard::Docker;

/// Trait for Docker exec operations.
///
/// Enables mocking in tests to verify error handling without Docker failures.
/// Uses async-trait for dyn dispatch (Rust async fn in traits doesn't support dyn yet).
#[async_trait]
pub trait DockerOperations: Send + Sync {
    /// Create an exec instance in a container.
    async fn create_exec(
        &self,
        container_id: &str,
        options: CreateExecOptions<String>,
    ) -> Result<CreateExecResults>;

    /// Start an exec instance.
    async fn start_exec(
        &self,
        exec_id: &str,
        options: Option<StartExecOptions>,
    ) -> Result<StartExecResults>;

    /// Inspect an exec instance to get exit code.
    async fn inspect_exec(&self, exec_id: &str) -> Result<ExecInspectResponse>;
}

/// Real implementation wrapping [`bollard::Docker`].
///
/// This is the default implementation used in production.
pub struct BollardDocker {
    inner: Docker,
}

impl BollardDocker {
    /// Create a new `BollardDocker` from a [`bollard::Docker`] instance.
    #[must_use]
    pub fn new(docker: Docker) -> Self {
        Self { inner: docker }
    }
}

#[async_trait]
impl DockerOperations for BollardDocker {
    async fn create_exec(
        &self,
        container_id: &str,
        options: CreateExecOptions<String>,
    ) -> Result<CreateExecResults> {
        self.inner
            .create_exec(container_id, options)
            .await
            .map_err(|e| anyhow::anyhow!("create_exec failed: {e}"))
    }

    async fn start_exec(
        &self,
        exec_id: &str,
        options: Option<StartExecOptions>,
    ) -> Result<StartExecResults> {
        self.inner
            .start_exec(exec_id, options)
            .await
            .map_err(|e| anyhow::anyhow!("start_exec failed: {e}"))
    }

    async fn inspect_exec(&self, exec_id: &str) -> Result<ExecInspectResponse> {
        self.inner
            .inspect_exec(exec_id)
            .await
            .map_err(|e| anyhow::anyhow!("inspect_exec failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_bollard_docker_new() {
        // Just verify the type compiles with the trait
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BollardDocker>();
    }
}
