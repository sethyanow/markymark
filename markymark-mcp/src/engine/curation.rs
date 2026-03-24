//! Engine handler for the `curation-diagnostics` MCP tool.
//!
//! Composes graph-analysis (orphans, hubs, broken links) with connectivity
//! scoring to produce a unified curation report with actionable suggestions.

use markymark_core::engine::{
    ConnectivityDoc, CoreOperationResult, CurationReportData, CurationStats, CurationSuggestion,
    CurationSuggestionType,
};
use markymark_core::DocumentUri;
use markymark_index::RealmIndex;

use std::collections::HashMap;

/// Low-connectivity threshold: documents with fewer than this many total links
/// (in + out) AND below the median are flagged.
const LOW_CONNECTIVITY_THRESHOLD: u32 = 2;

/// Execute curation diagnostics on a realm.
///
/// Composes graph-analysis results to produce:
/// 1. Orphan detection (in-degree == 0 AND out-degree == 0)
/// 2. Connectivity scoring per document
/// 3. Cross-link suggestions (orphans → nearest hubs)
pub fn handle_curation_diagnostics(
    realm_key: &str,
    realm: &RealmIndex,
    include_suggestions: bool,
    max_suggestions: u32,
    max_items_per_category: u32,
) -> CoreOperationResult {
    // Compose graph-analysis to get orphans, hubs, and link stats.
    let graph_result = crate::graph::execute_graph_analysis(realm_key, realm, 100, false);

    let gd = extract_graph_data(&graph_result, realm);

    // Compute connectivity per document.
    let mut connectivities: Vec<(DocumentUri, u32, u32, u32)> = Vec::new();
    for (uri, _doc) in realm.iter_documents() {
        let uri_str = uri.as_str().to_string();
        let in_d = gd.in_degree.get(&uri_str).copied().unwrap_or(0);
        let out_d = gd.out_degree.get(&uri_str).copied().unwrap_or(0);
        connectivities.push((uri.clone(), in_d + out_d, in_d, out_d));
    }

    // Compute median connectivity.
    let median = compute_median(&connectivities);

    // Identify low-connectivity documents.
    let mut low_connectivity: Vec<ConnectivityDoc> = connectivities
        .iter()
        .filter(|(_, conn, _, _)| *conn < LOW_CONNECTIVITY_THRESHOLD && (*conn as f32) < median)
        .map(|(uri, conn, in_d, out_d)| ConnectivityDoc {
            uri: uri.clone(),
            connectivity: *conn,
            in_degree: *in_d,
            out_degree: *out_d,
        })
        .collect();
    low_connectivity.sort_by(|a, b| {
        a.connectivity
            .cmp(&b.connectivity)
            .then_with(|| a.uri.as_str().cmp(b.uri.as_str()))
    });

    // Cap categories.
    let max_cat = max_items_per_category as usize;
    let mut capped_orphans: Vec<DocumentUri> = gd.orphans;
    capped_orphans.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    capped_orphans.truncate(max_cat);
    low_connectivity.truncate(max_cat);

    // Generate suggestions.
    let suggestions = if include_suggestions && !capped_orphans.is_empty() && !gd.hubs.is_empty() {
        generate_suggestions(&capped_orphans, &gd.hubs, max_suggestions as usize)
    } else {
        vec![]
    };

    // Compute stats.
    let orphan_count = capped_orphans.len() as u32;
    let orphan_percentage = if gd.total_docs > 0 {
        (orphan_count as f32 / gd.total_docs as f32) * 100.0
    } else {
        0.0
    };

    let avg_connectivity = if connectivities.is_empty() {
        0.0
    } else {
        connectivities
            .iter()
            .map(|(_, c, _, _)| *c as f32)
            .sum::<f32>()
            / connectivities.len() as f32
    };

    let stats = CurationStats {
        total_docs: gd.total_docs,
        orphan_count,
        orphan_percentage,
        avg_connectivity,
        median_connectivity: median,
        broken_link_count: gd.broken_link_count,
    };

    CoreOperationResult::CurationReport {
        realm: realm_key.to_string(),
        report: CurationReportData {
            orphan_docs: capped_orphans,
            low_connectivity_docs: low_connectivity,
            suggestions,
            stats,
        },
    }
}

/// Extracted data from graph analysis plus computed degree maps.
struct GraphData {
    total_docs: u32,
    orphans: Vec<DocumentUri>,
    hubs: Vec<(DocumentUri, u32)>,
    broken_link_count: u32,
    in_degree: HashMap<String, u32>,
    out_degree: HashMap<String, u32>,
}

/// Extract data from a GraphAnalysis result plus compute per-doc degree maps.
fn extract_graph_data(result: &CoreOperationResult, realm: &RealmIndex) -> GraphData {
    let (total_docs, orphans, hubs, broken_link_count) = match result {
        CoreOperationResult::GraphAnalysis {
            total_docs,
            orphans,
            hubs,
            broken_links,
            ..
        } => (
            *total_docs,
            orphans.clone(),
            hubs.clone(),
            broken_links.len() as u32,
        ),
        _ => (0, vec![], vec![], 0),
    };

    // Recompute per-document degree maps from the realm index.
    // We need per-doc in/out degree which graph-analysis computes internally
    // but doesn't expose. Rebuild it from the same data.
    let (in_degree, out_degree) = compute_degree_maps(realm);

    GraphData {
        total_docs,
        orphans,
        hubs,
        broken_link_count,
        in_degree,
        out_degree,
    }
}

