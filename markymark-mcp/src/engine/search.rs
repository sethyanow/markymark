//! SearchSymbols and SemanticSearch operation handlers.

use std::borrow::Cow;

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

    // Collect document refs before building candidates so that the borrow on
    // each `&DocumentIndex` has a lifetime that spans the entire candidate
    // collection phase (not just a single loop iteration).
    let docs: Vec<(&DocumentUri, &markymark_index::DocumentIndex)> =
        realm.iter_documents().collect();

    // Candidates use Cow<str> to borrow heading text from the arena instead of
    // cloning every name upfront.  Only the final ranked results (≤ TOP_K_LIMIT)
    // are converted to owned Strings.
    let mut candidates: Vec<(Cow<'_, str>, DocumentUri, Range)> = Vec::new();

    // Collect markdown heading candidates — borrow text from arena, no clone.
    for (uri, index) in &docs {
        for heading in index.headings() {
            candidates.push((Cow::Borrowed(heading.text), (*uri).clone(), heading.range));
        }
    }

    // Collect code span candidates — borrow text from arena, no clone.
    // Dedup by text within each document to avoid flooding results.
    for (uri, index) in &docs {
        let mut seen = std::collections::HashSet::new();
        for cs in index.code_spans() {
            if seen.insert(cs.text) {
                candidates.push((Cow::Borrowed(cs.text), (*uri).clone(), cs.range));
            }
        }
    }

    // Collect structured key-path candidates (pre-filtered by search_key_paths,
    // unlike headings which are collected exhaustively then ranked by fuzzy score).
    for (uri, path, _key, _kind, range) in realm.search_key_paths(&query) {
        candidates.push((Cow::Owned(path), uri, range));
    }

    let candidate_refs: Vec<&str> = candidates
        .iter()
        .map(|(name, _, _)| name.as_ref())
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
                        (
                            m.score,
                            m.starts_with,
                            name.as_ref().to_string(),
                            uri.clone(),
                            *range,
                        )
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
                if let Ok(m) = fuzzy_match(&query, name.as_ref()) {
                    if m.score > 0 {
                        scored_matches.push((
                            m.score,
                            m.starts_with,
                            name.as_ref().to_string(),
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
pub(crate) async fn handle_semantic_search(
    semantic_index: std::sync::Arc<tokio::sync::Mutex<markymark_index::SemanticIndex>>,
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

    let results = {
        let guard = semantic_index.lock().await;
        match guard.search(&query, top_k, min_score.clamp(0.0, 1.0)).await {
            Ok(results) => results,
            Err(err) => {
                return CoreOperationResult::Error(CoreError::Message(format!(
                    "semantic search failed: {err}"
                )));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_index::DocumentIndex;
    use std::path::PathBuf;

    fn uri(name: &str) -> DocumentUri {
        DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}")))
    }

    fn symbol_names(result: CoreOperationResult) -> Vec<String> {
        match result {
            CoreOperationResult::Symbols(matches) => {
                matches.into_iter().map(|(name, _, _)| name).collect()
            }
            other => panic!("expected Symbols result, got: {other:?}"),
        }
    }

    /// Build a DocumentIndex with code spans by embedding backtick references in source.
    fn make_index_with_code_spans(code_span_texts: &[&str]) -> DocumentIndex {
        let mut source = String::from("# Types\n\n");
        for text in code_span_texts {
            source.push('`');
            source.push_str(text);
            source.push_str("` ");
        }
        source.push('\n');
        DocumentIndex::from_text(&source)
    }

    #[tokio::test]
    async fn search_symbols_includes_code_span_candidates() {
        let mut realm = RealmIndex::new();
        let index = make_index_with_code_spans(&["HashMap"]);
        realm.add_document(uri("types.md"), index).await;

        let names = symbol_names(handle_search_symbols(&realm, "HashMap".to_string()));
        assert!(
            names.iter().any(|n| n == "HashMap"),
            "expected code span 'HashMap' in search results, got: {names:?}"
        );
    }

    #[tokio::test]
    async fn search_symbols_dedup_code_spans_per_document() {
        let mut realm = RealmIndex::new();
        // Build source with 3 occurrences of `Result` — dedup should produce 1 entry
        let index = make_index_with_code_spans(&["Result", "Result", "Result"]);
        realm.add_document(uri("results.md"), index).await;

        let names = symbol_names(handle_search_symbols(&realm, "Result".to_string()));
        let result_count = names.iter().filter(|n| *n == "Result").count();
        assert_eq!(
            result_count, 1,
            "same text 3x in one doc should produce 1 candidate, got {result_count}"
        );
    }
}
