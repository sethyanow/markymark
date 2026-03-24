//! enrich-document tool handler.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{parse_file_uri, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) async fn handle_enrich_document(
    engine: &dyn CoreEngine,
    req: EnrichDocumentRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    let sidecar_dir = req.sidecar_dir.map(std::path::PathBuf::from);

    let result = engine
        .execute(CoreOperation::EnrichDocument {
            uri,
            realm: req.realm,
            sidecar_dir,
            force: req.force,
        })
        .await;

    match result {
        CoreOperationResult::EnrichmentResult {
            uri,
            sections_enriched,
            was_stale,
            model_id,
        } => Ok(CallToolResult::structured(json!(EnrichDocumentResponse {
            uri: uri.as_str().to_string(),
            sections_enriched,
            was_stale,
            model_id,
        }))),
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("enrich-document", &other)),
    }
}
