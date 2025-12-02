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

use mdbook_preprocessor::book::{Book, BookItem, Chapter};
use mdbook_preprocessor::Preprocessor;
use mdbook_validator::config::{Config, ValidatorConfig};
use mdbook_validator::ValidatorPreprocessor;
use std::collections::HashMap;
use std::path::PathBuf;

/// Creates a test config with sqlite validator
fn create_sqlite_config() -> Config {
    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: Some("sqlite3 -json /tmp/test.db".to_string()),
        },
    );

    Config {
        validators,
        fail_fast: true,
        fixtures_dir: None,
    }
}

#[test]
fn preprocessor_has_correct_name() {
    let preprocessor = ValidatorPreprocessor::new();
    assert_eq!(preprocessor.name(), "validator");
}

#[test]
fn preprocessor_supports_html_renderer() {
    let preprocessor = ValidatorPreprocessor::new();
    assert!(preprocessor.supports_renderer("html").unwrap());
}

#[test]
fn preprocessor_supports_all_renderers() {
    // We validate and strip markers, producing valid markdown for any output format
    let preprocessor = ValidatorPreprocessor::new();
    assert!(preprocessor.supports_renderer("pdf").unwrap());
    assert!(preprocessor.supports_renderer("epub").unwrap());
    assert!(preprocessor.supports_renderer("markdown").unwrap());
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
    book.items.push(BookItem::Chapter(chapter));
    book
}

/// Test: Preprocessor validates and strips markers from code blocks.
///
/// This test requires Docker to be running.
#[test]
fn preprocessor_validates_and_strips_markers() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    let chapter_content = r"# Test Chapter

Some introductory text.

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS test(id INTEGER);'
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

    // Run preprocessor using process_book_with_config (uses real sqlite validator)
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            // Get the processed chapter content
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
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
                output.contains("validator=sqlite"),
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
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

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

    // No validator blocks means no validation runs, but we still use process_book_with_config
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

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

/// Test: Preprocessor strips @@ hidden lines from OUTPUT
///
/// Note: The @@ feature strips lines from rendered output. This test uses
/// `process_book_with_script` (not a real validator) because the @@ stripping
/// for validation input is a separate feature that needs implementation.
/// This test verifies OUTPUT stripping works correctly.
#[test]
fn preprocessor_strips_hidden_lines_from_output() {
    // Use process_book_with_script with a passing script to test output stripping
    // without needing the full @@ validation stripping implementation
    let passing_script = b"#!/bin/sh\nexit 0\n";

    let chapter_content = r"# Test Chapter

```sql validator=test
@@SELECT 'hidden_setup' as setup;
SELECT 'visible' as result;
@@SELECT 'hidden_cleanup' as cleanup;
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_script(book, passing_script);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // Hidden lines should be stripped from output
            assert!(
                !output.contains("@@"),
                "@@ prefix should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("hidden_setup"),
                "Hidden setup line should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("hidden_cleanup"),
                "Hidden cleanup line should be stripped. Output:\n{output}"
            );

            // Visible content remains
            assert!(
                output.contains("SELECT 'visible'"),
                "Visible content should remain. Output:\n{output}"
            );

            println!("Hidden lines test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

/// Test: @@ prefix stripped from VALIDATION content (but lines kept)
///
/// This is the key integration test for the @@ prefix bug fix:
/// - `@@SELECT 1;` is INVALID SQL (if @@ not stripped, validator fails)
/// - After fix, @@ prefix is stripped → `SELECT 1;` → valid SQL → validator passes
/// - Output still removes entire @@ lines (existing behavior)
///
/// Uses real sqlite validator to prove the fix works end-to-end.
#[test]
fn double_at_prefix_stripped_for_validation_content() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    // Content has @@ prefixed SQL which would be INVALID if @@ wasn't stripped
    // @@SELECT 1; → SELECT 1; (valid SQL) for validation
    // @@SELECT 1; → (line removed) for output
    let chapter_content = r"# Hidden Line Test

```sql validator=sqlite
@@SELECT 1 as hidden_result;
SELECT 2 as visible_result;
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    // This will FAIL if @@ prefix is NOT stripped (@@SELECT is invalid SQL)
    // This will PASS if @@ prefix IS stripped (SELECT 1 is valid SQL)
    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // @@ lines should be REMOVED from output (existing behavior)
            assert!(
                !output.contains("hidden_result"),
                "Hidden line content should be removed from output. Output:\n{output}"
            );

            // Non-@@ lines should remain in output
            assert!(
                output.contains("SELECT 2 as visible_result"),
                "Visible content should remain. Output:\n{output}"
            );

            println!("Hidden line validation test passed! Output:\n{output}");
        }
        Err(e) => {
            // If this fails, the @@ prefix was NOT stripped before validation
            panic!("Validator failed - @@ prefix was likely NOT stripped before validation: {e}");
        }
    }
}

