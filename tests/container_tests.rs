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
