//! Tests for the recommend-docs engine handler.

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

fn extract_recommendations(
    result: CoreOperationResult,
) -> (String, String, Vec<markymark_core::engine::DocRecommendation>) {
    match result {
        CoreOperationResult::Recommendations {
            realm,
            query,
            results,
        } => (realm, query, results),
        other => panic!("expected Recommendations, got: {other:?}"),
    }
}

// ── Tests ──

#[tokio::test]
async fn empty_realm_returns_empty_recommendations() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "anything".to_string(),
            realm: None,
            top_k: 5,
            include_sections: false,
        })
        .await;

    let (realm, query, results) = extract_recommendations(result);
    assert_eq!(realm, "default");
    assert_eq!(query, "anything");
    assert!(results.is_empty());
}

#[tokio::test]
async fn nonexistent_realm_returns_error() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "test".to_string(),
            realm: Some("nonexistent".to_string()),
            top_k: 5,
            include_sections: false,
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error for nonexistent realm, got {result:?}"
    );
}

#[tokio::test]
async fn top_k_zero_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("doc.md"), "# Hello World\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Hello".to_string(),
            realm: None,
            top_k: 0,
            include_sections: false,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert!(results.is_empty());
}

#[tokio::test]
async fn basic_query_returns_matching_docs() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("rust.md"), "# Rust Guide\n\nLearn Rust programming.\n").unwrap();
    fs::write(
        dir.path().join("python.md"),
        "# Python Guide\n\nLearn Python programming.\n",
    )
    .unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Rust".to_string(),
            realm: None,
            top_k: 5,
            include_sections: false,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert!(!results.is_empty(), "should find at least one doc");

    // The Rust doc should be the top result (title match scores 1.0)
    let top = &results[0];
    assert_eq!(top.title, "Rust Guide");
    assert!(top.search_score > 0.0);
    assert!(top.relevance_score > 0.0);
}

#[tokio::test]
async fn hub_docs_get_boosted_score() {
    let dir = tempfile::tempdir().unwrap();
    // Create a hub: many docs link to hub.md
    fs::write(
        dir.path().join("hub.md"),
        "# Hub Document\n\nThe central reference.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("a.md"),
        "# Doc A\n\nSee [Hub Document](hub.md) for details.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("b.md"),
        "# Doc B\n\nRefer to [Hub Document](hub.md) always.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("c.md"),
        "# Doc C\n\nCheck [Hub Document](hub.md) first.\n",
    )
    .unwrap();
    // Orphan doc, no links in or out
    fs::write(
        dir.path().join("orphan.md"),
        "# Orphan Document\n\nNo links here.\n",
    )
    .unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Document".to_string(),
            realm: None,
            top_k: 10,
            include_sections: false,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert!(results.len() >= 2, "should find multiple docs");

    // Find hub and orphan in results
    let hub = results.iter().find(|r| r.title == "Hub Document");
    let orphan = results.iter().find(|r| r.title == "Orphan Document");

    assert!(hub.is_some(), "hub doc should appear in results");
    assert!(orphan.is_some(), "orphan doc should appear in results");

    let hub = hub.unwrap();
    let orphan = orphan.unwrap();

    // Hub should have a positive hub_score; orphan should have 0
    assert!(
        hub.hub_score > 0.0,
        "hub should have positive hub_score, got {}",
        hub.hub_score
    );
    assert!(
        orphan.hub_score == 0.0,
        "orphan should have zero hub_score, got {}",
        orphan.hub_score
    );

    // Hub's relevance_score should be higher than orphan's (same search score, but hub boost)
    assert!(
        hub.relevance_score > orphan.relevance_score,
        "hub ({}) should rank higher than orphan ({})",
        hub.relevance_score,
        orphan.relevance_score
    );
}

#[tokio::test]
async fn top_k_limits_results() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..10 {
        fs::write(
            dir.path().join(format!("doc{i}.md")),
            format!("# Guide {i}\n\nContent about guides.\n"),
        )
        .unwrap();
    }
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Guide".to_string(),
            realm: None,
            top_k: 3,
            include_sections: false,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert_eq!(results.len(), 3, "should return exactly top_k results");
}

#[tokio::test]
async fn results_sorted_by_relevance_descending() {
    let dir = tempfile::tempdir().unwrap();
    // Title match (score 1.0) vs heading match (score 0.8)
    fs::write(
        dir.path().join("exact.md"),
        "# Rust Programming\n\nA guide to Rust.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("heading.md"),
        "# Other Topic\n\n## Rust Programming\n\nSome content.\n",
    )
    .unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Rust Programming".to_string(),
            realm: None,
            top_k: 10,
            include_sections: false,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert!(results.len() >= 2);

    // Verify descending order
    for window in results.windows(2) {
        assert!(
            window[0].relevance_score >= window[1].relevance_score,
            "results not sorted: {} < {}",
            window[0].relevance_score,
            window[1].relevance_score
        );
    }
}

#[tokio::test]
async fn no_summaries_without_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("doc.md"), "# Test Doc\n\nContent.\n").unwrap();
    let engine = make_engine_with_root(dir.path()).await;

    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Test".to_string(),
            realm: None,
            top_k: 5,
            include_sections: true,
        })
        .await;

    let (_, _, results) = extract_recommendations(result);
    assert!(!results.is_empty());

    // Without sidecars, document_summary and sections should be None
    for rec in &results {
        assert!(
            rec.document_summary.is_none(),
            "no sidecar = no document_summary"
        );
        assert!(rec.sections.is_none(), "no sidecar = no sections");
    }
}

#[tokio::test]
async fn named_realm_works() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("doc.md"), "# Special Doc\n\nContent.\n").unwrap();
    let engine = make_engine_with_custom_realm("test-realm", dir.path()).await;

    // Should fail on default realm
    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Special".to_string(),
            realm: None,
            top_k: 5,
            include_sections: false,
        })
        .await;
    let (_, _, results) = extract_recommendations(result);
    assert!(
        results.is_empty(),
        "default realm should have no docs"
    );

    // Should succeed on named realm
    let result = engine
        .execute(CoreOperation::RecommendDocs {
            query: "Special".to_string(),
            realm: Some("test-realm".to_string()),
            top_k: 5,
            include_sections: false,
        })
        .await;
    let (realm, _, results) = extract_recommendations(result);
    assert_eq!(realm, "test-realm");
    assert!(!results.is_empty(), "named realm should find the doc");
}
