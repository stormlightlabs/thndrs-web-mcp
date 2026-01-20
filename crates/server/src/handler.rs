//! MCP server handler implementation.
//!
//! This module defines the main server handler that routes tool calls
//! to the appropriate implementations.

use crate::tools::cache::{CacheGetParams, CachePurgeParams, get_impl, purge_impl};
use crate::tools::web_extract::{WebExtractParams, extract_impl};

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{
        tool::{ToolCallContext, ToolRouter},
        wrapper::Parameters,
    },
    model::{
        CallToolRequestParam, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities, ServerInfo,
    },
    service::{RequestContext, RoleServer},
    tool, tool_router,
};
use thndrs_core::CacheDb;

/// Default database path for the cache.
const DEFAULT_DB_PATH: &str = "./mcp-web-cache.sqlite";

/// The main MCP server handler for mcp-web.
#[derive(Clone)]
pub struct McpWebServer {
    tool_router: ToolRouter<Self>,
    cache: CacheDb,
}

/// Tool router implementation using the #[tool_router] macro.
///
/// This macro generates the routing logic that maps tool names to handler methods.
#[tool_router]
impl McpWebServer {
    /// Create a new server handler.
    ///
    /// Opens the SQLite cache database at the default path or from the
    /// MCP_WEB_DB_PATH environment variable.
    pub async fn new() -> Result<Self, anyhow::Error> {
        let db_path = std::env::var("MCP_WEB_DB_PATH")
            .ok()
            .unwrap_or_else(|| DEFAULT_DB_PATH.to_string());

        let cache = CacheDb::open(&db_path).await?;

        Ok(Self { tool_router: Self::tool_router(), cache })
    }

    /// Extract readable content from HTML.
    ///
    /// This tool takes raw HTML and extracts the main article content, returning it as Markdown.
    /// No network requests are made.
    #[tool(description = "Extract readable content from HTML. Returns Markdown with title, links, and main content.")]
    async fn web_extract(&self, params: Parameters<WebExtractParams>) -> Result<CallToolResult, McpError> {
        extract_impl(params.0).await
    }

    /// Retrieve a cached snapshot by hash.
    ///
    /// Returns the full cached document including metadata and extracted content.
    #[tool(description = "Retrieve a cached snapshot by its content hash.")]
    async fn cache_get(&self, params: Parameters<CacheGetParams>) -> Result<CallToolResult, McpError> {
        get_impl(&self.cache, params.0).await
    }

    /// Purge cache entries by age, domain, or count.
    ///
    /// Supports multiple purge strategies:
    /// - older_than_days: Delete entries older than N days
    /// - domain: Delete entries matching a domain pattern
    /// - max_entries: Keep only the newest N entries (LRU)
    #[tool(description = "Purge cache entries by age, domain, or count.")]
    async fn cache_purge(&self, params: Parameters<CachePurgeParams>) -> Result<CallToolResult, McpError> {
        purge_impl(&self.cache, params.0).await
    }
}

impl ServerHandler for McpWebServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "mcp-web".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self, _request: Option<PaginatedRequestParam>, _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::model::ErrorData> {
        Ok(ListToolsResult { meta: None, tools: self.tool_router.list_all(), next_cursor: None })
    }

    async fn call_tool(
        &self, request: CallToolRequestParam, context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        self.tool_router
            .call(ToolCallContext::new(self, request, context))
            .await
    }
}
