//! find-references and rename tool handlers.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{Position, Range};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{parse_file_uri, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) fn handle_find_references(
    engine: &dyn CoreEngine,
    req: FindReferencesRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    let position = Range::new(
        Position::new(req.line, req.character),
        Position::new(req.line, req.character),
    );

    match engine.execute(CoreOperation::FindReferences {
        uri,
        position,
        realm: req.realm.clone(),
    }) {
        CoreOperationResult::Locations(locations) => {
            let mut mapped: Vec<LocationDto> = locations
                .into_iter()
                .map(|(uri, range)| LocationDto {
                    uri: uri.as_str().to_string(),
                    range: range_to_dto(range),
                })
                .collect();
            mapped.sort();

            Ok(CallToolResult::structured(json!(FindReferencesResponse {
                uri: req.uri,
                locations: mapped,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("find-references", &other)),
    }
}

pub(crate) fn handle_rename(
    engine: &dyn CoreEngine,
    req: RenameRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    let new_name = req.new_name.trim().to_string();
    if new_name.is_empty() {
        return Ok(tool_error(
            "invalid_name",
            "new_name must not be empty for rename",
        ));
    }

    let position = Range::new(
        Position::new(req.line, req.character),
        Position::new(req.line, req.character),
    );

    match engine.execute(CoreOperation::Rename {
        uri,
        position,
        new_name,
        realm: req.realm.clone(),
    }) {
        CoreOperationResult::WorkspaceEdit(edits) => {
            let mut changes: Vec<DocumentEditDto> = edits
                .into_iter()
                .map(|(uri, text_edits)| DocumentEditDto {
                    uri: uri.as_str().to_string(),
                    edits: text_edits
                        .into_iter()
                        .map(|(range, new_text)| TextEditDto {
                            range: range_to_dto(range),
                            new_text,
                        })
                        .collect(),
                })
                .collect();
            changes.sort();

            Ok(CallToolResult::structured(json!(RenameResponse {
                changes
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("rename", &other)),
    }
}
