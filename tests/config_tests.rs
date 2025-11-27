//! Configuration parsing tests
//!
//! Tests for book.toml configuration loading.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::needless_raw_string_hashes
)]

use std::path::PathBuf;

use mdbook_validator::config::{Config, ValidatorConfig};

/// Test: ValidatorConfig can be deserialized from TOML
#[test]
fn validator_config_deserializes_from_toml() {
    let toml_str = r#"
        container = "osquery/osquery:5.17.0-ubuntu22.04"
        script = "validators/validate-osquery.sh"
    "#;

    let config: ValidatorConfig = toml::from_str(toml_str).expect("should parse");

    assert_eq!(config.container, "osquery/osquery:5.17.0-ubuntu22.04");
    assert_eq!(
        config.script,
        PathBuf::from("validators/validate-osquery.sh")
    );
}

/// Test: Config parses validators HashMap
#[test]
fn config_parses_validators_section() {
    let toml_str = r#"
        fail_fast = true

        [validators.osquery]
        container = "osquery/osquery:5.17.0-ubuntu22.04"
        script = "validators/validate-osquery.sh"

        [validators.sqlite]
        container = "keinos/sqlite3:3.47.2"
        script = "validators/validate-sqlite.sh"
    "#;

    let config: Config = toml::from_str(toml_str).expect("should parse");

    assert!(config.fail_fast);
    assert_eq!(config.validators.len(), 2);

    let osquery = config
        .validators
        .get("osquery")
        .expect("osquery should exist");
    assert_eq!(osquery.container, "osquery/osquery:5.17.0-ubuntu22.04");
    assert_eq!(
        osquery.script,
        PathBuf::from("validators/validate-osquery.sh")
    );

    let sqlite = config
        .validators
        .get("sqlite")
        .expect("sqlite should exist");
    assert_eq!(sqlite.container, "keinos/sqlite3:3.47.2");
}

/// Test: Config defaults fail_fast to true
#[test]
fn config_defaults_fail_fast_to_true() {
    let toml_str = r#"
        [validators.test]
        container = "alpine:3"
        script = "test.sh"
    "#;

    let config: Config = toml::from_str(toml_str).expect("should parse");

    assert!(config.fail_fast, "fail_fast should default to true");
}

/// Test: Config defaults validators to empty HashMap
#[test]
fn config_defaults_validators_to_empty() {
    let toml_str = r#"
        fail_fast = false
    "#;

    let config: Config = toml::from_str(toml_str).expect("should parse");

    assert!(!config.fail_fast);
    assert!(config.validators.is_empty());
}

/// Test: get_validator returns config for known validator
#[test]
fn get_validator_returns_config_for_known_validator() {
    let toml_str = r#"
        [validators.osquery]
        container = "osquery/osquery:5.17.0-ubuntu22.04"
        script = "validators/validate-osquery.sh"
    "#;

    let config: Config = toml::from_str(toml_str).expect("should parse");

    let osquery = config
        .get_validator("osquery")
        .expect("should find osquery");
    assert_eq!(osquery.container, "osquery/osquery:5.17.0-ubuntu22.04");
}

/// Test: get_validator returns error for unknown validator
#[test]
fn get_validator_errors_for_unknown_validator() {
    let toml_str = r#"
        [validators.osquery]
        container = "osquery/osquery:5.17.0-ubuntu22.04"
        script = "validators/validate-osquery.sh"
    "#;

    let config: Config = toml::from_str(toml_str).expect("should parse");

    let err = config.get_validator("nonexistent").unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("Unknown validator 'nonexistent'"),
        "Error should mention unknown validator: {msg}"
    );
    assert!(
        msg.contains("[preprocessor.validator.validators.nonexistent]"),
        "Error should suggest config location: {msg}"
    );
}

/// Test: ValidatorConfig.validate() errors on empty container
#[test]
fn validator_config_validate_errors_on_empty_container() {
    let config = ValidatorConfig {
        container: String::new(),
        script: PathBuf::from("test.sh"),
        setup_command: None,
        query_command: None,
    };

    let err = config.validate().unwrap_err();
    assert!(
        err.to_string().contains("container cannot be empty"),
        "Should mention empty container: {}",
        err
    );
}

/// Test: ValidatorConfig.validate() errors on empty script
#[test]
fn validator_config_validate_errors_on_empty_script() {
    let config = ValidatorConfig {
        container: "alpine:3".to_owned(),
        script: PathBuf::new(),
        setup_command: None,
        query_command: None,
    };

    let err = config.validate().unwrap_err();
    assert!(
        err.to_string().contains("script path cannot be empty"),
        "Should mention empty script: {}",
        err
    );
}

/// Test: ValidatorConfig.validate() passes for valid config
#[test]
fn validator_config_validate_passes_for_valid_config() {
    let config = ValidatorConfig {
        container: "osquery/osquery:5.17.0-ubuntu22.04".to_owned(),
        script: PathBuf::from("validators/validate-osquery.sh"),
        setup_command: None,
        query_command: None,
    };

    config.validate().expect("should pass validation");
}
