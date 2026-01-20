//! web_open tool implementation.
//!
//! Fetches a URL and extracts readable content using the full fetch pipeline.

use chrono::Utc;
use rmcp::{ErrorData as McpError, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use thndrs_client::{ExtractConfig, Extractor, FetchClient, FetchConfig, LectitoExtractor, normalize_markdown};
use thndrs_core::{AppConfig, CacheDb, Error, Snapshot, cache::hash::compute_cache_key};

/// Input parameters for web_open tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebOpenParams {
    /// The URL to fetch.
    pub url: String,

    /// Extraction mode: "readable" (default) or "raw".
    /// "rendered" mode is not yet implemented.
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Maximum response body size in bytes (default: 5MB).
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,

    /// Force a refresh, bypassing the cache.
    #[serde(default = "default_false")]
    pub force_refresh: bool,

    /// Request timeout in milliseconds (default: 20000).
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Optional Accept header override.
    #[serde(default)]
    pub accept: Option<String>,

    /// Optional extraction tuning parameters.
    #[serde(default)]
    pub extract: Option<ExtractTuning>,

    /// Enable extraction diagnostics output for debugging.
    #[serde(default)]
    pub debug: bool,
}

fn default_mode() -> String {
    "readable".into()
}

fn default_max_bytes() -> usize {
    5 * 1024 * 1024
}

fn default_false() -> bool {
    false
}

fn default_timeout_ms() -> u64 {
    20000
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExtractTuning {
    /// Minimum character threshold for content blocks.
    pub char_threshold: Option<usize>,
    /// Maximum number of top candidates to consider.
    pub max_top_candidates: Option<usize>,
}

/// Extraction diagnostics for debugging and tuning.
///
/// Note: Full diagnostics (candidates_considered, winning_candidate selector,
/// siblings_included) require lectito_core to expose internal extraction state.
/// This basic implementation captures timing and character count information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractionDiagnostics {
    /// Character count of extracted markdown content.
    pub char_count: usize,
    /// Number of links extracted from the content.
    pub links_count: usize,
    /// Extraction time in milliseconds.
    pub extraction_time_ms: u64,
}

/// Output structure for web_open tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebOpenOutput {
    /// The original URL requested.
    pub url: String,
    /// The final URL after redirects.
    pub final_url: String,
    /// Content-Type header.
    pub content_type: Option<String>,
    /// ISO8601 timestamp of when the content was fetched.
    pub fetched_at: String,
    /// The mode used for extraction.
    pub mode: String,
    /// Raw HTML content (only if mode=raw).
    pub raw: Option<String>,
    /// Extracted Markdown content (if mode=readable).
    pub markdown: Option<String>,
    /// Extracted page title.
    pub title: Option<String>,
    /// Harvested links from the content.
    pub links: Vec<ExtractedLink>,
    /// Content hash for cache lookup.
    pub hash: String,
    /// Extraction diagnostics (only if debug=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<ExtractionDiagnostics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedLink {
    pub text: String,
    pub href: String,
}

