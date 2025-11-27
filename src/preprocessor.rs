//! mdBook preprocessor implementation
//!
//! Bridges the synchronous mdBook Preprocessor trait to async container validation.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use mdbook::book::{Book, BookItem, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::config::{Config, ValidatorConfig};
use crate::container::ValidatorContainer;
use crate::parser::{extract_markers, parse_info_string, ExtractedMarkers};
use crate::transpiler::strip_markers;

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

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        // Parse config from book.toml
        let config = Config::from_context(ctx)
            .map_err(|e| Error::msg(format!("Failed to parse config: {e}")))?;

        // Create tokio runtime for async->sync bridge
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(async {
            self.run_async_with_config(&mut book, &config, &ctx.root)
                .await
        })?;

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

/// Default validator script that always passes (exit 0)
const DEFAULT_VALIDATOR_SCRIPT: &[u8] = b"#!/bin/sh\nexit 0\n";

impl ValidatorPreprocessor {
    /// Process a book for validation without a `PreprocessorContext`.
    ///
    /// This is useful for testing when you don't have access to create a context.
    /// Uses a default validator that always passes.
    pub fn process_book(&self, book: Book) -> Result<Book, Error> {
        self.process_book_with_script(book, DEFAULT_VALIDATOR_SCRIPT)
    }

    /// Process a book with a custom validator script.
    ///
    /// This is primarily for testing different validator behaviors.
    /// Uses the default Alpine container with the provided script.
    pub fn process_book_with_script(
        &self,
        mut book: Book,
        validator_script: &[u8],
    ) -> Result<Book, Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(async {
            self.run_async_with_script(&mut book, validator_script)
                .await
        })?;

