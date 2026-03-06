//! Tests for the enrich-document engine operation.

use super::*;

use markymark_core::engine::OutlineTreeNode;
use markymark_core::inference::InferenceError;
use markymark_core::sidecar::{DocumentSidecar, SectionSummary, SIDECAR_VERSION};

use crate::engine::enrich;

// ---------------------------------------------------------------------------
// Test inference provider
// ---------------------------------------------------------------------------

struct TestInferenceProvider;

#[async_trait]
impl markymark_core::inference::InferenceProvider for TestInferenceProvider {
    async fn summarize(&self, text: &str, context: Option<&str>) -> Result<String, InferenceError> {
        if text.is_empty() {
            return Err(InferenceError::InvalidInput("empty text".to_string()));
        }
        let ctx = context.unwrap_or("none");
        Ok(format!("[{ctx}] summary({} chars)", text.len()))
    }

    fn model_id(&self) -> &str {
        "test-model-v1"
    }
}

// ---------------------------------------------------------------------------
// enrich_document tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enrich_document_no_provider_returns_not_implemented() {
    let dir = make_temp_realm_dir("enrich-no-provider");
    fs::write(dir.path().join("doc.md"), "# Hello\n\nSome content.\n").unwrap();
    let engine = make_engine_with_custom_realm("enrich-np", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::EnrichDocument {
            uri,
            realm: Some("enrich-np".to_string()),
            sidecar_dir: None,
            force: false,
        })
        .await;

    assert!(
        matches!(
            result,
            CoreOperationResult::Error(CoreError::NotImplemented(_))
        ),
        "expected NotImplemented without provider, got {result:?}"
    );
}

#[tokio::test]
async fn enrich_document_basic() {
    let dir = make_temp_realm_dir("enrich-basic");
    let doc_content = "# Title\n\nSome content here.\n\n## Section\n\nMore content.\n";
    fs::write(dir.path().join("doc.md"), doc_content).unwrap();

    let engine = make_engine_with_custom_realm("enrich-basic", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-basic").unwrap();

    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;

    match result {
        CoreOperationResult::EnrichmentResult {
            sections_enriched,
            was_stale,
            model_id,
            ..
        } => {
            assert_eq!(sections_enriched, 2, "should enrich 2 heading sections");
            assert!(was_stale, "fresh enrichment should report was_stale=true");
            assert_eq!(model_id, "test-model-v1");
        }
        other => panic!("expected EnrichmentResult, got {other:?}"),
    }

    // Verify sidecar was written.
    let sidecar_path = dir.path().join(".markymark/doc.md.json");
    assert!(sidecar_path.exists(), "sidecar file should exist");

    let sidecar: DocumentSidecar =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).unwrap()).unwrap();
    assert_eq!(sidecar.version, SIDECAR_VERSION);
    assert_eq!(sidecar.model_id, "test-model-v1");
    assert_eq!(sidecar.sections.len(), 2);
    assert!(sidecar.document_summary.is_some());
    assert!(!sidecar.content_hash.is_empty());
}

