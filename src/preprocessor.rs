//! mdBook preprocessor implementation

use mdbook::book::Book;
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};

/// The mdbook-validator preprocessor
pub struct ValidatorPreprocessor;

impl ValidatorPreprocessor {
    /// Create a new preprocessor instance
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ValidatorPreprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Preprocessor for ValidatorPreprocessor {
    fn name(&self) -> &'static str {
        "validator"
    }

    fn run(&self, _ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        // Placeholder - will validate code blocks and strip markers
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}
