//! Markdown parsing and code block extraction

/// Parses an info string from a fenced code block.
///
/// Returns `(language, validator, skip)` tuple.
///
/// # Examples
///
/// - `"sql validator=sqlite"` → `("sql", Some("sqlite"), false)`
/// - `"rust"` → `("rust", None, false)`
/// - `"sql validator=osquery skip"` → `("sql", Some("osquery"), true)`
#[must_use]
pub fn parse_info_string(info: &str) -> (String, Option<String>, bool) {
    let parts: Vec<&str> = info.split_whitespace().collect();

    let language = parts.first().map_or(String::new(), |s| (*s).to_owned());

    let validator = parts
        .iter()
        .find_map(|part| part.strip_prefix("validator=").map(ToOwned::to_owned))
        .filter(|v| !v.is_empty());

    let skip = parts.contains(&"skip");

    (language, validator, skip)
}

/// Result of extracting markers from code block content.
#[derive(Debug, Clone, Default)]
pub struct ExtractedMarkers {
    /// Setup content from `<!--SETUP-->` marker
    pub setup: Option<String>,
    /// Assertions from `<!--ASSERT-->` marker
    pub assertions: Option<String>,
    /// Expected output from `<!--EXPECT-->` marker
    pub expect: Option<String>,
    /// The visible content (with all markers removed)
    pub visible_content: String,
}

impl ExtractedMarkers {
    /// Get content for validation (with `@@` prefix stripped but lines kept).
    ///
    /// This returns `visible_content` with the `@@` prefix removed from each line,
    /// but the line content is preserved (unlike output which removes entire lines).
    #[must_use]
    pub fn validation_content(&self) -> String {
        strip_double_at_prefix(&self.visible_content)
    }
}

/// Extracts markers from code block content.
///
/// Parses `<!--SETUP-->`, `<!--ASSERT-->`, and `<!--EXPECT-->` blocks,
/// returning their content and the remaining visible content.
#[must_use]
pub fn extract_markers(content: &str) -> ExtractedMarkers {
    let mut result = ExtractedMarkers::default();
    let mut remaining = content.to_owned();

    // Extract SETUP block
    if let Some((before, inner, after)) = extract_marker_block(&remaining, "<!--SETUP") {
        result.setup = Some(inner);
        remaining = format!("{before}{after}");
    }

    // Extract ASSERT block
    if let Some((before, inner, after)) = extract_marker_block(&remaining, "<!--ASSERT") {
        result.assertions = Some(inner);
        remaining = format!("{before}{after}");
    }

    // Extract EXPECT block
    if let Some((before, inner, after)) = extract_marker_block(&remaining, "<!--EXPECT") {
        result.expect = Some(inner);
        remaining = format!("{before}{after}");
    }

    // Trim leading/trailing whitespace from visible content
    remaining.trim().clone_into(&mut result.visible_content);

    result
}

