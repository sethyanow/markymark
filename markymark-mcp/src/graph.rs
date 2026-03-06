//! Graph analysis for the link graph of a markdown workspace.
//!
//! Provides orphan detection, hub detection, broken link detection,
//! cluster analysis, and summary statistics.

use std::collections::{HashMap, HashSet};

use markymark_core::{engine::CoreOperationResult, DocumentUri};
use markymark_index::RealmIndex;

/// Analyse the link graph of a realm.
///
/// # Algorithm
/// 1. Build a stem→URI lookup for wiki link resolution.
/// 2. For each document, resolve outgoing wiki links and local markdown links.
/// 3. Unresolvable links are broken links; resolved links contribute to in-degree.
/// 4. Orphans: docs with in-degree == 0 AND out-degree == 0.
/// 5. Hubs: top `top_n_hubs` docs by in-degree, sorted descending.
/// 6. Clusters: weakly-connected components via union-find (optional).
pub fn execute_graph_analysis(
    realm_key: &str,
    realm: &RealmIndex,
    top_n_hubs: u32,
    include_clusters: bool,
) -> CoreOperationResult {
    // Step 1: collect all document URIs and build stem→URI for wiki link resolution.
    let mut stem_to_uri: HashMap<String, DocumentUri> = HashMap::new();
    let mut all_uris: Vec<DocumentUri> = Vec::new();

    for (uri, _doc) in realm.iter_documents() {
        if let Some(path) = uri.to_file_path() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                stem_to_uri.insert(stem.to_lowercase(), uri.clone());
            }
        }
        all_uris.push(uri.clone());
    }

    let doc_uri_set: HashSet<String> = all_uris.iter().map(|u| u.as_str().to_string()).collect();

    // Step 2: build adjacency (out-edges), in-degree, and broken links.
    // out_degree tracks how many *resolved* links each document has.
    let mut out_degree: HashMap<String, u32> = HashMap::new();
    let mut in_degree: HashMap<String, u32> = HashMap::new();
    // adjacency list of resolved edges for cluster analysis
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut broken_links: Vec<(DocumentUri, String, String)> = Vec::new();
    let mut total_internal_links: u32 = 0;

    // Initialise counters for every known document.
    for uri in &all_uris {
        let key = uri.as_str().to_string();
        out_degree.entry(key.clone()).or_insert(0);
        in_degree.entry(key.clone()).or_insert(0);
        adj.entry(key).or_default();
    }

    for (uri, doc) in realm.iter_documents() {
        let source = uri.as_str().to_string();

        // Track seen (source, resolved_target) pairs to deduplicate edges.
        // Multiple occurrences of the same link within one document should
        // count as a single edge (marky-agv).
        let mut seen_targets: HashSet<String> = HashSet::new();

        // --- Wiki links ---
        for wl in doc.wiki_links() {
            let target = wl.target;
            // Strip heading anchor (PageName#Heading).
            let stem = target.split('#').next().unwrap_or(target).trim();
            if stem.is_empty() {
                // Self-heading link like [[#Heading]]; skip.
                continue;
            }
            match stem_to_uri.get(&stem.to_lowercase()) {
                Some(resolved) => {
                    let resolved_str = resolved.as_str().to_string();
                    if seen_targets.insert(resolved_str.clone()) {
                        *in_degree.entry(resolved_str.clone()).or_insert(0) += 1;
                        *out_degree.entry(source.clone()).or_insert(0) += 1;
                        adj.entry(source.clone())
                            .or_default()
                            .push(resolved_str.clone());
                        adj.entry(resolved_str).or_default().push(source.clone());
                        total_internal_links += 1;
                    }
                }
                None => {
                    broken_links.push((uri.clone(), target.to_string(), "wiki".to_string()));
                }
            }
        }

        // --- Markdown links (internal only, path-based resolution) ---
        for ml in doc.markdown_links() {
            let url = ml.url;
            // Skip external and anchor-only links.
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with('#')
                || url.is_empty()
            {
                continue;
            }
            match crate::engine::helpers::resolve_markdown_link(uri, url, &doc_uri_set) {
                Some(resolved_str) => {
                    if seen_targets.insert(resolved_str.clone()) {
                        *in_degree.entry(resolved_str.clone()).or_insert(0) += 1;
                        *out_degree.entry(source.clone()).or_insert(0) += 1;
                        adj.entry(source.clone())
                            .or_default()
                            .push(resolved_str.clone());
                        adj.entry(resolved_str).or_default().push(source.clone());
                        total_internal_links += 1;
                    }
                }
                None => {
                    broken_links.push((uri.clone(), url.to_string(), "markdown".to_string()));
                }
            }
        }
    }

    // Step 3: Orphan detection — zero in AND zero out (resolved).
    let mut orphans: Vec<DocumentUri> = all_uris
        .iter()
        .filter(|uri| {
            let key = uri.as_str();
            in_degree.get(key).copied().unwrap_or(0) == 0
                && out_degree.get(key).copied().unwrap_or(0) == 0
        })
        .cloned()
        .collect();
    orphans.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    // Step 4: Hub detection — top_n_hubs by in-degree, descending.
    let mut sorted_hubs: Vec<(DocumentUri, u32)> = all_uris
        .iter()
        .map(|uri| {
            let count = in_degree.get(uri.as_str()).copied().unwrap_or(0);
            (uri.clone(), count)
        })
        .collect();
    sorted_hubs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.as_str().cmp(b.0.as_str())));
    let n = top_n_hubs as usize;
    let hubs: Vec<(DocumentUri, u32)> = sorted_hubs.into_iter().take(n).collect();

    // Step 5: Cluster analysis (optional).
    let clusters = if include_clusters {
        Some(find_weakly_connected(&doc_uri_set, &adj))
    } else {
        None
    };

    CoreOperationResult::GraphAnalysis {
        realm: realm_key.to_string(),
        total_docs: all_uris.len() as u32,
        total_internal_links,
        orphans,
        hubs,
        broken_links,
        clusters,
    }
}

