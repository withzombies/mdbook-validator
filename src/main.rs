//! mdbook-validator CLI entry point

use mdbook::preprocess::Preprocessor;
use mdbook_validator::ValidatorPreprocessor;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // For now, just print version info
    // Full mdBook integration will be implemented later
    let preprocessor = ValidatorPreprocessor::new();
    println!("mdbook-validator preprocessor: {}", preprocessor.name());
}
