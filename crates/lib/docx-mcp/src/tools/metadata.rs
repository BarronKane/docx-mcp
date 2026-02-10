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

use crate::DocxMcp;
use crate::helpers;

/// Parameters for listing projects in a solution.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListProjectsParams {
    pub solution: String,
    pub limit: Option<usize>,
}

/// Parameters for searching projects in a solution.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchProjectsParams {
    pub solution: String,
    pub pattern: String,
    pub limit: Option<usize>,
}

/// Parameters for listing ingests in a project.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListIngestsParams {
    pub solution: String,
    pub project_id: String,
    pub limit: Option<usize>,
}

/// Parameters for fetching an ingest by id.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetIngestParams {
    pub solution: String,
    pub ingest_id: String,
}

/// Parameters for listing document sources in a project.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListDocSourcesParams {
    pub solution: String,
    pub project_id: String,
    pub ingest_id: Option<String>,
    pub limit: Option<usize>,
}

/// Parameters for fetching a document source by id.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetDocSourceParams {
    pub solution: String,
    pub doc_source_id: String,
}

#[tool_router(router = tool_router_metadata, vis = "pub")]
impl<C: Connection> DocxMcp<C> {
    #[tool(description = "List all configured solution names.")]
    async fn list_solutions(&self) -> Result<CallToolResult, ErrorData> {
        let mut solutions = self.solution_names().await;
        solutions.sort();
        Ok(CallToolResult::success(vec![Content::json(solutions)?]))
    }

    #[tool(description = "List projects for a solution.")]
    async fn list_projects(
        &self,
        Parameters(params): Parameters<ListProjectsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let projects = control.list_projects(limit).await.map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(projects)?]))
    }

    #[tool(description = "Search projects by wildcard pattern (e.g. DL.*).")]
    async fn search_projects(
        &self,
        Parameters(params): Parameters<SearchProjectsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let projects = control
            .search_projects(&params.pattern, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(projects)?]))
    }

    #[tool(description = "List ingests for a project.")]
    async fn list_ingests(
        &self,
        Parameters(params): Parameters<ListIngestsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let control = self.control_for_solution(&params.solution).await?;
        let ingests = control
            .list_ingests(&params.project_id, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(ingests)?]))
    }

    #[tool(description = "Fetch an ingest by id.")]
    async fn get_ingest(
        &self,
        Parameters(params): Parameters<GetIngestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let ingest = control
            .get_ingest(&params.ingest_id)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(ingest)?]))
    }

    #[tool(description = "List document sources for a project.")]
    async fn list_doc_sources(
        &self,
        Parameters(params): Parameters<ListDocSourcesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(200);
        let ingest_id = params
            .ingest_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let control = self.control_for_solution(&params.solution).await?;
        let sources = control
            .list_doc_sources(&params.project_id, ingest_id, limit)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(sources)?]))
    }

    #[tool(description = "Fetch a document source by id.")]
    async fn get_doc_source(
        &self,
        Parameters(params): Parameters<GetDocSourceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let source = control
            .get_doc_source(&params.doc_source_id)
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(source)?]))
    }
}
