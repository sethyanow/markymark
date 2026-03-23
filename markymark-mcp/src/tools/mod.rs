//! MCP tool handler implementations.
//!
//! Each submodule contains the body logic for one group of `#[tool]` methods.
//! The `#[tool]`-decorated method signatures remain in `lib.rs` and delegate here.

pub(crate) mod blocks;
pub(crate) mod curation;
pub(crate) mod diagnostics;
pub(crate) mod enrich;
pub(crate) mod export_docs_index;
pub(crate) mod graph;
pub(crate) mod outline;
pub(crate) mod realm;
pub(crate) mod recommend;
pub(crate) mod refs;
pub(crate) mod search;

use markymark_core::engine::CoreOperationResult;
use markymark_core::{CoreError, DocumentUri};
use rmcp::model::CallToolResult;
use serde_json::json;

use crate::dto::ToolErrorEnvelope;
use crate::ToolErrorPayload;

pub(crate) fn parse_file_uri(uri: &str) -> Result<DocumentUri, ToolErrorPayload> {
    if !uri.starts_with("file://") {
        return Err(ToolErrorPayload {
            code: "non_file_uri".to_string(),
            message: format!("only file:// URIs are supported, got: {uri}"),
        });
    }
    DocumentUri::new(uri).map_err(|err| ToolErrorPayload {
        code: "invalid_uri".to_string(),
        message: err.to_string(),
    })
}

pub(crate) fn round_score(score: f32) -> f32 {
    (score * 10_000.0).round() / 10_000.0
}

pub(crate) fn tool_error(code: &str, message: impl Into<String>) -> CallToolResult {
    CallToolResult::structured_error(json!(ToolErrorEnvelope {
        error: ToolErrorPayload {
            code: code.to_string(),
            message: message.into(),
        }
    }))
}

pub(crate) fn tool_error_from_core(err: CoreError) -> CallToolResult {
    match err {
        CoreError::InvalidUri(message) => tool_error("invalid_uri", message),
        CoreError::NotImplemented(message) => tool_error("not_implemented", message),
        CoreError::Message(message) => tool_error("core_error", message),
    }
}

pub(crate) fn unexpected_result_error(tool: &str, result: &CoreOperationResult) -> CallToolResult {
    tool_error(
        "unexpected_core_result",
        format!("tool {tool} received unsupported core result variant: {result:?}"),
    )
}
