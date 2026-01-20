//! Readable content extraction using Lectito.
//!
//! Provides a stable extraction abstraction that can be swapped later.
//!
//! ### Primary Algorithm
//! - Uses Lectito's extraction pipeline (Readability.js-inspired).
//! - Preprocessing, scoring, best-candidate selection, and cleanup.
//!
//! ### Stable Abstraction
//! - Uses the `Extractor` trait for loose coupling between tools and the extraction engine.
//!
//! ### Output Normalization
//! - Enforces consistent Markdown headers: `title`, `source`, `fetched_at`, `extractor`, `siteconfig`.
//! - Ensures reproducibility by storing siteconfig IDs and extractor versions.

pub mod links;
pub mod normalize;

pub use links::{Link, extract_links};
pub use normalize::{ExtractedDoc, normalize_markdown};

use lectito_core::{Document, ExtractConfig as LectitoConfig};
use thndrs_core::Error;
use url::Url;

/// Configuration for content extraction.
#[derive(Debug, Clone)]
pub struct ExtractConfig {
    /// Minimum character count for content (default: 200)
    pub char_threshold: Option<usize>,

    /// Maximum number of top candidates to consider (default: 5)
    pub max_top_candidates: Option<usize>,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self { char_threshold: Some(200), max_top_candidates: Some(5) }
    }
}

impl ExtractConfig {
    /// Convert to Lectito's config type.
    fn to_lectito_config(&self) -> LectitoConfig {
        let mut cfg = LectitoConfig::default();
        if let Some(threshold) = self.char_threshold {
            cfg.char_threshold = threshold;
        }
        if let Some(max) = self.max_top_candidates {
            cfg.max_top_candidates = max;
        }
        cfg
    }
}

/// Result of content extraction.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Page title
    pub title: Option<String>,
    /// Extracted markdown content
    pub markdown: String,
    /// Extracted links
    pub links: Vec<Link>,
    /// Extractor version string
    pub extractor_version: String,
}

/// Stable extractor trait for content extraction.
///
/// This allows swapping the extraction engine later without changing tool code.
pub trait Extractor: Send + Sync {
    /// Extract readable content from HTML.
    fn extract(&self, html: &str, base_url: &Url, config: &ExtractConfig) -> Result<ExtractionResult, Error>;
}

/// Lectito-based extractor implementation.
pub struct LectitoExtractor {
    version: &'static str,
}

impl LectitoExtractor {
    /// Create a new Lectito extractor.
    pub fn new() -> Self {
        Self { version: "lectito-core@0.2.0" }
    }
}

impl Default for LectitoExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for LectitoExtractor {
    fn extract(&self, html: &str, base_url: &Url, config: &ExtractConfig) -> Result<ExtractionResult, Error> {
        let doc = Document::parse(html).map_err(|e| Error::ExtractFailed(format!("failed to parse HTML: {}", e)))?;

        let lectito_cfg = config.to_lectito_config();
        let extracted = lectito_core::extract_content(&doc, &lectito_cfg)
            .map_err(|e| Error::ExtractFailed(format!("extraction failed: {}", e)))?;

        let metadata = doc.extract_metadata();
        let title = metadata.title.clone();

        let markdown = lectito_core::convert_to_markdown(&extracted.content, &metadata, &Default::default())
            .map_err(|e| Error::ExtractFailed(format!("markdown conversion failed: {}", e)))?;

        let links = extract_links(html, base_url);

        Ok(ExtractionResult { title, markdown, links, extractor_version: self.version.to_string() })
    }
}

/// Extract readable content from HTML using the default extractor.
///
/// This is a convenience function that uses the Lectito extractor.
pub fn extract_readable(html: &str, base_url: &Url) -> Result<ExtractionResult, Error> {
    let extractor = LectitoExtractor::new();
    extractor.extract(html, base_url, &ExtractConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Test Article</title>
        </head>
        <body>
            <article>
                <h1>Main Heading</h1>
                <p>This is a test paragraph with some content.</p>
                <p>Another paragraph to make the content long enough.</p>
                <a href="https://example.com">Example Link</a>
            </article>
        </body>
        </html>
    "#;

    #[test]
    fn test_extract_config_default() {
        let config = ExtractConfig::default();
        assert_eq!(config.char_threshold, Some(200));
        assert_eq!(config.max_top_candidates, Some(5));
    }

    #[test]
    fn test_lectito_extractor_new() {
        let extractor = LectitoExtractor::new();
        assert_eq!(extractor.version, "lectito-core@0.2.0");
    }

    #[test]
    fn test_extract_readable_basic() {
        let base = Url::parse("https://example.com").unwrap();
        let result = extract_readable(SIMPLE_HTML, &base);

        assert!(result.is_ok());
        let extracted = result.unwrap();

        assert_eq!(extracted.title, Some("Test Article".to_string()));
        assert!(!extracted.markdown.is_empty());
        assert_eq!(extracted.links.len(), 1);
        assert_eq!(extracted.links[0].href, "https://example.com/");
        assert_eq!(extracted.extractor_version, "lectito-core@0.2.0");
    }

    #[test]
    fn test_extract_readable_with_relative_link() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body>
                <article>
                    <h1>Main Content</h1>
                    <p>This is a substantial paragraph with plenty of content to ensure we meet
                    the character threshold for extraction. We need multiple paragraphs with
                    meaningful content to pass the extraction algorithm's requirements.</p>
                    <p>Here is another paragraph with even more content to ensure that the
                    extraction will succeed. This paragraph adds more text and increases the
                    overall character count significantly.</p>
                    <p>A third paragraph providing additional content that helps ensure the
                    document is substantial enough for successful extraction. The readability
                    algorithm requires a minimum amount of content to identify the main article.</p>
                    <a href="/about">About Page</a>
                </article>
            </body>
            </html>
        "#;

        let base = Url::parse("https://example.com/path/").unwrap();
        let result = extract_readable(html, &base);

        assert!(result.is_ok());
        let extracted = result.unwrap();
        assert_eq!(extracted.links.len(), 1);
        assert_eq!(extracted.links[0].href, "https://example.com/about");
    }

    #[test]
    fn test_extract_custom_config() {
        let config = ExtractConfig { char_threshold: Some(100), max_top_candidates: Some(3) };
        let lectito_cfg = config.to_lectito_config();
        assert_eq!(lectito_cfg.char_threshold, 100);
        assert_eq!(lectito_cfg.max_top_candidates, 3);
    }

    #[test]
    fn test_extract_empty_html() {
        let base = Url::parse("https://example.com").unwrap();
        let result = extract_readable("", &base);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_invalid_html() {
        let base = Url::parse("https://example.com").unwrap();
        let result = extract_readable("not really html", &base);
        assert!(result.is_err());
    }
}
