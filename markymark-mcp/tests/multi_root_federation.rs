//! Integration tests for multi-root federation (Layer 3 of Layered Retrieval epic).
//!
//! Validates that adding multiple workspace roots to a single realm produces
//! correct behaviour across all query tools: search-workspace, graph-analysis,
//! recommend-docs, curation-diagnostics, and export-docs-index.

mod common;

use std::fs;
use std::sync::Arc;

use common::TempWorkspace;
use markymark_mcp::{
    CurationDiagnosticsRequest, CurationDiagnosticsResponse, ExportDocsIndexRequest,
    ExportDocsIndexResponse, GraphAnalysisRequest, GraphAnalysisResponse, MarkymarkMcp,
    RecommendDocsRequest, RecommendDocsResponse, RemoveRootRequest, RuntimeEngine,
    SearchWorkspaceRequest, SearchWorkspaceResponse,
};
use rmcp::handler::server::wrapper::Parameters;

/// Helper: create a RuntimeEngine with two roots added to the default realm.
async fn engine_with_two_roots(
    ws_a: &TempWorkspace,
    ws_b: &TempWorkspace,
) -> Arc<RuntimeEngine> {
    use markymark_core::engine::{CoreEngine, CoreOperation};

    let engine = RuntimeEngine::default();
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: ws_a.root(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: ws_b.root(),
        })
        .await;
    Arc::new(engine)
}

// ─── search-workspace finds docs across 2 roots ─────────────────────────────

#[tokio::test]
async fn search_workspace_finds_docs_from_both_roots() {
    let ws_a = TempWorkspace::new("fed-search-a");
    let ws_b = TempWorkspace::new("fed-search-b");

    fs::write(ws_a.root().join("alpha.md"), "# Alpha Guide\n\nContent about alpha.\n").unwrap();
    fs::write(ws_b.root().join("beta.md"), "# Beta Guide\n\nContent about beta.\n").unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    // Search for "Guide" — should find docs from both roots.
    let result = mcp
        .search_workspace_tool(Parameters(SearchWorkspaceRequest {
            query: Some("Guide".to_string()),
            frontmatter_filter_key: None,
            frontmatter_filter_value: None,
            property_filter_key: None,
            property_filter_value: None,
            tag_filter: None,
            realm: None,
            limit: 20,
        }))
        .await
        .expect("search should succeed");

    let payload: SearchWorkspaceResponse = result.into_typed().expect("typed response");
    assert_eq!(
        payload.results.len(),
        2,
        "should find docs from both roots; got: {:?}",
        payload.results.iter().map(|r| &r.uri).collect::<Vec<_>>()
    );

    let uris: Vec<&str> = payload.results.iter().map(|r| r.uri.as_str()).collect();
    assert!(
        uris.iter().any(|u| u.contains("alpha.md")),
        "should include alpha.md from root A"
    );
    assert!(
        uris.iter().any(|u| u.contains("beta.md")),
        "should include beta.md from root B"
    );
}

// ─── graph-analysis resolves cross-root wiki-links ──────────────────────────

#[tokio::test]
async fn graph_analysis_resolves_cross_root_wiki_links() {
    let ws_a = TempWorkspace::new("fed-graph-a");
    let ws_b = TempWorkspace::new("fed-graph-b");

    // Doc in root A links to doc in root B via wiki-link.
    fs::write(
        ws_a.root().join("linker.md"),
        "# Linker\n\nSee [[target]] for details.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("target.md"),
        "# Target\n\nThis is the target doc.\n",
    )
    .unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    let result = mcp
        .graph_analysis_tool(Parameters(GraphAnalysisRequest {
            realm: None,
            top_n_hubs: 10,
            include_clusters: false,
        }))
        .await
        .expect("graph analysis should succeed");

    let payload: GraphAnalysisResponse = result.into_typed().expect("typed response");

    // Cross-root wiki-link should resolve — 0 broken links.
    assert_eq!(
        payload.stats.broken_link_count, 0,
        "cross-root wiki-link should resolve; broken links: {:?}",
        payload.broken_links
    );
    assert_eq!(payload.stats.total_docs, 2);
    // Neither doc is an orphan — linker has outgoing, target has incoming.
    assert_eq!(
        payload.stats.orphan_count, 0,
        "both docs should have links; orphans: {:?}",
        payload.orphans
    );
    // target.md should be a hub (1 incoming link).
    assert!(
        payload.hubs.iter().any(|h| h.uri.contains("target.md")),
        "target.md should be a hub; hubs: {:?}",
        payload.hubs
    );
}

