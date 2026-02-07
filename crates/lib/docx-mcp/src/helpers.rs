use std::borrow::Cow;
use rmcp::ErrorData;
use rmcp::model::ErrorCode;

fn mcp_err(code: ErrorCode, message: impl Into<Cow<'static, str>>) -> ErrorData {
    ErrorData {
        code,
        message: message.into(),
        data: None,
    }
}
