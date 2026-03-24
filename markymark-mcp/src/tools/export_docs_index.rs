//! export-docs-index tool handler.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) async fn handle_export_docs_index(
    engine: &dyn CoreEngine,
    req: ExportDocsIndexRequest,
) -> Result<CallToolResult, McpError> {
    match engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: req.realm,
            name_override: req.name_override,
        })
        .await
    {
        CoreOperationResult::DocsIndexExport {
            realm,
            entries,
            doc_count,
            root_count,
            skipped_count,
        } => Ok(CallToolResult::structured(json!(ExportDocsIndexResponse {
            realm,
            entries,
            doc_count,
            root_count,
            skipped_count,
        }))),
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("export-docs-index", &other)),
    }
}