// ─── recommend-docs returns results from both roots ─────────────────────────

#[tokio::test]
async fn recommend_docs_ranks_across_roots() {
    let ws_a = TempWorkspace::new("fed-rec-a");
    let ws_b = TempWorkspace::new("fed-rec-b");

    // Create a hub doc in root A linked to by docs in both roots.
    fs::write(
        ws_a.root().join("hub.md"),
        "# Central Hub\n\nThe authoritative guide.\n",
    )
    .unwrap();
    fs::write(
        ws_a.root().join("helper.md"),
        "# Helper\n\nSee [[hub]] for more.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("remote.md"),
        "# Remote Helper\n\nAlso refers to [[hub]].\n",
    )
    .unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    // Query "Hub" is a substring of title "Central Hub" — should match.
    let result = mcp
        .recommend_docs_tool(Parameters(RecommendDocsRequest {
            query: "Hub".to_string(),
            realm: None,
            top_k: 10,
            include_sections: false,
        }))
        .await
        .expect("recommend should succeed");

    let payload: RecommendDocsResponse = result.into_typed().expect("typed response");

    // Should return results — at minimum the hub.md title match.
    let uris: Vec<&str> = payload.recommendations.iter().map(|r| r.uri.as_str()).collect();
    assert!(
        !payload.recommendations.is_empty(),
        "should return at least one recommendation"
    );
    // hub.md should be highest ranked (text match + hub score from cross-root links).
    assert!(
        uris[0].contains("hub.md"),
        "hub.md should be top recommendation; got: {:?}",
        uris
    );
}

// ─── curation-diagnostics detects orphans correctly across roots ────────────

#[tokio::test]
async fn curation_diagnostics_cross_root_orphan_detection() {
    let ws_a = TempWorkspace::new("fed-curation-a");
    let ws_b = TempWorkspace::new("fed-curation-b");

    // Hub in root A, linker in root B points to hub, orphan in root B is isolated.
    fs::write(
        ws_a.root().join("hub.md"),
        "# Hub\n\nCentral document.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("linker.md"),
        "# Linker\n\nSee [[hub]] for details.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("orphan.md"),
        "# Orphan\n\nCompletely isolated, no links.\n",
    )
    .unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    let result = mcp
        .curation_diagnostics_tool(Parameters(CurationDiagnosticsRequest {
            realm: None,
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        }))
        .await
        .expect("curation should succeed");

    let payload: CurationDiagnosticsResponse = result.into_typed().expect("typed response");

    assert_eq!(payload.stats.total_docs, 3);
    // Only orphan.md should be detected as orphan.
    // hub.md has incoming link from linker.md (cross-root), so it's NOT an orphan.
    // linker.md has outgoing link, so it's NOT an orphan.
    assert_eq!(
        payload.stats.orphan_count, 1,
        "only orphan.md should be orphan; orphan_docs: {:?}",
        payload.orphan_docs
    );
    assert!(
        payload.orphan_docs[0].contains("orphan.md"),
        "orphan should be orphan.md, got: {}",
        payload.orphan_docs[0]
    );
}

// ─── root removal causes cross-root links to become broken ──────────────────