/// Test: @@ prefix with SETUP and ASSERT markers combined
///
/// Verifies that @@ prefix stripping works correctly when combined with other markers.
#[test]
fn double_at_prefix_works_with_setup_and_assert() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    // Complex case: @@ lines + SETUP + ASSERT
    let chapter_content = r"# Hidden Lines with Markers

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS items(id INTEGER); INSERT INTO items VALUES(1);'
-->
@@SELECT id FROM items WHERE id = 1;
SELECT id FROM items;
<!--ASSERT
rows >= 1
-->
```
";

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // All markers stripped
            assert!(!output.contains("<!--SETUP"), "SETUP should be stripped");
            assert!(!output.contains("<!--ASSERT"), "ASSERT should be stripped");
            assert!(
                !output.contains("WHERE id = 1"),
                "Hidden query should be removed from output. Output:\n{output}"
            );

            // Visible content remains
            assert!(
                output.contains("SELECT id FROM items"),
                "Visible SELECT should remain. Output:\n{output}"
            );

            println!("Hidden lines with markers test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed: {e}");
        }
    }
}

/// Test: Chapter with multiple validator= blocks (all validated, all stripped)
#[test]
fn preprocessor_handles_multiple_validator_blocks() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    let chapter_content = r"# Test Chapter

First block:

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS t1(id INTEGER);'
-->
SELECT 1;
```

