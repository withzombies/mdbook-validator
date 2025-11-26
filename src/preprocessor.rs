//! mdBook preprocessor implementation
//!
//! Bridges the synchronous mdBook Preprocessor trait to async container validation.

use std::fmt::Write;

use mdbook::book::{Book, BookItem, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

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

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        // Create tokio runtime for async->sync bridge
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(async { self.run_async(&mut book).await })?;

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl ValidatorPreprocessor {
    /// Process a book for validation without a `PreprocessorContext`.
    ///
    /// This is useful for testing when you don't have access to create a context.
    pub fn process_book(&self, mut book: Book) -> Result<Book, Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(async { self.run_async(&mut book).await })?;

        Ok(book)
    }

    async fn run_async(&self, book: &mut Book) -> Result<(), Error> {
        // Simple test validator script that exits 0 (pass)
        let validator_script = b"#!/bin/sh\nexit 0\n";

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

    async fn process_chapter(
        &self,
        chapter: &mut Chapter,
        container: &ValidatorContainer,
    ) -> Result<(), Error> {
        let Some(ref content) = chapter.content.clone().into() else {
            return Ok(());
        };

        // Collect all code blocks that need validation
        let blocks = Self::find_validator_blocks(content);

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
        chapter.content = Self::strip_markers_from_chapter(content);

        Ok(())
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
                            blocks.push(ValidatorBlock { markers, skip });
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

        result.trim().to_string()
    }
}

/// A code block that requires validation
struct ValidatorBlock {
    markers: ExtractedMarkers,
    skip: bool,
}
