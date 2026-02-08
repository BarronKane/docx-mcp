//! Daemon entry point for the docx MCP server.
//!
//! Loads configuration from the environment, initializes the solution registry,
//! and serves the MCP protocol over stdio.

mod config;
mod registry;

use docx_mcp::DocxMcp;
use rmcp::serve_server;
use rmcp::transport::io::stdio;

use crate::config::DocxConfig;
use crate::registry::build_registry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DocxConfig::from_env()?;
    let registry = build_registry(&config);
    let _sweeper = registry.clone().spawn_sweeper();

    let service = DocxMcp::new(registry);
    let (stdin, stdout) = stdio();
    let running = serve_server(service, (stdin, stdout)).await?;
    let _ = running.waiting().await?;
    Ok(())
}
