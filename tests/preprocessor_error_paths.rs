//! Error path tests for preprocessor.rs to increase coverage
//!
//! These tests target specific uncovered lines in preprocessor.rs to achieve 95%+ coverage.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::str_to_string,
    clippy::needless_raw_string_hashes,
    clippy::uninlined_format_args
)]

use mdbook::book::{Book, BookItem, Chapter};
use mdbook_validator::config::{Config, ValidatorConfig};
use mdbook_validator::ValidatorPreprocessor;
use std::collections::HashMap;
use std::path::PathBuf;

/// Helper to create a chapter with sub-chapters
fn chapter_with_subs(name: &str, content: &str, subs: Vec<Chapter>) -> Chapter {
    let mut chapter = Chapter::new(name, content.to_string(), PathBuf::from("test.md"), vec![]);
    chapter.sub_items = subs.into_iter().map(BookItem::Chapter).collect();
    chapter
}

// =============================================================================
// Test 1: Nested chapters with sub_items (recursive processing)
// Target: preprocessor.rs:175-179, 194-201
// =============================================================================
#[test]
fn test_nested_chapters_validate_recursively() {
    // Create a sub-chapter with a validator block
    let sub_chapter = Chapter::new(
        "Sub Chapter",
        r#"# Sub Chapter

```sql validator=test
SELECT 'sub';
```
"#
        .to_string(),
        PathBuf::from("sub.md"),
        vec![],
    );

    // Create parent with validator block and sub-chapter
    let parent = chapter_with_subs(
        "Parent Chapter",
        r#"# Parent Chapter

```sql validator=test
SELECT 'parent';
```
"#,
        vec![sub_chapter],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(parent));

    let preprocessor = ValidatorPreprocessor::new();

    // Uses default validator that always passes (exit 0)
    let result = preprocessor.process_book(book);

    assert!(
        result.is_ok(),
        "Nested chapters should validate successfully: {:?}",
        result
    );

    // Verify parent was processed
    let processed = result.unwrap();
    let Some(BookItem::Chapter(parent_ch)) = processed.sections.first() else {
        panic!("Expected parent chapter");
    };

    assert!(
        parent_ch.content.contains("SELECT 'parent'"),
        "Parent content should be present"
    );

    // Verify sub-chapter was processed (recursive call worked)
    let Some(BookItem::Chapter(sub_ch)) = parent_ch.sub_items.first() else {
        panic!("Expected sub-chapter");
    };

    assert!(
        sub_ch.content.contains("SELECT 'sub'"),
        "Sub-chapter content should be present"
    );
}

