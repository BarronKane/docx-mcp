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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListProjectsParams {
    pub solution: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchProjectsParams {
    pub solution: String,
    pub pattern: String,
    pub limit: Option<usize>,
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
}
