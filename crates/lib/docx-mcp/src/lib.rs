//! MCP server implementation for docx-mcp.
//!
//! This crate wires the control plane into rmcp tool handlers and exposes the
//! MCP-facing API surface for ingestion and query.

mod helpers;
mod tools;

use std::sync::Arc;

use docx_core::control::DocxControlPlane;
use docx_core::services::{RegistryError, SolutionRegistry};
use rmcp::{
    ErrorData,
    ServerHandler,
    handler::server::tool::ToolRouter,
    tool,
    tool_handler,
    tool_router,
};
use rmcp::model::{CallToolResult, Content};
use surrealdb::Connection;

/// MCP server wrapper around the solution registry and tool routers.
#[derive(Clone)]
pub struct DocxMcp<C: Connection> {
    tool_router: ToolRouter<Self>,
    registry: Arc<SolutionRegistry<C>>,
}

impl<C: Connection> DocxMcp<C> {
    /// Creates a new server using a registry by value.
    #[must_use]
    pub fn new(registry: SolutionRegistry<C>) -> Self {
        Self::with_registry(Arc::new(registry))
    }

    /// Creates a new server using a shared registry handle.
    #[must_use]
    pub fn with_registry(registry: Arc<SolutionRegistry<C>>) -> Self {
        let tool_router = Self::tool_router_core()
            + Self::tool_router_metadata()
            + Self::tool_router_data();
        Self {
            tool_router,
            registry,
        }
    }

    /// Lists known solution names in the registry.
    pub async fn solution_names(&self) -> Vec<String> {
        self.registry.list_solutions().await
    }

    /// Retrieves the control plane for a solution, initializing it if needed.
    pub(crate) async fn control_for_solution(
        &self,
        solution: &str,
    ) -> Result<DocxControlPlane<C>, ErrorData> {
        let handle = self
            .registry
            .get_or_init(solution)
            .await
            .map_err(map_registry_err)?;
        Ok(handle.control())
    }
}

fn map_registry_err(err: RegistryError) -> ErrorData {
    match err {
        RegistryError::UnknownSolution(solution) => helpers::mcp_err(
            rmcp::model::ErrorCode::RESOURCE_NOT_FOUND,
            format!("unknown solution: {solution}"),
        ),
        RegistryError::CapacityReached { max } => helpers::mcp_err(
            rmcp::model::ErrorCode::INTERNAL_ERROR,
            format!("solution registry capacity reached (max {max})"),
        ),
        RegistryError::BuildFailed(message) => helpers::mcp_err(
            rmcp::model::ErrorCode::INTERNAL_ERROR,
            format!("failed to build solution handle: {message}"),
        ),
    }
}

#[tool_router(router = tool_router_core, vis = "pub")]
impl<C: Connection> DocxMcp<C> {
    #[tool(description = "Health check. Returns 'ok'.")]
    async fn health(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }
}

#[tool_handler]
impl<C: Connection> ServerHandler for DocxMcp<C> {}
