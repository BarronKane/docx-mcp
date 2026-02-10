use docx_core::control::{CsharpIngestRequest, RustdocIngestRequest};
use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorCode},
    schemars,
    tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::{DocxMcp, helpers};

/// Parameters for ingesting .NET XML documentation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CsharpIngestParams {
    pub solution: String,
    pub project_id: String,
    pub xml: Option<String>,
    pub xml_path: Option<String>,
    pub ingest_id: Option<String>,
    pub source_path: Option<String>,
    pub source_modified_at: Option<String>,
    pub tool_version: Option<String>,
    pub source_hash: Option<String>,
}

/// Parameters for ingesting rustdoc JSON documentation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RustdocIngestParams {
    pub solution: String,
    pub project_id: String,
    pub json: Option<String>,
    pub json_path: Option<String>,
    pub ingest_id: Option<String>,
    pub source_path: Option<String>,
    pub source_modified_at: Option<String>,
    pub tool_version: Option<String>,
    pub source_hash: Option<String>,
}

#[tool_router(router = tool_router_ingest, vis = "pub")]
impl<C: Connection> DocxMcp<C> {
    #[tool(description = "Ingest C# XML documentation into the solution store. Provide xml or xml_path.")]
    async fn ingest_csharp_xml(
        &self,
        Parameters(params): Parameters<CsharpIngestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let control = self.control_for_solution(&params.solution).await?;
        let report = control
            .ingest_csharp_xml(CsharpIngestRequest {
                project_id: params.project_id,
                xml: params.xml,
                xml_path: params.xml_path,
                ingest_id: params.ingest_id,
                source_path: params.source_path,
                source_modified_at: params.source_modified_at,
                tool_version: params.tool_version,
                source_hash: params.source_hash,
            })
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(report)?]))
    }

    #[tool(description = "Ingest rustdoc JSON documentation into the solution store. Provide json (raw rustdoc JSON text) or json_path.")]
    async fn ingest_rustdoc_json(
        &self,
        Parameters(params): Parameters<RustdocIngestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let json = normalize_payload(params.json);
        let json_path = normalize_payload(params.json_path);
        if json.is_none() && json_path.is_none() {
            return Err(helpers::mcp_err(
                ErrorCode::INVALID_PARAMS,
                "json is required (provide json or json_path)",
            ));
        }
        let control = self.control_for_solution(&params.solution).await?;
        let report = control
            .ingest_rustdoc_json(RustdocIngestRequest {
                project_id: params.project_id,
                json,
                json_path,
                ingest_id: params.ingest_id,
                source_path: params.source_path,
                source_modified_at: params.source_modified_at,
                tool_version: params.tool_version,
                source_hash: params.source_hash,
            })
            .await
            .map_err(helpers::map_err)?;
        Ok(CallToolResult::success(vec![Content::json(report)?]))
    }
}

fn normalize_payload(value: Option<String>) -> Option<String> {
    value.and_then(|payload| {
        let trimmed = payload.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(payload)
        }
    })
}