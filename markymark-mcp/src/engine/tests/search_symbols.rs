//! Tests for the MCP-level `search-symbols` engine operation.
//!
//! Open-at-execution decision (marky-n1h step 8): standalone file rather
//! than absorbed. The MCP-level test (this file) exercises realm routing
//! through the full RuntimeEngine dispatch; unit-level tests for search
//! ranking live in `engine::search::tests` (inside `src/engine/search.rs`).
//! Different concerns, different homes.

use super::*;

#[tokio::test]
async fn search_symbols_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# UniqueHeadingXYZ\n").unwrap();
    let engine = make_engine_with_custom_realm("search-realm", dir.path()).await;

    // Default realm should return no matches for the unique heading
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: None,
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(
            matches.is_empty(),
            "default realm should not have the heading"
        );
    } else {
        panic!("expected Symbols result");
    }

    // Named realm should find it
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: Some("search-realm".to_string()),
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(!matches.is_empty(), "named realm should have the heading");
    } else {
        panic!("expected Symbols result");
    }
}
