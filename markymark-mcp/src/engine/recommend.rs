//! Engine handler for the `recommend-docs` MCP tool.
//!
//! Composes search-workspace (text relevance) with graph-analysis (hub scores)
//! to produce ranked document recommendations. Optionally loads enrichment
//! sidecars for document and section summaries.

use markymark_core::engine::{
    CoreOperationResult, DocRecommendation, RecommendedSection, WorkspaceSearchResult,
};
use markymark_index::RealmIndex;

use std::collections::HashMap;
use std::path::PathBuf;

use super::outline::try_load_sidecar;

/// Weight for text search score in the combined relevance calculation.
const SEARCH_WEIGHT: f32 = 0.7;
/// Weight for graph hub score in the combined relevance calculation.
const HUB_WEIGHT: f32 = 0.3;

/// Execute the recommend-docs operation.
///
/// Two-stage retrieval:
/// 1. Rank documents by combining search-workspace text scores with graph hub scores
/// 2. For top-K results, optionally load sidecar summaries
pub fn handle_recommend_docs(
    realm_key: &str,
    realm: &RealmIndex,
    roots: &[PathBuf],
    query: &str,
    top_k: u32,
    include_sections: bool,
) -> CoreOperationResult {
    let top_k = (top_k as usize).min(20);
    if top_k == 0 {
        return CoreOperationResult::Recommendations {
            realm: realm_key.to_string(),
            query: query.to_string(),
            results: vec![],
        };
    }

    // Stage 1a: Text search — get scored documents matching the query.
    let search_results = extract_search_results(&crate::search::execute_search_workspace(
        realm_key,
        realm,
        Some(query.to_string()),
        None,
        None,
        None,
        100, // Get a wide net; we'll narrow to top_k after merging hub scores.
    ));

    // Stage 1b: Graph analysis — get hub documents for centrality boost.
    let hub_map = extract_hub_map(&crate::graph::execute_graph_analysis(
        realm_key, realm, 100, false,
    ));

    // Stage 2: Merge scores and rank.
    let max_incoming = hub_map.values().copied().max().unwrap_or(1) as f32;

    let mut recommendations: Vec<DocRecommendation> = search_results
        .into_iter()
        .map(|sr| {
            let hub_raw = hub_map.get(sr.uri.as_str()).copied().unwrap_or(0) as f32;
            let hub_score = if max_incoming > 0.0 {
                hub_raw / max_incoming
            } else {
                0.0
            };
            let relevance_score = SEARCH_WEIGHT * sr.score + HUB_WEIGHT * hub_score;

            DocRecommendation {
                uri: sr.uri,
                title: sr.title,
                relevance_score,
                search_score: sr.score,
                hub_score,
                matched_fields: sr.matched_fields,
                tags: sr.tags,
                document_summary: None,
                sections: None,
            }
        })
        .collect();

    // Sort by combined score descending, then URI for determinism.
    recommendations.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.uri.as_str().cmp(b.uri.as_str()))
    });

    recommendations.truncate(top_k);

    // Stage 3: Load sidecar summaries for the top-K results.
    for rec in &mut recommendations {
        if let Some(sidecar) = try_load_sidecar(&rec.uri, roots) {
            rec.document_summary = sidecar.document_summary.clone();
            if include_sections {
                rec.sections = Some(
                    sidecar
                        .sections
                        .iter()
                        .map(|s| RecommendedSection {
                            heading_path: s.heading_path.clone(),
                            level: s.level,
                            summary: s.summary.clone(),
                        })
                        .collect(),
                );
            }
        }
    }

    CoreOperationResult::Recommendations {
        realm: realm_key.to_string(),
        query: query.to_string(),
        results: recommendations,
    }
}

/// Extract search results from a `CoreOperationResult::WorkspaceSearchResults`.
fn extract_search_results(result: &CoreOperationResult) -> Vec<WorkspaceSearchResult> {
    match result {
        CoreOperationResult::WorkspaceSearchResults { results, .. } => results.clone(),
        _ => vec![],
    }
}

/// Extract hub URI → incoming_count map from a `CoreOperationResult::GraphAnalysis`.
fn extract_hub_map(result: &CoreOperationResult) -> HashMap<String, u32> {
    match result {
        CoreOperationResult::GraphAnalysis { hubs, .. } => hubs
            .iter()
            .map(|(uri, count)| (uri.as_str().to_string(), *count))
            .collect(),
        _ => HashMap::new(),
    }
}