Second block:

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS t2(id INTEGER);'
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

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // All markers stripped from both validator blocks
            assert!(
                !output.contains("<!--SETUP"),
                "SETUP markers should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("CREATE TABLE"),
                "First setup should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains("t1(id"),
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

/// Creates a book with nested chapters (uses sqlite validator)
fn create_book_with_nested_chapters() -> Book {
    let parent_content = r"# Parent Chapter

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS parent(id INTEGER);'
-->
SELECT 'parent';
```
";

    let child_content = r"# Child Chapter

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS child(id INTEGER);'
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
    book.items.push(BookItem::Chapter(parent_chapter));
    book
}

/// Test: Nested sub-chapters processed recursively
#[test]
fn preprocessor_handles_nested_chapters() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    let book = create_book_with_nested_chapters();
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            // Check parent chapter
            let Some(BookItem::Chapter(parent)) = processed_book.items.first() else {
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
            exec_command: None,
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
        fixtures_dir: None,
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
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
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
        fixtures_dir: None,
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

/// Test: EXPECT marker passes when output matches expected
///
/// Full end-to-end test with `SQLite` container, EXPECT marker parsed from markdown.
#[test]
fn preprocessor_expect_marker_passes_when_output_matches() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Configure SQLite validator
    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: None,
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
        fixtures_dir: None,
    };

    // Create book with EXPECT marker that should match
    // We use a deterministic query with known output
    let chapter_content = r#"# EXPECT Test

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE items(id INTEGER); INSERT INTO items VALUES(1);'
-->
SELECT id FROM items;
<!--EXPECT
[{"id":1}]
-->
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter");
            };

            let output = &chapter.content;

            // EXPECT marker should be stripped
            assert!(
                !output.contains("<!--EXPECT"),
                "EXPECT marker should be stripped. Output:\n{output}"
            );
            assert!(
                !output.contains(r#"[{"id":1}]"#),
                "Expected output should be stripped. Output:\n{output}"
            );

            // SETUP should also be stripped
            assert!(
                !output.contains("<!--SETUP"),
                "SETUP marker should be stripped. Output:\n{output}"
            );

            // Visible content remains
            assert!(
                output.contains("SELECT id FROM items"),
                "SQL query should remain. Output:\n{output}"
            );

            println!("EXPECT pass test succeeded! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor should pass when EXPECT matches actual output: {e}");
        }
    }
}

/// Test: EXPECT marker fails when output doesn't match expected
///
/// Verifies that EXPECT marker comparison produces clear error on mismatch.
#[test]
fn preprocessor_expect_marker_fails_when_output_differs() {
    let book_root = std::env::current_dir().expect("should get current dir");

    // Configure SQLite validator
    let mut validators = HashMap::new();
    validators.insert(
        "sqlite".to_string(),
        ValidatorConfig {
            container: "keinos/sqlite3:3.47.2".to_string(),
            script: PathBuf::from("validators/validate-sqlite.sh"),
            exec_command: None,
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
        fixtures_dir: None,
    };

    // Create book with EXPECT marker that WON'T match (expecting id=999, actual is id=1)
    let chapter_content = r#"# EXPECT Mismatch Test

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE items(id INTEGER); INSERT INTO items VALUES(1);'
-->
SELECT id FROM items;
<!--EXPECT
[{"id":999}]
-->
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(_) => {
            panic!("Preprocessor should fail when EXPECT doesn't match actual output");
        }
        Err(e) => {
            let error_msg = format!("{e}");

            // Error should indicate validation failure
            assert!(
                error_msg.contains("Validation failed") || error_msg.contains("mismatch"),
                "Error should mention validation failure or mismatch. Got: {error_msg}"
            );

            println!("EXPECT fail test succeeded! Error:\n{error_msg}");
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
            exec_command: None,
        },
    );

    let config = Config {
        validators,
        fail_fast: true,
        fixtures_dir: None,
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

/// Test: hidden and skip together returns E011 error
///
/// Verifies that `hidden` and `skip` are mutually exclusive.
/// Using both should produce a clear E011 error.
#[test]
fn preprocessor_errors_on_hidden_and_skip_together() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    // Code block with both hidden AND skip - should fail with E011
    let chapter_content = r#"# Mutual Exclusivity Test

```sql validator=sqlite hidden skip
SELECT 1;
```
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(_) => {
            panic!("Should have failed with E011 for hidden+skip combination");
        }
        Err(e) => {
            let error_msg = format!("{e}");

            // Verify E011 error message
            assert!(
                error_msg.contains("E011") || error_msg.contains("mutually exclusive"),
                "Error should mention E011 or mutual exclusivity. Got: {error_msg}"
            );
            assert!(
                error_msg.contains("hidden") && error_msg.contains("skip"),
                "Error should mention both 'hidden' and 'skip'. Got: {error_msg}"
            );

            println!("E011 mutual exclusivity test passed! Error: {error_msg}");
        }
    }
}

/// Test: hidden attribute removes entire code block from output
///
/// Full end-to-end test verifying that:
/// 1. Code block with `hidden` attribute is validated (query runs)
/// 2. Entire code fence is removed from output (no fence delimiters, no content)
/// 3. Non-hidden blocks in same document remain visible
#[test]
fn preprocessor_hidden_attribute_removes_entire_block() {
    let book_root = std::env::current_dir().expect("should get current dir");
    let config = create_sqlite_config();

    // Document has: hidden block (should be removed) + visible block (should remain)
    let chapter_content = r#"# Hidden Block Test

Setup text before.

```sql validator=sqlite hidden
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE IF NOT EXISTS hidden_test(id INTEGER); INSERT INTO hidden_test VALUES(42);'
-->
SELECT id FROM hidden_test;
<!--ASSERT
rows >= 1
-->
```

Middle text.

```sql validator=sqlite
SELECT 'visible_query' as result;
<!--ASSERT
rows >= 1
-->
```

End text.
"#;

    let book = create_book_with_content(chapter_content);
    let preprocessor = ValidatorPreprocessor::new();

    let result = preprocessor.process_book_with_config(book, &config, &book_root);

    match result {
        Ok(processed_book) => {
            let Some(BookItem::Chapter(chapter)) = processed_book.items.first() else {
                panic!("Expected chapter in processed book");
            };

            let output = &chapter.content;

            // Hidden block should be COMPLETELY removed (no fence, no content)
            assert!(
                !output.contains("hidden_test"),
                "Hidden block table name should not appear. Output:\n{output}"
            );
            assert!(
                !output.contains("SELECT id FROM"),
                "Hidden block query should not appear. Output:\n{output}"
            );

            // Verify no fence delimiters for hidden block remain
            // Count sql blocks - should only be 1 (the visible one)
            let sql_block_count = output.matches("```sql").count();
            assert_eq!(
                sql_block_count, 1,
                "Should have exactly 1 sql block (visible only). Output:\n{output}"
            );

            // Visible block should remain
            assert!(
                output.contains("visible_query"),
                "Visible block should remain. Output:\n{output}"
            );

            // Text content should remain
            assert!(
                output.contains("Setup text before"),
                "Text before should remain. Output:\n{output}"
            );
            assert!(
                output.contains("Middle text"),
                "Middle text should remain. Output:\n{output}"
            );
            assert!(
                output.contains("End text"),
                "End text should remain. Output:\n{output}"
            );

            // Markers should be stripped from visible block
            assert!(
                !output.contains("<!--ASSERT"),
                "ASSERT marker should be stripped. Output:\n{output}"
            );

            println!("Hidden attribute E2E test passed! Output:\n{output}");
        }
        Err(e) => {
            panic!("Preprocessor failed - hidden block should still validate: {e}");
        }
    }
}
