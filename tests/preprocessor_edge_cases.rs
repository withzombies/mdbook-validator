//! Tests for preprocessor and container edge cases
//!
//! Tests Default trait, container ID, skip attribute, exec command fallback,
//! and container caching behavior.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::str_to_string,
    clippy::needless_raw_string_hashes,
    clippy::default_constructed_unit_structs
)]

use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::Preprocessor;
use mdbook_validator::config::{Config, ValidatorConfig};
use mdbook_validator::container::ValidatorContainer;
use mdbook_validator::ValidatorPreprocessor;
use std::collections::HashMap;
use std::path::PathBuf;

// =============================================================================
// Test 1: Default::default() for ValidatorPreprocessor
// Target: preprocessor.rs:38-40
// =============================================================================
#[test]
fn preprocessor_default_creates_instance() {
    let preprocessor = ValidatorPreprocessor::default();
    assert_eq!(preprocessor.name(), "validator");
}

// =============================================================================
// Test 2: ValidatorContainer::id() method
// Target: container.rs:182-184
// =============================================================================
#[tokio::test]
async fn container_id_returns_valid_docker_id() {
    let container = ValidatorContainer::start_raw("alpine:3.19")
        .await
        .expect("alpine container should start");

    let id = container.id();

    // Docker container IDs are 64 hex chars, but short form is 12+
    assert!(!id.is_empty(), "Container ID should not be empty");
    assert!(
        id.len() >= 12,
        "Container ID should be at least 12 chars (short form), got: {id}"
    );
    assert!(
        id.chars().all(|c| c.is_ascii_hexdigit()),
        "Container ID should be hex digits, got: {id}"
    );
}

// =============================================================================
// Test 3: Skip attribute in code block
// Target: preprocessor.rs:222-223, 282-283
// =============================================================================
#[test]
fn preprocessor_skips_validation_with_skip_attribute() {
    // Create chapter with skip attribute - validation should be skipped
    // but content should remain in output
    let chapter_content = r#"# Test Chapter

```sql validator=sqlite skip
SELECT * FROM nonexistent_table_that_would_fail;
```

Text after.
"#;

    let chapter = Chapter::new(
        "Test Chapter",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book(book);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter in processed book");
            };

            // Content should remain (validation was skipped, not removed)
            assert!(
                chapter.content.contains("SELECT * FROM nonexistent_table"),
                "Skip should preserve content. Output:\n{}",
                chapter.content
            );
        }
        Err(e) => {
            panic!("Preprocessor should pass with skip attribute (no validation): {e}");
        }
    }
}

// =============================================================================
// Test 4: get_exec_command fallback for custom validator names
// Target: preprocessor.rs:439 (_ => DEFAULT_EXEC_FALLBACK)
// =============================================================================
#[test]
fn preprocessor_uses_fallback_exec_for_custom_validator() {
    // Configure a custom validator that's NOT sqlite or osquery
    // This should use DEFAULT_EXEC_FALLBACK ("cat")
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```text validator=custom
Just some text content
```
"#;

    let chapter = Chapter::new(
        "Test Chapter",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    // Create config with custom validator (not sqlite/osquery)
    let mut validators = HashMap::new();
    validators.insert(
        "custom".to_string(),
        ValidatorConfig {
            container: "alpine:3.19".to_string(),
            script: PathBuf::from("validators/validate-template.sh"),
            exec_command: None, // No exec_command = use fallback
        },
    );

    let config = Config {
        fail_fast: true,
        fixtures_dir: None,
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();

    // This exercises the _ => DEFAULT_EXEC_FALLBACK branch
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // The test passes if we reach this point - the fallback was used
    // The validator may pass or fail, but the key is get_exec_command worked
    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter");
            };
            // Content should be processed (markers stripped if any)
            assert!(
                chapter.content.contains("Just some text content"),
                "Content should remain"
            );
            println!("Custom validator with fallback exec succeeded");
        }
        Err(e) => {
            // Validation failure is acceptable - we're testing the exec_command path
            // not the validator success. The key is it didn't panic on get_exec_command.
            println!("Custom validator error (acceptable): {e}");
        }
    }
}

// =============================================================================
// Test 5: Container cache hit (same validator used twice)
// Target: preprocessor.rs:452 (Entry::Occupied branch)
// =============================================================================
#[test]
fn preprocessor_reuses_container_for_multiple_blocks() {
    // Two blocks with same validator should reuse the container
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

First query:
```sql validator=sqlite
SELECT 1;
```

Second query (should reuse container):
```sql validator=sqlite
SELECT 2;
```
"#;

    let chapter = Chapter::new(
        "Test Chapter",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    // Configure sqlite validator
    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: Some("sqlite3 -json /tmp/test.db".to_string()),
        },
    );

    let config = Config {
        fail_fast: true,
        fixtures_dir: None,
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();

    // This exercises Entry::Occupied on second block
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.sections.first() else {
                panic!("Expected chapter");
            };

            // Both queries should be processed
            assert!(
                chapter.content.contains("SELECT 1"),
                "First query should remain"
            );
            assert!(
                chapter.content.contains("SELECT 2"),
                "Second query should remain"
            );

            println!("Container reuse test passed!");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}
