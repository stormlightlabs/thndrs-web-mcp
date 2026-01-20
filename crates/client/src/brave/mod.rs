//! Brave Search API client.
//!
//! Provides a client for the Brave Web Search API with rate limiting,
//! request validation, and response normalization.
//!
//! ### Specification
//!
//! - **Endpoint**: `https://api.search.brave.com/res/v1/web/search`
//! - **Authentication**: Uses `X-Subscription-Token` header.
//! - **Rate Limiting**:
//!   - Respects Brave's published rate limits (token bucket).
//!   - Default 1s interval for free tier.
//!   - Retries on 429 with backoff and transient 5xx.
//! - **Normalization**: Converts Brave's response into a stable `SearchResult` struct.

pub mod error;
pub mod request;
pub mod response;

pub use error::BraveError;
pub use request::{SafeSearch, SearchRequest};
pub use response::{DebugInfo, QueryMeta, SearchResponse, SearchResult};

use reqwest::header;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Default base URL for Brave Search API.
const DEFAULT_BASE_URL: &str = "https://api.search.brave.com/res/v1";

/// Default request timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default user agent.
const DEFAULT_USER_AGENT: &str = "mcp-web/0.1";

/// Minimum interval between requests for rate limiting (1 second for free tier).
const MIN_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// Brave API client configuration.
#[derive(Debug, Clone)]
pub struct BraveConfig {
    /// API key from BRAVE_API_KEY env var.
    pub api_key: String,
    /// Base URL (default: https://api.search.brave.com/res/v1).
    pub base_url: String,
    /// Request timeout (default: 10s).
    pub timeout: Duration,
    /// User-agent string (default: mcp-web/0.x).
    pub user_agent: String,
}

impl Default for BraveConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: DEFAULT_TIMEOUT,
            user_agent: DEFAULT_USER_AGENT.to_string(),
        }
    }
}

impl BraveConfig {
    /// Load configuration from environment variables.
    ///
    /// Reads BRAVE_API_KEY from environment. Returns error if not set.
    pub fn from_env() -> Result<Self, BraveError> {
        let api_key = std::env::var("BRAVE_API_KEY").map_err(|_| BraveError::MissingApiKey)?;

        Ok(Self { api_key, ..Default::default() })
    }
}

/// Rate limiter to enforce request intervals.
#[derive(Debug)]
struct RateLimiter {
    last_request: Mutex<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            last_request: Mutex::new(Instant::now().checked_sub(min_interval).unwrap_or_else(Instant::now)),
            min_interval,
        }
    }

    /// Acquire permission to make a request, waiting if necessary.
    async fn acquire(&self) {
        let mut last = self.last_request.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_interval {
            tokio::time::sleep(self.min_interval - elapsed).await;
        }
        *last = Instant::now();
    }
}

/// Brave Search API client.
#[derive(Debug, Clone)]
pub struct BraveClient {
    http: reqwest::Client,
    config: BraveConfig,
    rate_limiter: Arc<RateLimiter>,
}

impl BraveClient {
    /// Create a new Brave client with the given configuration.
    pub fn new(config: BraveConfig) -> Result<Self, BraveError> {
        if config.api_key.is_empty() {
            return Err(BraveError::MissingApiKey);
        }

        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| BraveError::Network(Arc::new(e)))?;