        Ok(book)
    }

    /// Process a book with explicit config (for testing).
    ///
    /// Allows testing with a custom config without needing a full `PreprocessorContext`.
    pub fn process_book_with_config(
        &self,
        mut book: Book,
        config: &Config,
        book_root: &Path,
    ) -> Result<Book, Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(async {
            self.run_async_with_config(&mut book, config, book_root)
                .await
        })?;

        Ok(book)
    }

    /// Run with explicit config - starts per-validator containers.
    async fn run_async_with_config(
        &self,
        book: &mut Book,
        config: &Config,
        book_root: &Path,
    ) -> Result<(), Error> {
        // Cache started containers by validator name
        let mut containers: HashMap<String, ValidatorContainer> = HashMap::new();

        for item in &mut book.sections {
            self.process_book_item_with_config(item, config, book_root, &mut containers)
                .await?;
        }

        Ok(())
    }

    /// Run with default script (for testing without config).
    async fn run_async_with_script(
        &self,
        book: &mut Book,
        validator_script: &[u8],
    ) -> Result<(), Error> {
        let container = ValidatorContainer::start(validator_script)
            .await
            .map_err(|e| Error::msg(format!("Failed to start container: {e}")))?;

        for item in &mut book.sections {
            self.process_book_item(item, &container).await?;
        }

        Ok(())
    }

    async fn process_book_item(
        &self,
        item: &mut BookItem,
        container: &ValidatorContainer,
    ) -> Result<(), Error> {
        if let BookItem::Chapter(chapter) = item {
            self.process_chapter(chapter, container).await?;

            // Process sub-items recursively
            for sub_item in &mut chapter.sub_items {
                Box::pin(self.process_book_item(sub_item, container)).await?;
            }
        }
        Ok(())
    }

    async fn process_book_item_with_config(
        &self,
        item: &mut BookItem,
        config: &Config,
        book_root: &Path,
        containers: &mut HashMap<String, ValidatorContainer>,
    ) -> Result<(), Error> {
        if let BookItem::Chapter(chapter) = item {
            self.process_chapter_with_config(chapter, config, book_root, containers)
                .await?;

            // Process sub-items recursively
            for sub_item in &mut chapter.sub_items {
                Box::pin(
                    self.process_book_item_with_config(sub_item, config, book_root, containers),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn process_chapter(
        &self,
        chapter: &mut Chapter,
        container: &ValidatorContainer,
    ) -> Result<(), Error> {
        if chapter.content.is_empty() {
            return Ok(());
        }

        // Collect all code blocks that need validation
        let blocks = Self::find_validator_blocks(&chapter.content);

        if blocks.is_empty() {
            return Ok(());
        }

        // Validate each block
        for block in &blocks {
            if block.skip {
                continue;
            }

            let result = container
                .exec_with_env(
                    block.markers.setup.as_deref(),
                    &block.markers.visible_content,
                    block.markers.assertions.as_deref(),
                    block.markers.expect.as_deref(),
                )
                .await
                .map_err(|e| {
                    Error::msg(format!(
                        "Validation exec failed in '{}': {}",
                        chapter.name, e
                    ))
                })?;

            if result.exit_code != 0 {
                let mut error_msg = format!(
                    "Validation failed in '{}' (exit code {}):\n\nCode:\n{}\n",
                    chapter.name, result.exit_code, block.markers.visible_content
                );
                if !result.stderr.is_empty() {
                    let _ = write!(error_msg, "\nValidator stderr:\n{}", result.stderr);
                }
                if !result.stdout.is_empty() {
                    let _ = write!(error_msg, "\nValidator stdout:\n{}", result.stdout);
                }
                return Err(Error::msg(error_msg));
            }
        }

        // All validations passed - strip markers from chapter content
        chapter.content = Self::strip_markers_from_chapter(&chapter.content);

        Ok(())
    }

    async fn process_chapter_with_config(
        &self,
        chapter: &mut Chapter,
        config: &Config,
        book_root: &Path,
        containers: &mut HashMap<String, ValidatorContainer>,
    ) -> Result<(), Error> {
        if chapter.content.is_empty() {
            return Ok(());
        }

        // Collect all code blocks that need validation
        let blocks = Self::find_validator_blocks(&chapter.content);

        if blocks.is_empty() {
            return Ok(());
        }

        // Validate each block using configured validator
        for block in &blocks {
            if block.skip {
                continue;
            }

            // Get or start container for this validator
            let container = self
                .get_or_start_container(&block.validator_name, config, book_root, containers)
                .await?;

            let result = container
                .exec_with_env(
                    block.markers.setup.as_deref(),
                    &block.markers.visible_content,
                    block.markers.assertions.as_deref(),
                    block.markers.expect.as_deref(),
                )
                .await
                .map_err(|e| {
                    Error::msg(format!(
                        "Validation exec failed in '{}' for validator '{}': {}",
                        chapter.name, block.validator_name, e
                    ))
                })?;

            if result.exit_code != 0 {
                let mut error_msg = format!(
                    "Validation failed in '{}' (validator: {}, exit code {}):\n\nCode:\n{}\n",
                    chapter.name,
                    block.validator_name,
                    result.exit_code,
                    block.markers.visible_content
                );
                if !result.stderr.is_empty() {
                    let _ = write!(error_msg, "\nValidator stderr:\n{}", result.stderr);
                }
                if !result.stdout.is_empty() {
                    let _ = write!(error_msg, "\nValidator stdout:\n{}", result.stdout);
                }
                return Err(Error::msg(error_msg));
            }
        }

        // All validations passed - strip markers from chapter content
        chapter.content = Self::strip_markers_from_chapter(&chapter.content);

        Ok(())
    }

    /// Get an existing container or start a new one for the given validator.
    async fn get_or_start_container<'a>(
        &self,
        validator_name: &str,
        config: &Config,
        book_root: &Path,
        containers: &'a mut HashMap<String, ValidatorContainer>,
    ) -> Result<&'a ValidatorContainer, Error> {
        // Check if we already have this container running
        if containers.contains_key(validator_name) {
            // Safe because we just checked contains_key
            return Ok(containers
                .get(validator_name)
                .unwrap_or_else(|| unreachable!()));
        }

        // Look up validator config
        let validator_config = config
            .get_validator(validator_name)
            .map_err(|e| Error::msg(format!("Unknown validator '{validator_name}': {e}")))?;

        // Validate config values
        validator_config.validate().map_err(|e| {
            Error::msg(format!(
                "Invalid validator config for '{validator_name}': {e}"
            ))
        })?;

        // Start the container
        let container = self
            .start_validator_container(validator_config, book_root)
            .await?;

        containers.insert(validator_name.to_owned(), container);

        // Safe because we just inserted
        Ok(containers
            .get(validator_name)
            .unwrap_or_else(|| unreachable!()))
    }

    /// Start a container for the given validator config.
    async fn start_validator_container(
        &self,
        config: &ValidatorConfig,
        book_root: &Path,
    ) -> Result<ValidatorContainer, Error> {
        // Load script from configured path (relative to book root)
        let script_path = book_root.join(&config.script);
        let script_content = std::fs::read(&script_path).map_err(|e| {
            Error::msg(format!(
                "Failed to read validator script '{}': {}",
                script_path.display(),
                e
            ))
        })?;

        // Start container with configured image
        ValidatorContainer::start_with_image(&config.container, &script_content)
            .await
            .map_err(|e| {
                Error::msg(format!(
                    "Failed to start container '{}': {}",
                    config.container, e
                ))
            })
    }

    /// Find all code blocks with `validator=` attribute
    fn find_validator_blocks(content: &str) -> Vec<ValidatorBlock> {
        let mut blocks = Vec::new();
        let parser = Parser::new(content);

        let mut in_code_block = false;
        let mut current_info = String::new();
        let mut current_content = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                    in_code_block = true;
                    current_info = info.to_string();
                    current_content.clear();
                }
                Event::Text(text) if in_code_block => {
                    current_content.push_str(&text);
                }
                Event::End(TagEnd::CodeBlock) if in_code_block => {
                    in_code_block = false;

                    let (_language, validator, skip) = parse_info_string(&current_info);

                    // Only process blocks with validator= attribute
                    if let Some(validator_name) = validator {
                        // Handle empty validator= as "no validator"
                        if !validator_name.is_empty() {
                            let markers = extract_markers(&current_content);
                            blocks.push(ValidatorBlock {
                                validator_name,
                                markers,
                                skip,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        blocks
    }

    /// Strip all validation markers from chapter content, preserving code block structure
    fn strip_markers_from_chapter(content: &str) -> String {
        let mut result = String::new();
        let parser = Parser::new(content);

        let mut in_code_block = false;
        let mut current_info = String::new();

        for event in parser {
            match &event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                    in_code_block = true;
                    current_info = info.to_string();
                    result.push_str("```");
                    result.push_str(&current_info);
                    result.push('\n');
                }
                Event::Text(text) if in_code_block => {
                    let (_language, validator, _skip) = parse_info_string(&current_info);

                    // Strip markers only from blocks with validator= attribute
                    if validator.is_some() {
                        let stripped = strip_markers(text);
                        // Trim and add back newline
                        let trimmed = stripped.trim();
                        if !trimmed.is_empty() {
                            result.push_str(trimmed);
                            result.push('\n');
                        }
                    } else {
                        result.push_str(text);
                    }
                }
                Event::End(TagEnd::CodeBlock) if in_code_block => {
                    in_code_block = false;
                    result.push_str("```\n");
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                    // Handle indented code blocks - pass through unchanged
                    in_code_block = true;
                    current_info.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !in_code_block {
                        result.push('\n');
                    }
                }
                Event::Text(text) if !in_code_block => {
                    result.push_str(text);
                }
                Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                    result.push_str("\n\n");
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    result.push_str(&"#".repeat(*level as usize));
                    result.push(' ');
                }
                _ => {}
            }
        }

        result.trim().to_owned()
    }
}

/// A code block that requires validation
struct ValidatorBlock {
    /// Name of the validator (e.g., "osquery", "sqlite")
    validator_name: String,
    /// Extracted markers from the code block
    markers: ExtractedMarkers,
    /// Whether to skip validation
    skip: bool,
}
