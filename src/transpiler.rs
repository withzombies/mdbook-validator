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
    let mut result = content.to_string();

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
    let mut result = content.to_string();

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