// =============================================================================
// Test 2: Empty chapter content returns early
// Target: preprocessor.rs:210-211, 270-271
// =============================================================================
#[test]
fn test_empty_chapter_returns_early() {
    let chapter = Chapter::new(
        "Empty Chapter",
        String::new(),
        PathBuf::from("empty.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book(book);

    assert!(
        result.is_ok(),
        "Empty chapter should succeed (early return)"
    );

    // Verify chapter content remains empty
    let processed = result.unwrap();
    let Some(BookItem::Chapter(ch)) = processed.sections.first() else {
        panic!("Expected chapter");
    };

    assert!(ch.content.is_empty(), "Empty chapter should remain empty");
}

// =============================================================================
// Test 3: Chapter with no validator blocks returns early
// Target: preprocessor.rs:217-218, 277-278
// =============================================================================
#[test]
fn test_chapter_no_validator_blocks_unchanged() {
    let content = r#"# Regular Chapter

Some regular content.

```rust
fn main() {
    println!("Hello!");
}
```

More text here.
"#;

    let chapter = Chapter::new(
        "Regular Chapter",
        content.to_string(),
        PathBuf::from("regular.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book(book);

    assert!(
        result.is_ok(),
        "Chapter without validator blocks should succeed"
    );
}

// =============================================================================
// Test 4: Setup script failure returns error
// Target: preprocessor.rs:350-356
// =============================================================================
#[test]
fn test_setup_script_failure_returns_error() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Content with setup that will fail (invalid SQL)
    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
<!--SETUP
THIS_IS_NOT_VALID_SQL;
-->
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Setup Fail Chapter",
        chapter_content.to_string(),
        PathBuf::from("setup-fail.md"),
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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Setup should fail due to invalid SQL
    assert!(result.is_err(), "Setup script should fail with invalid SQL");
    let err_msg = result.unwrap_err().to_string();
    // The error may be from setup or query execution - both are acceptable
    // as long as we trigger the error path
    assert!(
        err_msg.contains("Setup") || err_msg.contains("failed") || err_msg.contains("Error"),
        "Error should mention failure: {}",
        err_msg
    );
}

// =============================================================================
// Test 5: Empty query content fails
// Target: preprocessor.rs:362-367
// =============================================================================
#[test]
fn test_empty_query_content_fails() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Content with only setup, no visible query content
    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS t(id INT)'
-->
```
"#;

    let chapter = Chapter::new(
        "Empty Query Chapter",
        chapter_content.to_string(),
        PathBuf::from("empty-query.md"),
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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Should fail because visible content is empty
    assert!(
        result.is_err(),
        "Empty query content should fail validation"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("empty") || err_msg.contains("Query"),
        "Error should mention empty query: {}",
        err_msg
    );
}

// =============================================================================
// Test 6: Validation failure includes stdout in error
// Target: preprocessor.rs:250-251, 418-423
// =============================================================================
#[test]
fn test_validation_failure_includes_output() {
    // Use the simple process_book with default validator but a block that
    // would fail if it was actually validated (which it is via exec_with_env)
    // Default validator always passes, so we need to test with config

    let book_root = std::env::current_dir().expect("should get current dir");

    // Content with assertion that will fail
    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
SELECT 1 as value;
<!--ASSERT
rows = 999
-->
```
"#;

    let chapter = Chapter::new(
        "Failing Validation",
        chapter_content.to_string(),
        PathBuf::from("fail.md"),
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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Should fail because assertion rows=999 won't match rows=1
    assert!(result.is_err(), "Assertion should fail");
    let err_msg = result.unwrap_err().to_string();
    // Error message should include the code that failed
    assert!(
        err_msg.contains("SELECT 1") || err_msg.contains("Validation failed"),
        "Error should include context: {}",
        err_msg
    );
}

// =============================================================================
// Test 7: Nested chapters with config-based validation
// Target: preprocessor.rs:194-200 (process_book_item_with_config recursive)
// =============================================================================
#[test]
fn test_nested_chapters_with_config_validate_recursively() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Create a sub-chapter with a validator block
    let sub_chapter = Chapter::new(
        "Sub Chapter",
        r#"# Sub Chapter

```sql validator=sqlite
SELECT 'sub' as name;
```
"#
        .to_string(),
        PathBuf::from("sub.md"),
        vec![],
    );

    // Create parent with validator block and sub-chapter
    let parent = chapter_with_subs(
        "Parent Chapter",
        r#"# Parent Chapter

```sql validator=sqlite
SELECT 'parent' as name;
```
"#,
        vec![sub_chapter],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(parent));

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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(
        result.is_ok(),
        "Nested chapters with config should validate: {:?}",
        result
    );

    // Verify both chapters were processed
    let processed = result.unwrap();
    let Some(BookItem::Chapter(parent_ch)) = processed.sections.first() else {
        panic!("Expected parent chapter");
    };

    // Verify sub-chapter was processed (recursive call worked)
    let Some(BookItem::Chapter(sub_ch)) = parent_ch.sub_items.first() else {
        panic!("Expected sub-chapter");
    };

    assert!(
        sub_ch.content.contains("SELECT 'sub'"),
        "Sub-chapter should be processed"
    );
}

// =============================================================================
// Test 8: Empty chapter with config returns early
// Target: preprocessor.rs:270-271
// =============================================================================
#[test]
fn test_empty_chapter_with_config_returns_early() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter = Chapter::new("Empty", String::new(), PathBuf::from("empty.md"), vec![]);

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    // Configure sqlite validator (won't be used since chapter is empty)
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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(
        result.is_ok(),
        "Empty chapter with config should return early"
    );
}

// =============================================================================
// Test 9: Chapter with no validator blocks and config returns early
// Target: preprocessor.rs:277-278
// =============================================================================
#[test]
fn test_no_validator_blocks_with_config_returns_early() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter = Chapter::new(
        "Regular",
        "# Hello\n\nSome content.\n".to_string(),
        PathBuf::from("regular.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    // Configure sqlite validator (won't be used since no validator blocks)
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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(
        result.is_ok(),
        "Chapter without validator blocks should return early"
    );
}

// =============================================================================
// Test 10: Fixtures directory does not exist
// Target: preprocessor.rs:478-483
// =============================================================================
#[test]
fn test_fixtures_dir_does_not_exist_fails() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Test",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

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
        fixtures_dir: Some(PathBuf::from("nonexistent_fixtures_dir_12345")),
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(
        result.is_err(),
        "Should fail when fixtures_dir does not exist"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("does not exist"),
        "Error should mention 'does not exist': {}",
        err_msg
    );
}

// =============================================================================
// Test 11: Fixtures directory is a file (not a directory)
// Target: preprocessor.rs:484-489
// =============================================================================
#[test]
fn test_fixtures_dir_is_file_fails() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Test",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: Some("sqlite3 -json /tmp/test.db".to_string()),
        },
    );

    // Use Cargo.toml as fixtures_dir (it exists but is a file, not a directory)
    let config = Config {
        fail_fast: true,
        fixtures_dir: Some(PathBuf::from("Cargo.toml")),
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(result.is_err(), "Should fail when fixtures_dir is a file");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not a directory"),
        "Error should mention 'not a directory': {}",
        err_msg
    );
}

