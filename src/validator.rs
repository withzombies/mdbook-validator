//! Orchestrates validation of code blocks

use serde::Serialize;

use crate::parser::CodeBlock;

/// Input sent to validator scripts as JSON
#[derive(Debug, Clone, Serialize)]
pub struct ValidatorInput {
    /// Setup content (e.g., CREATE TABLE statements)
    pub setup: Option<String>,
    /// The content to validate
    pub content: String,
    /// Assertion rules
    pub assertions: Option<String>,
    /// Expected output for exact matching
    pub expect: Option<String>,
}

impl From<&CodeBlock> for ValidatorInput {
    fn from(block: &CodeBlock) -> Self {
        Self {
            setup: block.setup.clone(),
            content: block.content.clone(),
            assertions: block.assertions.clone(),
            expect: block.expect.clone(),
        }
    }
}
