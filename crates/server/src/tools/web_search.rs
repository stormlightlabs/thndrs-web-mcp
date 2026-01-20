//! web_search tool implementation.
//!
//! Performs web searches using the Brave Search API with caching.

use rmcp::{ErrorData as McpError, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thndrs_client::{BraveClient, BraveConfig, SafeSearch, SearchRequest};
use thndrs_core::{AppConfig, CacheDb, Error};

/// Input parameters for web_search tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchParams {
    /// Search query (required).
    pub query: String,

    /// Number of results (1-20, default 20).
    #[serde(default = "default_count")]
    pub count: Option<u8>,

    /// Page offset (0-9, default 0).
    #[serde(default)]
    pub offset: Option<u8>,

    /// Freshness filter: pd (past day), pw (past week), pm (past month), py (past year).
    #[serde(default)]
    pub freshness: Option<String>,

    /// Safe search: off, moderate (default), strict.
    #[serde(default)]
    pub safesearch: Option<String>,

    /// Country code (ISO 3166-1 alpha-2, e.g., "US").
    #[serde(default)]
    pub country: Option<String>,

    /// Content language (ISO 639-1, e.g., "en").
    #[serde(default)]
    pub search_lang: Option<String>,

    /// UI/response metadata language (e.g., "en-US").
    #[serde(default)]
    pub ui_lang: Option<String>,

    /// Enable up to 5 extra snippets per result.
    #[serde(default)]
    pub extra_snippets: Option<bool>,

    /// Goggles URL or inline definition for custom re-ranking.
    #[serde(default)]
    pub goggles: Option<String>,

    /// Force a refresh, bypassing the cache.
    #[serde(default = "default_false")]
    pub force_refresh: bool,

    /// Optional domain allowlist to filter results.
    #[serde(default)]
    pub domain_allowlist: Option<Vec<String>>,
}

fn default_count() -> Option<u8> {
    Some(20)
}

fn default_false() -> bool {
    false
}

/// Output structure for web_search tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchOutput {
    /// The search results.
    pub results: Vec<SearchResult>,
    /// Query metadata.
    pub query: QueryMeta,
    /// Debug information.
    pub debug: DebugInfo,
}

/// Individual search result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    /// Result title.
    pub title: String,
    /// Result URL.
    pub url: String,
    /// Result description/snippet.
    pub description: String,
    /// Extra snippets (if requested).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_snippets: Vec<String>,
    /// Search source (always "brave").
    pub source: String,
    /// Result rank (1-indexed).
    pub rank: usize,
}

/// Query metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryMeta {
    /// Original query string.
    pub original: String,
    /// Whether more results are available.
    pub more_results_available: bool,
}

/// Debug information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DebugInfo {
    /// Request ID or timing info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Cache hit status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,
}