/// Implementation of the web_open tool.
pub async fn open_impl(db: &CacheDb, config: &AppConfig, params: WebOpenParams) -> Result<CallToolResult, McpError> {
    if params.url.is_empty() {
        return Err(Error::InvalidInput("url cannot be empty".into()).into());
    }

    if params.mode != "readable" && params.mode != "raw" {
        return Err(Error::InvalidInput(format!("unsupported mode: {}", params.mode)).into());
    }

    if params.mode == "rendered" {
        return Err(Error::RenderDisabled.into());
    }

    let vary_headers = params.accept.as_deref().unwrap_or("");
    let hash = compute_cache_key(&params.url, vary_headers, &params.mode);

    if !params.force_refresh
        && let Ok(Some(snapshot)) = db.get_snapshot(&hash).await
    {
        tracing::debug!("cache hit for {}", params.url);

        let output = WebOpenOutput {
            url: snapshot.url,
            final_url: snapshot.final_url,
            content_type: snapshot.content_type,
            fetched_at: snapshot.fetched_at,
            mode: snapshot.mode,
            raw: snapshot.raw_bytes.map(|b| String::from_utf8_lossy(&b).to_string()),
            markdown: snapshot.markdown,
            title: snapshot.title,
            links: snapshot
                .links_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default(),
            hash,
            debug: None,
        };

        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]));
    }

    let fetch_config = FetchConfig {
        max_bytes: params.max_bytes,
        timeout: std::time::Duration::from_millis(params.timeout_ms),
        user_agent: config.user_agent.clone(),
        respect_robots: config.respect_robots,
        ..Default::default()
    };

    let fetch_client = FetchClient::new(fetch_config)?;
    let response = fetch_client.fetch(&params.url).await?;
    let fetched_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let (title, markdown, raw, links, debug_info) = match params.mode.as_str() {
        "raw" => {
            let html = String::from_utf8_lossy(&response.bytes).to_string();
            (None, None, Some(html), Vec::new(), None)
        }
        "readable" => {
            let html = String::from_utf8_lossy(&response.bytes).to_string();

            let extract_config = params
                .extract
                .as_ref()
                .map(|t| ExtractConfig { char_threshold: t.char_threshold, max_top_candidates: t.max_top_candidates })
                .unwrap_or_default();

            let extract_start = Instant::now();

            let extractor = thndrs_client::LectitoExtractor::new();
            let result = extractor.extract(&html, &response.final_url, &extract_config)?;
            let extraction_time_ms = extract_start.elapsed().as_millis() as u64;

            let doc = thndrs_client::ExtractedDoc {
                title: result.title.clone(),
                markdown: result.markdown.clone(),
                extractor_version: result.extractor_version,
            };

            let normalized = normalize_markdown(&doc, &response.final_url, &Utc::now(), None);

            let links: Vec<ExtractedLink> = result
                .links
                .into_iter()
                .map(|l| ExtractedLink { text: l.text, href: l.href })
                .collect();

            let debug_info = params.debug.then_some(ExtractionDiagnostics {
                char_count: normalized.len(),
                links_count: links.len(),
                extraction_time_ms,
            });

            (result.title, Some(normalized), None, links, debug_info)
        }
        _ => return Err(Error::InvalidInput(format!("unsupported mode: {}", params.mode)).into()),
    };

    let snapshot = Snapshot {
        hash: hash.clone(),
        url: response.url.to_string(),
        final_url: response.final_url.to_string(),
        mode: params.mode.clone(),
        content_type: response.content_type.clone(),
        status_code: Some(response.status.as_u16() as i32),
        fetched_at: fetched_at.clone(),
        expires_at: None,
        etag: response
            .headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
        last_modified: response
            .headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
        raw_bytes: raw.clone().map(|s| s.into_bytes()),
        raw_truncated: response.bytes.len() >= params.max_bytes,
        title: title.clone(),
        markdown: markdown.clone(),
        text: None,
        links_json: Some(serde_json::to_string(&links).unwrap_or_default()),
        extractor_name: Some("lectito-core".to_string()),
        extractor_version: Some("0.2.0".to_string()),
        siteconfig_id: None,
        extract_cfg_json: None,
        headers_json: None,
        fetch_ms: Some(response.fetch_ms as i64),
        extract_ms: debug_info.as_ref().map(|d| d.extraction_time_ms as i64),
    };

    db.upsert_snapshot(&snapshot).await?;

    let output = WebOpenOutput {
        url: response.url.to_string(),
        final_url: response.final_url.to_string(),
        content_type: response.content_type,
        fetched_at,
        mode: params.mode,
        raw,
        markdown,
        title,
        links,
        hash,
        debug: debug_info,
    };

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&output).unwrap_or_default(),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_empty_url() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default();
        let params = WebOpenParams {
            url: "".into(),
            mode: "readable".into(),
            max_bytes: 5 * 1024 * 1024,
            force_refresh: false,
            timeout_ms: 20000,
            accept: None,
            extract: None,
            debug: false,
        };

        let result = open_impl(&db, &config, params).await;
        assert!(result.is_err());
    }
}
