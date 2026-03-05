//! curation-diagnostics tool handler.

use markymark_core::engine::{
    CoreEngine, CoreOperation, CoreOperationResult, CurationSuggestionType,
};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) async fn handle_curation_diagnostics(
    engine: &dyn CoreEngine,
    req: CurationDiagnosticsRequest,
) -> Result<CallToolResult, McpError> {
    match engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: req.realm,
            include_suggestions: req.include_suggestions,
            max_suggestions: req.max_suggestions,
            max_items_per_category: req.max_items_per_category,
        })
        .await
    {
        CoreOperationResult::CurationReport { realm, report } => {
            let orphan_docs: Vec<String> = report
                .orphan_docs
                .into_iter()
                .map(|u| u.as_str().to_string())
                .collect();

            let low_connectivity_docs: Vec<ConnectivityDocDto> = report
                .low_connectivity_docs
                .into_iter()
                .map(|d| ConnectivityDocDto {
                    uri: d.uri.as_str().to_string(),
                    connectivity: d.connectivity,
                    in_degree: d.in_degree,
                    out_degree: d.out_degree,
                })
                .collect();

            let suggestions: Vec<CurationSuggestionDto> = report
                .suggestions
                .into_iter()
                .map(|s| CurationSuggestionDto {
                    source_doc: s.source_doc.as_str().to_string(),
                    target_doc: s.target_doc.as_str().to_string(),
                    reason: s.reason,
                    suggestion_type: match s.suggestion_type {
                        CurationSuggestionType::CrossLink => "cross_link".to_string(),
                        CurationSuggestionType::ReduceOrphan => "reduce_orphan".to_string(),
                    },
                })
                .collect();

            let stats = CurationStatsDto {
                total_docs: report.stats.total_docs,
                orphan_count: report.stats.orphan_count,
                orphan_percentage: report.stats.orphan_percentage,
                avg_connectivity: report.stats.avg_connectivity,
                median_connectivity: report.stats.median_connectivity,
                broken_link_count: report.stats.broken_link_count,
            };

            Ok(CallToolResult::structured(json!(
                CurationDiagnosticsResponse {
                    realm,
                    orphan_docs,
                    low_connectivity_docs,
                    suggestions,
                    stats,
                }
            )))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("curation-diagnostics", &other)),
    }
}
