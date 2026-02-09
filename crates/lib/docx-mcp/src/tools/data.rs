use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars,
    tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::{DocxMcp, helpers};

/// Parameters for listing symbol kinds in a project.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListSymbolTypesParams {
    pub solution: String,
    pub project_id: String,
}

/// Parameters for listing members in a qualified scope.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetMembersParams {
    pub solution: String,
    pub project_id: String,
    pub scope: String,
    pub limit: Option<usize>,
}

/// Parameters for fetching a symbol by key.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolParams {
    pub solution: String,
    pub project_id: String,
    pub symbol_key: String,
}

/// Parameters for listing documentation blocks for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListDocBlocksParams {
    pub solution: String,
    pub project_id: String,
    pub symbol_key: String,
    pub ingest_id: Option<String>,
}

/// Parameters for fetching adjacency and relations for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolAdjacencyParams {
    pub solution: String,
    pub project_id: String,
    pub symbol_key: String,
    pub limit: Option<usize>,
}

/// Parameters for searching symbols by name.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsParams {
    pub solution: String,
    pub project_id: String,
    pub name: String,
    pub limit: Option<usize>,
}

/// Parameters for searching documentation blocks by text.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchDocBlocksParams {
    pub solution: String,
    pub project_id: String,
    pub text: String,
    pub limit: Option<usize>,
}

#[tool_router(router = tool_router_data, vis = "pub")]
impl<C: Connection> DocxMcp<C> {
    #[tool(description = "List symbol kinds present in a project.")]
    async fn list_symbol_types(
        &self,
        Parameters(params): Parameters<ListSymbolTypesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let kinds = control
            .list_symbol_kinds(&params.project_id)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(kinds)?]))
    }

    #[tool(description = "List members under a namespace/module scope.")]
    async fn get_members(
        &self,
        Parameters(params): Parameters<GetMembersParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let members = control
            .list_members_by_scope(&params.project_id, &params.scope, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(members)?]))
    }

    #[tool(description = "Fetch a symbol by its key.")]
    async fn get_symbol(
        &self,
        Parameters(params): Parameters<GetSymbolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let symbol = control
            .get_symbol(&params.project_id, &params.symbol_key)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(symbol)?]))
    }

    #[tool(description = "List doc blocks for a symbol.")]
    async fn list_doc_blocks(
        &self,
        Parameters(params): Parameters<ListDocBlocksParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let blocks = control
            .list_doc_blocks(
                &params.project_id,
                &params.symbol_key,
                params.ingest_id.as_deref(),
            )
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(blocks)?]))
    }

    #[tool(description = "Fetch a symbol with doc metadata, relation edges, and related symbols.")]
    async fn get_symbol_adjacency(
        &self,
        Parameters(params): Parameters<GetSymbolAdjacencyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let adjacency = control
            .get_symbol_adjacency(&params.project_id, &params.symbol_key, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(adjacency)?]))
    }

    #[tool(description = "Search symbols by name fragment.")]
    async fn search_symbols(
        &self,
        Parameters(params): Parameters<SearchSymbolsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let symbols = control
            .search_symbols(&params.project_id, &params.name, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(symbols)?]))
    }

    #[tool(description = "Search doc blocks by text fragment.")]
    async fn search_doc_blocks(
        &self,
        Parameters(params): Parameters<SearchDocBlocksParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let blocks = control
            .search_doc_blocks(&params.project_id, &params.text, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(blocks)?]))
    }
}
