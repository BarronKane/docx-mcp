//! MCP server runners for docx-mcp.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use docx_core::services::SolutionRegistry;
use rmcp::serve_server;
use rmcp::transport::io::stdio;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig,
    StreamableHttpService,
    session::local::LocalSessionManager,
};
use surrealdb::Connection;

use crate::DocxMcp;

/// Configuration for the MCP streamable HTTP server.
#[derive(Debug, Clone)]
pub struct McpHttpServerConfig {
    pub addr: SocketAddr,
    pub stateful_mode: bool,
    pub sse_keep_alive: Option<Duration>,
    pub sse_retry: Option<Duration>,
}

impl McpHttpServerConfig {
    #[must_use]
    pub const fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            stateful_mode: true,
            sse_keep_alive: Some(Duration::from_secs(15)),
            sse_retry: Some(Duration::from_secs(3)),
        }
    }

    #[must_use]
    pub const fn with_stateful_mode(mut self, stateful_mode: bool) -> Self {
        self.stateful_mode = stateful_mode;
        self
    }

    #[must_use]
    pub const fn with_sse_keep_alive(mut self, sse_keep_alive: Option<Duration>) -> Self {
        self.sse_keep_alive = sse_keep_alive;
        self
    }

    #[must_use]
    pub const fn with_sse_retry(mut self, sse_retry: Option<Duration>) -> Self {
        self.sse_retry = sse_retry;
        self
    }
}

impl Default for McpHttpServerConfig {
    fn default() -> Self {
        Self::new("127.0.0.1:4020".parse().expect("valid MCP HTTP address"))
    }
}

/// Serves the MCP server over stdio.
///
/// # Errors
/// Returns any transport or server error.
pub async fn serve_stdio<C: Connection>(
    registry: Arc<SolutionRegistry<C>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = DocxMcp::with_registry(registry);
    let (stdin, stdout) = stdio();
    let running = serve_server(service, (stdin, stdout)).await?;
    let _ = running.waiting().await?;
    Ok(())
}

/// Serves the MCP server using streamable HTTP transport.
///
/// # Errors
/// Returns any listener or server error.
pub async fn serve_streamable_http<C>(
    registry: Arc<SolutionRegistry<C>>,
    config: McpHttpServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    C: Connection + Send + Sync + 'static,
{
    let service_registry = registry.clone();
    let service: StreamableHttpService<DocxMcp<C>, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(DocxMcp::with_registry(service_registry.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig {
                sse_keep_alive: config.sse_keep_alive,
                sse_retry: config.sse_retry,
                stateful_mode: config.stateful_mode,
                ..Default::default()
            },
        );

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
