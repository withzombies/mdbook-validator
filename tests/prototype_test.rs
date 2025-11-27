//! Prototype tests for container + bollard integration
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

#[tokio::test]
async fn test_env_var_data_flow() {
    // Minimal validator that echoes environment variables
    let validator_script = b"#!/bin/sh
echo \"Setup: $VALIDATOR_SETUP\"
echo \"Content: $VALIDATOR_CONTENT\"
echo \"Assertions: $VALIDATOR_ASSERTIONS\"
echo \"Expect: $VALIDATOR_EXPECT\"
exit 0
";

    // 1. Start container with validator script copied in
    // Note: Alpine exits immediately without a command, so use "sleep infinity" to keep it running
    let container = GenericImage::new("alpine", "3")
        .with_copy_to("/validate.sh", validator_script.to_vec())
        .with_cmd(["sleep", "infinity"])
        .start()
        .await;

    let container = match container {
        Ok(c) => c,
        Err(e) => panic!("Container should start. Is Docker running? Error: {e}"),
    };

    // 2. Get container ID and docker client
    let container_id = container.id();
    let docker = docker_client_instance()
        .await
        .expect("Docker client should be available");

    // 3. Create exec with environment variables
    let exec_result = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                env: Some(vec![
                    "VALIDATOR_SETUP=CREATE TABLE test;".to_string(),
                    "VALIDATOR_CONTENT=SELECT 1;".to_string(),
                    "VALIDATOR_ASSERTIONS=rows >= 1".to_string(),
                    "VALIDATOR_EXPECT=[{\"id\": 1}]".to_string(),
                ]),
                cmd: Some(vec!["sh".to_string(), "/validate.sh".to_string()]),
                ..Default::default()
            },
        )
        .await;

    let exec_id = match exec_result {
        Ok(r) => r.id,
        Err(e) => panic!("Create exec should succeed: {e}"),
    };

    // 4. Start exec and collect output
    let start_result = docker
        .start_exec(&exec_id, Some(StartExecOptions::default()))
        .await;

    let start_exec_result = match start_result {
        Ok(r) => r,
        Err(e) => panic!("Start exec should succeed: {e}"),
    };

    let StartExecResults::Attached { mut output, .. } = start_exec_result else {
        panic!("Exec should be attached");
    };

    // 5. Collect stdout
    let mut stdout = Vec::new();
    while let Some(result) = output.next().await {
        match result {
            Ok(bollard::container::LogOutput::StdOut { message }) => {
                stdout.extend_from_slice(&message);
            }
            Ok(bollard::container::LogOutput::StdErr { message }) => {
                eprintln!("stderr: {}", String::from_utf8_lossy(&message));
            }
            Ok(_) => {}
            Err(e) => panic!("Output stream error: {e}"),
        }
    }

    // 6. Get exit code
    let inspect = docker
        .inspect_exec(&exec_id)
        .await
        .expect("Inspect exec should succeed");
    let exit_code = inspect.exit_code.unwrap_or(-1);

    // 7. Verify results
    let output_str = String::from_utf8_lossy(&stdout);
    println!("Output: {output_str}");

    assert_eq!(exit_code, 0, "Validator should exit 0");
    assert!(
        output_str.contains("SELECT 1"),
        "Should see content in output: {output_str}"
    );
    assert!(
        output_str.contains("CREATE TABLE"),
        "Should see setup in output: {output_str}"
    );
    assert!(
        output_str.contains("rows >= 1"),
        "Should see assertions in output: {output_str}"
    );

    println!("Prototype test passed! Environment variable data flow works.");
}

#[tokio::test]
async fn test_validator_exit_nonzero_captured() {
    // Validator that fails
    let validator_script = b"#!/bin/sh
echo \"Failing validator\"
exit 1
";

    // Note: Alpine exits immediately without a command, so use "sleep infinity" to keep it running
    let container = GenericImage::new("alpine", "3")
        .with_copy_to("/validate.sh", validator_script.to_vec())
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("Container should start");

    let container_id = container.id();
    let docker = docker_client_instance().await.expect("Docker client");

    let exec_id = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["sh".to_string(), "/validate.sh".to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("Create exec")
        .id;

    let StartExecResults::Attached { mut output, .. } = docker
        .start_exec(&exec_id, Some(StartExecOptions::default()))
        .await
        .expect("Start exec")
    else {
        panic!("Should be attached");
    };

    // Drain output
    while output.next().await.is_some() {}

    let inspect = docker.inspect_exec(&exec_id).await.expect("Inspect");
    let exit_code = inspect.exit_code.unwrap_or(-1);

    assert_eq!(exit_code, 1, "Validator should exit 1");
    println!("Non-zero exit code correctly captured.");
}
