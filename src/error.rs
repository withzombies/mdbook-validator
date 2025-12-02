//! Structured error types for mdbook-validator.
//!
//! Each variant has an error code (E001-E010) for grep-ability
//! and structured fields for programmatic access.

use thiserror::Error;

/// Errors that can occur during mdbook-validator operations.
///
/// Error codes are stable and should not be renumbered.
/// Add new codes at E011+ if needed in the future.
#[derive(Debug, Error)]
pub enum ValidatorError {
    /// Configuration error (E001)
    #[error("[E001] Configuration error: {message}")]
    Config { message: String },

    /// Container startup failed (E002)
    #[error("[E002] Container startup failed: {message}")]
    ContainerStartup { message: String },

    /// Container exec failed (E003)
    #[error("[E003] Container exec failed: {message}")]
    ContainerExec { message: String },

    /// Setup script failed (E004)
    #[error("[E004] Setup script failed (exit {exit_code}): {message}")]
    SetupFailed { exit_code: i32, message: String },

    /// Query execution failed (E005)
    #[error("[E005] Query execution failed (exit {exit_code}): {message}")]
    QueryFailed { exit_code: i32, message: String },

    /// Validation failed (E006)
    #[error("[E006] Validation failed (exit {exit_code}): {message}")]
    ValidationFailed { exit_code: i32, message: String },

    /// Unknown validator (E007)
    #[error("[E007] Unknown validator '{name}'")]
    UnknownValidator { name: String },

    /// Invalid validator config (E008)
    #[error("[E008] Invalid validator config for '{name}': {reason}")]
    InvalidConfig { name: String, reason: String },

    /// Fixtures directory error (E009)
    #[error("[E009] Fixtures directory error: {message}")]
    FixturesError { message: String },

    /// Script not found (E010)
    #[error("[E010] Script not found: {path}")]
    ScriptNotFound { path: String },

    /// Mutually exclusive attributes (E011)
    #[error("[E011] 'hidden' and 'skip' are mutually exclusive")]
    MutuallyExclusiveAttributes,
}

impl ValidatorError {
    /// Returns the error code (E001-E011) for this error variant.
    ///
    /// Error codes are stable and can be used for programmatic matching.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config { .. } => "E001",
            Self::ContainerStartup { .. } => "E002",
            Self::ContainerExec { .. } => "E003",
            Self::SetupFailed { .. } => "E004",
            Self::QueryFailed { .. } => "E005",
            Self::ValidationFailed { .. } => "E006",
            Self::UnknownValidator { .. } => "E007",
            Self::InvalidConfig { .. } => "E008",
            Self::FixturesError { .. } => "E009",
            Self::ScriptNotFound { .. } => "E010",
            Self::MutuallyExclusiveAttributes => "E011",
        }
    }
}
