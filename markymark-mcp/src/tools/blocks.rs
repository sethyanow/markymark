//! get-content-blocks tool handler.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{parse_file_uri, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

/// Validate a kind filter string, returning an error message if invalid.
fn validate_kind_filter(kind: &str) -> Result<(), String> {
    match kind {
        "paragraph" | "list_item" | "code_block" | "blockquote" | "thematic_break" | "table" => {
            Ok(())
        }
        _ => Err(format!(
            "invalid block kind filter: \"{kind}\". Valid values: paragraph, list_item, code_block, blockquote, thematic_break, table"
        )),
    }
}

pub(crate) async fn handle_get_content_blocks(
    engine: &dyn CoreEngine,
    req: GetContentBlocksRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    // Validate kind filter before dispatching to engine.
    if let Some(ref kind) = req.kind {
        if let Err(msg) = validate_kind_filter(kind) {
            return Ok(tool_error("invalid_argument", msg));
        }
    }

    // Normalize empty realm to None.
    let realm = req.realm.filter(|r| !r.is_empty());

    match engine
        .execute(CoreOperation::GetContentBlocks {
            uri,
            realm,
            kind_filter: req.kind,
            heading_filter: req.heading,
            block_id: req.block_id,
            include_text: req.include_text,
        })
        .await
    {
        CoreOperationResult::ContentBlocks { uri, blocks } => {
            let blocks: Vec<ContentBlockDto> = blocks
                .into_iter()
                .map(|b| ContentBlockDto {
                    kind: b.kind,
                    range: range_to_dto(b.range),
                    parent_heading_slug: b.parent_heading_slug,
                    block_id: b.block_id,
                    text: b.text,
                })
                .collect();

            Ok(CallToolResult::structured(json!(
                GetContentBlocksResponse {
                    uri: uri.as_str().to_string(),
                    blocks,
                }
            )))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("get-content-blocks", &other)),
    }
}
