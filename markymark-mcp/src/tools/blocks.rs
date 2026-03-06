//! Handlers for content-block MCP tools.
//!
//! Part of epic marky-z7uc: expose ContentBlock model via MCP tools.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{parse_file_uri, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

/// Handle a `get-content-blocks` tool call.
///
/// Converts the DTO request into a `CoreOperation::GetContentBlocks`,
/// dispatches it, and maps the result to a structured `CallToolResult`.
pub(crate) async fn handle_get_content_blocks(
    engine: &dyn CoreEngine,
    req: GetContentBlocksRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(super::tool_error(&err.code, err.message)),
    };

    match engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: req.realm,
            kind_filter: req.kind,
            heading_filter: req.heading,
            block_id: req.block_id,
            include_text: req.include_text,
        })
        .await
    {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            let block_dtos: Vec<ContentBlockDto> = blocks
                .into_iter()
                .map(|b| ContentBlockDto {
                    kind: b.kind,
                    range: range_to_dto(b.range),
                    parent_heading_slug: b.parent_heading_slug,
                    block_id: b.block_id,
                    text: b.text,
                })
                .collect();

            Ok(CallToolResult::structured(
                json!(GetContentBlocksResponse {
                    uri: req.uri,
                    content_blocks: block_dtos,
                }),
            ))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("get-content-blocks", &other)),
    }
}