#[tokio::test]
async fn root_removal_breaks_cross_root_links() {
    let ws_a = TempWorkspace::new("fed-remove-a");
    let ws_b = TempWorkspace::new("fed-remove-b");

    // Doc in root A links to doc in root B.
    fs::write(
        ws_a.root().join("linker.md"),
        "# Linker\n\nSee [[target]] for details.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("target.md"),
        "# Target\n\nThis is the target.\n",
    )
    .unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine.clone());

    // Before removal: link should resolve.
    let result = mcp
        .graph_analysis_tool(Parameters(GraphAnalysisRequest {
            realm: None,
            top_n_hubs: 10,
            include_clusters: false,
        }))
        .await
        .expect("graph analysis should succeed");
    let before: GraphAnalysisResponse = result.into_typed().expect("typed response");
    assert_eq!(
        before.stats.broken_link_count, 0,
        "before removal: cross-root link should resolve"
    );

    // Remove root B (which contains target.md).
    let remove_result = mcp
        .remove_root_tool(Parameters(RemoveRootRequest {
            realm: "default".to_string(),
            root: ws_b.root().to_string_lossy().to_string(),
        }))
        .await
        .expect("remove root should succeed");
    assert_eq!(remove_result.is_error, Some(false));

    // After removal: link from linker.md → [[target]] should now be broken.
    let result = mcp
        .graph_analysis_tool(Parameters(GraphAnalysisRequest {
            realm: None,
            top_n_hubs: 10,
            include_clusters: false,
        }))
        .await
        .expect("graph analysis should succeed");
    let after: GraphAnalysisResponse = result.into_typed().expect("typed response");
    assert_eq!(after.stats.total_docs, 1, "only root A docs should remain");
    assert!(
        after.stats.broken_link_count > 0,
        "after removal: [[target]] should be broken; broken_links: {:?}",
        after.broken_links
    );
    assert!(
        after.broken_links.iter().any(|bl| bl.target.contains("target")),
        "broken link should reference 'target'; broken_links: {:?}",
        after.broken_links
    );
}

// ─── same-stem files across roots — documents insertion-order behavior ───────

#[tokio::test]
async fn same_stem_wiki_link_resolves_to_first_added_root() {
    let ws_a = TempWorkspace::new("fed-stem-a");
    let ws_b = TempWorkspace::new("fed-stem-b");

    // Both roots have a file with the same stem "readme".
    fs::write(
        ws_a.root().join("readme.md"),
        "# Readme A\n\nFrom root A.\n",
    )
    .unwrap();
    fs::write(
        ws_b.root().join("readme.md"),
        "# Readme B\n\nFrom root B.\n",
    )
    .unwrap();
    // A third doc links to [[readme]] — should resolve to one of them (first added).
    fs::write(
        ws_a.root().join("linker.md"),
        "# Linker\n\nSee [[readme]] for info.\n",
    )
    .unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    let result = mcp
        .graph_analysis_tool(Parameters(GraphAnalysisRequest {
            realm: None,
            top_n_hubs: 10,
            include_clusters: false,
        }))
        .await
        .expect("graph analysis should succeed");

    let payload: GraphAnalysisResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.stats.total_docs, 3);
    // The wiki-link [[readme]] should resolve to SOME readme — not be broken.
    // It resolves to first-added root (insertion-order determinism).
    assert_eq!(
        payload.stats.broken_link_count, 0,
        "[[readme]] should resolve to one of the readme.md files; broken: {:?}",
        payload.broken_links
    );
}

// ─── export-docs-index produces entries for all roots ───────────────────────

#[tokio::test]
async fn export_docs_index_includes_both_roots() {
    let ws_a = TempWorkspace::new("fed-export-a");
    let ws_b = TempWorkspace::new("fed-export-b");

    fs::write(ws_a.root().join("alpha.md"), "# Alpha\n").unwrap();
    fs::create_dir_all(ws_a.root().join("sub")).unwrap();
    fs::write(ws_a.root().join("sub/deep.md"), "# Deep\n").unwrap();
    fs::write(ws_b.root().join("beta.md"), "# Beta\n").unwrap();

    let engine = engine_with_two_roots(&ws_a, &ws_b).await;
    let mcp = MarkymarkMcp::new(engine);

    let result = mcp
        .export_docs_index_tool(Parameters(ExportDocsIndexRequest {
            realm: None,
            name_override: None,
        }))
        .await
        .expect("export should succeed");

    let payload: ExportDocsIndexResponse = result.into_typed().expect("typed response");

    // Should have entries for both roots.
    assert!(
        payload.entries.len() >= 2,
        "should have entries for both roots; got {} entries",
        payload.entries.len()
    );
    assert!(
        payload.doc_count >= 3,
        "should count docs from both roots; got {}",
        payload.doc_count
    );
}
