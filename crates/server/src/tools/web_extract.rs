//! web_extract tool implementation.
//!
//! This tool extracts readable content from HTML using Lectito.
//! No network I/O is performed - HTML is provided by the client.

use lectito_core::{Readability, ReadabilityConfig, parse, parse_with_url};
use rmcp::{ErrorData as McpError, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thndrs_core::Error;

/// Input parameters for web_extract tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebExtractParams {
    /// The raw HTML content to extract from.
    pub html: String,

    /// Base URL for resolving relative links (optional).
    /// If not provided, relative links will be preserved as-is.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Extraction strategy to use.
    /// - "readability": Main content extraction (default)
    /// - "plain_text": Simple text extraction, no structure
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Whether to output as Markdown (true) or plain text (false).
    #[serde(default = "default_true")]
    pub to_markdown: bool,

    /// Optional extraction tuning parameters.
    #[serde(default)]
    pub config: Option<ExtractTuning>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExtractTuning {
    /// Minimum character threshold for content blocks.
    pub char_threshold: Option<usize>,
    /// Maximum number of top candidates to consider.
    pub max_top_candidates: Option<usize>,
    /// Minimum score threshold for extraction.
    pub min_score: Option<f64>,
}

fn default_strategy() -> String {
    "readability".into()
}

fn default_true() -> bool {
    true
}

/// Output structure for web_extract tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebExtractOutput {
    /// Extracted page title (if found).
    pub title: Option<String>,
    /// Extracted content as Markdown (if to_markdown=true).
    pub markdown: Option<String>,
    /// Extracted content as plain text (if to_markdown=false).
    pub text: Option<String>,
    /// Harvested links from the content.
    pub links: Vec<ExtractedLink>,
    /// The extraction strategy that was used.
    pub strategy_used: String,
    /// Word count of extracted content.
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedLink {
    pub text: String,
    pub href: String,
}

/// Implementation of the web_extract tool.
pub async fn extract_impl(params: WebExtractParams) -> Result<CallToolResult, McpError> {
    if params.html.is_empty() {
        return Err(Error::InvalidInput("html cannot be empty".into()).into());
    }

    let article = if let Some(ref tuning) = params.config {
        let mut config_builder = ReadabilityConfig::builder();
        if let Some(threshold) = tuning.char_threshold {
            config_builder = config_builder.char_threshold(threshold);
        }
        if let Some(max_candidates) = tuning.max_top_candidates {
            config_builder = config_builder.nb_top_candidates(max_candidates);
        }
        if let Some(min_score) = tuning.min_score {
            config_builder = config_builder.min_score(min_score);
        }
        let config = config_builder.build();
        let reader = Readability::with_config(config);

        if let Some(ref base_url) = params.base_url {
            reader
                .parse_with_url(&params.html, base_url)
                .map_err(|e| Error::ExtractFailed(format!("Failed to parse HTML: {}", e)))?
        } else {
            reader
                .parse(&params.html)
                .map_err(|e| Error::ExtractFailed(format!("Failed to parse HTML: {}", e)))?
        }
    } else if let Some(ref base_url) = params.base_url {
        parse_with_url(&params.html, base_url)
            .map_err(|e| Error::ExtractFailed(format!("Failed to parse HTML: {}", e)))?
    } else {
        parse(&params.html).map_err(|e| Error::ExtractFailed(format!("Failed to parse HTML: {}", e)))?
    };

    let links = extract_links_from_html(&article.content, params.base_url.as_deref());

    let (markdown, text) = if params.to_markdown {
        let md = article
            .to_markdown()
            .map_err(|e| Error::ExtractFailed(format!("Markdown conversion failed: {}", e)))?;
        (Some(md), None)
    } else {
        (None, Some(article.to_text()))
    };

    let output = WebExtractOutput {
        title: article.metadata.title,
        markdown,
        text,
        links,
        strategy_used: params.strategy.clone(),
        word_count: article.word_count,
    };

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&output).unwrap_or_default(),
    )]))
}

