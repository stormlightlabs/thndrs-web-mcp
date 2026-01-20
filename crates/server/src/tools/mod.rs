//! MCP tool implementations.
//!
//! This module contains all tools exposed by the mcp-web server.
#![allow(unused_imports)]

pub mod cache;
pub mod web_extract;
pub mod web_open;

pub use web_extract::{WebExtractOutput, WebExtractParams};
pub use web_open::{WebOpenOutput, WebOpenParams};