/// Implementation of the web_search tool.
pub async fn search_impl(
    db: &CacheDb, config: &AppConfig, params: WebSearchParams,
) -> Result<CallToolResult, McpError> {
    if params.query.is_empty() {
        return Err(Error::InvalidInput("query cannot be empty".into()).into());
    }

    let safesearch = match params.safesearch.as_deref() {
        Some("off") => Some(SafeSearch::Off),
        Some("moderate") | None => Some(SafeSearch::Moderate),
        Some("strict") => Some(SafeSearch::Strict),
        Some(other) => {
            return Err(Error::InvalidInput(format!("invalid safesearch: {}", other)).into());
        }
    };

    let ttl = BraveClient::ttl_for_freshness(&params.freshness);

    let req = SearchRequest {
        q: params.query.clone(),
        count: params.count,
        offset: params.offset,
        freshness: params.freshness,
        safesearch,
        country: params.country,
        search_lang: params.search_lang,
        ui_lang: params.ui_lang,
        extra_snippets: params.extra_snippets,
        goggles: params.goggles,
        spellcheck: None,
    };

    req.validate().map_err(|e| Error::InvalidInput(e.to_string()))?;

    let cache_key = BraveClient::cache_key(&req);

    if !params.force_refresh
        && let Ok(Some(cached_json)) = db.get_search(&cache_key).await
        && let Ok(cached) = serde_json::from_str::<WebSearchOutput>(&cached_json)
    {
        tracing::debug!("cache hit for search query: {}", params.query);
        let mut output = cached;
        output.debug.cache_hit = Some(true);
        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]));
    }

    let client = BraveClient::new(BraveConfig {
        api_key: config
            .require_brave_api_key()
            .map_err(|e| Error::BraveAuthError(e.to_string()))?
            .to_string(),
        user_agent: config.user_agent.clone(),
        timeout: config.timeout(),
        ..Default::default()
    })
    .map_err(|e| match e {
        thndrs_client::BraveError::MissingApiKey => Error::BraveAuthError(e.to_string()),
        _ => Error::HttpError(e.to_string()),
    })?;

    let response = client.search(req).await.map_err(|e| match e {
        thndrs_client::BraveError::AuthError => Error::BraveAuthError(e.to_string()),
        thndrs_client::BraveError::RateLimited => Error::BraveRateLimited(e.to_string()),
        thndrs_client::BraveError::InvalidQuery(msg) => Error::InvalidInput(msg),
        thndrs_client::BraveError::HttpError { status } => Error::HttpError(format!("HTTP {}", status)),
        _ => Error::HttpError(e.to_string()),
    })?;

    let results = if let Some(allowlist) = &params.domain_allowlist {
        filter_by_domains(&response.results, allowlist)
    } else {
        response.results
    };

    let output = WebSearchOutput {
        results: results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                description: r.description,
                extra_snippets: r.extra_snippets,
                source: r.source,
                rank: r.rank,
            })
            .collect(),
        query: QueryMeta {
            original: response.query.original,
            more_results_available: response.query.more_results_available,
        },
        debug: DebugInfo { request_id: response.debug.request_id, cache_hit: Some(false) },
    };

    let query_json = serde_json::to_string(&params.query).unwrap_or_default();
    let response_json = serde_json::to_string(&output).unwrap_or_default();
    if let Err(e) = db.put_search(&cache_key, &query_json, &response_json, ttl).await {
        tracing::warn!("failed to cache search result: {}", e);
    }

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&output).unwrap_or_default(),
    )]))
}

/// Filter search results by domain allowlist.
fn filter_by_domains(
    results: &[thndrs_client::SearchResult], allowlist: &[String],
) -> Vec<thndrs_client::SearchResult> {
    results
        .iter()
        .filter(|r| {
            if let Ok(url) = url::Url::parse(&r.url)
                && let Some(host) = url.host_str()
            {
                return allowlist
                    .iter()
                    .any(|domain| host == domain.as_str() || host.ends_with(&format!(".{}", domain)));
            }
            false
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_query() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default();
        let params = WebSearchParams { query: "".into(), ..Default::default() };

        let result = search_impl(&db, &config, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_safesearch() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default();
        let params = WebSearchParams { query: "test".into(), safesearch: Some("invalid".into()), ..Default::default() };

        let result = search_impl(&db, &config, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_api_key() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let config = AppConfig::default(); // No brave_api_key set
        let params = WebSearchParams { query: "test".into(), ..Default::default() };

        let result = search_impl(&db, &config, params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_by_domains() {
        use thndrs_client::SearchResult;

        let results = vec![
            SearchResult {
                title: "Example 1".into(),
                url: "https://example.com/page1".into(),
                description: "Test 1".into(),
                extra_snippets: vec![],
                source: "test".into(),
                rank: 1,
            },
            SearchResult {
                title: "Other".into(),
                url: "https://other.com/page".into(),
                description: "Test 2".into(),
                extra_snippets: vec![],
                source: "test".into(),
                rank: 2,
            },
            SearchResult {
                title: "Example 2".into(),
                url: "https://sub.example.com/page2".into(),
                description: "Test 3".into(),
                extra_snippets: vec![],
                source: "test".into(),
                rank: 3,
            },
        ];

        let allowlist = vec!["example.com".to_string()];
        let filtered = filter_by_domains(&results, &allowlist);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].url, "https://example.com/page1");
        assert_eq!(filtered[1].url, "https://sub.example.com/page2");
    }
}
