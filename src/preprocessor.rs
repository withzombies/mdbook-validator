//! mdBook preprocessor implementation
//!
//! Bridges the synchronous mdBook Preprocessor trait to async container validation.

use tracing::{debug, info, trace};

// Default exec commands for validators when not configured
const DEFAULT_EXEC_SQLITE: &str = "sqlite3 -json /tmp/test.db";
const DEFAULT_EXEC_OSQUERY: &str = "osqueryi --json";
const DEFAULT_EXEC_FALLBACK: &str = "cat";

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use mdbook_preprocessor::book::{Book, BookItem, Chapter};
use mdbook_preprocessor::errors::Error;
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::command::RealCommandRunner;
use crate::config::{Config, ValidatorConfig};
use crate::container::ValidatorContainer;
use crate::error::ValidatorError;
use crate::host_validator;
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

    fn supports_renderer(&self, renderer: &str) -> Result<bool, anyhow::Error> {
        // Support all renderers - we validate and strip markers,
        // producing valid markdown for any output format
        let _ = renderer;
        Ok(true)
    }
}

impl ValidatorPreprocessor {
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

        for item in &mut book.items {
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

        for item in &mut book.items {
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

            let validation_content = block.markers.validation_content();
            let result = container
                .exec_with_env(
                    block.markers.setup.as_deref(),
                    &validation_content,
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

        info!(chapter = %chapter.name, blocks = blocks.len(), "Validating");

        // Check for mutually exclusive attributes (fail fast)
        for block in &blocks {
            if block.skip && block.hidden {
                return Err(Error::new(ValidatorError::MutuallyExclusiveAttributes));
            }
        }

        // Validate each block using configured validator
        for (idx, block) in blocks.iter().enumerate() {
            if block.skip {
                debug!(block = idx + 1, validator = %block.validator_name, "Skipping (skip=true)");
                continue;
            }

            debug!(block = idx + 1, validator = %block.validator_name, "Validating block");

            // Get validator config
            let validator_config = config.get_validator(&block.validator_name).map_err(|e| {
                Error::msg(format!(
                    "Unknown validator '{}': {}",
                    block.validator_name, e
                ))
            })?;

            // Get or start container for this validator
            let container = self
                .get_or_start_container(&block.validator_name, config, book_root, containers)
                .await?;

            // Use host-based validation: run query in container, validate on host
            self.validate_block_host_based(
                container,
                validator_config,
                block,
                &chapter.name,
                book_root,
            )
            .await?;
        }

        // All validations passed - strip markers from chapter content
        chapter.content = Self::strip_markers_from_chapter(&chapter.content);

        info!(chapter = %chapter.name, "✓ Passed");

        Ok(())
    }

    /// Validate a code block using host-based validation.
    ///
    /// This runs the query in the container and validates the output on the host.
    async fn validate_block_host_based(
        &self,
        container: &ValidatorContainer,
        validator_config: &ValidatorConfig,
        block: &ValidatorBlock,
        chapter_name: &str,
        book_root: &Path,
    ) -> Result<(), Error> {
        // 0. Verify validator script exists first (fail fast before container work)
        let script_path = book_root.join(&validator_config.script);
        if !script_path.exists() {
            return Err(Error::msg(format!(
                "Failed to read validator script '{}': file not found",
                script_path.display()
            )));
        }

        debug!(script = %script_path.display(), "Using validator script");

        // Get exec command (use defaults if not configured)
        let exec_cmd = Self::get_exec_command(&block.validator_name, validator_config);
        debug!(exec_command = %exec_cmd, "Container exec command");

        // 1. Run setup script in container (if any)
        // SETUP content IS the shell command - run directly via sh -c
        if let Some(setup) = &block.markers.setup {
            let setup_script = setup.trim();
            if !setup_script.is_empty() {
                debug!("Running SETUP script");
                trace!(setup = %setup_script, "SETUP content");
                let setup_result = container
                    .exec_raw(&["sh", "-c", setup_script])
                    .await
                    .map_err(|e| Error::msg(format!("Setup exec failed: {e}")))?;

                if setup_result.exit_code != 0 {
                    #[allow(clippy::cast_possible_truncation)]
                    return Err(ValidatorError::SetupFailed {
                        exit_code: setup_result.exit_code as i32,
                        message: format!(
                            "in '{}' (validator: {}):\n\nScript:\n{}\n\nError:\n{}",
                            chapter_name, block.validator_name, setup_script, setup_result.stderr
                        ),
                    }
                    .into());
                }
            }
        }

        // 2. Run query in container, get JSON output
        // Content is passed via stdin to avoid shell injection
        // Use validation_content() to strip @@ prefix (but keep line content)
        let query_sql = block.markers.validation_content();
        let query_sql = query_sql.trim();
        if query_sql.is_empty() {
            return Err(Error::msg(format!(
                "Validation failed in '{}' (validator: {}): Query content is empty",
                chapter_name, block.validator_name
            )));
        }

        debug!("Executing query in container");
        trace!(query = %query_sql, "Query content");

        // Pass content via stdin (secure) instead of shell interpolation (vulnerable)
        let query_result = container
            .exec_with_stdin(&["sh", "-c", &exec_cmd], query_sql)
            .await
            .map_err(|e| Error::msg(format!("Query exec failed: {e}")))?;

        trace!(exit_code = query_result.exit_code, stdout = %query_result.stdout, stderr = %query_result.stderr, "Query result");

        if query_result.exit_code != 0 {
            return Err(Error::msg(format!(
                "Query failed in '{}' (validator: {}):\n\nSQL:\n{}\n\nError:\n{}",
                chapter_name, block.validator_name, query_sql, query_result.stderr
            )));
        }

        // 3. Validate JSON output on host using validator script
        // (script_path already validated at the start of this function)
        let script_path_str = script_path
            .to_str()
            .ok_or_else(|| Error::msg(format!("Invalid script path: {}", script_path.display())))?;

        debug!("Running host validator");
        let validation_result = host_validator::run_validator(
            &RealCommandRunner,
            script_path_str,
            &query_result.stdout,
            block.markers.assertions.as_deref(),
            block.markers.expect.as_deref(),
            Some(&query_result.stderr), // Pass container stderr for warning detection
        )
        .map_err(|e| {
            Error::msg(format!(
                "Host validator failed in '{}' (validator: {}): {}",
                chapter_name, block.validator_name, e
            ))
        })?;

        trace!(exit_code = validation_result.exit_code, stdout = %validation_result.stdout, stderr = %validation_result.stderr, "Validator result");

        if validation_result.exit_code != 0 {
            let mut error_msg = format!(
                "in '{}' (validator: {}):\n\nCode:\n{}\n",
                chapter_name, block.validator_name, block.markers.visible_content
            );
            if !validation_result.stderr.is_empty() {
                let _ = write!(
                    error_msg,
                    "\nValidator stderr:\n{}",
                    validation_result.stderr
                );
            }
            if !validation_result.stdout.is_empty() {
                let _ = write!(
                    error_msg,
                    "\nValidator stdout:\n{}",
                    validation_result.stdout
                );
            }
            return Err(ValidatorError::ValidationFailed {
                exit_code: validation_result.exit_code,
                message: error_msg,
            }
            .into());
        }

        Ok(())
    }

    /// Get exec command for a validator.
    ///
    /// Uses configured command if available, otherwise uses defaults based on validator name.
    fn get_exec_command(validator_name: &str, config: &ValidatorConfig) -> String {
        config
            .exec_command
            .clone()
            .unwrap_or_else(|| match validator_name {
                "sqlite" => DEFAULT_EXEC_SQLITE.to_owned(),
                "osquery" => DEFAULT_EXEC_OSQUERY.to_owned(),
                _ => DEFAULT_EXEC_FALLBACK.to_owned(),
            })
    }

    /// Get an existing container or start a new one for the given validator.
    async fn get_or_start_container<'a>(
        &self,
        validator_name: &str,
        config: &Config,
        book_root: &Path,
        containers: &'a mut HashMap<String, ValidatorContainer>,
    ) -> Result<&'a ValidatorContainer, Error> {
        match containers.entry(validator_name.to_owned()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                // Look up validator config
                let validator_config = config.get_validator(validator_name).map_err(|e| {
                    Error::msg(format!("Unknown validator '{validator_name}': {e}"))
                })?;

                // Validate config values
                validator_config.validate(validator_name)?;

                // Resolve and validate fixtures_dir if configured
                let mount = if let Some(ref fixtures_dir) = config.fixtures_dir {
                    // Resolve relative path from book_root
                    let fixtures_path = if fixtures_dir.is_absolute() {
                        fixtures_dir.clone()
                    } else {
                        book_root.join(fixtures_dir)
                    };

                    // Validate fixtures_dir exists and is a directory
                    if !fixtures_path.exists() {
                        return Err(Error::msg(format!(
                            "fixtures_dir '{}' does not exist",
                            fixtures_path.display()
                        )));
                    }
                    if !fixtures_path.is_dir() {
                        return Err(Error::msg(format!(
                            "fixtures_dir '{}' is not a directory",
                            fixtures_path.display()
                        )));
                    }

                    // Canonicalize to resolve symlinks (Docker requires real paths)
                    let fixtures_path = fixtures_path.canonicalize().map_err(|e| {
                        Error::msg(format!(
                            "fixtures_dir '{}' could not be canonicalized: {}",
                            fixtures_path.display(),
                            e
                        ))
                    })?;

                    Some((fixtures_path, "/fixtures"))
                } else {
                    None
                };

                // Start the container with optional mount
                let container = ValidatorContainer::start_raw_with_mount(
                    &validator_config.container,
                    mount.as_ref().map(|(p, c)| (p.as_path(), *c)),
                )
                .await
                .map_err(|e| {
                    Error::msg(format!(
                        "Failed to start container '{}': {}",
                        validator_config.container, e
                    ))
                })?;

                Ok(entry.insert(container))
            }
        }
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

