//! MCP server handler implementation.
//!
//! This module defines the main server handler that
//! routes tool calls to the appropriate implementations.
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

/// The main MCP server handler for mcp-web.
#[derive(Clone)]
pub struct McpWebServer {
    tool_router: ToolRouter<Self>,
}

/// Tool router implementation using the #[tool_router] macro.
///
/// This macro generates the routing logic that maps tool names to handler methods.
#[tool_router]
impl McpWebServer {
    /// Create a new server handler.
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    /// Extract readable content from HTML.
    ///
    /// This tool takes raw HTML and extracts the main article content, returning it as Markdown.
    /// No network requests are made.
    #[tool(description = "Extract readable content from HTML. Returns Markdown with title, links, and main content.")]
    async fn web_extract(&self, params: Parameters<WebExtractParams>) -> Result<CallToolResult, McpError> {
        extract_impl(params.0).await
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
