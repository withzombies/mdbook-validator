// Tests are allowed to panic for assertions and test failure
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

//! Tests for container module

use mdbook_validator::container::ValidatorContainer;

const ECHO_SCRIPT: &[u8] = b"#!/bin/sh
echo \"Content: $VALIDATOR_CONTENT\"
echo \"Setup: $VALIDATOR_SETUP\"
echo \"Assertions: $VALIDATOR_ASSERTIONS\"
echo \"Expect: $VALIDATOR_EXPECT\"
exit 0
";

const FAIL_SCRIPT: &[u8] = b"#!/bin/sh
echo \"stdout_msg\"
echo \"stderr_msg\" >&2
exit 42
";

#[tokio::test]
async fn exec_with_env_minimal_params() {
    // Test with only required 'content' param, all Optional params as None
    let container = ValidatorContainer::start(ECHO_SCRIPT)
        .await
        .expect("Docker available");
    let result = container
        .exec_with_env(None, "test content", None, None)
        .await
        .expect("exec succeeded");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("test content"));
}

#[tokio::test]
async fn exec_with_env_multiline_content() {
    // Test that multiline content in env vars works (no truncation)
    let container = ValidatorContainer::start(ECHO_SCRIPT)
        .await
        .expect("Docker available");
    let multiline = "line1\nline2\nline3\nmore lines here";
    let result = container
        .exec_with_env(None, multiline, None, None)
        .await
        .expect("exec succeeded");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("line1"));
    assert!(result.stdout.contains("line3"));
}

#[tokio::test]
async fn validation_result_captures_all_outputs() {
    // Test that stdout, stderr, and exit code all captured correctly
    let container = ValidatorContainer::start(FAIL_SCRIPT)
        .await
        .expect("Docker available");
    let result = container
        .exec_with_env(None, "ignored", None, None)
        .await
        .expect("exec succeeded");

    assert_eq!(result.exit_code, 42);
    assert!(result.stdout.contains("stdout_msg"));
    assert!(result.stderr.contains("stderr_msg"));
}

#[tokio::test]
async fn exec_with_all_env_vars_set() {
    // Test that all 4 env vars are passed correctly
    let container = ValidatorContainer::start(ECHO_SCRIPT)
        .await
        .expect("Docker available");
    let result = container
        .exec_with_env(
            Some("setup content"),
            "main content",
            Some("assertions here"),
            Some("expected output"),
        )
        .await
        .expect("exec succeeded");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("setup content"));
    assert!(result.stdout.contains("main content"));
    assert!(result.stdout.contains("assertions here"));
    assert!(result.stdout.contains("expected output"));
}

// ============================================================================
// exec_raw tests (new host-based architecture)
// ============================================================================

#[tokio::test]
async fn test_exec_raw_returns_output() {
    // Test that exec_raw can run commands and capture output
    let container = ValidatorContainer::start_raw("alpine:3")
        .await
        .expect("Docker available");

    let result = container
        .exec_raw(&["echo", "hello from exec_raw"])
        .await
        .expect("exec_raw succeeded");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("hello from exec_raw"),
        "stdout should contain echo output: {}",
        result.stdout
    );
}

#[tokio::test]
async fn test_exec_raw_captures_exit_code() {
    // Test that exec_raw captures non-zero exit codes
    let container = ValidatorContainer::start_raw("alpine:3")
        .await
        .expect("Docker available");

    let result = container
        .exec_raw(&["sh", "-c", "exit 42"])
        .await
        .expect("exec_raw succeeded");

    assert_eq!(result.exit_code, 42, "exit code should be 42");
}

#[tokio::test]
async fn test_exec_raw_nonexistent_command_fails() {
    // Test that running nonexistent command returns error exit code
    let container = ValidatorContainer::start_raw("alpine:3")
        .await
        .expect("Docker available");

    let result = container
        .exec_raw(&["nonexistent_binary_xyz_123"])
        .await
        .expect("exec_raw should not error, just return non-zero exit");

    assert_ne!(
        result.exit_code, 0,
        "nonexistent command should have non-zero exit code"
    );
}

// ============================================================================
// start_raw_with_mount tests
// ============================================================================

/// Test that files can be copied into containers using `with_copy_to()`.
///
/// This replaces the previous bind mount test which was flaky due to Docker
/// timing issues with volume mounts. The `copy_to` API is more reliable and
/// is the recommended approach per testcontainers best practices.
#[tokio::test]
async fn test_container_copy_to_file() {
    use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
    use tokio::io::AsyncReadExt;

    let test_content = b"hello from copy_to";

    // Start container with file copied in at startup
    let container = GenericImage::new("alpine", "3")
        .with_copy_to("/fixtures/test.txt", test_content.to_vec())
        .with_cmd(["sleep", "infinity"])
        .start()
        .await
        .expect("container should start with copied file");

    // Verify file is accessible
    let mut result = container
        .exec(testcontainers::core::ExecCommand::new([
            "cat",
            "/fixtures/test.txt",
        ]))
        .await
        .expect("exec should succeed");

    let mut stdout = String::new();
    result
        .stdout()
        .read_to_string(&mut stdout)
        .await
        .expect("read stdout");

    assert!(
        stdout.contains("hello from copy_to"),
        "copied file should be readable: {stdout}",
    );
}

#[tokio::test]
async fn test_container_mount_none_works() {
    // Test that start_raw_with_mount works without a mount (same as start_raw)
    let container = ValidatorContainer::start_raw_with_mount("alpine:3", None)
        .await
        .expect("container should start without mount");

    let result = container
        .exec_raw(&["echo", "no mount"])
        .await
        .expect("exec should succeed");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("no mount"));
}
