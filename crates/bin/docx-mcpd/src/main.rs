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
    let config = DocxConfig::from_args()?;
    if !config.mcp_serve && config.db_in_memory && !config.test_mode {
        return Err("refusing to start: MCP HTTP disabled with in-memory database (set DOCX_DB_IN_MEMORY=0 or pass --test)".into());
    }
    if !config.enable_stdio && !config.mcp_serve && !config.ingest_serve {
        return Err("refusing to start: no MCP or ingest servers enabled".into());
    }
    let (mcp_ipv4, mcp_ipv6) = dual_stack_addrs(config.mcp_http_addr);
    let (ingest_ipv4, ingest_ipv6) = dual_stack_addrs(config.ingest_addr);

    if config.mcp_serve {
        println!("docx-mcp http listening on IPv4 {mcp_ipv4} and IPv6 {mcp_ipv6}");
    }
    if config.ingest_serve {
        println!("docx-ingest listening on IPv4 {ingest_ipv4} and IPv6 {ingest_ipv6}");
    }
    let registry = build_registry(&config);
    let _sweeper = registry.clone().spawn_sweeper();
    let registry = Arc::new(registry);

    let ingest_server = if config.ingest_serve {
        let ingest_config = IngestServerConfig::new(config.ingest_addr)
            .with_max_body_bytes(config.ingest_max_body_bytes)
            .with_request_timeout(config.ingest_timeout);
        Some(IngestServer::new(registry.clone(), ingest_config))
    } else {
        None
    };

    if config.enable_stdio && !config.mcp_serve && ingest_server.is_none() {
        serve_stdio(registry).await?;
        return Ok(());
    }

    if config.enable_stdio {
        let registry = registry.clone();
        tokio::spawn(async move {
            if let Err(err) = serve_stdio(registry).await {
                eprintln!("docx-mcp stdio server exited: {err}");
            }
        });
    }

    let ingest_task = ingest_server.map(|server| tokio::spawn(async move { server.serve().await }));
    let mcp_task = if config.mcp_serve {
        let registry = registry.clone();
        Some(tokio::spawn(async move {
            serve_streamable_http(registry, McpHttpServerConfig::new(config.mcp_http_addr)).await
        }))
    } else {
        None
    };

    match (ingest_task, mcp_task) {
        (Some(ingest_task), Some(mcp_task)) => {
            let (ingest_result, mcp_result) = tokio::try_join!(ingest_task, mcp_task)?;
            ingest_result?;
            mcp_result?;
        }
        (Some(task), None) | (None, Some(task)) => {
            task.await??;
        }
        (None, None) => {}
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