/// Union-find weakly-connected component detection.
fn find_weakly_connected(
    nodes: &HashSet<String>,
    adj: &HashMap<String, Vec<String>>,
) -> Vec<Vec<DocumentUri>> {
    let node_vec: Vec<&String> = nodes.iter().collect();
    let n = node_vec.len();
    if n == 0 {
        return vec![];
    }

    // Map node → index.
    let index_of: HashMap<&str, usize> = node_vec
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    fn union(parent: &mut Vec<usize>, a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    for (src, neighbours) in adj {
        if let Some(&si) = index_of.get(src.as_str()) {
            for dst in neighbours {
                if let Some(&di) = index_of.get(dst.as_str()) {
                    union(&mut parent, si, di);
                }
            }
        }
    }

    // Group by root.
    let mut components: HashMap<usize, Vec<DocumentUri>> = HashMap::new();
    for (i, uri_str) in node_vec.iter().enumerate() {
        let root = find(&mut parent, i);
        let uri = DocumentUri::new(uri_str).unwrap_or_else(|_| {
            // Fallback: this shouldn't happen since these came from DocumentUri::as_str().
            DocumentUri::from_file_path(std::path::Path::new(uri_str.as_str()))
        });
        components.entry(root).or_default().push(uri);
    }

    let mut result: Vec<Vec<DocumentUri>> = components.into_values().collect();
    // Sort clusters by size descending, then by first URI for determinism.
    result.sort_by(|a, b| {
        b.len().cmp(&a.len()).then_with(|| {
            a.first()
                .map(|u| u.as_str())
                .cmp(&b.first().map(|u| u.as_str()))
        })
    });
    result
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use markymark_core::{engine::CoreOperationResult, DocumentUri};
    use markymark_index::{document::DocumentIndex, RealmIndex};

    fn uri(name: &str) -> DocumentUri {
        DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}.md")))
    }

    fn make_index(source: &str) -> DocumentIndex {
        let ast = markymark_parser::parse(source).unwrap();
        DocumentIndex::from_ast(ast)
    }

    async fn make_realm(docs: &[(&str, &str)]) -> RealmIndex {
        let mut realm = RealmIndex::new();
        for (name, source) in docs {
            realm.add_document(uri(name), make_index(source)).await;
        }
        realm
    }

    #[tokio::test]
    async fn empty_realm_returns_zero_stats() {
        let realm = RealmIndex::new();
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                total_docs,
                total_internal_links,
                orphans,
                hubs,
                broken_links,
                clusters,
                ..
            } => {
                assert_eq!(total_docs, 0);
                assert_eq!(total_internal_links, 0);
                assert!(orphans.is_empty());
                assert!(hubs.is_empty());
                assert!(broken_links.is_empty());
                assert!(clusters.is_none());
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn isolated_document_is_orphan() {
        let realm = make_realm(&[("lonely", "# Lonely\nNo links here.")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { orphans, .. } => {
                assert_eq!(orphans.len(), 1);
                assert_eq!(orphans[0], uri("lonely"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn linked_documents_are_not_orphans() {
        let realm = make_realm(&[("a", "[[b]]"), ("b", "# B\nNo outgoing links.")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { orphans, .. } => {
                // b has in-degree 1, a has out-degree 1; neither is orphan.
                assert!(orphans.is_empty(), "orphans: {orphans:?}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn broken_wiki_link_detected() {
        let realm = make_realm(&[("a", "[[nonexistent]]")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                broken_links,
                orphans,
                ..
            } => {
                // a has a broken outgoing link → no resolved links → still an orphan.
                assert_eq!(broken_links.len(), 1);
                let (src, target, kind) = &broken_links[0];
                assert_eq!(src, &uri("a"));
                assert_eq!(target, "nonexistent");
                assert_eq!(kind, "wiki");
                assert_eq!(orphans.len(), 1);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn hub_sorted_by_incoming_count() {
        // b is linked from both a and c.
        let realm = make_realm(&[("a", "[[b]]"), ("b", "# B"), ("c", "[[b]]")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { hubs, .. } => {
                assert!(!hubs.is_empty());
                let (top_uri, top_count) = &hubs[0];
                assert_eq!(top_uri, &uri("b"));
                assert_eq!(*top_count, 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn top_n_hubs_limit_respected() {
        let realm = make_realm(&[("a", "[[c]]"), ("b", "[[c]]"), ("c", "# C"), ("d", "# D")]).await;
        match super::execute_graph_analysis("default", &realm, 2, false) {
            CoreOperationResult::GraphAnalysis { hubs, .. } => {
                assert!(hubs.len() <= 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn external_markdown_links_are_not_broken() {
        let realm = make_realm(&[(
            "a",
            "[Claude](https://claude.ai)\n[Other](http://example.com)",
        )])
        .await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { broken_links, .. } => {
                assert!(
                    broken_links.is_empty(),
                    "external links should not be broken: {broken_links:?}"
                );
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn clusters_none_when_not_requested() {
        let realm = make_realm(&[("a", "# A"), ("b", "# B")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { clusters, .. } => {
                assert!(clusters.is_none());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn clusters_detected_when_requested() {
        // a→b linked; c isolated → 2 weakly-connected components.
        let realm = make_realm(&[("a", "[[b]]"), ("b", "# B"), ("c", "# Isolated")]).await;
        match super::execute_graph_analysis("default", &realm, 10, true) {
            CoreOperationResult::GraphAnalysis { clusters, .. } => {
                let clusters = clusters.expect("clusters should be Some");
                assert_eq!(clusters.len(), 2, "expected 2 clusters: {clusters:?}");
                // Largest cluster should have 2 members.
                assert_eq!(clusters[0].len(), 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn internal_markdown_link_resolved() {
        let realm = make_realm(&[("a", "[link](b.md)"), ("b", "# B")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                broken_links,
                total_internal_links,
                ..
            } => {
                assert!(broken_links.is_empty(), "broken: {broken_links:?}");
                assert_eq!(total_internal_links, 1);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn duplicate_wiki_links_within_document_count_once() {
        // marky-agv: [[b]] appears 3 times in document a.
        // In-degree for b should be 1 (one unique source), not 3.
        let realm = make_realm(&[
            ("a", "[[b]]\n\nSome text.\n\n[[b]]\n\nMore text.\n\n[[b]]"),
            ("b", "# B"),
        ])
        .await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                hubs,
                total_internal_links,
                ..
            } => {
                let b_hub = hubs
                    .iter()
                    .find(|(u, _)| u == &uri("b"))
                    .expect("b should appear in hubs");
                assert_eq!(
                    b_hub.1, 1,
                    "duplicate wiki links within same document should count once, got {}",
                    b_hub.1
                );
                assert_eq!(
                    total_internal_links, 1,
                    "total_internal_links should count deduplicated edges, got {}",
                    total_internal_links
                );
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn duplicate_markdown_links_within_document_count_once() {
        // marky-agv: [link](b.md) appears twice in document a.
        let realm =
            make_realm(&[("a", "[link](b.md)\n\ntext\n\n[other](b.md)"), ("b", "# B")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                hubs,
                total_internal_links,
                ..
            } => {
                let b_hub = hubs
                    .iter()
                    .find(|(u, _)| u == &uri("b"))
                    .expect("b should appear in hubs");
                assert_eq!(
                    b_hub.1, 1,
                    "duplicate markdown links within same document should count once, got {}",
                    b_hub.1
                );
                assert_eq!(
                    total_internal_links, 1,
                    "total_internal_links should count deduplicated edges, got {}",
                    total_internal_links
                );
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn unique_sources_still_count_separately() {
        // a→b and c→b are distinct edges; b should have in-degree 2.
        let realm = make_realm(&[("a", "[[b]]"), ("b", "# B"), ("c", "[[b]]")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                hubs,
                total_internal_links,
                ..
            } => {
                let b_hub = hubs
                    .iter()
                    .find(|(u, _)| u == &uri("b"))
                    .expect("b should appear in hubs");
                assert_eq!(b_hub.1, 2, "two unique sources should give in-degree 2");
                assert_eq!(total_internal_links, 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn self_heading_wiki_link_skipped() {
        // [[#Heading]] is a self-heading anchor; should not appear as a broken link.
        let realm = make_realm(&[("a", "# A\n[[#A]]")]).await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis { broken_links, .. } => {
                assert!(broken_links.is_empty(), "broken: {broken_links:?}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    /// Helper to create a realm with docs at specific paths (not flat /vault/).
    async fn make_realm_with_paths(docs: &[(&str, &str)]) -> RealmIndex {
        let mut realm = RealmIndex::new();
        for (path, source) in docs {
            let doc_uri = DocumentUri::from_file_path(&PathBuf::from(path));
            realm.add_document(doc_uri, make_index(source)).await;
        }
        realm
    }

    #[tokio::test]
    async fn path_based_markdown_link_resolution() {
        // docs/a/index.md links to ../b/page.md
        // docs/b/page.md exists
        // docs/c/page.md also exists (same stem!)
        // The link should resolve to docs/b/page.md, NOT docs/c/page.md.
        let realm = make_realm_with_paths(&[
            ("/docs/a/index.md", "[link](../b/page.md)"),
            ("/docs/b/page.md", "# Page B"),
            ("/docs/c/page.md", "# Page C"),
        ])
        .await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                broken_links,
                total_internal_links,
                hubs,
                ..
            } => {
                assert!(broken_links.is_empty(), "broken: {broken_links:?}");
                assert_eq!(
                    total_internal_links, 1,
                    "should resolve exactly one internal link"
                );
                // docs/b/page.md should have in-degree 1 (linked from a/index.md)
                let b_uri = DocumentUri::from_file_path(&PathBuf::from("/docs/b/page.md"));
                let b_hub = hubs.iter().find(|(u, _)| u == &b_uri);
                assert!(
                    b_hub.is_some(),
                    "docs/b/page.md should be in hubs list"
                );
                assert_eq!(b_hub.unwrap().1, 1, "docs/b/page.md should have in-degree 1");

                // docs/c/page.md should NOT have in-degree (link was NOT to it)
                let c_uri = DocumentUri::from_file_path(&PathBuf::from("/docs/c/page.md"));
                let c_hub = hubs.iter().find(|(u, _)| u == &c_uri);
                if let Some((_, count)) = c_hub {
                    assert_eq!(*count, 0, "docs/c/page.md should have in-degree 0");
                }
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn same_stem_different_dirs_not_confused() {
        // root/index.md links to sub/README.md
        // root/sub/README.md exists
        // root/other/README.md also exists (same stem)
        // Only root/sub/README.md should get in-degree.
        let realm = make_realm_with_paths(&[
            ("/root/index.md", "[link](sub/README.md)"),
            ("/root/sub/README.md", "# Sub README"),
            ("/root/other/README.md", "# Other README"),
        ])
        .await;
        match super::execute_graph_analysis("default", &realm, 10, false) {
            CoreOperationResult::GraphAnalysis {
                broken_links,
                total_internal_links,
                hubs,
                ..
            } => {
                assert!(broken_links.is_empty(), "broken: {broken_links:?}");
                assert_eq!(total_internal_links, 1);

                let sub_uri =
                    DocumentUri::from_file_path(&PathBuf::from("/root/sub/README.md"));
                let sub_hub = hubs.iter().find(|(u, _)| u == &sub_uri);
                assert!(sub_hub.is_some(), "sub/README.md should be in hubs");
                assert_eq!(sub_hub.unwrap().1, 1, "sub/README.md in-degree should be 1");

                let other_uri =
                    DocumentUri::from_file_path(&PathBuf::from("/root/other/README.md"));
                let other_hub = hubs.iter().find(|(u, _)| u == &other_uri);
                if let Some((_, count)) = other_hub {
                    assert_eq!(
                        *count, 0,
                        "other/README.md should have in-degree 0, got {count}"
                    );
                }
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
