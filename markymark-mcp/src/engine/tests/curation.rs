//! Tests for the curation-diagnostics engine handler.

use super::*;

// ── Helpers ──

async fn make_engine_with_root(dir: &Path) -> RuntimeEngine {
    let engine = RuntimeEngine::default();
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: dir.to_path_buf(),
        })
        .await;
    engine
}

fn extract_curation_report(
    result: CoreOperationResult,
) -> (String, markymark_core::engine::CurationReportData) {
    match result {
        CoreOperationResult::CurationReport { realm, report } => (realm, report),
        other => panic!("expected CurationReport, got: {other:?}"),
    }
}

// ── Tests ──

#[tokio::test]
async fn empty_realm_returns_empty_report() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    let (realm, report) = extract_curation_report(result);
    assert_eq!(realm, "default");
    assert!(report.orphan_docs.is_empty());
    assert!(report.low_connectivity_docs.is_empty());
    assert!(report.suggestions.is_empty());
    assert_eq!(report.stats.total_docs, 0);
    assert_eq!(report.stats.orphan_count, 0);
    assert_eq!(report.stats.orphan_percentage, 0.0);
    assert_eq!(report.stats.broken_link_count, 0);
}

#[tokio::test]
async fn nonexistent_realm_returns_error() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: Some("nonexistent".to_string()),
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error for nonexistent realm, got {result:?}"
    );
}

#[tokio::test]
async fn single_orphan_detected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("alone.md"), "# Alone\n\nNo links.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert_eq!(report.stats.total_docs, 1);
    assert_eq!(report.stats.orphan_count, 1);
    assert_eq!(report.stats.orphan_percentage, 100.0);
    assert_eq!(report.orphan_docs.len(), 1);
    // Single doc: no suggestions possible (can't cross-link to self)
    assert!(report.suggestions.is_empty());
}

#[tokio::test]
async fn linked_docs_not_orphans() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.md"),
        "# Doc A\n\nSee [Doc B](b.md) for more.\n",
    )
    .unwrap();
    fs::write(dir.path().join("b.md"), "# Doc B\n\nReferenced by A.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: false,
            max_suggestions: 0,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert_eq!(report.stats.total_docs, 2);
    assert_eq!(report.stats.orphan_count, 0);
    assert_eq!(report.stats.orphan_percentage, 0.0);
    assert!(report.orphan_docs.is_empty());
}

#[tokio::test]
async fn detects_orphans_among_linked_docs() {
    let dir = tempfile::tempdir().unwrap();
    // Hub doc: linked to by a and b
    fs::write(dir.path().join("hub.md"), "# Hub\n\nThe central doc.\n").unwrap();
    fs::write(
        dir.path().join("a.md"),
        "# Doc A\n\nSee [Hub](hub.md) for details.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("b.md"),
        "# Doc B\n\nRefer to [Hub](hub.md) always.\n",
    )
    .unwrap();
    // Orphan: no links in or out
    fs::write(
        dir.path().join("orphan.md"),
        "# Orphan\n\nCompletely isolated.\n",
    )
    .unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert_eq!(report.stats.total_docs, 4);
    assert_eq!(report.stats.orphan_count, 1);
    assert_eq!(report.orphan_docs.len(), 1);
    assert!(
        report.orphan_docs[0].as_str().contains("orphan.md"),
        "orphan doc should be orphan.md"
    );

    // Should suggest linking orphan to hub
    assert!(
        !report.suggestions.is_empty(),
        "should suggest cross-links for orphan"
    );
}

#[tokio::test]
async fn connectivity_scoring() {
    let dir = tempfile::tempdir().unwrap();
    // Hub: linked by a and b (in-degree=2)
    fs::write(dir.path().join("hub.md"), "# Hub\n\nCentral.\n").unwrap();
    // a links to hub (out-degree=1)
    fs::write(dir.path().join("a.md"), "# Doc A\n\nSee [Hub](hub.md).\n").unwrap();
    // b links to hub (out-degree=1)
    fs::write(dir.path().join("b.md"), "# Doc B\n\n[Hub](hub.md) ref.\n").unwrap();
    // orphan: connectivity=0
    fs::write(dir.path().join("orphan.md"), "# Orphan\n\nAlone.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: false,
            max_suggestions: 0,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    // avg connectivity should be > 0 (hub has connectivity 2, a and b have 1 each, orphan has 0)
    assert!(
        report.stats.avg_connectivity > 0.0,
        "avg connectivity should be positive"
    );
    // Low connectivity docs: orphan has 0 links which is below median (1) and below threshold (2)
    assert!(
        !report.low_connectivity_docs.is_empty(),
        "should have low-connectivity docs"
    );
    // Orphan should appear in low-connectivity list
    let orphan_in_low = report
        .low_connectivity_docs
        .iter()
        .any(|d| d.uri.as_str().contains("orphan.md"));
    assert!(orphan_in_low, "orphan should be in low-connectivity docs");
}

#[tokio::test]
async fn max_suggestions_capped() {
    let dir = tempfile::tempdir().unwrap();
    // Create a hub and many orphans
    fs::write(dir.path().join("hub.md"), "# Hub\n\nCentral reference.\n").unwrap();
    fs::write(dir.path().join("linked.md"), "# Linked\n\n[Hub](hub.md)\n").unwrap();
    for i in 0..10 {
        fs::write(
            dir.path().join(format!("orphan{i}.md")),
            format!("# Orphan {i}\n\nIsolated doc.\n"),
        )
        .unwrap();
    }
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: true,
            max_suggestions: 3,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert!(
        report.suggestions.len() <= 3,
        "suggestions should be capped at max_suggestions=3, got {}",
        report.suggestions.len()
    );
}

#[tokio::test]
async fn max_items_per_category_capped() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..10 {
        fs::write(
            dir.path().join(format!("orphan{i}.md")),
            format!("# Orphan {i}\n\nIsolated.\n"),
        )
        .unwrap();
    }
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: false,
            max_suggestions: 0,
            max_items_per_category: 3,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert!(
        report.orphan_docs.len() <= 3,
        "orphan_docs should be capped at max_items_per_category=3, got {}",
        report.orphan_docs.len()
    );
    assert!(
        report.low_connectivity_docs.len() <= 3,
        "low_connectivity_docs should be capped at 3, got {}",
        report.low_connectivity_docs.len()
    );
}

