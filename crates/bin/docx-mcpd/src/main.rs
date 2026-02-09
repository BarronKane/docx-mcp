//! Daemon entry point for the docx MCP server.
//!
//! Loads configuration from the environment, initializes the solution registry,
//! and serves MCP over stdio alongside the HTTP ingest API.

mod config;
mod registry;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use docx_ingest::{IngestServer, IngestServerConfig};
use docx_mcp::server::{McpHttpServerConfig, serve_stdio, serve_streamable_http};

use crate::config::DocxConfig;
use crate::registry::build_registry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = DocxConfig::from_env()?;
    let (mcp_ipv4, mcp_ipv6) = dual_stack_addrs(config.mcp_http_addr);
    let (ingest_ipv4, ingest_ipv6) = dual_stack_addrs(config.ingest_addr);

    println!(
        "docx-mcp http listening on IPv4 {mcp_ipv4} and IPv6 {mcp_ipv6}"
    );
    println!(
        "docx-ingest listening on IPv4 {ingest_ipv4} and IPv6 {ingest_ipv6}"
    );
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

fn dual_stack_addrs(addr: SocketAddr) -> (SocketAddr, SocketAddr) {
    let port = addr.port();
    match addr.ip() {
        IpAddr::V4(ipv4) => {
            let ipv6 = if ipv4.is_loopback() {
                Ipv6Addr::LOCALHOST
            } else if ipv4.is_unspecified() {
                Ipv6Addr::UNSPECIFIED
            } else {
                ipv4.to_ipv6_mapped()
            };
            (
                SocketAddr::new(IpAddr::V4(ipv4), port),
                SocketAddr::new(IpAddr::V6(ipv6), port),
            )
        }
        IpAddr::V6(ipv6) => {
            let ipv4 = ipv6.to_ipv4().unwrap_or_else(|| {
                if ipv6.is_loopback() {
                    Ipv4Addr::LOCALHOST
                } else {
                    Ipv4Addr::UNSPECIFIED
                }
            });
            (
                SocketAddr::new(IpAddr::V4(ipv4), port),
                SocketAddr::new(IpAddr::V6(ipv6), port),
            )
        }
    }
}
