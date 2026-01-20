//! MCP server handler implementation.
//!
//! This module defines the main server handler that routes tool calls
//! to the appropriate implementations.

use crate::tools::cache::{CacheGetParams, CachePurgeParams, get_impl, purge_impl};
use crate::tools::web_batch_open::{WebBatchOpenParams, batch_open_impl};
use crate::tools::web_extract::{WebExtractParams, extract_impl};
use crate::tools::web_open::{WebOpenParams, open_impl};
use crate::tools::web_search::{WebSearchParams, search_impl};

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
use std::sync::Arc;
use thndrs_core::{AppConfig, CacheDb};

/// The main MCP server handler for mcp-web.
#[derive(Clone)]
pub struct McpWebServer {
    config: Arc<AppConfig>,
    tool_router: ToolRouter<Self>,
    cache: CacheDb,
}

/// Tool router implementation using the #[tool_router] macro.
///
/// This macro generates the routing logic that maps tool names to handler methods.
#[tool_router]
impl McpWebServer {
    /// Create a new server handler with the given configuration.
    ///
    /// Opens the SQLite cache database at the configured path and initializes
    /// the Brave client if an API key is provided.
    pub async fn new(config: AppConfig) -> Result<Self, anyhow::Error> {
        let config = Arc::new(config);

        let cache = CacheDb::open(&config.db_path).await?;

        Ok(Self { config, tool_router: Self::tool_router(), cache })
    }

    /// Extract readable content from HTML.
    ///
    /// This tool takes raw HTML and extracts the main article content, returning it as Markdown.
    /// No network requests are made.
    #[tool(description = "Extract readable content from HTML. Returns Markdown with title, links, and main content.")]
    async fn web_extract(&self, params: Parameters<WebExtractParams>) -> Result<CallToolResult, McpError> {
        extract_impl(params.0).await
    }

    /// Fetch a URL and extract readable content.
    ///
    /// Performs HTTP fetch with SSRF protection and robots.txt compliance,
    /// then extracts the main content as Markdown.
    /// Modes: "readable" (default) or "raw".
    #[tool(description = "Fetch a URL and extract readable content with SSRF protection and robots.txt compliance.")]
    async fn web_open(&self, params: Parameters<WebOpenParams>) -> Result<CallToolResult, McpError> {
        open_impl(&self.cache, &self.config, params.0).await
    }

    /// Fetch multiple URLs and extract readable content in parallel.
    ///
    /// Performs concurrent HTTP fetches with bounded concurrency, SSRF protection,
    /// and robots.txt compliance. Results are returned in input order.
    #[tool(description = "Fetch multiple URLs in parallel with bounded concurrency and SSRF protection.")]
    async fn web_batch_open(&self, params: Parameters<WebBatchOpenParams>) -> Result<CallToolResult, McpError> {
        batch_open_impl(&self.cache, &self.config, params.0).await
    }

    /// Search the web using Brave Search API.
    ///
    /// Performs web search with optional filtering and caching.
    /// Requires MCP_WEB_BRAVE_API_KEY environment variable to be set.
    #[tool(description = "Search the web using Brave Search API with caching and optional domain filtering.")]
    async fn web_search(&self, params: Parameters<WebSearchParams>) -> Result<CallToolResult, McpError> {
        search_impl(&self.cache, &self.config, params.0).await
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