/// Extract links from HTML content.
fn extract_links_from_html(html: &str, base_url: Option<&str>) -> Vec<ExtractedLink> {
    let mut links = Vec::new();

    if let Ok(doc) = lectito_core::Document::parse(html)
        && let Ok(elements) = doc.select("a")
    {
        for element in elements {
            if let Some(href) = element.attr("href") {
                let resolved_href = resolve_url(href, base_url);
                let text = element.text();
                let trimmed_text = text.trim();
                if !trimmed_text.is_empty() && !resolved_href.is_empty() {
                    links.push(ExtractedLink { text: trimmed_text.to_string(), href: resolved_href });
                }
            }
        }
    }

    links
}

/// Resolve a URL relative to a base URL.
fn resolve_url(href: &str, base_url: Option<&str>) -> String {
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//") {
        return href.to_string();
    }
    if let Some(base) = base_url {
        if href.starts_with('/') {
            let parts: Vec<&str> = base.split('/').collect();
            if parts.len() >= 3 {
                let origin = format!("{}/{}/{}", parts[0], parts[1], parts[2]);
                return format!("{}{}", origin, href);
            }
        } else {
            let base_dir = base.rsplit_once('/').map(|(b, _)| b).unwrap_or(base);
            return format!("{}/{}", base_dir, href);
        }
    }
    href.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Article</title></head>
        <body>
            <article>
                <h1>Main Title</h1>
                <p>This is the article content with enough text to pass thresholds.
                   We need sufficient content here to ensure extraction works properly.
                   This is a substantial paragraph with meaningful content that continues
                   to provide more text and increase the overall character count.
                   The readability algorithm requires enough content to properly identify
                   the main article and distinguish it from sidebars and navigation.</p>
                <p>This is a second paragraph with additional content to further improve
                   the extraction score. It contains more meaningful text with commas,
                   periods, and proper sentence structure. The goal is to ensure that
                   the content is clearly identifiable as the main article content.</p>
                <p>A third paragraph that adds even more substantial content to the
                   article. This ensures that the readability algorithm can properly
                   detect and extract the content with a high confidence score. The
                   more quality content we provide, the better the extraction works.</p>
                <a href="/about">About Page</a>
                <a href="https://example.com">External Link</a>
            </article>
        </body>
        </html>
    "#;

    #[tokio::test]
    async fn test_extract_simple_article() {
        let params = WebExtractParams {
            html: TEST_HTML.into(),
            base_url: Some("https://test.com".into()),
            strategy: "readability".into(),
            to_markdown: true,
            config: Some(ExtractTuning { char_threshold: None, max_top_candidates: None, min_score: Some(15.0) }),
        };

        let result = extract_impl(params).await;
        assert!(result.is_ok(), "extraction should succeed");

        let call_result = result.unwrap();
        assert!(!call_result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn test_extract_empty_html_fails() {
        let params = WebExtractParams {
            html: "".into(),
            base_url: None,
            strategy: "readability".into(),
            to_markdown: true,
            config: None,
        };

        let result = extract_impl(params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_url_absolute() {
        let resolved = resolve_url("https://other.com/page", Some("https://example.com"));
        assert_eq!(resolved, "https://other.com/page");
    }

    #[test]
    fn test_resolve_url_absolute_path() {
        let resolved = resolve_url("/path/to/page", Some("https://example.com/dir/file.html"));
        assert_eq!(resolved, "https://example.com/path/to/page");
    }

    #[test]
    fn test_resolve_url_relative_path() {
        let resolved = resolve_url("other.html", Some("https://example.com/dir/file.html"));
        assert_eq!(resolved, "https://example.com/dir/other.html");
    }

    #[test]
    fn test_resolve_url_no_base() {
        let resolved = resolve_url("https://example.com/page", None);
        assert_eq!(resolved, "https://example.com/page");
    }
}