#[tokio::test]
async fn named_realm_works() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("doc.md"), "# Doc\n\nContent.\n").unwrap();
    let engine = make_engine_with_custom_realm("test-realm", dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: Some("test-realm".to_string()),
            include_suggestions: false,
            max_suggestions: 0,
            max_items_per_category: 50,
        })
        .await;

    let (realm, report) = extract_curation_report(result);
    assert_eq!(realm, "test-realm");
    assert_eq!(report.stats.total_docs, 1);
}

#[tokio::test]
async fn suggestion_types_are_reduce_orphan() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("hub.md"), "# Hub\n\nCentral.\n").unwrap();
    fs::write(dir.path().join("linked.md"), "# Linked\n\n[Hub](hub.md)\n").unwrap();
    fs::write(dir.path().join("orphan.md"), "# Orphan\n\nIsolated.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: true,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    for suggestion in &report.suggestions {
        assert_eq!(
            suggestion.suggestion_type,
            markymark_core::engine::CurationSuggestionType::ReduceOrphan,
            "orphan suggestions should be ReduceOrphan type"
        );
        assert!(
            !suggestion.reason.is_empty(),
            "suggestion reason should not be empty"
        );
    }
}

#[tokio::test]
async fn include_suggestions_false_returns_no_suggestions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("hub.md"), "# Hub\n\nCentral.\n").unwrap();
    fs::write(dir.path().join("linked.md"), "# Linked\n\n[Hub](hub.md)\n").unwrap();
    fs::write(dir.path().join("orphan.md"), "# Orphan\n\nIsolated.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: false,
            max_suggestions: 20,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert!(
        report.suggestions.is_empty(),
        "include_suggestions=false should skip suggestions"
    );
    // But orphans should still be detected
    assert!(
        !report.orphan_docs.is_empty(),
        "orphans should still be detected even without suggestions"
    );
}

#[tokio::test]
async fn cross_directory_link_counted_in_degrees() {
    // Setup: sub/a.md links to ../b.md via relative path
    // b.md should have in-degree from this cross-directory link.
    // c.md has same stem as b.md in different location — should NOT get in-degree.
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir_all(&sub).unwrap();
    // sub/a.md links to ../b.md (relative path crossing directories)
    fs::write(sub.join("a.md"), "# Doc A\n\n[see B](../b.md)\n").unwrap();
    // b.md at root
    fs::write(dir.path().join("b.md"), "# Doc B\n\nTarget.\n").unwrap();
    // other/b.md — same stem, different directory (should NOT be linked)
    let other = dir.path().join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("b.md"), "# Other B\n\nDifferent file.\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::CurationDiagnostics {
            realm: None,
            include_suggestions: false,
            max_suggestions: 0,
            max_items_per_category: 50,
        })
        .await;

    let (_, report) = extract_curation_report(result);
    assert_eq!(report.stats.total_docs, 3);

    // sub/a.md links to ../b.md, so:
    // - b.md should have in-degree >= 1 (linked)
    // - other/b.md should have in-degree 0 (not the target)
    // - sub/a.md should have out-degree >= 1

    // With correct path-based resolution, b.md is NOT an orphan
    // (it has in-degree from sub/a.md). other/b.md IS an orphan.
    let b_uri_str = markymark_core::DocumentUri::from_file_path(&dir.path().join("b.md"))
        .as_str()
        .to_string();
    let other_b_uri_str = markymark_core::DocumentUri::from_file_path(&other.join("b.md"))
        .as_str()
        .to_string();

    // b.md should NOT be in orphans (it's linked to)
    let b_is_orphan = report.orphan_docs.iter().any(|o| o.as_str() == b_uri_str);
    assert!(
        !b_is_orphan,
        "b.md should not be orphan — sub/a.md links to it via ../b.md"
    );

    // other/b.md SHOULD be orphan (nothing links to it)
    let other_b_is_orphan = report
        .orphan_docs
        .iter()
        .any(|o| o.as_str() == other_b_uri_str);
    assert!(
        other_b_is_orphan,
        "other/b.md should be orphan — nothing links to it"
    );
}
