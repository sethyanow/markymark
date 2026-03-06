use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

/// Regression test for marky-uux: search-symbols must not return results from a
/// different realm. Both realms exist simultaneously; querying realm_a must not
/// surface symbols that only exist in realm_b.
#[tokio::test]
async fn search_symbols_scopes_results_to_realm() {
    let ws_a = TempWorkspace::new("realm-isolate-a");
    let ws_b = TempWorkspace::new("realm-isolate-b");

    // Each realm has a document with a heading that is unique to that realm.
    fs::write(ws_a.root().join("a.md"), "# HeadingOnlyInRealmAlpha\n")
        .expect("a.md should be created");
    fs::write(ws_b.root().join("b.md"), "# HeadingOnlyInRealmBeta\n")
        .expect("b.md should be created");

    let engine = RuntimeEngine::default();

    engine
        .execute(CoreOperation::CreateRealm {
            name: "realm-alpha".to_string(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: "realm-alpha".to_string(),
            root: ws_a.root(),
        })
        .await;

    engine
        .execute(CoreOperation::CreateRealm {
            name: "realm-beta".to_string(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: "realm-beta".to_string(),
            root: ws_b.root(),
        })
        .await;

    // Searching realm-alpha for the beta-unique heading must return empty.
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "HeadingOnlyInRealmBeta".to_string(),
            realm: Some("realm-alpha".to_string()),
        })
        .await;
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                matches.is_empty(),
                "realm-alpha search must not return realm-beta symbol; got: {matches:?}"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }

    // Searching realm-beta for the alpha-unique heading must also return empty.
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "HeadingOnlyInRealmAlpha".to_string(),
            realm: Some("realm-beta".to_string()),
        })
        .await;
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                matches.is_empty(),
                "realm-beta search must not return realm-alpha symbol; got: {matches:?}"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }

    // Each realm correctly finds its own symbol.
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "HeadingOnlyInRealmAlpha".to_string(),
            realm: Some("realm-alpha".to_string()),
        })
        .await;
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                !matches.is_empty(),
                "realm-alpha search must find its own symbol"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }
}

/// Regression test for marky-uux: after destroy-realm, search-symbols for that
/// realm must return an error (realm no longer exists), not stale results.
#[tokio::test]
async fn search_symbols_returns_empty_after_destroy_realm() {
    let ws = TempWorkspace::new("realm-destroy-symbols");
    fs::write(ws.root().join("doc.md"), "# UniqueHeadingForDestroyTest\n")
        .expect("doc.md should be created");

    let engine = RuntimeEngine::default();

    engine
        .execute(CoreOperation::CreateRealm {
            name: "transient-realm".to_string(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: "transient-realm".to_string(),
            root: ws.root(),
        })
        .await;

    // Verify symbol is found before destroy.
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingForDestroyTest".to_string(),
            realm: Some("transient-realm".to_string()),
        })
        .await;
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                !matches.is_empty(),
                "symbol should be found before realm is destroyed"
            );
        }
        other => panic!("expected Symbols result before destroy, got: {other:?}"),
    }

    // Destroy the realm.
    let destroy = engine
        .execute(CoreOperation::DestroyRealm {
            name: "transient-realm".to_string(),
        })
        .await;
    assert!(
        matches!(destroy, CoreOperationResult::Ok),
        "destroy-realm should succeed; got: {destroy:?}"
    );

    // After destroy, searching for the symbol in that realm must return an error
    // (realm does not exist), not a stale Symbols result.
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingForDestroyTest".to_string(),
            realm: Some("transient-realm".to_string()),
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "search-symbols must return error for destroyed realm, not stale results; got: {result:?}"
    );
}

/// Querying a realm that was **never created** must return a structured error
/// (`CoreOperationResult::Error`), not panic or return empty data. This covers
/// every query operation that accepts a realm parameter.
///
/// Regression coverage for marky-w85.
#[tokio::test]
async fn query_operations_error_on_never_created_realm() {
    let engine = RuntimeEngine::default();
    let bogus = "never-created-realm";
    let dummy_uri = DocumentUri::from_file_path(std::path::Path::new("/tmp/dummy.md"));
    let dummy_pos = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 5,
        },
    };

    let operations: Vec<(&str, CoreOperation)> = vec![
        (
            "SearchSymbols",
            CoreOperation::SearchSymbols {
                query: "anything".to_string(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "GetOutline",
            CoreOperation::GetOutline {
                uri: dummy_uri.clone(),
                realm: Some(bogus.to_string()),
                format: "flat".to_string(),
                include_text: false,
            },
        ),
        (
            "FindReferences",
            CoreOperation::FindReferences {
                uri: dummy_uri.clone(),
                position: dummy_pos,
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "Rename",
            CoreOperation::Rename {
                uri: dummy_uri.clone(),
                position: dummy_pos,
                new_name: "new".to_string(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "ExportIndex",
            CoreOperation::ExportIndex {
                uri: dummy_uri.clone(),
                realm: Some(bogus.to_string()),
                include_blocks: false,
            },
        ),
        (
            "SearchWorkspace",
            CoreOperation::SearchWorkspace {
                query: Some("anything".to_string()),
                frontmatter_filter: None,
                property_filter: None,
                tag_filter: None,
                realm: Some(bogus.to_string()),
                limit: 20,
            },
        ),
        (
            "SearchForPattern",
            CoreOperation::SearchForPattern {
                pattern: "test".to_string(),
                include_glob: None,
                context_lines: 0,
                limit: 10,
                case_insensitive: false,
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "GraphAnalysis",
            CoreOperation::GraphAnalysis {
                realm: Some(bogus.to_string()),
                top_n_hubs: 10,
                include_clusters: false,
            },
        ),
        (
            "DependencyGraph",
            CoreOperation::DependencyGraph {
                realm: bogus.to_string(),
                format: "json".to_string(),
            },
        ),
        // SemanticSearch validates realm existence before checking feature flags,
        // so it returns "realm does not exist" regardless of feature config.
        (
            "SemanticSearch",
            CoreOperation::SemanticSearch {
                query: "anything".to_string(),
                realm: Some(bogus.to_string()),
                top_k: 5,
                min_score: 0.0,
            },
        ),
    ];

    for (label, op) in operations {
        let result = engine.execute(op).await;
        match &result {
            CoreOperationResult::Error(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("realm does not exist"),
                    "{label}: error message should mention 'realm does not exist', got: {msg}"
                );
            }
            other => panic!("{label}: expected Error for non-existent realm, got: {other:?}"),
        }
    }
}
