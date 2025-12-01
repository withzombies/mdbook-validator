//! Strip validation markers from output

/// Strips all validation markers from a code block, returning clean content.
///
/// This removes:
/// - `<!--SETUP-->` ... `-->` blocks
/// - `<!--ASSERT-->` ... `-->` blocks
/// - `<!--EXPECT-->` ... `-->` blocks
/// - Lines starting with `@@` prefix
#[must_use]
pub fn strip_markers(content: &str) -> String {
    let mut result = content.to_owned();

    // Strip <!--SETUP ... --> blocks
    result = strip_marker_block(&result, "<!--SETUP");

    // Strip <!--ASSERT ... --> blocks
    result = strip_marker_block(&result, "<!--ASSERT");

    // Strip <!--EXPECT ... --> blocks
    result = strip_marker_block(&result, "<!--EXPECT");

    // Strip lines starting with @@
    result = strip_double_at_lines(&result);

    result
}

fn strip_double_at_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.starts_with("@@"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_marker_block(content: &str, marker: &str) -> String {
    let mut result = content.to_owned();

    while let Some(start) = result.find(marker) {
        if let Some(end_offset) = result[start..].find("-->") {
            let end = start + end_offset + 3; // Include "-->"

            // Remove trailing newline if present
            let end = if result.get(end..end + 1) == Some("\n") {
                end + 1
            } else {
                end
            };

            // Remove leading newline if present
            let start = if start > 0 && result.get(start - 1..start) == Some("\n") {
                start - 1
            } else {
                start
            };

            result = format!("{}{}", &result[..start], &result[end..]);
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== strip_markers tests ====================

    #[test]
    fn strip_markers_removes_setup() {
        let content = "<!--SETUP\nCREATE TABLE t;\n-->\nSELECT * FROM t;";
        let result = strip_markers(content);
        assert!(!result.contains("SETUP"));
        assert!(!result.contains("CREATE TABLE"));
        assert!(result.contains("SELECT * FROM t;"));
    }

    #[test]
    fn strip_markers_removes_assert() {
        let content = "SELECT * FROM t;\n<!--ASSERT\nrows >= 1\n-->";
        let result = strip_markers(content);
        assert!(!result.contains("ASSERT"));
        assert!(!result.contains("rows >= 1"));
        assert!(result.contains("SELECT * FROM t;"));
    }

    #[test]
    fn strip_markers_removes_expect() {
        let content = "SELECT 1;\n<!--EXPECT\n[{\"id\": 1}]\n-->";
        let result = strip_markers(content);
        assert!(!result.contains("EXPECT"));
        assert!(!result.contains("[{\"id\": 1}]"));
        assert!(result.contains("SELECT 1;"));
    }

    #[test]
    fn strip_markers_removes_all_three() {
        let content =
            "<!--SETUP\nsetup;\n-->\nquery;\n<!--ASSERT\nassert;\n-->\n<!--EXPECT\nexpect;\n-->";
        let result = strip_markers(content);
        assert!(!result.contains("SETUP"));
        assert!(!result.contains("ASSERT"));
        assert!(!result.contains("EXPECT"));
        assert!(!result.contains("setup;"));
        assert!(!result.contains("assert;"));
        assert!(!result.contains("expect;"));
        assert!(result.contains("query;"));
    }

    #[test]
    fn strip_markers_no_markers() {
        let content = "SELECT * FROM users;";
        let result = strip_markers(content);
        assert_eq!(result, "SELECT * FROM users;");
    }

    #[test]
    fn strip_markers_preserves_non_marker_comments() {
        let content = "-- This is a comment\nSELECT 1;";
        let result = strip_markers(content);
        assert!(result.contains("-- This is a comment"));
        assert!(result.contains("SELECT 1;"));
    }

    // ==================== strip_double_at_lines tests ====================

    #[test]
    fn strip_double_at_lines_removes_prefixed_lines() {
        let content = "line1\n@@hidden\nline2";
        let result = strip_double_at_lines(content);
        assert!(result.contains("line1"));
        assert!(!result.contains("hidden"));
        assert!(result.contains("line2"));
    }

    #[test]
    fn strip_double_at_lines_multiple_hidden() {
        let content = "@@first\nvisible\n@@second\n@@third\nlast";
        let result = strip_double_at_lines(content);
        assert!(!result.contains("first"));
        assert!(!result.contains("second"));
        assert!(!result.contains("third"));
        assert!(result.contains("visible"));
        assert!(result.contains("last"));
    }

    #[test]
    fn strip_double_at_lines_no_prefixed_lines() {
        let content = "line1\nline2\nline3";
        let result = strip_double_at_lines(content);
        assert_eq!(result, content);
    }

    #[test]
    fn strip_double_at_lines_all_hidden() {
        let content = "@@line1\n@@line2";
        let result = strip_double_at_lines(content);
        assert_eq!(result, "");
    }

    #[test]
    fn strip_double_at_lines_empty_at_line() {
        let content = "before\n@@\nafter";
        let result = strip_double_at_lines(content);
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("@@"));
    }

    #[test]
    fn strip_double_at_lines_at_in_middle_not_stripped() {
        // @@ must be at the START of the line
        let content = "line with @@ in middle";
        let result = strip_double_at_lines(content);
        assert_eq!(result, content);
    }

    // ==================== strip_marker_block tests ====================

    #[test]
    fn strip_marker_block_single_block() {
        let content = "before\n<!--SETUP\ncontent\n-->\nafter";
        let result = strip_marker_block(content, "<!--SETUP");
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("SETUP"));
        assert!(!result.contains("content"));
    }

    #[test]
    fn strip_marker_block_multiple_same_type() {
        let content = "<!--ASSERT\nfirst\n-->\nmiddle\n<!--ASSERT\nsecond\n-->";
        let result = strip_marker_block(content, "<!--ASSERT");
        assert!(!result.contains("first"));
        assert!(!result.contains("second"));
        assert!(result.contains("middle"));
    }

    #[test]
    fn strip_marker_block_unclosed_marker() {
        // Unclosed marker should stop stripping (no -->)
        let content = "before\n<!--SETUP\nno end marker";
        let result = strip_marker_block(content, "<!--SETUP");
        // Should return original content since marker is unclosed
        assert_eq!(result, content);
    }

    #[test]
    fn strip_marker_block_not_found() {
        let content = "just some content";
        let result = strip_marker_block(content, "<!--SETUP");
        assert_eq!(result, content);
    }

    // === Edge cases migrated from tests/transpiler_tests.rs ===

    #[test]
    fn strip_markers_double_at_at_end_no_newline() {
        // @@ at end without trailing newline
        let input = "SELECT 1;\n@@hidden";
        let result = strip_markers(input);
        assert_eq!(result, "SELECT 1;");
    }

    #[test]
    fn strip_markers_only_markers_returns_empty() {
        // Content with ONLY markers returns empty string
        let input = "<!--SETUP\nCREATE TABLE t;\n-->\n<!--ASSERT\nrows >= 1\n-->";
        let result = strip_markers(input);
        assert_eq!(result, "");
    }
}
