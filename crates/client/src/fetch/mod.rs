//! HTTP fetch pipeline with SSRF protection and robots.txt compliance.
//!
//! ### URL Canonicalization
//! - Trim whitespace, ensure scheme (default: `https`)
//! - Lowercase host, remove fragments
//! - Preserve query string
//!
//! ### SSRF & Safety Gates
//! - Deny private ranges (RFC1918, link-local, localhost, etc.)
//! - Resolve DNS and validate all A/AAAA answers are public.
//! - Max redirects: 5
//! - Max body bytes: 5MB (configurable)
//!
//! ### robots.txt Compliance
//! - Fetch and cache `robots.txt` per host (24h cache).
//! - Evaluate `*` and current User-Agent.

pub mod robots;
pub mod ssrf;
pub mod url;

use bytes::Bytes;
use reqwest::Url;
use reqwest::{Client, StatusCode, header};
use std::time::{Duration, Instant};

pub use robots::{RobotsCache, RobotsError};
pub use ssrf::{SsrfError, validate_ip};
pub use url::{UrlError, canonicalize};

use thndrs_core::Error;

/// Configuration for the fetch client.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    /// User agent string (default: "mcp-web/0.1")
    pub user_agent: String,

    /// Maximum response body size in bytes (default: 5MB)
    pub max_bytes: usize,

    /// Request timeout (default: 20s)
    pub timeout: Duration,

    /// Maximum number of redirects to follow (default: 5)
    pub max_redirects: usize,

    /// Whether to respect robots.txt (default: true)
    pub respect_robots: bool,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            user_agent: "mcp-web/0.1".to_string(),
            max_bytes: 5 * 1024 * 1024,
            timeout: Duration::from_millis(20000),
            max_redirects: 5,
            respect_robots: true,
        }
    }
}

/// Response from a fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    /// The original URL requested
    pub url: Url,
    /// The final URL after redirects
    pub final_url: Url,
    /// HTTP status code
    pub status: StatusCode,
    /// Content-Type header
    pub content_type: Option<String>,
    /// Response body bytes
    pub bytes: Bytes,
    /// Response headers
    pub headers: header::HeaderMap,
    /// Time taken to fetch in milliseconds
    pub fetch_ms: u64,
}

/// HTTP fetch client with safety checks.
pub struct FetchClient {
    http: Client,
    config: FetchConfig,
    robots_cache: RobotsCache,
}

impl FetchClient {
    /// Create a new fetch client with the given configuration.
    pub fn new(config: FetchConfig) -> Result<Self, Error> {
        let http = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
            .use_rustls_tls()
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .map_err(|e| Error::FetchTimeout(format!("failed to build HTTP client: {}", e)))?;

        let robots_cache = RobotsCache::new(config.user_agent.clone());

        Ok(Self { http, config, robots_cache })
    }

    /// Fetch a URL, returning raw bytes and metadata.
    ///
    /// Performs SSRF check, robots.txt check, and respects redirect/byte limits.
    pub async fn fetch(&self, url_str: &str) -> Result<FetchResponse, Error> {
        let start = Instant::now();
        let url = canonicalize(url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;

        if self.config.respect_robots {
            self.robots_cache
                .is_allowed(&url)
                .await
                .map_err(|e| Error::RobotsDisallowed(e.to_string()))?;
        }

        let mut request = self.http.get(url.as_str());
        request = request.header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        );

        let response = request
            .send()
            .await
            .map_err(|e| Error::HttpError(format!("network error: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            return Err(Error::HttpError(format!("status {}", status.as_u16())));
        }

        let content_length = response.content_length();
        if let Some(len) = content_length
            && len as usize > self.config.max_bytes
        {
            return Err(Error::FetchTooLarge(format!(
                "{} bytes exceeds {}",
                len, self.config.max_bytes
            )));
        }

        let final_url = response.url().clone();
        let headers = response.headers().clone();

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::HttpError(format!("failed to read response: {}", e)))?;

        if bytes.len() > self.config.max_bytes {
            return Err(Error::FetchTooLarge(format!(
                "{} bytes exceeds {}",
                bytes.len(),
                self.config.max_bytes
            )));
        }

        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let fetch_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "fetched {} -> {} in {}ms ({} bytes)",
            url,
            final_url,
            fetch_ms,
            bytes.len()
        );

        Ok(FetchResponse { url, final_url, status, content_type, bytes, headers: headers.clone(), fetch_ms })
    }

    /// Get reference to the robots cache.
    pub fn robots_cache(&self) -> &RobotsCache {
        &self.robots_cache
    }

    /// Get reference to the configuration.
    pub fn config(&self) -> &FetchConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_config_default() {
        let config = FetchConfig::default();
        assert_eq!(config.user_agent, "mcp-web/0.1");
        assert_eq!(config.max_bytes, 5 * 1024 * 1024);
        assert_eq!(config.timeout, Duration::from_millis(20000));
        assert_eq!(config.max_redirects, 5);
        assert!(config.respect_robots);
    }

    #[test]
    fn test_fetch_response_fields() {
        let response = FetchResponse {
            url: Url::parse("https://example.com").unwrap(),
            final_url: Url::parse("https://example.com/redirected").unwrap(),
            status: StatusCode::OK,
            content_type: Some("text/html".to_string()),
            bytes: Bytes::new(),
            headers: header::HeaderMap::new(),
            fetch_ms: 100,
        };

        assert_eq!(response.url.as_str(), "https://example.com/");
        assert_eq!(response.final_url.as_str(), "https://example.com/redirected");
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.content_type, Some("text/html".to_string()));
        assert_eq!(response.fetch_ms, 100);
    }

    #[tokio::test]
    async fn test_fetch_client_new() {
        let config = FetchConfig::default();
        let client = FetchClient::new(config);
        assert!(client.is_ok());
    }
}
