//! Brave Search API response types and normalization.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Raw response from Brave Web Search API.
#[derive(Debug, Deserialize)]
pub struct BraveApiResponse {
    pub query: QueryInfo,
    #[serde(default)]
    pub web: Option<WebResults>,
}

/// Query metadata from Brave response.
#[derive(Debug, Deserialize)]
pub struct QueryInfo {
    pub original: String,
    #[serde(default)]
    #[serde(alias = "moreResultsAvailable")]
    pub more_results_available: bool,
}

/// Web search results container.
#[derive(Debug, Deserialize)]
pub struct WebResults {
    pub results: Vec<WebResult>,
}

/// Individual web search result from Brave.
#[derive(Debug, Deserialize)]
pub struct WebResult {
    pub title: String,
    #[serde(alias = "url")]
    pub source_url: String,
    pub description: String,
    #[serde(default)]
    pub extra_snippets: Vec<String>,
}

/// Normalized search response for internal use.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub query: QueryMeta,
    pub debug: DebugInfo,
}

/// Normalized search result.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_snippets: Vec<String>,
    pub source: String,
    pub rank: usize,
}

/// Normalized query metadata.
#[derive(Debug, Clone, Serialize)]
pub struct QueryMeta {
    pub original: String,
    pub more_results_available: bool,
}

/// Debug information for the search.
#[derive(Debug, Clone, Serialize)]
pub struct DebugInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl From<BraveApiResponse> for SearchResponse {
    /// Convert raw Brave API response to normalized internal format.
    fn from(raw: BraveApiResponse) -> Self {
        let results = raw
            .web
            .map(|w| {
                w.results
                    .into_iter()
                    .enumerate()
                    .map(|(idx, r)| SearchResult {
                        title: r.title,
                        url: r.source_url.clone(),
                        description: r.description,
                        extra_snippets: r.extra_snippets,
                        source: "brave".to_string(),
                        rank: idx + 1,
                    })
                    .collect()
            })
            .unwrap_or_default();

        SearchResponse {
            results,
            query: QueryMeta { original: raw.query.original, more_results_available: raw.query.more_results_available },
            debug: DebugInfo { request_id: None },
        }
    }
}

impl SearchResponse {
    /// Create a new search response with timing info.
    pub fn with_timing(mut self, start: Instant) -> Self {
        self.debug.request_id = Some(format!("{:?}", start.elapsed()));
        self
    }

    /// Check if there are more results available.
    pub fn has_more(&self) -> bool {
        self.query.more_results_available
    }

    /// Get the number of results.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_JSON: &str = r#"{
        "query": {
            "original": "test query",
            "moreResultsAvailable": true
        },
        "web": {
            "results": [
                {
                    "title": "Example Domain",
                    "url": "https://example.com",
                    "description": "This domain is for use in illustrative examples",
                    "extra_snippets": ["First snippet", "Second snippet"]
                },
                {
                    "title": "Test Page",
                    "url": "https://test.com",
                    "description": "A test page",
                    "extra_snippets": []
                }
            ]
        }
    }"#;

    #[test]
    fn test_deserialize_brave_response() {
        let response: BraveApiResponse = serde_json::from_str(FIXTURE_JSON).unwrap();
        assert_eq!(response.query.original, "test query");
        assert!(response.query.more_results_available);
        assert!(response.web.is_some());
        assert_eq!(response.web.unwrap().results.len(), 2);
    }

    #[test]
    fn test_normalize_to_search_response() {
        let raw: BraveApiResponse = serde_json::from_str(FIXTURE_JSON).unwrap();
        let normalized: SearchResponse = raw.into();

        assert_eq!(normalized.query.original, "test query");
        assert!(normalized.query.more_results_available);
        assert_eq!(normalized.results.len(), 2);

        let first = &normalized.results[0];
        assert_eq!(first.rank, 1);
        assert_eq!(first.title, "Example Domain");
        assert_eq!(first.url, "https://example.com");
        assert_eq!(first.source, "brave");
        assert_eq!(first.extra_snippets.len(), 2);

        let second = &normalized.results[1];
        assert_eq!(second.rank, 2);
        assert_eq!(second.extra_snippets.len(), 0);
    }

    #[test]
    fn test_empty_results() {
        let json = r#"{"query": {"original": "test"}, "web": {"results": []}}"#;
        let raw: BraveApiResponse = serde_json::from_str(json).unwrap();
        let normalized: SearchResponse = raw.into();

        assert_eq!(normalized.results.len(), 0);
        assert!(!normalized.has_more());
    }

    #[test]
    fn test_response_helper_methods() {
        let raw: BraveApiResponse = serde_json::from_str(FIXTURE_JSON).unwrap();
        let response: SearchResponse = raw.into();

        assert!(response.has_more());
        assert_eq!(response.result_count(), 2);
    }
}