#[tokio::test]
async fn enrich_document_skips_when_fresh() {
    let dir = make_temp_realm_dir("enrich-fresh");
    let doc_content = "# Title\n\nContent.\n";
    fs::write(dir.path().join("doc.md"), doc_content).unwrap();

    let engine = make_engine_with_custom_realm("enrich-fresh", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-fresh").unwrap();

    // First enrichment.
    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;
    assert!(matches!(
        result,
        CoreOperationResult::EnrichmentResult {
            was_stale: true,
            ..
        }
    ));

    // Second enrichment should skip (sidecar is fresh).
    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;
    match result {
        CoreOperationResult::EnrichmentResult { was_stale, .. } => {
            assert!(!was_stale, "second enrichment should skip (sidecar fresh)");
        }
        other => panic!("expected EnrichmentResult, got {other:?}"),
    }
}

#[tokio::test]
async fn enrich_document_force_regenerates() {
    let dir = make_temp_realm_dir("enrich-force");
    fs::write(dir.path().join("doc.md"), "# Title\n\nContent.\n").unwrap();

    let engine = make_engine_with_custom_realm("enrich-force", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-force").unwrap();

    // First enrichment.
    enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;

    // Force re-enrichment should regenerate.
    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        true,
        Some(&provider),
    )
    .await;
    match result {
        CoreOperationResult::EnrichmentResult { was_stale, .. } => {
            assert!(was_stale, "force=true should always regenerate");
        }
        other => panic!("expected EnrichmentResult, got {other:?}"),
    }
}

#[tokio::test]
async fn enrich_document_stale_after_content_change() {
    let dir = make_temp_realm_dir("enrich-stale");
    let doc_path = dir.path().join("doc.md");
    fs::write(&doc_path, "# Title\n\nOriginal content.\n").unwrap();

    let engine = make_engine_with_custom_realm("enrich-stale", dir.path()).await;
    let uri = DocumentUri::from_file_path(&doc_path);

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-stale").unwrap();

    // Enrich once.
    enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;

    // Modify the source file (changes content hash).
    fs::write(&doc_path, "# Title\n\nModified content!\n").unwrap();

    // Re-enrich should detect staleness.
    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;
    match result {
        CoreOperationResult::EnrichmentResult { was_stale, .. } => {
            assert!(was_stale, "should be stale after content change");
        }
        other => panic!("expected EnrichmentResult, got {other:?}"),
    }
}

#[tokio::test]
async fn enrich_document_custom_sidecar_dir() {
    let dir = make_temp_realm_dir("enrich-custom-dir");
    let custom_sidecar = dir.path().join("custom-sidecars");
    fs::write(dir.path().join("doc.md"), "# Title\n\nContent.\n").unwrap();

    let engine = make_engine_with_custom_realm("enrich-custom", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-custom").unwrap();

    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        Some(custom_sidecar.as_path()),
        false,
        Some(&provider),
    )
    .await;

    assert!(matches!(
        result,
        CoreOperationResult::EnrichmentResult { .. }
    ));

    // Sidecar should be in the custom directory.
    let sidecar_path = custom_sidecar.join("doc.md.json");
    assert!(
        sidecar_path.exists(),
        "sidecar should be in custom directory: {}",
        sidecar_path.display()
    );
}

#[tokio::test]
async fn enrich_document_no_headings() {
    let dir = make_temp_realm_dir("enrich-no-headings");
    fs::write(dir.path().join("doc.md"), "Just plain text.\n").unwrap();

    let engine = make_engine_with_custom_realm("enrich-nohead", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-nohead").unwrap();

    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        None,
        false,
        Some(&provider),
    )
    .await;

    match result {
        CoreOperationResult::EnrichmentResult {
            sections_enriched, ..
        } => {
            assert_eq!(sections_enriched, 0, "no headings = no sections enriched");
        }
        other => panic!("expected EnrichmentResult, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Error propagation and path collision tests (marky-niw2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn write_sidecar_error_propagated() {
    let dir = make_temp_realm_dir("enrich-write-error");
    fs::write(dir.path().join("doc.md"), "# Title\n\nContent.\n").unwrap();

    // Create a read-only directory as sidecar override — writes will fail.
    let readonly_dir = dir.path().join("readonly-sidecar");
    fs::create_dir(&readonly_dir).unwrap();
    // Create a file that blocks the sidecar path so create_dir_all fails.
    // (sidecar_path appends ".json", so write to "doc.md.json" which is a file,
    // then sidecar tries to create_dir_all on the parent which includes this file.)
    // Simpler: make the entire directory read-only.
    let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&readonly_dir, perms).unwrap();

    let engine = make_engine_with_custom_realm("enrich-write-err", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-write-err").unwrap();

    let result = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri,
        Some(readonly_dir.as_path()),
        false,
        Some(&provider),
    )
    .await;

    // Restore permissions for cleanup.
    let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(&readonly_dir, perms).unwrap();

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected Error when sidecar write fails, got {result:?}"
    );
    if let CoreOperationResult::Error(err) = result {
        let msg = err.to_string();
        assert!(
            msg.contains("sidecar"),
            "error should mention sidecar, got: {msg}"
        );
    }
}

#[tokio::test]
async fn sidecar_override_no_collision() {
    let dir = make_temp_realm_dir("enrich-no-collision");
    let custom_sidecar = dir.path().join("shared-sidecars");

    // Create two files with the same name in different subdirectories.
    let sub_a = dir.path().join("a");
    let sub_b = dir.path().join("b");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();
    fs::write(sub_a.join("README.md"), "# Alpha\n\nAlpha content.\n").unwrap();
    fs::write(sub_b.join("README.md"), "# Beta\n\nBeta content.\n").unwrap();

    let engine = make_engine_with_custom_realm("enrich-collision", dir.path()).await;
    let uri_a = DocumentUri::from_file_path(&sub_a.join("README.md"));
    let uri_b = DocumentUri::from_file_path(&sub_b.join("README.md"));

    let provider = TestInferenceProvider;
    let state = engine.state.read().await;
    let realm_data = state.get("enrich-collision").unwrap();

    // Enrich both documents with the same sidecar override directory.
    let _result_a = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri_a,
        Some(custom_sidecar.as_path()),
        false,
        Some(&provider),
    )
    .await;

    let _result_b = enrich::handle_enrich_document(
        &realm_data.index,
        &realm_data.roots,
        &uri_b,
        Some(custom_sidecar.as_path()),
        false,
        Some(&provider),
    )
    .await;

    // Both should produce distinct sidecar files (not collide on the same path).
    let sidecar_a = custom_sidecar.join("a").join("README.md.json");
    let sidecar_b = custom_sidecar.join("b").join("README.md.json");
    assert!(
        sidecar_a.exists(),
        "sidecar for a/README.md should be at {}, not just README.md.json",
        sidecar_a.display()
    );
    assert!(
        sidecar_b.exists(),
        "sidecar for b/README.md should be at {}, not just README.md.json",
        sidecar_b.display()
    );

    // Read both sidecars and verify they have different content hashes
    // (proving the second didn't silently overwrite the first).
    let json_a: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_a).unwrap()).unwrap();
    let json_b: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_b).unwrap()).unwrap();
    assert_ne!(
        json_a["content_hash"], json_b["content_hash"],
        "sidecars should have different content hashes (different source files)"
    );
}

