//! Unified error types for mcp-web.
//!
//! These map to the error codes defined in the roadmap §H3.

use rmcp::model::{ErrorCode, ErrorData as McpError};
use tokio_rusqlite::rusqlite;

/// Unified error types for the mcp-web server.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid input parameters (e.g., empty HTML).
    #[error("INVALID_INPUT: {0}")]
    InvalidInput(String),

    /// Content extraction failed.
    #[error("EXTRACT_FAILED: {0}")]
    ExtractFailed(String),

    /// No cache entry found for the given hash.
    #[error("CACHE_MISS: {0}")]
    CacheMiss(String),

    /// Database operation failed.
    #[error("CACHE_ERROR: {0}")]
    Database(tokio_rusqlite::Error),

    /// Migration failed to apply.
    #[error("CACHE_ERROR: migration failed: {0}")]
    MigrationFailed(String),

    /// Invalid hash format.
    #[error("CACHE_ERROR: invalid hash format")]
    InvalidHash,

    /// Invalid URL.
    #[error("INVALID_URL: {0}")]
    InvalidUrl(String),

    /// SSRF blocked - private/internal address not allowed.
    #[error("SSRF_BLOCKED: {0}")]
    SsrfBlocked(String),

    /// Robots.txt disallowed access.
    #[error("ROBOTS_DISALLOWED: {0}")]
    RobotsDisallowed(String),

    /// Fetch timeout.
    #[error("FETCH_TIMEOUT: {0}")]
    FetchTimeout(String),

    /// Fetch response too large.
    #[error("FETCH_TOO_LARGE: {0}")]
    FetchTooLarge(String),

    /// HTTP error response.
    #[error("HTTP_ERROR: {0}")]
    HttpError(String),

    /// Brave API authentication error.
    #[error("BRAVE_AUTH_ERROR: {0}")]
    BraveAuthError(String),

    /// Brave API rate limited.
    #[error("BRAVE_RATE_LIMITED: {0}")]
    BraveRateLimited(String),

    /// Render mode is disabled.
    #[error("RENDER_DISABLED")]
    RenderDisabled,

    /// Render failed.
    #[error("RENDER_FAILED: {0}")]
    RenderFailed(String),
}

impl From<tokio_rusqlite::Error<Error>> for Error {
    fn from(err: tokio_rusqlite::Error<Error>) -> Self {
        match err {
            tokio_rusqlite::Error::Error(e) => e,
            tokio_rusqlite::Error::ConnectionClosed => Error::Database(tokio_rusqlite::Error::ConnectionClosed),
            tokio_rusqlite::Error::Close(c) => Error::Database(tokio_rusqlite::Error::Close(c)),
            _ => Error::Database(tokio_rusqlite::Error::ConnectionClosed),
        }
    }
}

impl From<tokio_rusqlite::Error<rusqlite::Error>> for Error {
    fn from(err: tokio_rusqlite::Error<rusqlite::Error>) -> Self {
        Error::Database(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Database(tokio_rusqlite::Error::Error(err))
    }
}

impl From<Error> for McpError {
    fn from(err: Error) -> Self {
        let (code, message) = match &err {
            Error::InvalidInput(msg) => (-32602, msg.clone()),
            Error::ExtractFailed(msg) => (-32000, msg.clone()),
            Error::CacheMiss(msg) => (-32001, msg.clone()),
            Error::InvalidUrl(msg) => (-32003, msg.clone()),
            Error::SsrfBlocked(msg) => (-32004, msg.clone()),
            Error::RobotsDisallowed(msg) => (-32005, msg.clone()),
            Error::FetchTimeout(msg) => (-32006, msg.clone()),
            Error::FetchTooLarge(msg) => (-32007, msg.clone()),
            Error::HttpError(msg) => (-32008, msg.clone()),
            Error::BraveAuthError(msg) => (-32009, msg.clone()),
            Error::BraveRateLimited(msg) => (-32010, msg.clone()),
            Error::RenderDisabled => (-32011, "Render mode is disabled".to_string()),
            Error::RenderFailed(msg) => (-32012, msg.clone()),
            Error::Database(e) => (-32002, e.to_string()),
            Error::MigrationFailed(msg) => (-32002, msg.clone()),
            Error::InvalidHash => (-32002, "Invalid hash format".to_string()),
        };

        McpError { code: ErrorCode(code), message: message.into(), data: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::CacheMiss("abc123".to_string());
        assert!(err.to_string().contains("CACHE_MISS"));
        assert!(err.to_string().contains("abc123"));
    }

    #[test]
    fn test_error_to_mcp_error() {
        let err = Error::CacheMiss("abc123".to_string());
        let mcp_err: McpError = err.into();
        assert_eq!(mcp_err.code.0, -32001);
    }
}
