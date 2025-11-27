//! Tests for markdown parsing and code block extraction
#![allow(clippy::str_to_string)]

use mdbook_validator::parser::{extract_markers, parse_info_string};

#[test]
fn parse_info_string_extracts_language_and_validator() {
    let (lang, validator, skip) = parse_info_string("sql validator=sqlite");

    assert_eq!(lang, "sql");
    assert_eq!(validator, Some("sqlite".to_string()));
    assert!(!skip);
}

#[test]
fn parse_info_string_extracts_language_only() {
    let (lang, validator, skip) = parse_info_string("rust");

    assert_eq!(lang, "rust");
    assert_eq!(validator, None);
    assert!(!skip);
}

#[test]
fn parse_info_string_handles_skip_attribute() {
    let (lang, validator, skip) = parse_info_string("sql validator=osquery skip");

    assert_eq!(lang, "sql");
    assert_eq!(validator, Some("osquery".to_string()));
    assert!(skip);
}

#[test]
fn extract_markers_gets_setup_content() {
    let input = r"<!--SETUP
CREATE TABLE test (id INTEGER);
-->
SELECT * FROM test;";

    let markers = extract_markers(input);

    assert_eq!(
        markers.setup,
        Some("CREATE TABLE test (id INTEGER);".to_string())
    );
    assert_eq!(markers.visible_content, "SELECT * FROM test;");
}

#[test]
fn extract_markers_gets_assert_content() {
    let input = r"SELECT COUNT(*) FROM test
<!--ASSERT
rows = 1
-->";

    let markers = extract_markers(input);

    assert_eq!(markers.assertions, Some("rows = 1".to_string()));
    assert_eq!(markers.visible_content, "SELECT COUNT(*) FROM test");
}

#[test]
fn extract_markers_gets_all_marker_types() {
    let input = r#"<!--SETUP
CREATE TABLE t (x INTEGER);
-->
SELECT * FROM t
<!--ASSERT
rows >= 1
-->
<!--EXPECT
[{"x": 1}]
-->"#;

    let markers = extract_markers(input);

    assert_eq!(
        markers.setup,
        Some("CREATE TABLE t (x INTEGER);".to_string())
    );
    assert_eq!(markers.assertions, Some("rows >= 1".to_string()));
    assert_eq!(markers.expect, Some(r#"[{"x": 1}]"#.to_string()));
    assert_eq!(markers.visible_content, "SELECT * FROM t");
}

// === parse_info_string edge cases ===

#[test]
fn parse_info_string_empty_string() {
    let (lang, validator, skip) = parse_info_string("");
    assert_eq!(lang, "");
    assert_eq!(validator, None);
    assert!(!skip);
}

#[test]
fn parse_info_string_empty_validator_value() {
    // `sql validator=` should be treated as no validator (not Some(""))
    let (lang, validator, skip) = parse_info_string("sql validator=");
    assert_eq!(lang, "sql");
    assert_eq!(validator, None); // Empty = no validator
    assert!(!skip);
}

#[test]
fn parse_info_string_whitespace_only_validator() {
    // `sql validator= skip` - the whitespace after = means empty value
    let (lang, validator, skip) = parse_info_string("sql validator= skip");
    assert_eq!(lang, "sql");
    assert_eq!(validator, None); // Empty = no validator
    assert!(skip);
}

// === extract_markers edge cases ===

#[test]
fn extract_markers_malformed_no_closing() {
    // Malformed: no --> closing - should NOT extract marker
    let input = "<!--SETUP\nCREATE TABLE test;\nSELECT 1;";
    let markers = extract_markers(input);

    assert_eq!(markers.setup, None); // Can't extract without closing
                                     // Content preserved (including the malformed marker text)
    assert!(markers.visible_content.contains("SELECT 1"));
}

#[test]
fn extract_markers_empty_marker_content() {
    // Empty content between marker and closing
    let input = "<!--SETUP\n-->\nSELECT 1;";
    let markers = extract_markers(input);

    assert_eq!(markers.setup, Some(String::new())); // Empty, not None
    assert_eq!(markers.visible_content, "SELECT 1;");
}

#[test]
fn extract_markers_no_markers() {
    // Plain content without any markers
    let input = "SELECT 1;";
    let markers = extract_markers(input);

    assert_eq!(markers.setup, None);
    assert_eq!(markers.assertions, None);
    assert_eq!(markers.expect, None);
    assert_eq!(markers.visible_content, "SELECT 1;");
}
