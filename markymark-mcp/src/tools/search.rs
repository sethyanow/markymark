//! search-symbols, semantic-search, search-workspace, and search-for-pattern tool handlers.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{round_score, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;
use crate::SEMANTIC_SEARCH_MAX_TOP_K;

pub(crate) fn handle_search_symbols(
    engine: &dyn CoreEngine,
    req: SearchSymbolsRequest,
) -> Result<CallToolResult, McpError> {
    let query = req.query.trim().to_string();
    if query.is_empty() {
        return Ok(tool_error(
            "invalid_query",
            "query must not be empty for search-symbols",
        ));
    }

    match engine.execute(CoreOperation::SearchSymbols {
        query: query.clone(),
        realm: req.realm.clone(),
    }) {
        CoreOperationResult::Symbols(symbols) => {
            let mut mapped: Vec<SymbolMatchDto> = symbols
                .into_iter()
                .map(|(name, uri, range)| SymbolMatchDto {
                    name,
                    uri: uri.as_str().to_string(),
                    range: range_to_dto(range),
                })
                .collect();
            // Keep output ordering deterministic for stable clients/tests.
            mapped.sort();

            Ok(CallToolResult::structured(json!(SearchSymbolsResponse {
                query,
                symbols: mapped,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("search-symbols", &other)),
    }
}

pub(crate) fn handle_semantic_search(
    engine: &dyn CoreEngine,
    req: SemanticSearchRequest,
) -> Result<CallToolResult, McpError> {
    let query = req.query.trim().to_string();
    if query.is_empty() {
        return Ok(tool_error(
            "invalid_query",
            "query must not be empty for semantic-search",
        ));
    }

    let realm = req
        .realm
        .clone()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());
    let realm_name = realm.clone().unwrap_or_else(|| "default".to_string());
    let top_k = req.top_k.unwrap_or(10).min(SEMANTIC_SEARCH_MAX_TOP_K);
    let min_score = req.min_score.unwrap_or(0.5).clamp(0.0, 1.0);

    match engine.execute(CoreOperation::SemanticSearch {
        query: query.clone(),
        realm,
        top_k,
        min_score,
    }) {
        CoreOperationResult::SemanticMatches(matches) => {
            let results = matches
                .into_iter()
                .map(|m| SemanticSearchResultDto {
                    doc_uri: m.doc_uri.as_str().to_string(),
                    heading: m.heading,
                    heading_level: m.heading_level,
                    score: round_score(m.score),
                    section_preview: m.section_preview,
                })
                .collect();
            Ok(CallToolResult::structured(json!(SemanticSearchResponse {
                query,
                realm: realm_name,
                results,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("semantic-search", &other)),
    }
}

pub(crate) fn handle_search_workspace(
    engine: &dyn CoreEngine,
    req: SearchWorkspaceRequest,
) -> Result<CallToolResult, McpError> {
    // Validate paired filter params.
    match (&req.frontmatter_filter_key, &req.frontmatter_filter_value) {
        (Some(_), None) | (None, Some(_)) => {
            return Ok(tool_error(
                "invalid_params",
                "frontmatter_filter_key and frontmatter_filter_value must both be provided",
            ));
        }
        _ => {}
    }
    match (&req.property_filter_key, &req.property_filter_value) {
        (Some(_), None) | (None, Some(_)) => {
            return Ok(tool_error(
                "invalid_params",
                "property_filter_key and property_filter_value must both be provided",
            ));
        }
        _ => {}
    }

    let frontmatter_filter = req.frontmatter_filter_key.as_ref().and_then(|k| {
        req.frontmatter_filter_value
            .as_ref()
            .map(|v| (k.clone(), v.clone()))
    });
    let property_filter = req.property_filter_key.as_ref().and_then(|k| {
        req.property_filter_value
            .as_ref()
            .map(|v| (k.clone(), v.clone()))
    });

    match engine.execute(CoreOperation::SearchWorkspace {
        query: req.query.clone(),
        frontmatter_filter,
        property_filter,
        tag_filter: req.tag_filter.clone(),
        realm: req.realm.clone(),
        limit: req.limit,
    }) {
        CoreOperationResult::WorkspaceSearchResults {
            realm,
            query,
            results,
        } => {
            let dtos: Vec<WorkspaceSearchResultDto> = results
                .into_iter()
                .map(|r| WorkspaceSearchResultDto {
                    uri: r.uri.as_str().to_string(),
                    title: r.title,
                    score: round_score(r.score),
                    matched_fields: r.matched_fields,
                    frontmatter_preview: r.frontmatter_preview,
                    property_preview: r.property_preview,
                    tags: r.tags,
                    is_journal: r.is_journal,
                    journal_date: r
                        .journal_date
                        .map(|(y, m, d)| [y, u16::from(m), u16::from(d)]),
                })
                .collect();
            Ok(CallToolResult::structured(json!(SearchWorkspaceResponse {
                realm,
                query,
                results: dtos,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("search-workspace", &other)),
    }
}

pub(crate) fn handle_search_for_pattern(
    engine: &dyn CoreEngine,
    req: SearchForPatternRequest,
) -> Result<CallToolResult, McpError> {
    match engine.execute(CoreOperation::SearchForPattern {
        pattern: req.pattern.clone(),
        include_glob: req.include_glob.clone(),
        context_lines: req.context_lines,
        limit: req.limit,
        case_insensitive: req.case_insensitive,
        realm: req.realm.clone(),
    }) {
        CoreOperationResult::PatternSearchResults {
            realm,
            pattern,
            files_searched,
            files_skipped,
            matches,
            truncated,
        } => {
            let dtos: Vec<PatternMatchDto> = matches
                .into_iter()
                .map(|m| PatternMatchDto {
                    uri: m.uri.as_str().to_string(),
                    line: m.line,
                    column: m.column,
                    match_text: m.match_text,
                    line_text: m.line_text,
                    context_before: m.context_before,
                    context_after: m.context_after,
                    context_start_line: m.context_start_line,
                })
                .collect();
            Ok(CallToolResult::structured(json!(
                SearchForPatternResponse {
                    pattern,
                    realm,
                    files_searched,
                    files_skipped,
                    matches: dtos,
                    truncated,
                }
            )))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("search-for-pattern", &other)),
    }
}
