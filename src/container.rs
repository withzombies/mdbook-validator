//! Container lifecycle management using testcontainers

use crate::config::ValidatorConfig;

/// Manages a validator container
pub struct ValidatorContainer {
    config: ValidatorConfig,
}

impl ValidatorContainer {
    /// Create a new container manager
    #[must_use]
    pub fn new(config: ValidatorConfig) -> Self {
        Self { config }
    }

    /// Get the container configuration
    #[must_use]
    pub fn config(&self) -> &ValidatorConfig {
        &self.config
    }
}
