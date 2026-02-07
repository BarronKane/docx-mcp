mod helpers;

use std::{borrow::Cow, sync::Arc};

use rmcp::{
    ErrorData,
    ServerHandler,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::{Json, Parameters},
    schemars,
    tool,
    tool_handler,
    tool_router,
};
use rmcp::model::{CallToolResult, Content};
use serde::{
    Serialize,
    Deserialize,
};

#[derive(Clone)]
pub struct DocxMcp {
    tool_router: ToolRouter<Self>
}

impl DocxMcp {
    pub fn new() -> Self {
        Self {
            tool_router: ToolRouter::new()
        }
    }
}

#[tool_router]
impl DocxMcp {
    #[tool(description = "Health check. Returns 'ok'.")]
    async fn health(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }
}