/// Compute in-degree and out-degree maps from the realm index.
///
/// This mirrors the logic in `graph::execute_graph_analysis` for link resolution
/// but only extracts the degree counts.
fn compute_degree_maps(realm: &RealmIndex) -> (HashMap<String, u32>, HashMap<String, u32>) {
    use std::collections::HashSet;

    let mut stem_to_uri: HashMap<String, String> = HashMap::new();
    let mut doc_uri_set: HashSet<String> = HashSet::new();
    let mut in_degree: HashMap<String, u32> = HashMap::new();
    let mut out_degree: HashMap<String, u32> = HashMap::new();

    // Build stem lookup and initialize counters.
    for (uri, _doc) in realm.iter_documents() {
        let uri_str = uri.as_str().to_string();
        if let Some(path) = uri.to_file_path() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                stem_to_uri.insert(stem.to_lowercase(), uri_str.clone());
            }
        }
        doc_uri_set.insert(uri_str.clone());
        in_degree.insert(uri_str.clone(), 0);
        out_degree.insert(uri_str, 0);
    }

    // Resolve links and count degrees.
    for (uri, doc) in realm.iter_documents() {
        let source = uri.as_str().to_string();
        let mut seen_targets: HashSet<String> = HashSet::new();

        // Wiki links.
        for wl in doc.wiki_links() {
            let stem = wl.target.split('#').next().unwrap_or(wl.target).trim();
            if stem.is_empty() {
                continue;
            }
            if let Some(resolved) = stem_to_uri.get(&stem.to_lowercase()) {
                if seen_targets.insert(resolved.clone()) {
                    *in_degree.entry(resolved.clone()).or_insert(0) += 1;
                    *out_degree.entry(source.clone()).or_insert(0) += 1;
                }
            }
        }

        // Markdown links (local only, path-based resolution via shared helper).
        for ml in doc.markdown_links() {
            let url = ml.url;
            if url.starts_with("http://") || url.starts_with("https://") || url.starts_with('#') {
                continue;
            }
            if let Some(resolved) = super::helpers::resolve_markdown_link(uri, url, &doc_uri_set) {
                if seen_targets.insert(resolved.clone()) {
                    *in_degree.entry(resolved).or_insert(0) += 1;
                    *out_degree.entry(source.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    (in_degree, out_degree)
}

/// Compute median of connectivity values.
fn compute_median(connectivities: &[(DocumentUri, u32, u32, u32)]) -> f32 {
    if connectivities.is_empty() {
        return 0.0;
    }
    let mut vals: Vec<u32> = connectivities.iter().map(|(_, c, _, _)| *c).collect();
    vals.sort_unstable();
    let len = vals.len();
    if len.is_multiple_of(2) {
        (vals[len / 2 - 1] as f32 + vals[len / 2] as f32) / 2.0
    } else {
        vals[len / 2] as f32
    }
}

/// Generate cross-link suggestions for orphan documents.
///
/// Algorithm: for each orphan, suggest linking to the top hub(s) that share
/// the same directory tree or have overlapping heading keywords.
/// Complexity: O(orphans * hubs) — bounded by max_suggestions.
fn generate_suggestions(
    orphans: &[DocumentUri],
    hubs: &[(DocumentUri, u32)],
    max_suggestions: usize,
) -> Vec<CurationSuggestion> {
    let mut suggestions = Vec::new();

    for orphan in orphans {
        if suggestions.len() >= max_suggestions {
            break;
        }

        // Find the best hub to link to.
        // Prefer hubs in the same directory, then fall back to highest-degree hub.
        // Skip hubs that are the orphan itself (can't cross-link to self).
        let orphan_dir = orphan
            .to_file_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let best_hub = hubs
            .iter()
            .filter(|(hub_uri, _)| hub_uri.as_str() != orphan.as_str())
            .max_by_key(|(hub_uri, count)| {
                let same_dir = hub_uri
                    .to_file_path()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    == orphan_dir;
                // Score: same directory gets a bonus of 1000, plus the hub count.
                let dir_bonus: u32 = if same_dir { 1000 } else { 0 };
                dir_bonus + count
            });

        if let Some((hub_uri, _count)) = best_hub {
            let hub_name = hub_uri
                .to_file_path()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| hub_uri.as_str().to_string());
            let orphan_name = orphan
                .to_file_path()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| orphan.as_str().to_string());

            suggestions.push(CurationSuggestion {
                source_doc: orphan.clone(),
                target_doc: hub_uri.clone(),
                reason: format!(
                    "{orphan_name} is an orphan document with no links; consider adding a cross-reference to {hub_name} (a hub document)"
                ),
                suggestion_type: CurationSuggestionType::ReduceOrphan,
            });
        }
    }

    suggestions
}