// =============================================================================
// Test 12: Fixtures directory with absolute path
// Target: preprocessor.rs:471-472
// =============================================================================
#[test]
fn test_fixtures_dir_absolute_path() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Test",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: Some("sqlite3 -json /tmp/test.db".to_string()),
        },
    );

    // Use an absolute path to the validators directory (which exists)
    let fixtures_path = book_root.join("validators");
    let config = Config {
        fail_fast: true,
        fixtures_dir: Some(fixtures_path),
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Should succeed since validators/ exists and is a directory
    assert!(
        result.is_ok(),
        "Should succeed with absolute fixtures_dir path: {:?}",
        result
    );
}

// =============================================================================
// Test 13: Validation failure with stdout output in error message
// Target: preprocessor.rs:418-423 (stdout branch in error formatting)
// =============================================================================
#[test]
fn test_validation_failure_with_stdout_in_error() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Use a query that produces output AND fails validation
    let chapter_content = r#"# Test Chapter

```sql validator=sqlite
SELECT 'hello_world_test' as message;
<!--ASSERT
rows = 0
-->
```
"#;

    let chapter = Chapter::new(
        "Test Stdout",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Should fail because assertion rows=0 won't match rows=1
    assert!(result.is_err(), "Assertion should fail");
    let err_msg = result.unwrap_err().to_string();
    // Error message should include either the code or validation failure
    assert!(
        err_msg.contains("Validation failed")
            || err_msg.contains("SELECT")
            || err_msg.contains("stderr"),
        "Error should include context: {}",
        err_msg
    );
}

