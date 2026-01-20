//! Markdown normalization with YAML frontmatter.
//!
//! Enforces a consistent header format for cached documents.

use chrono::{DateTime, Utc};
use url::Url;

/// Extracted document with metadata.
#[derive(Debug, Clone)]
pub struct ExtractedDoc {
    /// Page title
    pub title: Option<String>,
    /// Markdown content
    pub markdown: String,
    /// Extractor version (e.g., "lectito-core@0.x")
    pub extractor_version: String,
}

/// Normalize extracted content with YAML frontmatter header.
///
/// Frontmatter format:
/// ```yaml
/// ---
/// title: <page title>
/// source: <final_url>
/// fetched_at: <ISO8601 timestamp>
/// extractor: lectito-core@<version>
/// siteconfig: <id or "none">
/// ---
/// <markdown body>
/// ```
pub fn normalize_markdown(
    doc: &ExtractedDoc, source_url: &Url, fetched_at: &DateTime<Utc>, siteconfig_id: Option<&str>,
) -> String {
    let title = doc.title.as_deref().unwrap_or("Untitled");
    let siteconfig = siteconfig_id.unwrap_or("none");

    format!(
        "---\ntitle: {title}\nsource: {source}\nfetched_at: {timestamp}\nextractor: {extractor}\nsiteconfig: {siteconfig}\n---\n{markdown}",
        title = escape_yaml(title),
        source = source_url.as_str(),
        timestamp = fetched_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        extractor = doc.extractor_version,
        markdown = doc.markdown.trim()
    )
}

/// Escape special YAML characters in a string.
fn escape_yaml(s: &str) -> String {
    if s.contains('\n') || s.contains(':') && s.len() > 1 {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else if s.is_empty() {
        "\"\"".to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_markdown_basic() {
        let doc = ExtractedDoc {
            title: Some("Test Title".to_string()),
            markdown: "# Heading\n\nContent".to_string(),
            extractor_version: "lectito-core@0.1.0".to_string(),
        };

        let url = Url::parse("https://example.com").unwrap();
        let fetched_at = DateTime::parse_from_rfc3339("2025-01-20T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let result = normalize_markdown(&doc, &url, &fetched_at, None);

        assert!(result.contains("---"));
        assert!(result.contains("title: Test Title"));
        assert!(result.contains("source: https://example.com/"));
        assert!(result.contains("fetched_at: 2025-01-20T00:00:00Z"));
        assert!(result.contains("extractor: lectito-core@0.1.0"));
        assert!(result.contains("siteconfig: none"));
        assert!(result.contains("# Heading"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn test_normalize_markdown_no_title() {
        let doc = ExtractedDoc {
            title: None,
            markdown: "Content".to_string(),
            extractor_version: "lectito-core@0.1.0".to_string(),
        };

        let url = Url::parse("https://example.com").unwrap();
        let fetched_at = Utc::now();
        let result = normalize_markdown(&doc, &url, &fetched_at, None);
        assert!(result.contains("title: Untitled"));
    }

    #[test]
    fn test_normalize_markdown_with_siteconfig() {
        let doc = ExtractedDoc {
            title: Some("Test".to_string()),
            markdown: "Content".to_string(),
            extractor_version: "lectito-core@0.1.0".to_string(),
        };

        let url = Url::parse("https://example.com").unwrap();
        let fetched_at = Utc::now();
        let result = normalize_markdown(&doc, &url, &fetched_at, Some("custom-config"));
        assert!(result.contains("siteconfig: custom-config"));
    }

    #[test]
    fn test_normalize_markdown_trims_content() {
        let doc = ExtractedDoc {
            title: Some("Test".to_string()),
            markdown: "  \n  Content  \n  ".to_string(),
            extractor_version: "lectito-core@0.1.0".to_string(),
        };

        let url = Url::parse("https://example.com").unwrap();
        let fetched_at = Utc::now();
        let result = normalize_markdown(&doc, &url, &fetched_at, None);
        assert!(result.ends_with("Content"));
    }

    #[test]
    fn test_escape_yaml_simple() {
        let escaped = escape_yaml("simple text");
        assert_eq!(escaped, "simple text");
    }

    #[test]
    fn test_escape_yaml_empty() {
        let escaped = escape_yaml("");
        assert_eq!(escaped, "\"\"");
    }

    #[test]
    fn test_escape_yaml_multiline() {
        let escaped = escape_yaml("line1\nline2");
        assert_eq!(escaped, "\"line1\nline2\"");
    }

    #[test]
    fn test_escape_yaml_with_colon() {
        let escaped = escape_yaml("Title: Subtitle");
        assert_eq!(escaped, "\"Title: Subtitle\"");
    }

    #[test]
    fn test_escape_yaml_single_colon() {
        let escaped = escape_yaml("a:b");
        assert!(escaped.contains("a:b"));
    }
}
