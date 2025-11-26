//! mdbook-validator library
//!
//! An mdBook preprocessor that validates code blocks using Docker containers.

pub mod config;
pub mod container;
pub mod error;
pub mod parser;
pub mod preprocessor;
pub mod transpiler;
pub mod validator;

pub use error::{Result, ValidatorError};
pub use preprocessor::ValidatorPreprocessor;
