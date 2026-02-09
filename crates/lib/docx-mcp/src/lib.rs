//! MCP server implementation for docx-mcp.
//!
//! This crate wires the control plane into rmcp tool handlers and exposes the
//! MCP-facing API surface for ingestion and query.

mod helpers;
mod tools;
pub mod server;

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
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use surrealdb::Connection;

const SERVER_INSTRUCTIONS: &str = r"docx-mcp provides MCP tools for ingesting documentation and querying a metadata-rich graph.

Workflow:
1. Choose a `solution` name (tenant). If unsure, call `list_solutions`. If there is no solution
    that matches the one your in (by root folder name or similar means), choose a new one to use.
2. Ingest documentation into a `project_id` (project or crate) using:
   - `ingest_csharp_xml` for .NET XML documentation.
   - `ingest_rustdoc_json` for rustdoc JSON output.
   Include optional metadata: `ingest_id`, `source_path`, `source_modified_at`, `tool_version`, `source_hash`.
3. Query metadata:
   - `list_projects`, `search_projects`, `list_ingests`, `get_ingest`, `list_doc_sources`, `get_doc_source`.
4. Query symbols and docs:
   - `list_symbol_types`, `search_symbols`, `get_symbol`, `list_doc_blocks`, `search_doc_blocks`.
   - `get_symbol_adjacency` returns symbols, doc blocks, doc sources, and relation edges.

Notes:
- `symbol_key` format is `{language}|{project_id}|{qualified_name}` for rustdoc data.
- Symbol metadata includes source file paths, line/column, signatures, params, and return types when available.
- Relation edges include `member_of`, `contains`, `returns`, `param_type`, `see_also`, `inherits`, and `references`.
- Use `help`, `ingestion_help`, `dotnet_help`, and `rust_help` for detailed guidance.
- `health` returns `ok`.";

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
            + Self::tool_router_ingest()
            + Self::tool_router_metadata()
            + Self::tool_router_data()
            + Self::tool_router_context();
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
impl<C: Connection> ServerHandler for DocxMcp<C> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(SERVER_INSTRUCTIONS.to_string()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}