                    let (_language, validator, skip, hidden) = parse_info_string(&current_info);

                    // Only process blocks with validator= attribute
                    if let Some(validator_name) = validator {
                        // Handle empty validator= as "no validator"
                        if !validator_name.is_empty() {
                            let markers = extract_markers(&current_content);
                            blocks.push(ValidatorBlock {
                                validator_name,
                                markers,
                                skip,
                                hidden,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        blocks
    }

    /// Strip all validation markers from chapter content, preserving code block structure.
    ///
    /// Uses span-based editing to surgically modify only code block contents,
    /// preserving ALL other markdown formatting (lists, links, emphasis, etc.).
    ///
    /// If a code block has the `hidden` attribute, the entire fence is removed from output.
    fn strip_markers_from_chapter(content: &str) -> String {
        use std::ops::Range;

        // Represents an edit to apply to the source
        enum Edit {
            /// Replace a range with new content (for stripping markers)
            Replace {
                range: Range<usize>,
                content: String,
            },
            /// Delete a range entirely (for hidden blocks)
            Delete { range: Range<usize> },
        }

        let mut edits: Vec<Edit> = Vec::new();
        let parser = Parser::new(content).into_offset_iter();

        let mut current_block_start: Option<usize> = None;
        let mut current_hidden = false;
        let mut current_has_validator = false;
        let mut current_content_range: Option<Range<usize>> = None;

        for (event, range) in parser {
            match &event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                    let (_language, validator, _skip, hidden) = parse_info_string(info);
                    current_hidden = hidden;
                    current_has_validator = validator.is_some();
                    current_block_start = Some(range.start);
                    current_content_range = None;
                }
                Event::Text(_) if current_block_start.is_some() => {
                    // Track the content range within the code block
                    current_content_range = Some(range);
                }
                Event::End(TagEnd::CodeBlock) if current_block_start.is_some() => {
                    let Some(block_start) = current_block_start.take() else {
                        unreachable!("current_block_start must be Some here")
                    };

                    if current_hidden {
                        // Delete the entire code block (including surrounding whitespace)
                        // Find the start of the line containing the opening fence
                        let line_start = content[..block_start].rfind('\n').map_or(0, |i| i + 1);
                        // Find the end of the line containing the closing fence
                        let line_end = content[range.end..]
                            .find('\n')
                            .map_or(range.end, |i| range.end + i + 1);

                        edits.push(Edit::Delete {
                            range: line_start..line_end,
                        });
                    } else if current_has_validator {
                        // Strip markers from the content, but preserve the fence
                        if let Some(content_range) = current_content_range.take() {
                            let original_content = &content[content_range.clone()];
                            let stripped = strip_markers(original_content);
                            let trimmed = stripped.trim();
                            if trimmed != original_content.trim() {
                                // Only create an edit if content actually changed
                                edits.push(Edit::Replace {
                                    range: content_range,
                                    content: format!("{trimmed}\n"),
                                });
                            }
                        }
                    }

                    current_hidden = false;
                    current_has_validator = false;
                }
                _ => {}
            }
        }

        // Apply edits from end to start to preserve byte offsets
        edits.sort_by(|a, b| {
            let a_start = match a {
                Edit::Replace { range, .. } | Edit::Delete { range } => range.start,
            };
            let b_start = match b {
                Edit::Replace { range, .. } | Edit::Delete { range } => range.start,
            };
            b_start.cmp(&a_start) // Reverse order (end to start)
        });

        let mut result = content.to_owned();
        for edit in edits {
            match edit {
                Edit::Replace { range, content } => {
                    result.replace_range(range, &content);
                }
                Edit::Delete { range } => {
                    result.replace_range(range, "");
                }
            }
        }

        // Clean up any excessive blank lines left by deletions
        Self::normalize_blank_lines(&result)
    }

    /// Normalize blank lines: collapse 3+ consecutive newlines to 2, trim edges
    fn normalize_blank_lines(content: &str) -> String {
        let mut result = String::with_capacity(content.len());
        let mut consecutive_newlines = 0;

        for ch in content.chars() {
            if ch == '\n' {
                consecutive_newlines += 1;
                if consecutive_newlines <= 2 {
                    result.push(ch);
                }
            } else {
                consecutive_newlines = 0;
                result.push(ch);
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
    /// Whether to hide the block from output (but still validate)
    hidden: bool,
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
    use super::*;

    // ==================== strip_markers_from_chapter hidden block tests ====================

    #[test]
    fn strip_markers_from_chapter_removes_hidden_block() {
        let content = r#"Some text

```sql validator=sqlite hidden
SELECT 1;
```

More text"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Hidden block should be completely removed
        assert!(!result.contains("SELECT 1"));
        assert!(!result.contains("```sql"));
        assert!(result.contains("Some text"));
        assert!(result.contains("More text"));
    }

    #[test]
    fn strip_markers_from_chapter_keeps_non_hidden_block() {
        let content = r#"Some text

```sql validator=sqlite
SELECT 1;
```

More text"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Non-hidden block should be kept (with markers stripped)
        assert!(result.contains("SELECT 1"));
        assert!(result.contains("```sql"));
        assert!(result.contains("Some text"));
        assert!(result.contains("More text"));
    }

    #[test]
    fn strip_markers_from_chapter_mixed_hidden_and_non_hidden() {
        let content = r#"Start

```sql validator=sqlite hidden
HIDDEN QUERY;
```

Middle

```sql validator=sqlite
VISIBLE QUERY;
```

End"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Hidden block removed, non-hidden kept
        assert!(!result.contains("HIDDEN QUERY"));
        assert!(result.contains("VISIBLE QUERY"));
        assert!(result.contains("Start"));
        assert!(result.contains("Middle"));
        assert!(result.contains("End"));
    }

    #[test]
    fn strip_markers_from_chapter_adjacent_hidden_blocks() {
        let content = r#"Start

```sql validator=sqlite hidden
HIDDEN 1;
```

```sql validator=sqlite hidden
HIDDEN 2;
```

End"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Both hidden blocks should be removed
        assert!(!result.contains("HIDDEN 1"));
        assert!(!result.contains("HIDDEN 2"));
        assert!(result.contains("Start"));
        assert!(result.contains("End"));
    }

    #[test]
    fn strip_markers_from_chapter_hidden_block_at_start() {
        let content = r#"```sql validator=sqlite hidden
HIDDEN;
```

Visible content"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Hidden block at start should not leave leading whitespace
        assert!(!result.contains("HIDDEN"));
        assert!(result.contains("Visible content"));
        // Should not start with blank lines
        assert!(!result.starts_with('\n'));
    }

    #[test]
    fn strip_markers_from_chapter_hidden_block_at_end() {
        let content = r#"Visible content

```sql validator=sqlite hidden
HIDDEN;
```"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Hidden block at end should not leave trailing whitespace
        assert!(!result.contains("HIDDEN"));
        assert!(result.contains("Visible content"));
        // Should not end with excessive blank lines
        assert!(!result.ends_with("\n\n"));
    }

    #[test]
    fn strip_markers_from_chapter_only_hidden_block() {
        let content = r#"```sql validator=sqlite hidden
HIDDEN;
```"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Single hidden block should result in empty output
        assert!(!result.contains("HIDDEN"));
        assert!(result.is_empty() || result.trim().is_empty());
    }

    #[test]
    fn strip_markers_from_chapter_hidden_with_markers() {
        let content = r#"Text

```sql validator=sqlite hidden
<!--SETUP
CREATE TABLE t;
-->
SELECT * FROM t;
<!--ASSERT
rows >= 1
-->
```

More text"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Hidden block with markers should be completely removed
        assert!(!result.contains("SETUP"));
        assert!(!result.contains("ASSERT"));
        assert!(!result.contains("CREATE TABLE"));
        assert!(!result.contains("SELECT"));
        assert!(result.contains("Text"));
        assert!(result.contains("More text"));
    }

    // ==================== Regression tests for markdown preservation ====================
    // These tests ensure that strip_markers_from_chapter preserves all markdown formatting
    // that exists OUTSIDE of code blocks with validator= attributes.

    #[test]
    fn strip_markers_preserves_lists() {
        let content = r#"# Chapter

Some text:

- Item one
- Item two
- Item three

### Next Section

More text."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Lists must be preserved exactly
        assert!(
            result.contains("- Item one"),
            "List items must be preserved"
        );
        assert!(
            result.contains("- Item two"),
            "List items must be preserved"
        );
        assert!(
            result.contains("- Item three"),
            "List items must be preserved"
        );
        assert!(
            result.contains("### Next Section"),
            "Headings must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_lists_with_code_block() {
        let content = r#"# Chapter

Some text:

- Item one
- Item two
- Item three

```sql validator=sqlite
<!--SETUP
CREATE TABLE t;
-->
SELECT 1;
```

### Next Section

More text."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        // Lists must be preserved
        assert!(
            result.contains("- Item one"),
            "List items must be preserved"
        );
        assert!(
            result.contains("- Item two"),
            "List items must be preserved"
        );
        assert!(
            result.contains("- Item three"),
            "List items must be preserved"
        );
        // Code block content stripped of markers but preserved
        assert!(result.contains("SELECT 1"), "Code block content preserved");
        assert!(!result.contains("SETUP"), "Markers stripped");
        assert!(!result.contains("CREATE TABLE"), "Setup content stripped");
        // Headings preserved
        assert!(
            result.contains("### Next Section"),
            "Headings must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_numbered_lists() {
        let content = r#"Steps:

1. First step
2. Second step
3. Third step

Done."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(
            result.contains("1. First step"),
            "Numbered lists must be preserved"
        );
        assert!(
            result.contains("2. Second step"),
            "Numbered lists must be preserved"
        );
        assert!(
            result.contains("3. Third step"),
            "Numbered lists must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_blockquotes() {
        let content = r#"Quote:

> This is a blockquote
> with multiple lines

End."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(
            result.contains("> This is a blockquote"),
            "Blockquotes must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_links() {
        let content = r#"See [the documentation](https://example.com) for details.

And [another link](https://other.com)."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(
            result.contains("[the documentation](https://example.com)"),
            "Links must be preserved"
        );
        assert!(
            result.contains("[another link](https://other.com)"),
            "Links must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_inline_code() {
        let content = r#"Use the `SELECT` statement to query data.

Also `INSERT` works."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(result.contains("`SELECT`"), "Inline code must be preserved");
        assert!(result.contains("`INSERT`"), "Inline code must be preserved");
    }

    #[test]
    fn strip_markers_preserves_emphasis() {
        let content = r#"This is *italic* and **bold** text.

Also _underscores_ and __double__."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(result.contains("*italic*"), "Italic must be preserved");
        assert!(result.contains("**bold**"), "Bold must be preserved");
    }

    #[test]
    fn strip_markers_preserves_tables() {
        let content = r#"| Column A | Column B |
|----------|----------|
| Value 1  | Value 2  |
| Value 3  | Value 4  |"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(
            result.contains("| Column A | Column B |"),
            "Tables must be preserved"
        );
        assert!(
            result.contains("| Value 1  | Value 2  |"),
            "Table rows must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_code_blocks_without_validator() {
        let content = r#"Regular code:

```python
def hello():
    print("world")
```

End."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);
        assert!(result.contains("```python"), "Code fence must be preserved");
        assert!(
            result.contains("def hello():"),
            "Code content must be preserved"
        );
        assert!(
            result.contains("print(\"world\")"),
            "Code content must be preserved"
        );
    }

    #[test]
    fn strip_markers_complex_document() {
        // This tests a realistic document with mixed content
        let content = r#"# Getting Started

Welcome to the guide. Here's what you'll learn:

- How to query data
- How to filter results
- How to join tables

## Basic Queries

First, let's set up our database:

```sql validator=sqlite hidden
<!--SETUP
CREATE TABLE users (id INTEGER, name TEXT);
INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');
-->
SELECT 'setup complete';
```

Now run a simple query:

```sql validator=sqlite
SELECT * FROM users;
<!--ASSERT
rows >= 1
-->
```

> **Note**: The query above returns all users.

See [SQL documentation](https://sqlite.org) for more.

### Summary

1. We created a table
2. We queried the data
3. We verified the results

Done!"#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        // Lists preserved
        assert!(
            result.contains("- How to query data"),
            "Bullet lists preserved"
        );
        assert!(
            result.contains("1. We created a table"),
            "Numbered lists preserved"
        );

        // Hidden block removed
        assert!(
            !result.contains("CREATE TABLE users"),
            "Hidden block content removed"
        );
        assert!(
            !result.contains("INSERT INTO users"),
            "Hidden block content removed"
        );

        // Visible code block preserved (without markers)
        assert!(
            result.contains("SELECT * FROM users"),
            "Visible query preserved"
        );
        assert!(!result.contains("ASSERT"), "Markers stripped");

        // Blockquote preserved
        assert!(result.contains("> **Note**"), "Blockquote preserved");

        // Link preserved
        assert!(
            result.contains("[SQL documentation](https://sqlite.org)"),
            "Link preserved"
        );

        // Headings preserved
        assert!(result.contains("## Basic Queries"), "H2 preserved");
        assert!(result.contains("### Summary"), "H3 preserved");
    }

    #[test]
    fn strip_markers_preserves_headings_with_links() {
        // Regression test: headings containing links were being corrupted
        let content = r#"# Introduction

Some intro text.

### [Configuration Guide](https://example.com/config)

This section explains configuration.

### [API Reference](https://example.com/api)

API docs here.

```sql validator=sqlite
SELECT 1;
```

### [Advanced Topics](https://example.com/advanced)

More content."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        // Headings with links must be preserved exactly
        assert!(
            result.contains("### [Configuration Guide](https://example.com/config)"),
            "Heading with link must be preserved"
        );
        assert!(
            result.contains("### [API Reference](https://example.com/api)"),
            "Heading with link must be preserved"
        );
        assert!(
            result.contains("### [Advanced Topics](https://example.com/advanced)"),
            "Heading with link must be preserved"
        );
        // Code block still processed
        assert!(result.contains("SELECT 1"), "Code block content preserved");
    }

    #[test]
    fn strip_markers_preserves_paths_with_wildcards() {
        // Regression test: paths with * were being parsed as emphasis
        let content = r#"# File Patterns

Match all files in a directory:

- `/etc/osquery/*`
- `/var/log/*.log`
- `C:\Users\*\AppData`

You can also use `/some/path/**/*.json` for recursive matching.

```sql validator=sqlite
SELECT 1;
```

The path `/tmp/*` is commonly used."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        // Paths with wildcards must be preserved exactly
        assert!(
            result.contains("/etc/osquery/*"),
            "Path with wildcard must be preserved"
        );
        assert!(
            result.contains("/var/log/*.log"),
            "Path with wildcard must be preserved"
        );
        assert!(
            result.contains(r"C:\Users\*\AppData"),
            "Windows path with wildcard must be preserved"
        );
        assert!(
            result.contains("/some/path/**/*.json"),
            "Recursive glob must be preserved"
        );
        assert!(
            result.contains("/tmp/*"),
            "Inline path with wildcard must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_inline_code_with_special_chars() {
        // Regression test: inline code with special characters
        let content = r#"# Code Examples

Use `SELECT * FROM users` to get all users.

The command `rm -rf /tmp/*` removes temp files.

Run `echo $HOME` to print home directory.

Use `git log --oneline | head -10` for recent commits.

The regex `\d+\.\d+` matches decimals.

```sql validator=sqlite
SELECT 1;
```

Also try `jq '.[] | .name'` for JSON parsing."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        // Inline code must be preserved exactly
        assert!(
            result.contains("`SELECT * FROM users`"),
            "Inline code with * must be preserved"
        );
        assert!(
            result.contains("`rm -rf /tmp/*`"),
            "Inline code with path must be preserved"
        );
        assert!(
            result.contains("`echo $HOME`"),
            "Inline code with $ must be preserved"
        );
        assert!(
            result.contains("`git log --oneline | head -10`"),
            "Inline code with pipe must be preserved"
        );
        assert!(
            result.contains(r"`\d+\.\d+`"),
            "Inline code with backslashes must be preserved"
        );
        assert!(
            result.contains("`jq '.[] | .name'`"),
            "Inline code with quotes must be preserved"
        );
    }

    #[test]
    fn strip_markers_preserves_asterisks_in_text() {
        // Regression test: asterisks in regular text (not emphasis)
        let content = r#"# Wildcards

The pattern `*` matches everything.

File paths like /etc/* are common.

Use * for wildcards and ** for recursive.

Math: 5 * 3 = 15

```sql validator=sqlite
SELECT 1;
```

Done."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        // Asterisks in various contexts
        assert!(
            result.contains("The pattern `*` matches everything"),
            "Backtick asterisk preserved"
        );
        assert!(result.contains("/etc/*"), "Path asterisk preserved");
        assert!(result.contains("5 * 3 = 15"), "Math asterisk preserved");
    }

    #[test]
    fn strip_markers_preserves_complex_inline_formatting() {
        // Test various inline formatting combinations
        let content = r#"# Formatting Test

This has **bold** and *italic* text.

This has `code with **asterisks**` inside.

This has [link with `code`](https://example.com).

This has **bold with `code` inside**.

```sql validator=sqlite
SELECT 1;
```

End."#;
        let result = ValidatorPreprocessor::strip_markers_from_chapter(content);

        assert!(result.contains("**bold**"), "Bold preserved");
        assert!(result.contains("*italic*"), "Italic preserved");
        assert!(
            result.contains("`code with **asterisks**`"),
            "Code with asterisks preserved"
        );
        assert!(
            result.contains("[link with `code`](https://example.com)"),
            "Link with code preserved"
        );
        assert!(
            result.contains("**bold with `code` inside**"),
            "Bold with code preserved"
        );
    }
}
