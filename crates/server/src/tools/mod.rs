//! MCP tool implementations.
//!
//! This module contains all tools exposed by the mcp-web server.

pub mod cache;
pub mod web_extract;

#[allow(unused_imports)]
pub use web_extract::{WebExtractOutput, WebExtractParams};
