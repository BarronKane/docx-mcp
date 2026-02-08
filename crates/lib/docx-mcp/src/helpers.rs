use std::borrow::Cow;
use std::fmt;

use rmcp::ErrorData;
use rmcp::model::ErrorCode;

pub fn mcp_err(code: ErrorCode, message: impl Into<Cow<'static, str>>) -> ErrorData {
    ErrorData {
        code,
        message: message.into(),
        data: None,
    }
}

pub fn internal_err(message: impl Into<Cow<'static, str>>) -> ErrorData {
    ErrorData::internal_error(message, None)
}

pub fn map_err(err: impl fmt::Display) -> ErrorData {
    internal_err(err.to_string())
}