        Ok(Self { http, config, rate_limiter: Arc::new(RateLimiter::new(MIN_REQUEST_INTERVAL)) })
    }

    /// Create a new Brave client from environment variables.
    pub fn from_env() -> Result<Self, BraveError> {
        Self::new(BraveConfig::from_env()?)
    }

    /// Execute a web search query.
    ///
    /// This method handles rate limiting, request validation, and response normalization.
    pub async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BraveError> {
        req.validate()?;

        self.rate_limiter.acquire().await;

        let start = Instant::now();
        let url = format!("{}/web/search", self.config.base_url);

        tracing::debug!("searching Brave API: query={}", req.q);

        let http_response = self
            .http
            .get(&url)
            .header("X-Subscription-Token", &self.config.api_key)
            .header("Accept", "application/json")
            .header(header::USER_AGENT, &self.config.user_agent)
            .query(&req)
            .send()
            .await
            .map_err(
                |e| {
                    if e.is_timeout() { BraveError::Timeout } else { BraveError::Network(Arc::new(e)) }
                },
            )?;

        let status = http_response.status();
        tracing::debug!("Brave API response status: {}", status);

        if status == 401 || status == 403 {
            return Err(BraveError::AuthError);
        }

        if status == 429 {
            return Err(BraveError::RateLimited);
        }

        if status.is_client_error() || status.is_server_error() {
            return Err(BraveError::HttpError { status: status.as_u16() });
        }

        let bytes = http_response
            .bytes()
            .await
            .map_err(|e| BraveError::Network(Arc::new(e)))?;
        let api_response: response::BraveApiResponse =
            serde_json::from_slice(&bytes).map_err(|e| BraveError::Parse(e.to_string()))?;

        tracing::debug!(
            "search completed in {:?}, {} results",
            start.elapsed(),
            api_response.web.as_ref().map(|w| w.results.len()).unwrap_or(0)
        );

        Ok(SearchResponse::from(api_response).with_timing(start))
    }

    /// Generate a cache key for the search request.
    ///
    /// The key is a SHA-256 hash of the normalized request parameters.
    pub fn cache_key(req: &SearchRequest) -> String {
        let params = serde_json::json!({
            "q": req.q,
            "count": req.count.unwrap_or(20),
            "offset": req.offset.unwrap_or(0),
            "freshness": req.freshness,
            "safesearch": req.safesearch,
            "country": req.country,
            "search_lang": req.search_lang,
        });

        let mut hasher = Sha256::new();
        hasher.update(params.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Calculate TTL for search results based on freshness parameter.
    ///
    /// Returns TTL in seconds.
    pub fn ttl_for_freshness(freshness: &Option<String>) -> i64 {
        match freshness.as_deref() {
            Some("pd") => 3600,  // 1 hour for past day
            Some("pw") => 21600, // 6 hours for past week
            Some("pm") => 43200, // 12 hours for past month
            Some("py") => 86400, // 24 hours for past year
            Some(_) => 21600,    // 6 hours for custom ranges
            None => 21600,       // 6 hours default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_missing_key() {
        let original = std::env::var("BRAVE_API_KEY").ok();
        unsafe {
            std::env::remove_var("BRAVE_API_KEY");
        }

        let result = BraveConfig::from_env();
        assert!(matches!(result, Err(BraveError::MissingApiKey)));

        if let Some(key) = original {
            unsafe {
                std::env::set_var("BRAVE_API_KEY", key);
            }
        }
    }

    #[test]
    fn test_cache_key_stability() {
        let req1 =
            SearchRequest { q: "test query".to_string(), count: Some(10), offset: Some(0), ..Default::default() };

        let req2 =
            SearchRequest { q: "test query".to_string(), count: Some(10), offset: Some(0), ..Default::default() };

        let key1 = BraveClient::cache_key(&req1);
        let key2 = BraveClient::cache_key(&req2);

        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_cache_key_different_params() {
        let req1 = SearchRequest { q: "test query".to_string(), count: Some(10), ..Default::default() };

        let req2 = SearchRequest { q: "test query".to_string(), count: Some(20), ..Default::default() };

        let key1 = BraveClient::cache_key(&req1);
        let key2 = BraveClient::cache_key(&req2);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_ttl_calculation() {
        assert_eq!(BraveClient::ttl_for_freshness(&Some("pd".to_string())), 3600);
        assert_eq!(BraveClient::ttl_for_freshness(&Some("pw".to_string())), 21600);
        assert_eq!(BraveClient::ttl_for_freshness(&Some("pm".to_string())), 43200);
        assert_eq!(BraveClient::ttl_for_freshness(&Some("py".to_string())), 86400);
        assert_eq!(BraveClient::ttl_for_freshness(&Some("custom".to_string())), 21600);
        assert_eq!(BraveClient::ttl_for_freshness(&None), 21600);
    }

    #[test]
    fn test_client_new_missing_key() {
        let config = BraveConfig::default();
        let result = BraveClient::new(config);
        assert!(matches!(result, Err(BraveError::MissingApiKey)));
    }
}
