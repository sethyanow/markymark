//! recommend-docs tool handler.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{round_score, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) async fn handle_recommend_docs(
    engine: &dyn CoreEngine,
    req: RecommendDocsRequest,
) -> Result<CallToolResult, McpError> {
    if req.query.trim().is_empty() {
        return Ok(tool_error("invalid_query", "query must not be empty"));
    }

    match engine
        .execute(CoreOperation::RecommendDocs {
            query: req.query,
            realm: req.realm,
            top_k: req.top_k,
            include_sections: req.include_sections,
        })
        .await
    {
        CoreOperationResult::Recommendations {
            realm,
            query,
            results,
        } => {
            let recommendations: Vec<DocRecommendationDto> = results
                .into_iter()
                .map(|r| DocRecommendationDto {
                    uri: r.uri.as_str().to_string(),
                    title: r.title,
                    relevance_score: round_score(r.relevance_score),
                    search_score: round_score(r.search_score),
                    hub_score: round_score(r.hub_score),
                    matched_fields: r.matched_fields,
                    tags: r.tags,
                    document_summary: r.document_summary,
                    sections: r.sections.map(|secs| {
                        secs.into_iter()
                            .map(|s| RecommendedSectionDto {
                                heading_path: s.heading_path,
                                level: s.level,
                                summary: s.summary,
                            })
                            .collect()
                    }),
                })
                .collect();

            Ok(CallToolResult::structured(json!(RecommendDocsResponse {
                realm,
                query,
                recommendations,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("recommend-docs", &other)),
    }
}
