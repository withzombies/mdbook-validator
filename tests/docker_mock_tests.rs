//! Tests for `DockerOperations` trait error paths using mocks.
//!
//! These tests verify that container.rs properly handles and propagates
//! errors from Docker operations (`create_exec`, `start_exec`, `inspect_exec`).
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bollard::exec::{CreateExecOptions, CreateExecResults, StartExecOptions, StartExecResults};
use bollard::service::ExecInspectResponse;
use mdbook_validator::container::ValidatorContainer;
use mdbook_validator::docker::DockerOperations;
use mdbook_validator::error::ValidatorError;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

/// Mock that fails on `create_exec`
struct FailOnCreateExec {
    error_message: &'static str,
}

#[async_trait]
impl DockerOperations for FailOnCreateExec {
    async fn create_exec(
        &self,
        _container_id: &str,
        _options: CreateExecOptions<String>,
    ) -> Result<CreateExecResults> {
        Err(ValidatorError::ContainerExec {
            message: format!("create_exec failed: {}", self.error_message),
        }
        .into())
    }

    async fn start_exec(
        &self,
        _exec_id: &str,
        _options: Option<StartExecOptions>,
    ) -> Result<StartExecResults> {
        panic!("start_exec should not be called when create_exec fails");
    }

    async fn inspect_exec(&self, _exec_id: &str) -> Result<ExecInspectResponse> {
        panic!("inspect_exec should not be called when create_exec fails");
    }
}

/// Mock that succeeds on `create_exec` but fails on `start_exec`
struct FailOnStartExec {
    error_message: &'static str,
}

#[async_trait]
impl DockerOperations for FailOnStartExec {
    async fn create_exec(
        &self,
        _container_id: &str,
        _options: CreateExecOptions<String>,
    ) -> Result<CreateExecResults> {
        Ok(CreateExecResults {
            id: "test-exec-id".to_owned(),
        })
    }

    async fn start_exec(
        &self,
        _exec_id: &str,
        _options: Option<StartExecOptions>,
    ) -> Result<StartExecResults> {
        Err(ValidatorError::ContainerExec {
            message: format!("start_exec failed: {}", self.error_message),
        }
        .into())
    }

    async fn inspect_exec(&self, _exec_id: &str) -> Result<ExecInspectResponse> {
        panic!("inspect_exec should not be called when start_exec fails");
    }
}

// === Error path tests ===

#[tokio::test]
async fn test_create_exec_failure_returns_error() {
    // Start a real container (we need this for ValidatorContainer)
    let container = GenericImage::new("alpine", "3")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("Failed to start test container");

    // Create mock that fails on create_exec
    let mock_docker = Arc::new(FailOnCreateExec {
        error_message: "container not found",
    });

    // Create container with mock docker
    let validator = ValidatorContainer::with_docker(container, mock_docker);

    // Try to exec - should fail with create_exec error
    let result = validator.exec_raw(&["echo", "test"]).await;

    assert!(result.is_err(), "Expected error when create_exec fails");
    let err = result
        .unwrap_err()
        .downcast::<ValidatorError>()
        .expect("should be ValidatorError");
    assert!(matches!(err, ValidatorError::ContainerExec { .. }));
    assert!(
        err.to_string().contains("container not found"),
        "Error should contain our message: {}",
        err
    );
}

#[tokio::test]
async fn test_start_exec_failure_returns_error() {
    // Start a real container
    let container = GenericImage::new("alpine", "3")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("Failed to start test container");

    // Create mock that fails on start_exec
    let mock_docker = Arc::new(FailOnStartExec {
        error_message: "exec instance not running",
    });

    // Create container with mock docker
    let validator = ValidatorContainer::with_docker(container, mock_docker);

    // Try to exec - should fail with start_exec error
    let result = validator.exec_raw(&["echo", "test"]).await;

    assert!(result.is_err(), "Expected error when start_exec fails");
    let err = result
        .unwrap_err()
        .downcast::<ValidatorError>()
        .expect("should be ValidatorError");
    assert!(matches!(err, ValidatorError::ContainerExec { .. }));
    assert!(
        err.to_string().contains("exec instance not running"),
        "Error should contain our message: {}",
        err
    );
}

#[tokio::test]
async fn test_exec_with_env_create_exec_failure() {
    // Test that exec_with_env also propagates create_exec errors
    let container = GenericImage::new("alpine", "3")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("Failed to start test container");

    let mock_docker = Arc::new(FailOnCreateExec {
        error_message: "container paused",
    });

    let validator = ValidatorContainer::with_docker(container, mock_docker);

    let result = validator.exec_with_env(None, "content", None, None).await;

    assert!(result.is_err(), "Expected error when create_exec fails");
    let err = result
        .unwrap_err()
        .downcast::<ValidatorError>()
        .expect("should be ValidatorError");
    assert!(matches!(err, ValidatorError::ContainerExec { .. }));
    assert!(
        err.to_string().contains("container paused"),
        "Error should contain our message: {}",
        err
    );
}

#[tokio::test]
async fn test_exec_with_stdin_create_exec_failure() {
    // Test that exec_with_stdin also propagates create_exec errors
    let container = GenericImage::new("alpine", "3")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("Failed to start test container");

    let mock_docker = Arc::new(FailOnCreateExec {
        error_message: "no such container",
    });

    let validator = ValidatorContainer::with_docker(container, mock_docker);

    let result = validator.exec_with_stdin(&["cat"], "input").await;

    assert!(result.is_err(), "Expected error when create_exec fails");
    let err = result
        .unwrap_err()
        .downcast::<ValidatorError>()
        .expect("should be ValidatorError");
    assert!(matches!(err, ValidatorError::ContainerExec { .. }));
    assert!(
        err.to_string().contains("no such container"),
        "Error should contain our message: {}",
        err
    );
}

// === inspect_exec failure test ===
// Note: Testing inspect_exec failure with mocks is complex because we'd need
// to create fake tokio streams. Instead, we test the real implementation
// with an invalid exec ID to verify error propagation works.

#[tokio::test]
async fn test_inspect_exec_failure_returns_error() {
    use mdbook_validator::docker::BollardDocker;
    use testcontainers::core::client::docker_client_instance;

    // Get real Docker client
    let docker = docker_client_instance()
        .await
        .expect("Docker should be available");

    let bollard_docker = BollardDocker::new(docker);

    // Try to inspect a non-existent exec ID
    let result = bollard_docker.inspect_exec("nonexistent-exec-id").await;

    assert!(result.is_err(), "Expected error for invalid exec ID");
    let err = result
        .unwrap_err()
        .downcast::<ValidatorError>()
        .expect("should be ValidatorError");
    assert!(matches!(err, ValidatorError::ContainerExec { .. }));
    assert!(
        err.to_string().contains("inspect_exec failed"),
        "Error should be wrapped with context: {}",
        err
    );
}

// === Trait implementation tests ===

#[test]
fn test_docker_operations_is_send_sync() {
    // Verify trait bounds are met for trait objects
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn DockerOperations>();
}

#[test]
fn test_fail_mocks_implement_trait() {
    // Verify our mocks properly implement the trait
    fn assert_docker_ops<T: DockerOperations>() {}
    assert_docker_ops::<FailOnCreateExec>();
    assert_docker_ops::<FailOnStartExec>();
}
