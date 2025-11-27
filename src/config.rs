//! Configuration parsing from book.toml
//!
//! Parses [preprocessor.validator] section including validator definitions.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;

/// Configuration for a single validator
#[derive(Debug, Clone, Deserialize)]
pub struct ValidatorConfig {
    /// Docker image (e.g., "osquery/osquery:5.17.0-ubuntu22.04")
    pub container: String,
    /// Path to validator script relative to book root
    pub script: PathBuf,
    /// Command to run query with JSON output (e.g., "sqlite3 -json /tmp/test.db")
    /// If not set, defaults based on validator type
    #[serde(default)]
    pub query_command: Option<String>,
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
            bail!("No [preprocessor.validator] section in book.toml");
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
            anyhow::anyhow!(
                "Unknown validator '{name}'. Define it in book.toml under [preprocessor.validator.validators.{name}]"
            )
        })
    }
}

impl ValidatorConfig {
    /// Validate the configuration values.
    ///
    /// # Errors
    ///
    /// Returns error if container or script are empty.
    pub fn validate(&self) -> Result<()> {
        if self.container.is_empty() {
            bail!("Validator container cannot be empty");
        }
        if self.script.as_os_str().is_empty() {
            bail!("Validator script path cannot be empty");
        }
        Ok(())
    }
}
