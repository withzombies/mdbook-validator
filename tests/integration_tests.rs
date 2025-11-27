//! Integration tests for mdbook-validator
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::str_to_string,
    clippy::needless_raw_string_hashes
)]

use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::Preprocessor;
use mdbook_validator::ValidatorPreprocessor;
use std::path::PathBuf;

#[test]
fn preprocessor_has_correct_name() {
    let preprocessor = ValidatorPreprocessor::new();
    assert_eq!(preprocessor.name(), "validator");
}

#[test]
fn preprocessor_supports_html_renderer() {
    let preprocessor = ValidatorPreprocessor::new();
    assert!(preprocessor.supports_renderer("html"));
}

#[test]
fn preprocessor_does_not_support_other_renderers() {
    let preprocessor = ValidatorPreprocessor::new();
    assert!(!preprocessor.supports_renderer("pdf"));
    assert!(!preprocessor.supports_renderer("epub"));
}

/// Creates a Book with a single chapter containing the given content
fn create_book_with_content(chapter_content: &str) -> Book {
    let chapter = Chapter::new(
        "Test Chapter",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));
    book
}

/// Test: Preprocessor validates and strips markers from code blocks.
///
/// This test requires Docker to be running.
#[test]
fn preprocessor_validates_and_strips_markers() {
    let chapter_content = r"# Test Chapter

Some introductory text.

```sql validator=test
<!--SETUP
CREATE TABLE test;
-->
SELECT 1;
<!--ASSERT
rows >= 1
-->
```

More text after.
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    // Run preprocessor using process_book (bypasses context requirement)
    let result = preprocessor.process_book(book);

    match result {
        Ok(processed_book) => {
            // Get the processed chapter content
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter in processed book");
            };

            let output = &chapter.content;

            // Verify markers were stripped
            assert!(
                !output.contains("<!--SETUP"),
                "SETUP marker should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("CREATE TABLE"),
                "SETUP content should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("<!--ASSERT"),
                "ASSERT marker should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("rows >= 1"),
                "ASSERT content should be stripped. Output:\n{output}"
            );

            // Verify visible content remains
            assert!(
                output.contains("SELECT 1"),
                "Visible content should remain. Output:\n{output}"
            );
            assert!(
                output.contains("validator=test"),
                "Code block info should remain. Output:\n{output}"
            );

            println!("Integration test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

/// Test: Preprocessor handles chapters with no validator blocks
#[test]
fn preprocessor_handles_no_validator_blocks() {
    let chapter_content = r#"# Test Chapter

Just some text.

```rust
fn main() {
    println!("Hello");
}
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book(book);

    match result {
        Ok(_) => {
            println!("Test passed - chapters with no validator blocks handled correctly");
        }
        Err(e) => {
            panic!("Preprocessor failed on content with no validator blocks: {e}");
        }
    }
}

/// Test: Preprocessor returns error when validator exits non-zero.
///
/// This test requires Docker to be running.
#[test]
fn preprocessor_returns_error_on_validation_failure() {
    let chapter_content = r"# Test Chapter

```sql validator=test
SELECT 1;
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    // Use a validator script that always fails
    let failing_script = b"#!/bin/sh\necho 'Validation error: something went wrong' >&2\nexit 1\n";
    let result = preprocessor.process_book_with_script(book, failing_script);

    match result {
        Ok(_) => {
            panic!("Preprocessor should have returned an error for failing validator");
        }
        Err(e) => {
            let error_msg = format!("{e}");

            // Verify error contains expected information
            assert!(
                error_msg.contains("Validation failed"),
                "Error should mention validation failed. Got: {error_msg}"
            );
            assert!(
                error_msg.contains("exit code 1"),
                "Error should mention exit code. Got: {error_msg}"
            );
            assert!(
                error_msg.contains("something went wrong"),
                "Error should include validator stderr. Got: {error_msg}"
            );

            println!("Failure test passed! Error message:\n{error_msg}");
        }
    }
}

/// Test: Preprocessor strips @@ hidden lines
#[test]
fn preprocessor_strips_hidden_lines() {
    let chapter_content = r"# Test Chapter

```sql validator=test
@@CREATE TABLE hidden;
SELECT visible;
@@DROP TABLE hidden;
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book(book);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // Hidden lines should be stripped
            assert!(
                !output.contains("@@"),
                "@@ lines should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("CREATE TABLE hidden"),
                "Hidden content should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("DROP TABLE hidden"),
                "Hidden content should be stripped. Output:\n{output}"
            );

            // Visible content remains
            assert!(
                output.contains("SELECT visible"),
                "Visible content should remain. Output:\n{output}"
            );

            println!("Hidden lines test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

/// Test: Chapter with multiple validator= blocks (all validated, all stripped)
#[test]
fn preprocessor_handles_multiple_validator_blocks() {
    let chapter_content = r"# Test Chapter

First block:

```sql validator=test
<!--SETUP
CREATE TABLE t1;
-->
SELECT 1;
```

Second block:

```sql validator=test
<!--SETUP
CREATE TABLE t2;
-->
SELECT 2;
<!--ASSERT
rows >= 1
-->
```

Third block (no validator):

```rust
fn main() {}
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book(book);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // All markers stripped from both validator blocks
            assert!(
                !output.contains("<!--SETUP"),
                "SETUP markers should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("CREATE TABLE t1"),
                "First setup should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("CREATE TABLE t2"),
                "Second setup should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("<!--ASSERT"),
                "ASSERT marker should be stripped. Output:\n{output}"
            );

            // Visible content remains
            assert!(
                output.contains("SELECT 1"),
                "First SELECT should remain. Output:\n{output}"
            );
            assert!(
                output.contains("SELECT 2"),
                "Second SELECT should remain. Output:\n{output}"
            );
            assert!(
                output.contains("fn main()"),
                "Rust block should remain unchanged. Output:\n{output}"
            );

            println!("Multi-block test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

/// Creates a book with nested chapters
fn create_book_with_nested_chapters() -> Book {
    let parent_content = r"# Parent Chapter

```sql validator=test
<!--SETUP
CREATE TABLE parent;
-->
SELECT 'parent';
```
";

    let child_content = r"# Child Chapter

```sql validator=test
<!--SETUP
CREATE TABLE child;
-->
SELECT 'child';
```
";

    let child_chapter = Chapter::new(
        "Child Chapter",
        child_content.to_string(),
        PathBuf::from("child.md"),
        vec![],
    );

    let mut parent_chapter = Chapter::new(
        "Parent Chapter",
        parent_content.to_string(),
        PathBuf::from("parent.md"),
        vec![],
    );
    parent_chapter
        .sub_items
        .push(BookItem::Chapter(child_chapter));

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(parent_chapter));
    book
}

/// Test: Nested sub-chapters processed recursively
#[test]
fn preprocessor_handles_nested_chapters() {
    let book = create_book_with_nested_chapters();
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book(book);

    match result {
        Ok(processed_book) => {
            // Check parent chapter
            let Some(BookItem::Chapter(parent)) = processed_book.sections.first() else {
                panic!("Expected parent chapter");
            };

            assert!(
                !parent.content.contains("<!--SETUP"),
                "Parent SETUP should be stripped. Output:\n{}",
                parent.content
            );
            assert!(
                parent.content.contains("SELECT 'parent'"),
                "Parent SELECT should remain. Output:\n{}",
                parent.content
            );

            // Check child chapter
            let Some(BookItem::Chapter(child)) = parent.sub_items.first() else {
                panic!("Expected child chapter");
            };

            assert!(
                !child.content.contains("<!--SETUP"),
                "Child SETUP should be stripped. Output:\n{}",
                child.content
            );
            assert!(
                child.content.contains("SELECT 'child'"),
                "Child SELECT should remain. Output:\n{}",
                child.content
            );

            println!("Nested chapters test passed!");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

// ============================================================================
// Config-based validator tests
// ============================================================================

use mdbook_validator::config::{Config, ValidatorConfig};
use std::collections::HashMap;

/// Test: Preprocessor uses configured validator with osquery container
///
/// This is the key integration test that verifies end-to-end flow:
/// 1. Config is parsed
/// 2. Correct container image is used
/// 3. Validator script is loaded from configured path
/// 4. Validation runs and markers are stripped
#[test]
fn preprocessor_uses_configured_osquery_validator() {
    // Get the project root (where validators/ directory is)
    let book_root = std::env::current_dir().expect("should get current dir");

    // Build config programmatically
    let mut validators = HashMap::new();
    validators.insert(
        "osquery".to_string(),
        ValidatorConfig {
            container: "osquery/osquery:5.17.0-ubuntu22.04".to_string(),
            script: PathBuf::from("validators/validate-osquery.sh"),
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
    };

    // Verify the validator script exists
    let script_path = book_root.join("validators/validate-osquery.sh");
    assert!(
        script_path.exists(),
        "Validator script must exist at {}",
        script_path.display()
    );

    // Create a book with osquery SQL
    // Use simple query that works in any osquery container (no data dependencies)
    let chapter_content = r#"# osquery Test

```sql validator=osquery
SELECT uid, username FROM users LIMIT 1;
<!--ASSERT
rows >= 1
-->
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    // Process with config
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // Verify markers were stripped
            assert!(
                !output.contains("<!--ASSERT"),
                "ASSERT marker should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("rows >= 1"),
                "Assertion content should be stripped. Output:\n{output}"
            );

            // Verify visible content remains
            assert!(
                output.contains("SELECT uid, username FROM users"),
                "SQL query should remain. Output:\n{output}"
            );

            println!("osquery config test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed with configured osquery validator: {e}");
        }
    }
}

/// Test: Preprocessor errors for unknown validator name
#[test]
fn preprocessor_errors_for_unknown_validator() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Config with NO validators defined
    let config = Config {
        validators: HashMap::new(),
        fail_fast: true,
    };

    // Create a book with unknown validator
    let chapter_content = r#"# Test

```sql validator=nonexistent
SELECT 1;
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(_) => {
            panic!("Should have failed for unknown validator");
        }
        Err(e) => {
            let error_msg = format!("{e}");
            assert!(
                error_msg.contains("Unknown validator") || error_msg.contains("nonexistent"),
                "Error should mention unknown validator: {error_msg}"
            );
            println!("Unknown validator test passed! Error: {error_msg}");
        }
    }
}

/// Test: Preprocessor errors when validator script not found
#[test]
fn preprocessor_errors_for_missing_script() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Config with non-existent script
    let mut validators = HashMap::new();
    validators.insert(
        "test".to_string(),
        ValidatorConfig {
            container: "alpine:3".to_string(),
            script: PathBuf::from("validators/does-not-exist.sh"),
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
    };

    let chapter_content = r#"# Test

```sql validator=test
SELECT 1;
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(_) => {
            panic!("Should have failed for missing script");
        }
        Err(e) => {
            let error_msg = format!("{e}");
            assert!(
                error_msg.contains("Failed to read validator script")
                    || error_msg.contains("does-not-exist"),
                "Error should mention missing script: {error_msg}"
            );
            println!("Missing script test passed! Error: {error_msg}");
        }
    }
}