// =============================================================================
// Test 14: Invalid validator config (empty container)
// Target: preprocessor.rs:462-466
// =============================================================================
#[test]
fn test_invalid_validator_config_empty_container() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```sql validator=bad_validator
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Test Invalid Config",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let mut validators = HashMap::new();
    validators.insert(
        "bad_validator".to_string(),
        ValidatorConfig {
            container: String::new(), // Empty container is invalid
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: None,
        },
    );

    let config = Config {
        fail_fast: true,
        fixtures_dir: None,
        validators,
    };

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    assert!(result.is_err(), "Should fail with invalid validator config");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Invalid validator config") || err_msg.contains("container"),
        "Error should mention invalid config: {}",
        err_msg
    );
}

// =============================================================================
// Test 15: Deeply nested chapters (3 levels) with simple path
// Target: preprocessor.rs:175-179 (deeper recursive call)
// =============================================================================
#[test]
fn test_deeply_nested_chapters_simple_path() {
    // Create a 3-level deep nesting
    let level3 = Chapter::new(
        "Level 3",
        r#"# Level 3

```sql validator=test
SELECT 'level3';
```
"#
        .to_string(),
        PathBuf::from("level3.md"),
        vec![],
    );

    let level2 = chapter_with_subs(
        "Level 2",
        r#"# Level 2

```sql validator=test
SELECT 'level2';
```
"#,
        vec![level3],
    );

    let level1 = chapter_with_subs(
        "Level 1",
        r#"# Level 1

```sql validator=test
SELECT 'level1';
```
"#,
        vec![level2],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(level1));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book(book);

    assert!(
        result.is_ok(),
        "Deeply nested chapters should validate: {:?}",
        result
    );

    // Verify all levels were processed
    let processed = result.unwrap();
    let Some(BookItem::Chapter(l1)) = processed.sections.first() else {
        panic!("Expected level 1 chapter");
    };
    assert!(l1.content.contains("SELECT 'level1'"));

    let Some(BookItem::Chapter(l2)) = l1.sub_items.first() else {
        panic!("Expected level 2 chapter");
    };
    assert!(l2.content.contains("SELECT 'level2'"));

    let Some(BookItem::Chapter(l3)) = l2.sub_items.first() else {
        panic!("Expected level 3 chapter");
    };
    assert!(l3.content.contains("SELECT 'level3'"));
}

// =============================================================================
// Test 16: Validation failure in simple path (no config)
// Target: preprocessor.rs:236-240, 251 (validation exec failure)
// =============================================================================
#[test]
fn test_validation_failure_simple_path() {
    // Create a validator script that fails
    let failing_script =
        b"#!/bin/sh\necho 'stdout message' && echo 'stderr message' >&2 && exit 1\n";

    let chapter_content = r#"# Test Chapter

```sql validator=test
SELECT 1;
```
"#;

    let chapter = Chapter::new(
        "Test Failure",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book_with_script(book, failing_script);

    assert!(result.is_err(), "Should fail with failing validator script");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Validation failed") || err_msg.contains("exit code"),
        "Error should mention validation failure: {}",
        err_msg
    );
}

// =============================================================================
// Test 17: Chapter with skip attribute (simple path)
// Target: preprocessor.rs:223-225 (skip branch in process_chapter)
// =============================================================================
#[test]
fn test_skip_attribute_simple_path() {
    let chapter_content = r#"# Test Chapter

```sql validator=test skip
SELECT 'this is skipped';
```
"#;

    let chapter = Chapter::new(
        "Test Skip",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

    let preprocessor = ValidatorPreprocessor::new();
    let result = preprocessor.process_book(book);

    // Should succeed because skip blocks don't get validated
    assert!(result.is_ok(), "Skipped blocks should pass: {:?}", result);
}

// =============================================================================
// Test 18: Chapter with skip attribute (config path)
// Target: preprocessor.rs:283-285 (skip branch in process_chapter_with_config)
// =============================================================================
#[test]
fn test_skip_attribute_config_path() {
    let book_root = std::env::current_dir().expect("should get current dir");

    let chapter_content = r#"# Test Chapter

```sql validator=sqlite skip
SELECT 'this is skipped';
```
"#;

    let chapter = Chapter::new(
        "Test Skip Config",
        chapter_content.to_string(),
        PathBuf::from("test.md"),
        vec![],
    );

    let mut book = Book::new();
    book.sections.push(BookItem::Chapter(chapter));

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
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    // Should succeed because skip blocks don't get validated
    assert!(
        result.is_ok(),
        "Skipped blocks with config should pass: {:?}",
        result
    );
}
