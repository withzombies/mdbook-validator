//! Malformed input validation tests
//!
//! Tests that validators correctly reject invalid inputs with
//! appropriate error messages. These tests verify the error handling
//! paths in validator scripts work correctly (defense-in-depth).

// Tests are allowed to panic for assertions and test failure
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use mdbook_validator::command::RealCommandRunner;
use mdbook_validator::host_validator;

const SQLITE_VALIDATOR: &str = "validators/validate-sqlite.sh";

/// Run sqlite validator with given JSON input and assertions.
/// Returns (`exit_code`, stdout, stderr).
fn run_validator_with_input(json_input: &str, assertions: Option<&str>) -> (i32, String, String) {
    let runner = RealCommandRunner;
    let result = host_validator::run_validator(
        &runner,
        SQLITE_VALIDATOR,
        json_input,
        assertions,
        None,
        None,
    )
    .expect("validator should run");
    (result.exit_code, result.stdout, result.stderr)
}

// =============================================================================
// Invalid assertion syntax tests (3 tests)
// =============================================================================

#[test]
fn test_rows_non_numeric_value_rejected() {
    // rows = abc should fail with "invalid integer" error
    let (exit_code, _stdout, stderr) = run_validator_with_input("[]", Some("rows = abc"));

    assert_eq!(exit_code, 1, "should exit with code 1 for invalid integer");
    assert!(
        stderr.contains("invalid integer"),
        "stderr should contain 'invalid integer': {stderr}"
    );
}

#[test]
fn test_rows_empty_value_treated_as_unknown_syntax() {
    // "rows = " (with trailing space, no value) gets trimmed to "rows ="
    // which doesn't match "rows = *" pattern, so it's treated as unknown syntax
    let (exit_code, _stdout, stderr) = run_validator_with_input("[]", Some("rows = "));

    assert_eq!(
        exit_code, 1,
        "should exit with code 1 for malformed assertion"
    );
    // After xargs trim, "rows = " becomes "rows =" which doesn't match case patterns
    assert!(
        stderr.contains("Unknown assertion syntax"),
        "stderr should contain 'Unknown assertion syntax': {stderr}"
    );
}

#[test]
fn test_unknown_assertion_rejected() {
    // Completely unknown assertion type should be rejected
    let (exit_code, _stdout, stderr) = run_validator_with_input("[]", Some("foobar = 123"));

    assert_eq!(
        exit_code, 1,
        "should exit with code 1 for unknown assertion"
    );
    assert!(
        stderr.contains("Unknown assertion syntax"),
        "stderr should contain 'Unknown assertion syntax': {stderr}"
    );
}

// =============================================================================
// Empty/malformed JSON tests (3 tests)
// =============================================================================

#[test]
fn test_empty_string_json_passes_without_assertions() {
    // Empty string with no assertions: script exits successfully because
    // the jq empty check on empty stdin succeeds silently (no output = valid)
    // This documents actual behavior - empty input without assertions is OK
    let (exit_code, _stdout, _stderr) = run_validator_with_input("", None);

    assert_eq!(
        exit_code, 0,
        "empty input with no assertions should pass (jq empty succeeds)"
    );
}

#[test]
fn test_null_json_row_count_works() {
    // jq 'length' on null returns 0, so "rows = 0" should pass
    // This documents that null is treated as an empty array for row counting
    let (exit_code, _stdout, _stderr) = run_validator_with_input("null", Some("rows = 0"));

    assert_eq!(
        exit_code, 0,
        "null JSON with rows = 0 should pass (jq length null = 0)"
    );
}

#[test]
fn test_malformed_json_rejected() {
    // Malformed JSON should be rejected with "Invalid JSON output"
    let (exit_code, _stdout, stderr) = run_validator_with_input("{broken", None);

    assert_eq!(exit_code, 1, "should exit with code 1 for malformed JSON");
    assert!(
        stderr.contains("Invalid JSON"),
        "stderr should contain 'Invalid JSON': {stderr}"
    );
}

// =============================================================================
// Edge case tests (4 tests)
// =============================================================================

#[test]
fn test_negative_row_count_fails_comparison() {
    // is_integer() accepts negative numbers (regex: ^-?[0-9]+$)
    // But rows can't be negative in practice, so rows = -5 fails the comparison
    // (actual row count 0 != -5)
    let (exit_code, _stdout, stderr) = run_validator_with_input("[]", Some("rows = -5"));

    assert_eq!(
        exit_code, 1,
        "should exit with code 1 (0 rows != -5 expected)"
    );
    assert!(
        stderr.contains("got 0"),
        "stderr should show actual count: {stderr}"
    );
}

#[test]
fn test_contains_empty_string_always_matches() {
    // Every string contains the empty string, so this should pass
    let (exit_code, _stdout, _stderr) =
        run_validator_with_input(r#"[{"x": "test"}]"#, Some(r#"contains """#));

    assert_eq!(exit_code, 0, "contains empty string should always match");
}

#[test]
fn test_contains_with_quotes_in_value() {
    // Test that contains works with JSON containing quoted strings
    // The JSON value has quotes escaped as \"
    let (exit_code, _stdout, _stderr) = run_validator_with_input(
        r#"[{"msg": "say \"hello\" to the world"}]"#,
        Some(r#"contains "hello""#),
    );

    assert_eq!(
        exit_code, 0,
        "should find substring even with quotes in value"
    );
}

#[test]
fn test_multiple_assertions_first_fails() {
    // When multiple assertions are provided and the first one fails,
    // the validator should fail fast with the first error
    let assertions = "rows = abc\ncontains \"test\"";
    let (exit_code, _stdout, stderr) = run_validator_with_input("[]", Some(assertions));

    assert_eq!(exit_code, 1, "should fail on first bad assertion");
    assert!(
        stderr.contains("invalid integer"),
        "stderr should show first assertion failure: {stderr}"
    );
}
