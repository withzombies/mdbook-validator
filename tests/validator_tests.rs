// Tests are allowed to panic for assertions and test failure
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

//! Tests for validator module

use mdbook_validator::parser::CodeBlock;
use mdbook_validator::validator::ValidatorInput;

#[test]
fn validator_input_from_code_block_all_fields() {
    let block = CodeBlock {
        language: "sql".to_string(),
        validator: Some("sqlite".to_string()),
        skip: false,
        content: "SELECT 1;".to_string(),
        setup: Some("CREATE TABLE test;".to_string()),
        assertions: Some("rows >= 1".to_string()),
        expect: Some(r#"[{"1": 1}]"#.to_string()),
    };

    let input = ValidatorInput::from(&block);

    assert_eq!(input.content, "SELECT 1;");
    assert_eq!(input.setup, Some("CREATE TABLE test;".to_string()));
    assert_eq!(input.assertions, Some("rows >= 1".to_string()));
    assert_eq!(input.expect, Some(r#"[{"1": 1}]"#.to_string()));
}

#[test]
fn validator_input_from_code_block_minimal() {
    let block = CodeBlock {
        language: "sql".to_string(),
        validator: Some("test".to_string()),
        skip: false,
        content: "SELECT 1;".to_string(),
        setup: None,
        assertions: None,
        expect: None,
    };

    let input = ValidatorInput::from(&block);

    assert_eq!(input.content, "SELECT 1;");
    assert!(input.setup.is_none());
    assert!(input.assertions.is_none());
    assert!(input.expect.is_none());
}
