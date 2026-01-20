//! cache_get tool implementation.
//!
//! Retrieves a cached snapshot by hash.

use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, Content},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thndrs_core::{CacheDb, Error, Snapshot};

/// Parameters for the cache_get tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CacheGetParams {
    /// The hash of the cached snapshot to retrieve.
    pub hash: String,
}

/// Output from the cache_get tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CacheGetOutput {
    /// The cached snapshot.
    pub snapshot: Snapshot,
}

/// Implementation of the cache_get tool.
pub async fn get_impl(cache: &CacheDb, params: CacheGetParams) -> Result<CallToolResult, McpError> {
    let snapshot = cache
        .get_snapshot(&params.hash)
        .await?
        .ok_or_else(|| Error::CacheMiss(params.hash.clone()))?;

    let output = CacheGetOutput { snapshot };
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| Error::InvalidInput(format!("Failed to serialize snapshot: {e}")))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use thndrs_core::cache::hash::compute_cache_key;

    #[tokio::test]
    async fn test_get_impl_missing() {
        let cache = CacheDb::open_in_memory().await.unwrap();
        let params = CacheGetParams { hash: "nonexistent".to_string() };

        let result = get_impl(&cache, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_impl_found() {
        let cache = CacheDb::open_in_memory().await.unwrap();

        let hash = compute_cache_key("https://example.com", "", "readable");
        let snapshot = Snapshot {
            hash: hash.clone(),
            url: "https://example.com".to_string(),
            final_url: "https://example.com".to_string(),
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
        };

        cache.upsert_snapshot(&snapshot).await.unwrap();

        let params = CacheGetParams { hash };
        let result = get_impl(&cache, params).await;
        assert!(result.is_ok());
    }
}
