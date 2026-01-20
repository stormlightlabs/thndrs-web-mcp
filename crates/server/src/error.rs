//! Structured errors for the mcp-web server.
//!
//! These map to the error codes defined in the roadmap.

use rmcp::model::{ErrorCode, ErrorData as McpError};

/// Structured errors for the mcp-web server.
#[derive(Debug, thiserror::Error)]
pub enum WebError {
    /// Invalid input parameters (e.g., empty HTML).
    #[error("INVALID_INPUT: {0}")]
    InvalidInput(String),

    /// Content extraction failed.
    #[error("EXTRACT_FAILED: {0}")]
    ExtractFailed(String),
}

impl From<WebError> for McpError {
    fn from(err: WebError) -> Self {
        let (code, message) = match &err {
            WebError::InvalidInput(msg) => (-32602, msg.clone()),
            WebError::ExtractFailed(msg) => (-32000, msg.clone()),
        };

        McpError { code: ErrorCode(code), message: message.into(), data: None }
    }
}
