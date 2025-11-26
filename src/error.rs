//! Error types for mdbook-validator

use thiserror::Error;

/// Errors that can occur during validation
#[derive(Debug, Error)]
pub enum ValidatorError {
    /// Failed to parse markdown
    #[error("failed to parse markdown: {0}")]
    MarkdownParse(String),

    /// Failed to start container
    #[error("failed to start container '{container}': {source}")]
    ContainerStart {
        container: String,
        #[source]
        source: std::io::Error,
    },

    /// Validation failed
    #[error("validation failed for block at {file}:{line}: {message}")]
    ValidationFailed {
        file: String,
        line: usize,
        message: String,
    },

    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),
}

/// Result type alias using [`ValidatorError`]
pub type Result<T> = std::result::Result<T, ValidatorError>;
