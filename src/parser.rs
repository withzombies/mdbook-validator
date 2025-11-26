//! Markdown parsing and code block extraction

/// A code block extracted from markdown
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// The language (e.g., "sql", "json")
    pub language: String,
    /// The validator name from `validator=` attribute
    pub validator: Option<String>,
    /// Whether to skip validation
    pub skip: bool,
    /// The visible content (without markers)
    pub content: String,
    /// Setup content from <!--SETUP--> marker
    pub setup: Option<String>,
    /// Assertions from <!--ASSERT--> marker
    pub assertions: Option<String>,
    /// Expected output from <!--EXPECT--> marker
    pub expect: Option<String>,
}
