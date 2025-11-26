//! Integration tests for mdbook-validator

use mdbook::preprocess::Preprocessor;
use mdbook_validator::ValidatorPreprocessor;

#[test]
fn preprocessor_has_correct_name() {
    let preprocessor = ValidatorPreprocessor::new();
    assert_eq!(preprocessor.name(), "validator");
}

#[test]
fn preprocessor_supports_html_renderer() {
    let preprocessor = ValidatorPreprocessor::new();
    assert!(preprocessor.supports_renderer("html"));
}

#[test]
fn preprocessor_does_not_support_other_renderers() {
    let preprocessor = ValidatorPreprocessor::new();
    assert!(!preprocessor.supports_renderer("pdf"));
    assert!(!preprocessor.supports_renderer("epub"));
}
