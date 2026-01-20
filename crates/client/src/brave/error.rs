//! Brave API client error types.

use std::sync::Arc;

/// Errors from Brave Search API client.
#[derive(Debug, thiserror::Error)]
pub enum BraveError {
    /// Missing BRAVE_API_KEY environment variable.
    #[error("missing API key: BRAVE_API_KEY not set")]
    MissingApiKey,

    /// Invalid search query.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// Invalid count parameter (must be 1-20).
    #[error("invalid count: must be 1-20")]
    InvalidCount,

    /// Invalid offset parameter (must be 0-9).
    #[error("invalid offset: must be 0-9")]
    InvalidOffset,

    /// Invalid freshness format.
    #[error("invalid freshness format: {0}")]
    InvalidFreshness(String),

    /// Authentication failed (invalid API key).
    #[error("authentication failed: invalid API key")]
    AuthError,

    /// Rate limited by Brave API.
    #[error("rate limited: too many requests")]
    RateLimited,

    /// HTTP error response.
    #[error("HTTP error: {status}")]
    HttpError { status: u16 },

    /// Request timeout.
    #[error("request timeout")]
    Timeout,

    /// Network error.
    #[error("network error: {0}")]
    Network(Arc<reqwest::Error>),

    /// Response parse error.
    #[error("parse error: {0}")]
    Parse(String),
}

impl From<reqwest::Error> for BraveError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() { BraveError::Timeout } else { BraveError::Network(Arc::new(err)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = BraveError::MissingApiKey;
        assert!(err.to_string().contains("API key"));

        let err = BraveError::InvalidQuery("test".to_string());
        assert!(err.to_string().contains("invalid query"));
    }
}