// ---------------------------------------------------------------------------
// inject_summaries tests
// ---------------------------------------------------------------------------

#[test]
fn inject_summaries_populates_matching_nodes() {
    let sidecar = DocumentSidecar {
        version: SIDECAR_VERSION,
        content_hash: "test".to_string(),
        model_id: "test".to_string(),
        document_summary: Some("Doc summary".to_string()),
        sections: vec![
            SectionSummary {
                slug: "intro".to_string(),
                heading_path: "Introduction".to_string(),
                level: 1,
                summary: "Intro summary".to_string(),
            },
            SectionSummary {
                slug: "details".to_string(),
                heading_path: "Introduction > Details".to_string(),
                level: 2,
                summary: "Details summary".to_string(),
            },
        ],
    };

    let mut tree = OutlineTreeNode {
        title: String::new(),
        level: 0,
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        text: None,
        summary: None,
        children: vec![OutlineTreeNode {
            title: "Introduction".to_string(),
            level: 1,
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            text: None,
            summary: None,
            children: vec![OutlineTreeNode {
                title: "Details".to_string(),
                level: 2,
                range: Range::new(Position::new(2, 0), Position::new(2, 0)),
                text: None,
                summary: None,
                children: vec![],
            }],
        }],
    };

    enrich::inject_summaries(&mut tree, &sidecar);

    assert_eq!(tree.summary, Some("Doc summary".to_string()));
    assert_eq!(tree.children[0].summary, Some("Intro summary".to_string()));
    assert_eq!(
        tree.children[0].children[0].summary,
        Some("Details summary".to_string())
    );
}

#[test]
fn inject_summaries_no_match_leaves_none() {
    let sidecar = DocumentSidecar {
        version: SIDECAR_VERSION,
        content_hash: "test".to_string(),
        model_id: "test".to_string(),
        document_summary: None,
        sections: vec![SectionSummary {
            slug: "other".to_string(),
            heading_path: "Other Section".to_string(),
            level: 1,
            summary: "Other summary".to_string(),
        }],
    };

    let mut tree = OutlineTreeNode {
        title: String::new(),
        level: 0,
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        text: None,
        summary: None,
        children: vec![OutlineTreeNode {
            title: "Unmatched".to_string(),
            level: 1,
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            text: None,
            summary: None,
            children: vec![],
        }],
    };

    enrich::inject_summaries(&mut tree, &sidecar);

    assert!(tree.summary.is_none());
    assert!(tree.children[0].summary.is_none());
}

// ---------------------------------------------------------------------------
// End-to-end: enrich then get-outline returns summaries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_outline_tree_includes_sidecar_summaries() {
    let dir = make_temp_realm_dir("enrich-outline-e2e");
    fs::write(
        dir.path().join("doc.md"),
        "# Title\n\nSome content.\n\n## Section\n\nMore content.\n",
    )
    .unwrap();

    let engine = make_engine_with_custom_realm("enrich-e2e", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    // Step 1: Enrich the document via the handler directly.
    let provider = TestInferenceProvider;
    {
        let state = engine.state.read().await;
        let realm_data = state.get("enrich-e2e").unwrap();
        let result = enrich::handle_enrich_document(
            &realm_data.index,
            &realm_data.roots,
            &uri,
            None,
            false,
            Some(&provider),
        )
        .await;
        assert!(
            matches!(
                result,
                CoreOperationResult::EnrichmentResult {
                    was_stale: true,
                    ..
                }
            ),
            "enrichment should succeed"
        );
    }

    // Step 2: Get outline with format=tree — should include sidecar summaries.
    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("enrich-e2e".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::OutlineTree(tree) => {
            // Root node should have document summary.
            assert!(
                tree.summary.is_some(),
                "root node should have document summary from sidecar"
            );
            // H1 should have section summary.
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Title");
            assert!(
                h1.summary.is_some(),
                "h1 should have section summary from sidecar"
            );
            // H2 should have section summary.
            let h2 = &h1.children[0];
            assert_eq!(h2.title, "Section");
            assert!(
                h2.summary.is_some(),
                "h2 should have section summary from sidecar"
            );
        }
        other => panic!("expected OutlineTree with summaries, got {other:?}"),
    }
}

#[tokio::test]
async fn get_outline_tree_without_enrichment_has_no_summaries() {
    let dir = make_temp_realm_dir("outline-no-enrich");
    fs::write(dir.path().join("doc.md"), "# Title\n\nContent.\n").unwrap();

    let engine = make_engine_with_custom_realm("no-enrich", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("no-enrich".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert!(tree.summary.is_none(), "no sidecar = no summary");
            assert!(
                tree.children[0].summary.is_none(),
                "no sidecar = no section summary"
            );
        }
        other => panic!("expected OutlineTree without summaries, got {other:?}"),
    }
}
