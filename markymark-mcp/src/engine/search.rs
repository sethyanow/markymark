//! SearchSymbols and SemanticSearch operation handlers.

use markymark_core::engine::CoreOperationResult;
#[cfg(feature = "semantic-search")]
use markymark_core::engine::SemanticSearchMatch;
use markymark_core::{CoreError, DocumentUri, Range};
use markymark_index::RealmIndex;
use markymark_kernels::{fuzzy_match, fuzzy_match_batch};

use crate::rename_ops::compare_ranges;

/// Sort symbol results by score (desc), starts_with (desc), name, uri, range.
fn sort_symbol_results(results: &mut [(i32, bool, String, DocumentUri, Range)]) {
    results.sort_by(
        |(score_a, starts_a, name_a, uri_a, range_a),
         (score_b, starts_b, name_b, uri_b, range_b)| {
            score_b
                .cmp(score_a)
                .then_with(|| starts_b.cmp(starts_a))
                .then_with(|| name_a.cmp(name_b))
                .then_with(|| uri_a.as_str().cmp(uri_b.as_str()))
                .then_with(|| compare_ranges(*range_a, *range_b))
        },
    );
}

pub(crate) fn handle_search_symbols(realm: &RealmIndex, query: String) -> CoreOperationResult {
    let query = query.trim().to_string();
    if query.is_empty() {
        return CoreOperationResult::Error(CoreError::Message(
            "search query cannot be empty".to_string(),
        ));
    }

    let mut candidates: Vec<(String, DocumentUri, Range)> = Vec::new();

    // Collect markdown heading candidates.
    for (uri, index) in realm.iter_documents() {
        for heading in index.headings() {
            candidates.push((heading.text.to_string(), uri.clone(), heading.range));
        }
    }

    // Collect structured key-path candidates (pre-filtered by search_key_paths,
    // unlike headings which are collected exhaustively then ranked by fuzzy score).
    for (uri, path, _key, _kind, range) in realm.search_key_paths(&query) {
        candidates.push((path, uri, range));
    }

    let candidate_refs: Vec<&str> = candidates
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    // Cap top_k to avoid O(n log n) heap degradation when all candidates are ranked.
    const TOP_K_LIMIT: usize = 100;
    let top_k = candidate_refs.len().min(TOP_K_LIMIT);

    let matches = match fuzzy_match_batch(&query, &candidate_refs, top_k) {
        Ok(ranked) => {
            let mut results: Vec<(i32, bool, String, DocumentUri, Range)> = ranked
                .into_iter()
                .filter(|m| m.score > 0)
                .filter_map(|m| {
                    candidates.get(m.index as usize).map(|(name, uri, range)| {
                        (m.score, m.starts_with, name.clone(), uri.clone(), *range)
                    })
                })
                .collect();
            sort_symbol_results(&mut results);
            results
                .into_iter()
                .map(|(_, _, name, uri, range)| (name, uri, range))
                .collect::<Vec<_>>()
        }
        Err(_) => {
            // Fallback path keeps previous per-candidate behavior.
            let mut scored_matches: Vec<(i32, bool, String, DocumentUri, Range)> = Vec::new();
            for (name, uri, range) in &candidates {
                if let Ok(m) = fuzzy_match(&query, name) {
                    if m.score > 0 {
                        scored_matches.push((
                            m.score,
                            m.starts_with,
                            name.clone(),
                            uri.clone(),
                            *range,
                        ));
                    }
                }
            }

            sort_symbol_results(&mut scored_matches);

            scored_matches
                .into_iter()
                .map(|(_, _, name, uri, range)| (name, uri, range))
                .collect()
        }
    };

    CoreOperationResult::Symbols(matches)
}

#[cfg(feature = "semantic-search")]
pub(crate) fn handle_semantic_search(
    realm: &RealmIndex,
    query: String,
    top_k: u32,
    min_score: f32,
) -> CoreOperationResult {
    let query = query.trim().to_string();
    if query.is_empty() {
        return CoreOperationResult::Error(CoreError::Message(
            "semantic query cannot be empty".to_string(),
        ));
    }

    let results = match realm.semantic_search(&query, top_k, min_score.clamp(0.0, 1.0)) {
        Ok(results) => results,
        Err(err) => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "semantic search failed: {err}"
            )));
        }
    };

    CoreOperationResult::SemanticMatches(
        results
            .into_iter()
            .map(|result| {
                let section_preview = super::helpers::preview_for_range(
                    &result.doc_uri,
                    result.section_range,
                    &result.heading,
                );
                SemanticSearchMatch {
                    doc_uri: result.doc_uri,
                    heading: result.heading,
                    heading_level: result.heading_level,
                    score: result.score,
                    section_range: result.section_range,
                    section_preview,
                }
            })
            .collect(),
    )
}
