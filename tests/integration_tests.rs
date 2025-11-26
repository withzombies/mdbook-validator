//! Integration tests for mdbook-validator
//!
//! Tests are allowed to panic for assertions and test failure.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

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
            // If Docker isn't running, skip the test rather than failing
            let error_msg = format!("{e}");
            if error_msg.contains("Docker") || error_msg.contains("container") {
                println!("Skipping test - Docker may not be running: {e}");
                return;
            }
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
            let error_msg = format!("{e}");
            if error_msg.contains("Docker") || error_msg.contains("container") {
                println!("Skipping test - Docker may not be running: {e}");
                return;
            }
            panic!("Preprocessor failed on content with no validator blocks: {e}");
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
            let error_msg = format!("{e}");
            if error_msg.contains("Docker") || error_msg.contains("container") {
                println!("Skipping test - Docker may not be running: {e}");
                return;
            }
            panic!("Preprocessor failed: {e}");
        }
    }
}
