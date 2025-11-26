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

    let language = parts.first().map_or(String::new(), |s| (*s).to_string());

    let validator = parts
        .iter()
        .find_map(|part| part.strip_prefix("validator=").map(ToString::to_string))
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

/// Extracts markers from code block content.
///
/// Parses `<!--SETUP-->`, `<!--ASSERT-->`, and `<!--EXPECT-->` blocks,
/// returning their content and the remaining visible content.
#[must_use]
pub fn extract_markers(content: &str) -> ExtractedMarkers {
    let mut result = ExtractedMarkers::default();
    let mut remaining = content.to_string();

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
    result.visible_content = remaining.trim().to_string();

    result
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

    Some((before.to_string(), inner.to_string(), after.to_string()))
}

/// A code block extracted from markdown
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// The language (e.g., "sql", "json")
    pub language: String,
    /// The validator name from `validator=` attribute
    pub validator: Option<String>,
    /// Whether to skip validation
    pub skip: bool,
    /// The visible content (without markers)
    pub content: String,
    /// Setup content from <!--SETUP--> marker
    pub setup: Option<String>,
    /// Assertions from <!--ASSERT--> marker
    pub assertions: Option<String>,
    /// Expected output from <!--EXPECT--> marker
    pub expect: Option<String>,
}
