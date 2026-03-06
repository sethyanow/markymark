//! Integration tests for `search-block-text` MCP tool.
//!
//! Part of epic marky-z7uc: expose ContentBlock model via MCP tools.
//! These tests exercise the RuntimeEngine dispatch path end-to-end.

use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

/// Basic test: search across documents and find a matching block.
#[tokio::test]
async fn search_block_text_finds_matching_paragraph() {
    let ws = TempWorkspace::new("search-block-text-basic");
    fs::write(
        ws.root().join("doc1.md"),
        "# Introduction\n\nThis document discusses Rust programming.\n",
    )
    .expect("write doc1");
    fs::write(
        ws.root().join("doc2.md"),
        "# Summary\n\nPython is also popular.\n",
    )
    .expect("write doc2");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "Rust programming".to_string(),
            realm: None,
            kind_filter: None,
            limit: 10,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches { matches, query, .. } => {
            assert_eq!(query, "Rust programming");
            assert!(
                !matches.is_empty(),
                "should find at least one block matching 'Rust programming'"
            );
            // The match should include text when include_text=true
            let first = &matches[0];
            assert!(
                first.text.is_some(),
                "include_text=true should populate text field"
            );
            let text = first.text.as_deref().unwrap();
            assert!(
                text.to_lowercase().contains("rust programming"),
                "matched block text should contain query: {text}"
            );
        }
        CoreOperationResult::Error(e) => {
            panic!("expected BlockTextMatches, got error: {e}");
        }
        other => panic!("expected BlockTextMatches, got: {other:?}"),
    }
}

/// Verify case-insensitive matching.
#[tokio::test]
async fn search_block_text_case_insensitive() {
    let ws = TempWorkspace::new("search-block-text-case");
    fs::write(
        ws.root().join("test.md"),
        "# Title\n\nThe Quick Brown Fox Jumps Over The Lazy Dog.\n",
    )
    .expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "quick brown fox".to_string(),
            realm: None,
            kind_filter: None,
            limit: 10,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches { matches, .. } => {
            assert!(
                !matches.is_empty(),
                "case-insensitive search should match 'quick brown fox' in 'Quick Brown Fox'"
            );
        }
        other => panic!("expected BlockTextMatches, got: {other:?}"),
    }
}

/// Verify kind_filter restricts results to matching block kinds.
#[tokio::test]
async fn search_block_text_kind_filter() {
    let ws = TempWorkspace::new("search-block-text-kind");
    fs::write(
        ws.root().join("test.md"),
        "# Code\n\nThe function uses pattern matching.\n\n```rust\nfn pattern_match() {}\n```\n",
    )
    .expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    // Search for "pattern" but restrict to code_block kind
    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "pattern".to_string(),
            realm: None,
            kind_filter: Some("code_block".to_string()),
            limit: 10,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches { matches, .. } => {
            // All matches should be code blocks
            for m in &matches {
                assert_eq!(
                    m.kind, "code_block",
                    "kind_filter=code_block should only return code blocks"
                );
            }
        }
        other => panic!("expected BlockTextMatches, got: {other:?}"),
    }
}

/// Verify limit parameter bounds the number of results.
#[tokio::test]
async fn search_block_text_respects_limit() {
    let ws = TempWorkspace::new("search-block-text-limit");
    // Create multiple documents each containing "common keyword"
    for i in 0..5 {
        fs::write(
            ws.root().join(format!("doc{i}.md")),
            format!("# Doc {i}\n\nThis has a common keyword here.\n"),
        )
        .expect("write doc");
    }

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "common keyword".to_string(),
            realm: None,
            kind_filter: None,
            limit: 2,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches {
            matches, truncated, ..
        } => {
            assert!(
                matches.len() <= 2,
                "limit=2 should return at most 2 results, got {}",
                matches.len()
            );
            assert!(truncated, "should indicate results were truncated");
        }
        other => panic!("expected BlockTextMatches, got: {other:?}"),
    }
}

/// Verify include_text=false omits text from results.
#[tokio::test]
async fn search_block_text_omits_text_when_disabled() {
    let ws = TempWorkspace::new("search-block-text-no-text");
    fs::write(
        ws.root().join("test.md"),
        "# Title\n\nSearchable content here.\n",
    )
    .expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "Searchable content".to_string(),
            realm: None,
            kind_filter: None,
            limit: 10,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches { matches, .. } => {
            assert!(!matches.is_empty(), "should find match");
            for m in &matches {
                assert!(
                    m.text.is_none(),
                    "include_text=false should omit text from results"
                );
            }
        }
        other => panic!("expected BlockTextMatches, got: {other:?}"),
    }
}

/// Verify empty query returns an error (not all documents).
#[tokio::test]
async fn search_block_text_empty_query_returns_error() {
    let ws = TempWorkspace::new("search-block-text-empty");
    fs::write(ws.root().join("test.md"), "# Title\n\nContent.\n").expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "".to_string(),
            realm: None,
            kind_filter: None,
            limit: 10,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => {
            // Expected: empty query should be rejected
        }
        other => panic!("expected Error for empty query, got: {other:?}"),
    }
}

/// Verify no matches returns empty results (not an error).
#[tokio::test]
async fn search_block_text_no_matches_returns_empty() {
    let ws = TempWorkspace::new("search-block-text-nomatch");
    fs::write(
        ws.root().join("test.md"),
        "# Title\n\nSome unrelated content.\n",
    )
    .expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "xyzzy_nonexistent_term".to_string(),
            realm: None,
            kind_filter: None,
            limit: 10,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::BlockTextMatches {
            matches, truncated, ..
        } => {
            assert!(matches.is_empty(), "no matches expected for nonsense query");
            assert!(!truncated, "should not be truncated when empty");
        }
        other => panic!("expected BlockTextMatches (empty), got: {other:?}"),
    }
}

/// Verify non-existent realm returns error.
#[tokio::test]
async fn search_block_text_nonexistent_realm_returns_error() {
    let ws = TempWorkspace::new("search-block-text-badrealm");
    fs::write(ws.root().join("test.md"), "# Title\n\nContent.\n").expect("write test doc");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchBlockText {
            query: "Content".to_string(),
            realm: Some("nonexistent-realm".to_string()),
            kind_filter: None,
            limit: 10,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("nonexistent-realm"),
                "error should mention the realm name: {msg}"
            );
        }
        other => panic!("expected Error for nonexistent realm, got: {other:?}"),
    }
}
