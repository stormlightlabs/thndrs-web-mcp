//! robots.txt compliance with caching.
//!
//! Fetches and caches robots.txt files per-host, respecting a 24-hour TTL.

use robotstxt_rs::RobotsTxt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use url::Url;

/// Default TTL for robots.txt cache (24 hours).
const ROBOTS_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum size of robots.txt to fetch (1MB).
const MAX_ROBOTS_SIZE: usize = 1024 * 1024;

/// Error type for robots.txt operations.
#[derive(Debug, thiserror::Error)]
pub enum RobotsError {
    #[error("robots.txt disallowed: {path} (robots_url: {robots_url})")]
    Disallowed { path: String, robots_url: String },

    #[error("failed to fetch robots.txt: {0}")]
    FetchError(String),

    #[error("robots.txt too large")]
    TooLarge,
}

/// Cached robots.txt entry with timestamp.
struct CachedRobots {
    robots: RobotsTxt,
    fetched_at: Instant,
}

impl CachedRobots {
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > ROBOTS_TTL
    }
}

/// In-memory cache for robots.txt files.
///
/// Uses a simple HashMap with tokio RwLock for concurrent access.
pub struct RobotsCache {
    cache: Arc<RwLock<HashMap<String, CachedRobots>>>,
    user_agent: String,
    http: reqwest::Client,
}

impl RobotsCache {
    /// Create a new robots.txt cache.
    pub fn new(user_agent: String) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            user_agent,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Check if a URL path is allowed by robots.txt.
    ///
    /// This will fetch and cache robots.txt for the host if not already cached.
    pub async fn is_allowed(&self, url: &Url) -> Result<bool, RobotsError> {
        let robots_url = format!("{}://{}/robots.txt", url.scheme(), url.host_str().unwrap_or(""));
        let cache_key = robots_url.clone();

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key)
                && !cached.is_expired()
            {
                let allowed = cached.robots.can_fetch(&self.user_agent, url.as_str());
                tracing::debug!("robots.txt cache hit for {}: {}", cache_key, allowed);
                return Ok(allowed);
            }
        }

        let robots = self.fetch_robots(&robots_url).await?;

        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, CachedRobots { robots, fetched_at: Instant::now() });
        }
        let cache = self.cache.read().await;
        let cached = cache.get(&robots_url).unwrap();
        let allowed = cached.robots.can_fetch(&self.user_agent, url.as_str());

        if !allowed {
            return Err(RobotsError::Disallowed { path: url.path().to_string(), robots_url });
        }

        Ok(allowed)
    }

    /// Fetch robots.txt from the given URL.
    async fn fetch_robots(&self, url: &str) -> Result<RobotsTxt, RobotsError> {
        let response = self
            .http
            .get(url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await
            .map_err(|e| RobotsError::FetchError(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            if let Some(len) = response.content_length()
                && len as usize > MAX_ROBOTS_SIZE
            {
                return Err(RobotsError::TooLarge);
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|e| RobotsError::FetchError(e.to_string()))?;

            if bytes.len() > MAX_ROBOTS_SIZE {
                return Err(RobotsError::TooLarge);
            }

            let content = String::from_utf8_lossy(&bytes);
            Ok(RobotsTxt::parse(&content))
        } else if status.is_client_error() {
            tracing::debug!("robots.txt not found for {}, allowing all", url);
            Ok(RobotsTxt::parse(""))
        } else {
            Err(RobotsError::FetchError(format!("status {}", status)))
        }
    }

    /// Clear expired entries from the cache.
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, cached| !cached.is_expired());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_robots_expiry() {
        let robots = RobotsTxt::parse("User-agent: *\nAllow: /");
        let mut cached = CachedRobots { robots, fetched_at: Instant::now() };
        assert!(!cached.is_expired());

        cached.fetched_at = Instant::now() - ROBOTS_TTL - Duration::from_secs(1);
        assert!(cached.is_expired());
    }

    #[tokio::test]
    async fn test_robots_cache_new() {
        let cache = RobotsCache::new("mcp-web/0.1".to_string());
        assert_eq!(cache.user_agent, "mcp-web/0.1");
    }

    #[tokio::test]
    async fn test_robots_cache_cleanup() {
        let cache = RobotsCache::new("mcp-web/0.1".to_string());
        let mut c = cache.cache.write().await;
        c.insert(
            "https://example.com/robots.txt".to_string(),
            CachedRobots {
                robots: RobotsTxt::parse(
                    "User-agent: *
Allow: /",
                ),
                fetched_at: Instant::now() - ROBOTS_TTL - Duration::from_secs(1),
            },
        );
        drop(c);

        cache.cleanup_expired().await;

        let c = cache.cache.read().await;
        assert!(c.is_empty());
    }
}
