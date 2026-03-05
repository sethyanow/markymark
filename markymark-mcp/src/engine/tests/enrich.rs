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
    async fn summarize(
        &self,
        text: &str,
        context: Option<&str>,
    ) -> Result<String, InferenceError> {
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
        matches!(result, CoreOperationResult::Error(CoreError::NotImplemented(_))),
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
        CoreOperationResult::EnrichmentResult { was_stale: true, .. }
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
    assert_eq!(
        tree.children[0].summary,
        Some("Intro summary".to_string())
    );
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
