//! mcp-web server entry point.
//!
//! This is the main binary that boots the MCP server on stdio transport.
//! Logging goes to stderr to avoid interfering with the JSON-RPC protocol on stdout.

use anyhow::Result;
use rmcp::service::serve_server;
use rmcp::transport::io::stdio;
use thndrs_core::AppConfig;
use tracing_subscriber::EnvFilter;

mod handler;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .json()
        .init();

    let config = AppConfig::load()?;
    tracing::info!(
        db_path = %config.db_path.display(),
        timeout_ms = config.timeout_ms,
        max_bytes = config.max_bytes,
        "Configuration loaded"
    );

    tracing::info!("Starting mcp-web server on stdio transport");

    let handler = handler::McpWebServer::new(config).await?;
    let transport = stdio();
    let server = serve_server(handler, transport).await?;

    server.waiting().await?;

    Ok(())
}
