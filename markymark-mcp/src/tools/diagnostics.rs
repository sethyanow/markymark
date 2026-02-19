//! MCP tool handler for `get-diagnostics`.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult, DiagnosticSeverity};
use markymark_core::DocumentUri;
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::{
    DiagnosticItemDto, FileDiagnosticsDto, GetDiagnosticsRequest, GetDiagnosticsResponse, RangeDto,
};
use crate::PositionDto;

pub(crate) fn handle_get_diagnostics(
    engine: &dyn CoreEngine,
    request: GetDiagnosticsRequest,
) -> Result<CallToolResult, McpError> {
    // Resolve the optional URI parameter
    let uri = match request.uri.as_deref() {
        Some(s) => match DocumentUri::new(s) {
            Ok(u) => Some(u),
            Err(e) => return Ok(tool_error("invalid_uri", e.to_string())),
        },
        None => None,
    };

    let result = engine.execute(CoreOperation::GetDiagnostics {
        uri,
        realm: request.realm.clone(),
    });

    match result {
        CoreOperationResult::Diagnostics { realm, items } => {
            let file_dtos: Vec<FileDiagnosticsDto> = items
                .into_iter()
                .map(|(doc_uri, diags)| {
                    let diagnostic_dtos = diags
                        .into_iter()
                        .map(|d| DiagnosticItemDto {
                            range: RangeDto {
                                start: PositionDto {
                                    line: d.range.start.line,
                                    character: d.range.start.character,
                                },
                                end: PositionDto {
                                    line: d.range.end.line,
                                    character: d.range.end.character,
                                },
                            },
                            severity: match d.severity {
                                DiagnosticSeverity::Error => "error".to_string(),
                                DiagnosticSeverity::Warning => "warning".to_string(),
                            },
                            message: d.message,
                        })
                        .collect();
                    FileDiagnosticsDto {
                        uri: doc_uri.as_str().to_string(),
                        diagnostics: diagnostic_dtos,
                    }
                })
                .collect();

            let files_with_issues = file_dtos.len();
            Ok(CallToolResult::structured(json!(GetDiagnosticsResponse {
                realm,
                files_with_issues,
                diagnostics: file_dtos,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("get-diagnostics", &other)),
    }
}
