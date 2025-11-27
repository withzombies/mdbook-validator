//! Container image verification tests
//!
//! These tests prove we can start each planned container and execute within it.
//! Each test verifies: image pulls, container starts, expected binary executes, exit code 0.
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::str_to_string
)]

use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use futures_util::StreamExt;
use testcontainers::core::client::docker_client_instance;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

/// Result of executing a command in a container
struct ExecResult {
    exit_code: i64,
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
}

/// Execute a command in a running container and return the result
async fn exec_command(container_id: &str, cmd: &[&str]) -> ExecResult {
    let docker = docker_client_instance()
        .await
        .expect("Docker client should be available");

    let exec_id = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd.iter().map(|s| (*s).to_owned()).collect()),
                ..Default::default()
            },
        )
        .await
        .expect("Create exec should succeed")
        .id;

    let StartExecResults::Attached { mut output, .. } = docker
        .start_exec(&exec_id, Some(StartExecOptions::default()))
        .await
        .expect("Start exec should succeed")
    else {
        panic!("Exec should be attached");
    };

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
            Err(e) => panic!("Output stream error: {e}"),
        }
    }

    let inspect = docker
        .inspect_exec(&exec_id)
        .await
        .expect("Inspect exec should succeed");
    let exit_code = inspect.exit_code.unwrap_or(-1);

    ExecResult {
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    }
}

/// Test: osquery container starts and osqueryi executes
///
/// Image: osquery/osquery:5.17.0-ubuntu22.04
/// Verifies: osqueryi --version returns version info and exits 0
#[tokio::test]
async fn osquery_container_starts_and_executes() {
    let container = GenericImage::new("osquery/osquery", "5.17.0-ubuntu22.04")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await;

    let container = match container {
        Ok(c) => c,
        Err(e) => {
            panic!("osquery container should start: {e}");
        }
    };

    let result = exec_command(container.id(), &["osqueryi", "--version"]).await;

    println!("osquery output: {}", result.stdout);
    assert_eq!(result.exit_code, 0, "osqueryi --version should exit 0");
    assert!(
        result.stdout.contains("osqueryi version"),
        "Output should contain version info: {}",
        result.stdout
    );

    println!("osquery container test passed!");
}

/// Test: `SQLite` container starts and `sqlite3` executes
///
/// Image: keinos/sqlite3:3.47.2
/// Verifies: sqlite3 --version returns version info and exits 0
#[tokio::test]
async fn sqlite_container_starts_and_executes() {
    let container = GenericImage::new("keinos/sqlite3", "3.47.2")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await;

    let container = match container {
        Ok(c) => c,
        Err(e) => {
            panic!("SQLite container should start: {e}");
        }
    };

    let result = exec_command(container.id(), &["sqlite3", "--version"]).await;

    println!("SQLite output: {}", result.stdout);
    assert_eq!(result.exit_code, 0, "sqlite3 --version should exit 0");
    // SQLite version format: "3.X.Y date hash (bits)"
    // Accept any 3.x version since the container may have a newer version
    assert!(
        result.stdout.starts_with("3."),
        "Output should contain SQLite version (3.x.x): {}",
        result.stdout
    );

    println!("SQLite container test passed!");
}

/// Test: `ShellCheck` container starts and shellcheck executes
///
/// Image: koalaman/shellcheck-alpine:v0.10.0
/// Verifies: shellcheck --version returns version info and exits 0
#[tokio::test]
async fn shellcheck_container_starts_and_executes() {
    let container = GenericImage::new("koalaman/shellcheck-alpine", "v0.10.0")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await;

    let container = match container {
        Ok(c) => c,
        Err(e) => {
            panic!("ShellCheck container should start: {e}");
        }
    };

    let result = exec_command(container.id(), &["shellcheck", "--version"]).await;

    println!("ShellCheck output: {}", result.stdout);
    assert_eq!(result.exit_code, 0, "shellcheck --version should exit 0");
    assert!(
        result.stdout.contains("0.10") || result.stdout.to_lowercase().contains("shellcheck"),
        "Output should contain version info: {}",
        result.stdout
    );

    println!("ShellCheck container test passed!");
}

/// Test: Python container starts and python executes
///
/// Image: python:3.12-slim-bookworm
/// Verifies: python --version returns version info and exits 0
#[tokio::test]
async fn python_container_starts_and_executes() {
    let container = GenericImage::new("python", "3.12-slim-bookworm")
        .with_cmd(["sleep", "infinity"])
        .start()
        .await;

    let container = match container {
        Ok(c) => c,
        Err(e) => {
            panic!("Python container should start: {e}");
        }
    };

    let result = exec_command(container.id(), &["python", "--version"]).await;

    println!("Python output: {}", result.stdout);
    assert_eq!(result.exit_code, 0, "python --version should exit 0");
    assert!(
        result.stdout.contains("3.12") || result.stdout.to_lowercase().contains("python"),
        "Output should contain version info: {}",
        result.stdout
    );

    println!("Python container test passed!");
}
