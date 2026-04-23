//! Tests for batch + persistent-engine markdown indexing.
//!
//! Covers:
//! - B-8 migration (from_ast → from_scan) behaviour: code spans, frontmatter
//! - Phase 3 persistent-engine (marky-xfgb): create, stale fallback, cleanup
//! - LTO canary (grouped here rather than standalone `lto.rs` — the assertion
//!   is about engine-indexing behaviour under LTO, so topical neighbour is
//!   `engine_fallback_stale_on_update_failure`).

use super::*;

/// MCP batch-indexed markdown documents must have code spans extracted.
///
/// This tests the B-8 migration: from_ast → from_scan for MCP batch indexing.
/// The `from_scan` path (Zig extraction) extracts inline code spans, while
/// `from_ast` does not. After migration, searching for code span text should
/// return results.
#[tokio::test]
async fn batch_indexed_docs_have_code_spans() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# Code Spans Test\n\nThe `HashMap` type is a key-value store.\n\nUse `Vec<T>` for lists.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("code-spans-realm", dir.path()).await;

    // Search for code span text — should find matches if code spans are extracted
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "HashMap".to_string(),
            realm: Some("code-spans-realm".to_string()),
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(
            !matches.is_empty(),
            "batch-indexed docs should have code spans: searching for 'HashMap' should find the backtick code span"
        );
    } else {
        panic!("expected Symbols result, got {result:?}");
    }
}

/// MCP batch-indexed markdown documents must preserve frontmatter.
///
/// After B-8 migration to from_scan, frontmatter must still be accessible
/// for search filtering, preview, and export. This tests that the
/// `from_scan_with_frontmatter` constructor correctly preserves frontmatter.
#[tokio::test]
async fn batch_indexed_docs_preserve_frontmatter() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "---\ntitle: Test Document\ntags: [rust, zig]\n---\n\n# Content\n\nSome text here.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("fm-realm", dir.path()).await;

    // Search with frontmatter filter should find the document
    let result = engine
        .execute(CoreOperation::SearchWorkspace {
            query: None,
            realm: Some("fm-realm".to_string()),
            frontmatter_filter: Some(("title".to_string(), "Test Document".to_string())),
            property_filter: None,
            tag_filter: None,
            limit: 10,
        })
        .await;
    if let CoreOperationResult::WorkspaceSearchResults { results, .. } = result {
        assert!(
            !results.is_empty(),
            "frontmatter filtering should find the document after from_scan migration"
        );
    } else {
        panic!("expected WorkspaceSearchResults, got {result:?}");
    }
}

// -- Engine-based indexing tests (Phase 3: marky-xfgb) --

#[tokio::test]
async fn engine_index_creates_persistent_engines() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("one.md"), "# Heading One\n\nSome text.\n").unwrap();
    fs::write(dir.path().join("two.md"), "# Heading Two\n\nMore text.\n").unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Engine path should create a persistent engine per markdown file.
    assert_eq!(
        realm.engines.len(),
        2,
        "expected 2 persistent engines, one per markdown file"
    );

    // Documents should also be indexed (behavioral parity with scan path).
    assert_eq!(realm.index.document_count(), 2);
}

#[tokio::test]
async fn engine_fallback_stale_on_update_failure() {
    let dir = make_temp_realm_dir();
    // Magic filename triggers forced update failure on second index.
    let path = dir.path().join("__marky_test_force_update_fail__.md");
    fs::write(&path, "# Original\n\nFirst version.\n").unwrap();

    let mut realm = RealmData::new();

    // First index: engine created successfully with original content.
    index_root_into_realm(dir.path(), &mut realm).await;
    assert_eq!(realm.engines.len(), 1);
    assert_eq!(realm.index.document_count(), 1);

    // Modify the file — the update will be forced to fail.
    fs::write(&path, "# Changed\n\nSecond version.\n").unwrap();

    // Second index: update fails, should fall back to stale engine snapshot.
    index_root_into_realm(dir.path(), &mut realm).await;

    // Document should still be indexed (stale fallback, not empty).
    assert_eq!(
        realm.index.document_count(),
        1,
        "document should still be indexed via stale engine fallback"
    );

    // Verify content is present — stale snapshot should have the original heading.
    let uri = DocumentUri::from_file_path(&path);
    let doc = realm.index.get_document(&uri);
    assert!(
        doc.is_some(),
        "document should be retrievable after stale fallback"
    );
    assert!(
        !doc.unwrap().headings().is_empty(),
        "stale fallback document should have headings from original parse"
    );
}

/// LTO canary: verifies cross-language ThinLTO eliminates the test-only fault
/// injection in the Zig engine. Under LTO, the magic-filename check is optimized
/// away, so the engine creates successfully. Without LTO, the fault injection
/// fires and this test must be skipped.
///
/// Gated on `MARKYMARK_LTO_ENABLED=1` (set by Bazel LTO configs) rather than
/// `cfg!(debug_assertions)`, which is false in any opt build — not just LTO.
#[tokio::test]
async fn lto_eliminates_fault_injection() {
    // Only run under LTO builds — without LTO the fault injection is live.
    if std::env::var("MARKYMARK_LTO_ENABLED").as_deref() != Ok("1") {
        return;
    }

    let dir = make_temp_realm_dir();
    // Magic filename that triggers forced create failure WITHOUT LTO.
    let path = dir.path().join("__marky_test_force_create_fail__.md");
    fs::write(
        &path,
        "# LTO Canary\n\nEngine should create successfully under LTO.\n",
    )
    .unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Under LTO the fault injection is dead code — engine creates normally.
    assert_eq!(
        realm.engines.len(),
        1,
        "LTO should eliminate the fault injection, allowing engine creation"
    );

    assert_eq!(realm.index.document_count(), 1);

    let uri = DocumentUri::from_file_path(&path);
    let doc = realm.index.get_document(&uri);
    assert!(doc.is_some());
    assert!(!doc.unwrap().headings().is_empty());
}

#[tokio::test]
async fn engine_cleanup_on_root_removal() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Cleanup Test\n\nContent.\n").unwrap();

    let mut realm = RealmData::new();

    // Index the root — engine should be created.
    index_root_into_realm(dir.path(), &mut realm).await;
    assert_eq!(realm.engines.len(), 1, "engine should exist after indexing");
    assert_eq!(realm.index.document_count(), 1);

    // Remove the root — engine should be cleaned up.
    unindex_root_from_realm(dir.path(), &mut realm).await;
    assert_eq!(
        realm.engines.len(),
        0,
        "engine should be removed when root is unindexed"
    );
    assert_eq!(
        realm.index.document_count(),
        0,
        "documents should be removed when root is unindexed"
    );
}

#[tokio::test]
async fn engine_frontmatter_preserved() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "---\ntitle: Engine FM Test\ntags: [alpha, beta]\naliases: [efm]\n---\n\n# Content\n\nBody text.\n",
    )
    .unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Engine should be created (not scan path).
    assert_eq!(realm.engines.len(), 1);

    // Frontmatter should be accessible via search filter.
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));
    let doc = realm
        .index
        .get_document(&uri)
        .expect("document should exist");

    // Verify frontmatter entries are present.
    let fm = doc.frontmatter();
    assert!(
        !fm.is_empty(),
        "frontmatter should be preserved via engine path"
    );
    // Check that the title key is present.
    assert!(
        fm.iter().any(|e| e.key == "title"),
        "frontmatter should contain 'title' key"
    );
}