/// Strips the `@@` prefix from lines while keeping the content.
///
/// This is used for validation content - `@@` lines should be validated
/// but the `@@` prefix itself is not part of the syntax being validated.
///
/// # Examples
///
/// - `"@@SELECT 'hidden';\nSELECT 'visible';"` → `"SELECT 'hidden';\nSELECT 'visible';"`
/// - `"@@\nvisible"` → `"\nvisible"` (empty @@ line becomes empty line)
#[must_use]
pub fn strip_double_at_prefix(content: &str) -> String {
    content
        .lines()
        .map(|line| line.strip_prefix("@@").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extracts content between a marker and `-->`.
///
/// Returns `(before, inner_content, after)` if found.
fn extract_marker_block(content: &str, marker: &str) -> Option<(String, String, String)> {
    let start = content.find(marker)?;
    let marker_end = content[start..].find('\n').map(|i| start + i + 1)?;
    let end_marker = content[marker_end..].find("-->")?;
    let end = marker_end + end_marker;

    let before = &content[..start];
    let inner = content[marker_end..end].trim();
    let after = &content[end + 3..]; // Skip "-->"

    Some((before.to_owned(), inner.to_owned(), after.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== parse_info_string tests ====================

    #[test]
    fn parse_info_string_language_only() {
        let (lang, validator, skip) = parse_info_string("sql");
        assert_eq!(lang, "sql");
        assert_eq!(validator, None);
        assert!(!skip);
    }

    #[test]
    fn parse_info_string_with_validator() {
        let (lang, validator, skip) = parse_info_string("sql validator=sqlite");
        assert_eq!(lang, "sql");
        assert_eq!(validator, Some("sqlite".to_owned()));
        assert!(!skip);
    }

    #[test]
    fn parse_info_string_with_skip() {
        let (lang, validator, skip) = parse_info_string("sql validator=osquery skip");
        assert_eq!(lang, "sql");
        assert_eq!(validator, Some("osquery".to_owned()));
        assert!(skip);
    }

    #[test]
    fn parse_info_string_skip_without_validator() {
        let (lang, validator, skip) = parse_info_string("bash skip");
        assert_eq!(lang, "bash");
        assert_eq!(validator, None);
        assert!(skip);
    }

    #[test]
    fn parse_info_string_empty() {
        let (lang, validator, skip) = parse_info_string("");
        assert_eq!(lang, "");
        assert_eq!(validator, None);
        assert!(!skip);
    }

    #[test]
    fn parse_info_string_extra_whitespace() {
        let (lang, validator, skip) = parse_info_string("  sql   validator=sqlite   skip  ");
        assert_eq!(lang, "sql");
        assert_eq!(validator, Some("sqlite".to_owned()));
        assert!(skip);
    }

    #[test]
    fn parse_info_string_empty_validator_ignored() {
        let (lang, validator, skip) = parse_info_string("sql validator=");
        assert_eq!(lang, "sql");
        assert_eq!(validator, None); // Empty validator is filtered out
        assert!(!skip);
    }

    #[test]
    fn parse_info_string_multiple_validators_takes_first() {
        let (lang, validator, skip) = parse_info_string("sql validator=first validator=second");
        assert_eq!(lang, "sql");
        assert_eq!(validator, Some("first".to_owned()));
        assert!(!skip);
    }

    // ==================== extract_markers tests ====================

    #[test]
    fn extract_markers_setup_only() {
        let content = "<!--SETUP\nCREATE TABLE test;\n-->\nSELECT * FROM test;";
        let result = extract_markers(content);
        assert_eq!(result.setup, Some("CREATE TABLE test;".to_owned()));
        assert_eq!(result.assertions, None);
        assert_eq!(result.expect, None);
        assert_eq!(result.visible_content, "SELECT * FROM test;");
    }

    #[test]
    fn extract_markers_assert_only() {
        let content = "SELECT * FROM test;\n<!--ASSERT\nrows >= 1\n-->";
        let result = extract_markers(content);
        assert_eq!(result.setup, None);
        assert_eq!(result.assertions, Some("rows >= 1".to_owned()));
        assert_eq!(result.expect, None);
        assert_eq!(result.visible_content, "SELECT * FROM test;");
    }

    #[test]
    fn extract_markers_expect_only() {
        let content = "SELECT 1;\n<!--EXPECT\n[{\"1\": 1}]\n-->";
        let result = extract_markers(content);
        assert_eq!(result.setup, None);
        assert_eq!(result.assertions, None);
        assert_eq!(result.expect, Some("[{\"1\": 1}]".to_owned()));
        assert_eq!(result.visible_content, "SELECT 1;");
    }

    #[test]
    fn extract_markers_all_three() {
        let content = "<!--SETUP\nCREATE TABLE t;\n-->\nSELECT * FROM t;\n<!--ASSERT\nrows = 0\n-->\n<!--EXPECT\n[]\n-->";
        let result = extract_markers(content);
        assert_eq!(result.setup, Some("CREATE TABLE t;".to_owned()));
        assert_eq!(result.assertions, Some("rows = 0".to_owned()));
        assert_eq!(result.expect, Some("[]".to_owned()));
        assert_eq!(result.visible_content, "SELECT * FROM t;");
    }

    #[test]
    fn extract_markers_none() {
        let content = "SELECT * FROM users;";
        let result = extract_markers(content);
        assert_eq!(result.setup, None);
        assert_eq!(result.assertions, None);
        assert_eq!(result.expect, None);
        assert_eq!(result.visible_content, "SELECT * FROM users;");
    }

    #[test]
    fn extract_markers_multiline_setup() {
        let content = "<!--SETUP\nCREATE TABLE t (id INT);\nINSERT INTO t VALUES (1);\nINSERT INTO t VALUES (2);\n-->\nSELECT * FROM t;";
        let result = extract_markers(content);
        assert!(result.setup.is_some());
        let setup = result.setup.unwrap();
        assert!(setup.contains("CREATE TABLE"));
        assert!(setup.contains("INSERT INTO t VALUES (1)"));
        assert!(setup.contains("INSERT INTO t VALUES (2)"));
    }

    #[test]
    fn extract_markers_multiline_assertions() {
        let content = "SELECT * FROM t;\n<!--ASSERT\nrows >= 1\ncontains \"foo\"\n-->";
        let result = extract_markers(content);
        assert!(result.assertions.is_some());
        let assertions = result.assertions.unwrap();
        assert!(assertions.contains("rows >= 1"));
        assert!(assertions.contains("contains \"foo\""));
    }

    #[test]
    fn extract_markers_preserves_visible_content_order() {
        let content = "-- First line\n<!--SETUP\nsetup;\n-->\n-- Second line\nSELECT 1;";
        let result = extract_markers(content);
        assert!(result.visible_content.contains("First line"));
        assert!(result.visible_content.contains("Second line"));
        assert!(result.visible_content.contains("SELECT 1"));
    }

    // ==================== strip_double_at_prefix tests ====================

    #[test]
    fn strip_double_at_prefix_strips_prefix() {
        let content = "@@SELECT 'hidden';\nSELECT 'visible';";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "SELECT 'hidden';\nSELECT 'visible';");
    }

    #[test]
    fn strip_double_at_prefix_preserves_lines_without_prefix() {
        let content = "SELECT 'visible';\nSELECT 'also visible';";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, content);
    }

    #[test]
    fn strip_double_at_prefix_empty_at_line() {
        // @@ alone becomes empty line
        let content = "@@\nvisible";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "\nvisible");
    }

    #[test]
    fn strip_double_at_prefix_at_in_middle_unchanged() {
        // @@ in middle of line is NOT stripped (must be at start)
        let content = "line with @@ in middle";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, content);
    }

    #[test]
    fn strip_double_at_prefix_multiple_at_lines() {
        let content = "@@first\n@@second\nvisible\n@@third";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "first\nsecond\nvisible\nthird");
    }

    #[test]
    fn strip_double_at_prefix_only_at_lines() {
        let content = "@@line1\n@@line2";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn strip_double_at_prefix_double_prefix_strips_one() {
        // @@@@foo should become @@foo (only one @@ prefix stripped per line)
        // This is intentional: if user writes @@@@, they want @@ in validation content
        let content = "@@@@foo";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "@@foo");
    }

    #[test]
    fn strip_double_at_prefix_mixed_leading_and_middle() {
        // Only leading @@ should be stripped, @@ in middle of line stays
        let content = "@@first line\nline with @@ middle\n@@another hidden";
        let result = strip_double_at_prefix(content);
        assert_eq!(result, "first line\nline with @@ middle\nanother hidden");
    }

    // ==================== validation_content tests ====================

    #[test]
    fn extracted_markers_validation_content_strips_at_prefix() {
        let content = "@@SELECT 'hidden';\nSELECT 'visible';";
        let markers = extract_markers(content);
        assert_eq!(
            markers.validation_content(),
            "SELECT 'hidden';\nSELECT 'visible';"
        );
    }
}
