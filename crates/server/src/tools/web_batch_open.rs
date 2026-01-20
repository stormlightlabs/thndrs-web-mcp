//! web_batch_open tool implementation.
//!
//! Fetches and extracts multiple URLs in parallel with bounded concurrency.

use rmcp::{ErrorData as McpError, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thndrs_core::{AppConfig, CacheDb, Error};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::tools::web_open::{ExtractTuning, WebOpenOutput, WebOpenParams, open_impl};

/// Input parameters for web_batch_open tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct WebBatchOpenParams {
    /// URLs to fetch and extract.
    pub urls: Vec<String>,

    /// Extraction mode: "readable" (default) or "raw".
    /// "rendered" mode is not yet implemented.
    #[serde(default = "default_mode")]
    pub mode: Option<String>,

    /// Maximum response body size in bytes (default: 5MB).
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,

    /// Force a refresh, bypassing the cache.
    #[serde(default = "default_false")]
    pub force_refresh: bool,

    /// Request timeout in milliseconds (default: 20000).
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Maximum number of concurrent requests (default: 4, max: 16).
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: Option<u8>,

    /// Fail fast: stop on first error (default: false).
    #[serde(default = "default_false")]
    pub fail_fast: bool,

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

fn default_mode() -> Option<String> {
    Some("readable".to_string())
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

fn default_max_concurrency() -> Option<u8> {
    Some(4)
}

/// Batch item status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum BatchItemStatus {
    /// Successfully fetched and extracted.
    Success,
    /// Returned from cache.
    Cached,
    /// Failed to fetch or extract.
    Failed,
}

/// Individual batch result item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchItem {
    /// The original URL.
    pub url: String,
    /// Status of this item.
    pub status: BatchItemStatus,
    /// The successful result (if status is Success or Cached).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<WebOpenOutput>,
    /// Error message (if status is Failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSummary {
    /// Total number of URLs processed.
    pub total: u32,
    /// Number of successful extractions.
    pub succeeded: u32,
    /// Number of cached results returned.
    pub cached: u32,
    /// Number of failed extractions.
    pub failed: u32,
}

/// Output structure for web_batch_open tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebBatchOpenOutput {
    /// Individual results for each URL (in input order).
    pub results: Vec<BatchItem>,
    /// Summary statistics.
    pub summary: BatchSummary,
}

/// Implementation of the web_batch_open tool.
pub async fn batch_open_impl(
    db: &CacheDb, config: &AppConfig, params: WebBatchOpenParams,
) -> Result<CallToolResult, McpError> {
    if params.urls.is_empty() {
        return Err(Error::InvalidInput("urls cannot be empty".into()).into());
    }

    let max_concurrency = params.max_concurrency.unwrap_or(4).min(16) as usize;
    if max_concurrency == 0 {
        return Err(Error::InvalidInput("max_concurrency must be at least 1".into()).into());
    }

    let semaphore = Arc::new(Semaphore::new(max_concurrency));
    let mode = params.mode.clone().unwrap_or_else(|| "readable".to_string());

    let mut join_set = JoinSet::new();

    for url in params.urls.clone() {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let db = db.clone();
        let config = config.clone();

        let open_params = WebOpenParams {
            url: url.clone(),
            mode: mode.clone(),
            max_bytes: params.max_bytes,
            force_refresh: params.force_refresh,
            timeout_ms: params.timeout_ms,
            accept: params.accept.clone(),
            extract: params.extract.clone(),
            debug: params.debug,
        };

        join_set.spawn(async move {
            // NOTE: Hold permit for task duration to enforce concurrency limit
            let _permit = permit;
            let result = open_impl(&db, &config, open_params).await;
            (url, result)
        });
    }

    let mut results: Vec<BatchItem> = Vec::new();
    let mut succeeded = 0u32;
    let cached = 0u32;
    let mut failed = 0u32;

    while let Some(result) = join_set.join_next().await {
        let (url, task_result) = result.map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let item = match task_result {
            Ok(tool_result) => {
                let output_json = tool_result
                    .content
                    .first()
                    .map(|c| match c.as_text() {
                        Some(content) => content.text.clone(),
                        None => "{}".to_string(),
                    })
                    .unwrap();
                if let Ok(output) = serde_json::from_str::<WebOpenOutput>(&output_json) {
                    let status = BatchItemStatus::Success;
                    succeeded += 1;

                    BatchItem { url, status, result: Some(output), error: None }
                } else {
                    failed += 1;
                    BatchItem {
                        url,
                        status: BatchItemStatus::Failed,
                        result: None,
                        error: Some("Failed to parse output".to_string()),
                    }
                }
            }
            Err(e) => {
                failed += 1;
                BatchItem { url, status: BatchItemStatus::Failed, result: None, error: Some(e.message.to_string()) }
            }
        };

        results.push(item);

        if params.fail_fast && failed > 0 {
            join_set.shutdown().await;
            break;
        }
    }

    let output = WebBatchOpenOutput {
        summary: BatchSummary { total: results.len() as u32, succeeded, cached, failed },
        results,
    };

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&output).unwrap_or_default(),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_open_empty_urls() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default();
        let params = WebBatchOpenParams { urls: vec![], ..Default::default() };

        let result = batch_open_impl(&db, &config, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_open_invalid_concurrency() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default();
        let params = WebBatchOpenParams {
            urls: vec!["https://example.com".to_string()],
            max_concurrency: Some(0),
            ..Default::default()
        };

        let result = batch_open_impl(&db, &config, params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_default_max_concurrency() {
        assert_eq!(default_max_concurrency(), Some(4));
    }

    #[test]
    fn test_batch_item_status_serialization() {
        let status = BatchItemStatus::Success;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Success"));
    }
}
