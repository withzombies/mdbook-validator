//! Tests for markdown parsing and code block extraction

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
