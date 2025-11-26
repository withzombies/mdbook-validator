//! Configuration parsing for book.toml

use std::collections::HashMap;

/// Configuration for a validator
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Container image to use
    pub container: String,
    /// Command to run for validation
    pub validate_command: String,
}

/// Top-level preprocessor configuration
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Whether to fail on first error
    pub fail_fast: bool,
    /// Configured validators by name
    pub validators: HashMap<String, ValidatorConfig>,
}
