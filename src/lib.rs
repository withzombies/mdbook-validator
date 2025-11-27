//! mdbook-validator library
//!
//! An mdBook preprocessor that validates code blocks using Docker containers.

pub mod config;
pub mod container;
pub mod parser;
pub mod preprocessor;
pub mod transpiler;

pub use preprocessor::ValidatorPreprocessor;
