//! Configuration parsing from book.toml
//!
//! Parses [preprocessor.validator] section including validator definitions.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::error::ValidatorError;
use serde::Deserialize;

/// Configuration for a single validator
#[derive(Debug, Clone, Deserialize)]
pub struct ValidatorConfig {
    /// Docker image (e.g., "osquery/osquery:5.17.0-ubuntu22.04")
    pub container: String,
    /// Path to validator script relative to book root
    pub script: PathBuf,
    /// Command to execute content in container (e.g., "sqlite3 -json /tmp/test.db")
    /// If not set, defaults based on validator type
    #[serde(default)]
    pub exec_command: Option<String>,
}

/// Main preprocessor configuration from book.toml
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Map of validator name to config
    #[serde(default)]
    pub validators: HashMap<String, ValidatorConfig>,
    /// Stop on first validation failure (default: true)
    #[serde(default = "default_fail_fast")]
    pub fail_fast: bool,
    /// Optional path to fixtures directory - mounted to /fixtures in containers.
    /// Path must be absolute. Relative paths are resolved from book root.
    #[serde(default)]
    pub fixtures_dir: Option<PathBuf>,
}

const fn default_fail_fast() -> bool {
    true
}

impl Config {
    /// Parse config from mdBook preprocessor context.
    ///
    /// # Errors
    ///
    /// Returns error if the config section is missing or malformed.
    pub fn from_context(ctx: &mdbook::preprocess::PreprocessorContext) -> Result<Self> {
        let Some(table) = ctx.config.get_preprocessor("validator") else {
            return Err(ValidatorError::Config {
                message: "No [preprocessor.validator] section in book.toml".into(),
            }
            .into());
        };

        // Convert toml::Table to our Config struct via toml::Value
        let value = toml::Value::Table(table.clone());
        let config: Config = value.try_into()?;
        Ok(config)
    }

    /// Get validator config by name.
    ///
    /// # Errors
    ///
    /// Returns error if the validator is not defined.
    pub fn get_validator(&self, name: &str) -> Result<&ValidatorConfig> {
        self.validators.get(name).ok_or_else(|| {
            ValidatorError::UnknownValidator {
                name: name.to_owned(),
            }
            .into()
        })
    }
}

impl ValidatorConfig {
    /// Validate the configuration values.
    ///
    /// # Errors
    ///
    /// Returns error if container or script are empty.
    pub fn validate(&self, name: &str) -> Result<()> {
        if self.container.is_empty() {
            return Err(ValidatorError::InvalidConfig {
                name: name.to_owned(),
                reason: "container cannot be empty".into(),
            }
            .into());
        }
        if self.script.as_os_str().is_empty() {
            return Err(ValidatorError::InvalidConfig {
                name: name.to_owned(),
                reason: "script path cannot be empty".into(),
            }
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ValidatorError;

    // ==================== ValidatorConfig tests ====================

    #[test]
    fn validator_config_valid() {
        let config = ValidatorConfig {
            container: "ubuntu:22.04".to_owned(),
            script: PathBuf::from("validators/validate.sh"),
            exec_command: None,
        };
        assert!(config.validate("test").is_ok());
    }

    #[test]
    fn validator_config_empty_container() {
        let config = ValidatorConfig {
            container: String::new(),
            script: PathBuf::from("validators/validate.sh"),
            exec_command: None,
        };
        let err = config
            .validate("test")
            .unwrap_err()
            .downcast::<ValidatorError>()
            .expect("should be ValidatorError");
        assert!(matches!(
            err,
            ValidatorError::InvalidConfig { reason, .. } if reason.contains("container cannot be empty")
        ));
    }

    #[test]
    fn validator_config_empty_script() {
        let config = ValidatorConfig {
            container: "ubuntu:22.04".to_owned(),
            script: PathBuf::new(),
            exec_command: None,
        };
        let err = config
            .validate("test")
            .unwrap_err()
            .downcast::<ValidatorError>()
            .expect("should be ValidatorError");
        assert!(matches!(
            err,
            ValidatorError::InvalidConfig { reason, .. } if reason.contains("script path cannot be empty")
        ));
    }

    #[test]
    fn validator_config_with_exec_command() {
        let config = ValidatorConfig {
            container: "ubuntu:22.04".to_owned(),
            script: PathBuf::from("validators/validate.sh"),
            exec_command: Some("sqlite3 -json /tmp/test.db".to_owned()),
        };
        assert!(config.validate("test").is_ok());
        assert_eq!(
            config.exec_command,
            Some("sqlite3 -json /tmp/test.db".to_owned())
        );
    }

    // ==================== Config tests ====================

    #[test]
    fn config_get_validator_exists() {
        let mut validators = HashMap::new();
        validators.insert(
            "sqlite".to_owned(),
            ValidatorConfig {
                container: "keinos/sqlite3:3.47.2".to_owned(),
                script: PathBuf::from("validators/validate-sqlite.sh"),
                exec_command: None,
            },
        );
        let config = Config {
            validators,
            fail_fast: true,
            fixtures_dir: None,
        };

        let result = config.get_validator("sqlite");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().container, "keinos/sqlite3:3.47.2");
    }

    #[test]
    fn config_get_validator_not_found() {
        let config = Config::default();
        let result = config.get_validator("nonexistent");
        assert!(result.is_err());
        let err = result
            .unwrap_err()
            .downcast::<ValidatorError>()
            .expect("should be ValidatorError");
        assert!(matches!(
            err,
            ValidatorError::UnknownValidator { name } if name == "nonexistent"
        ));
    }

    #[test]
    fn config_default_fail_fast_true() {
        // Test the default_fail_fast function returns true
        assert!(default_fail_fast());
    }

    // ==================== TOML parsing tests ====================

    #[test]
    fn config_parse_from_toml() {
        let toml_str = r#"
            fail_fast = false
            [validators.sqlite]
            container = "keinos/sqlite3:3.47.2"
            script = "validators/validate-sqlite.sh"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.fail_fast);
        assert!(config.validators.contains_key("sqlite"));
    }

    #[test]
    fn config_parse_multiple_validators() {
        let toml_str = r#"
            [validators.sqlite]
            container = "keinos/sqlite3:3.47.2"
            script = "validators/validate-sqlite.sh"

            [validators.osquery]
            container = "osquery/osquery:5.17.0-ubuntu22.04"
            script = "validators/validate-osquery.sh"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.validators.len(), 2);
        assert!(config.validators.contains_key("sqlite"));
        assert!(config.validators.contains_key("osquery"));
    }

    #[test]
    fn config_parse_with_exec_command() {
        let toml_str = r#"
            [validators.custom]
            container = "ubuntu:22.04"
            script = "validators/validate-custom.sh"
            exec_command = "python3 -c"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let custom = config.validators.get("custom").unwrap();
        assert_eq!(custom.exec_command, Some("python3 -c".to_owned()));
    }

    #[test]
    fn config_parse_with_fixtures_dir() {
        let toml_str = r#"
            fixtures_dir = "test-fixtures"
            [validators.sqlite]
            container = "keinos/sqlite3:3.47.2"
            script = "validators/validate-sqlite.sh"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.fixtures_dir, Some(PathBuf::from("test-fixtures")));
    }

    #[test]
    fn config_parse_empty_validators() {
        let toml_str = r"
            fail_fast = true
        ";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.validators.is_empty());
        assert!(config.fail_fast);
    }
}
