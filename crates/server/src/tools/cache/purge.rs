//! cache_purge tool implementation.
//!
//! Purges cache entries by age, domain, or count.

use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, Content},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thndrs_core::{CacheDb, Error};

/// Parameters for the cache_purge tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CachePurgeParams {
    /// Purge entries older than this many days.
    pub older_than_days: Option<i64>,

    /// Purge entries matching this domain pattern.
    pub domain: Option<String>,

    /// Keep only the newest N entries (LRU purge).
    pub max_entries: Option<usize>,
}

/// Output from the cache_purge tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CachePurgeOutput {
    /// Number of entries deleted.
    pub deleted: u64,
}

/// Implementation of the cache_purge tool.
pub async fn purge_impl(cache: &CacheDb, params: CachePurgeParams) -> Result<CallToolResult, McpError> {
    if params.older_than_days.is_none() && params.domain.is_none() && params.max_entries.is_none() {
        return Err(Error::InvalidInput(
            "At least one of older_than_days, domain, or max_entries must be specified".to_string(),
        )
        .into());
    }

    let mut deleted_total = 0u64;

    if let Some(_days) = params.older_than_days {
        let deleted = cache.purge_expired_snapshots().await?;
        deleted_total += deleted;
    }

    if let Some(domain) = params.domain {
        let deleted = cache.purge_snapshots_by_domain(&domain).await?;
        deleted_total += deleted;
    }

    if let Some(max_entries) = params.max_entries {
        let deleted = cache.purge_lru_snapshots(max_entries).await?;
        deleted_total += deleted;
    }

    let output = CachePurgeOutput { deleted: deleted_total };
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| Error::InvalidInput(format!("Failed to serialize output: {e}")))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use thndrs_core::{Snapshot, cache::hash::compute_cache_key};

    fn make_test_snapshot(url: &str) -> Snapshot {
        let hash = compute_cache_key(url, "", "readable");
        Snapshot {
            hash,
            url: url.to_string(),
            final_url: url.to_string(),
            mode: "readable".to_string(),
            content_type: Some("text/html".to_string()),
            status_code: Some(200),
            fetched_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            etag: None,
            last_modified: None,
            raw_bytes: None,
            raw_truncated: false,
            title: Some("Test".to_string()),
            markdown: Some("# Test".to_string()),
            text: Some("Test".to_string()),
            links_json: None,
            extractor_name: Some("lectito-core".to_string()),
            extractor_version: Some("0.1.0".to_string()),
            siteconfig_id: None,
            extract_cfg_json: None,
            headers_json: None,
            fetch_ms: Some(100),
            extract_ms: Some(50),
        }
    }

    #[tokio::test]
    async fn test_purge_by_domain() {
        let cache = CacheDb::open_in_memory().await.unwrap();
        cache
            .upsert_snapshot(&make_test_snapshot("https://example.com/page1"))
            .await
            .unwrap();
        cache
            .upsert_snapshot(&make_test_snapshot("https://other.com/page2"))
            .await
            .unwrap();

        let params =
            CachePurgeParams { older_than_days: None, domain: Some("example.com".to_string()), max_entries: None };

        let result = purge_impl(&cache, params).await.unwrap();
        let content_val = serde_json::to_value(&result.content[0]).unwrap();
        let text = content_val
            .get("text")
            .and_then(|v| v.as_str())
            .expect("Expected text field in content");
        let output: CachePurgeOutput = serde_json::from_str(text).unwrap();
        assert_eq!(output.deleted, 1);
    }

    #[tokio::test]
    async fn test_purge_lru() {
        let cache = CacheDb::open_in_memory().await.unwrap();
        cache
            .upsert_snapshot(&make_test_snapshot("https://example.com/page1"))
            .await
            .unwrap();
        cache
            .upsert_snapshot(&make_test_snapshot("https://example.com/page2"))
            .await
            .unwrap();

        let params = CachePurgeParams { older_than_days: None, domain: None, max_entries: Some(1) };

        let result = purge_impl(&cache, params).await.unwrap();
        let content_val = serde_json::to_value(&result.content[0]).unwrap();
        let text = content_val
            .get("text")
            .and_then(|v| v.as_str())
            .expect("Expected text field in content");
        let output: CachePurgeOutput = serde_json::from_str(text).unwrap();
        assert_eq!(output.deleted, 1);
    }

    #[tokio::test]
    async fn test_purge_no_params() {
        let cache = CacheDb::open_in_memory().await.unwrap();
        let params = CachePurgeParams { older_than_days: None, domain: None, max_entries: None };

        let result = purge_impl(&cache, params).await;
        assert!(result.is_err());
    }
}
