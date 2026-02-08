//! Daemon entry point for the docx MCP server.
//!
//! Loads configuration from the environment, initializes the solution registry,
//! and serves MCP over stdio alongside the HTTP ingest API.

mod config;
mod registry;

use std::sync::Arc;

use docx_ingest::{IngestServer, IngestServerConfig};
use docx_mcp::server::{McpHttpServerConfig, serve_stdio, serve_streamable_http};

use crate::config::DocxConfig;
use crate::registry::build_registry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = DocxConfig::from_env()?;
    let registry = build_registry(&config);
    let _sweeper = registry.clone().spawn_sweeper();
    let registry = Arc::new(registry);

    let ingest_config = IngestServerConfig::new(config.ingest_addr)
        .with_max_body_bytes(config.ingest_max_body_bytes)
        .with_request_timeout(config.ingest_timeout);
    let ingest_server = IngestServer::new(registry.clone(), ingest_config);

    tokio::select! {
        result = ingest_server.serve() => result?,
        result = serve_stdio(registry.clone()) => result?,
        result = serve_streamable_http(
            registry.clone(),
            McpHttpServerConfig::new(config.mcp_http_addr),
        ) => result?,
    }

    Ok(())
}
