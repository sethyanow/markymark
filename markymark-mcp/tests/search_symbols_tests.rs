//! Regression tests for search-symbols covering structured-document key-path candidates.
//!
//! Split from `runtime_engine_tests.rs` as part of marky-n5w / marky-a90.
//! These tests validate that `SearchSymbols` correctly returns both markdown
//! heading candidates and structured-document key-path candidates.

mod common;

use std::fs;

use common::TempWorkspace;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

/// Returns the matched symbol names from a `CoreOperationResult::Symbols` result.
fn symbol_names(result: CoreOperationResult) -> Vec<String> {
    match result {
        CoreOperationResult::Symbols(matches) => {
            matches.into_iter().map(|(name, _, _)| name).collect()
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }
}

// ── heading candidate tests ──────────────────────────────────────────────────

/// search_symbols must return heading candidates from markdown files.
/// Regression test for marky-n5w: headings must still be returned after
/// the eager-alloc elimination refactor (Cow<'_, str> path).
#[test]
fn search_symbols_returns_headings_from_markdown() {
    let ws = TempWorkspace::new("headings-basic");
    fs::write(ws.root().join("a.md"), "# Introduction\n# Implementation\n")
        .expect("md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let names = symbol_names(engine.execute(CoreOperation::SearchSymbols {
        query: "intro".to_string(),
        realm: None,
    }));

    assert_eq!(names, vec!["Introduction".to_string()]);
}

// ── key-path candidate tests ─────────────────────────────────────────────────

/// search_symbols must return key-path candidates from JSON structured docs.
/// Regression test for marky-n5w: key-path candidates must survive the
/// Cow<'_, str> refactor of the candidates Vec.
#[test]
fn search_symbols_includes_json_key_paths() {
    let ws = TempWorkspace::new("json-key-paths");
    // JSON document with a known key that should be matched.
    fs::write(
        ws.root().join("config.json"),
        r#"{"database_host": "localhost", "database_port": 5432}"#,
    )
    .expect("json should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let names = symbol_names(engine.execute(CoreOperation::SearchSymbols {
        query: "database".to_string(),
        realm: None,
    }));

    // Both database_host and database_port should match.
    assert!(
        !names.is_empty(),
        "expected key-path matches for 'database', got none; names={names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("database")),
        "at least one result should contain 'database', got: {names:?}"
    );
}

/// search_symbols returns both markdown headings and JSON key paths in the
/// same result set when both match the query.  Regression test for marky-n5w.
#[test]
fn search_symbols_mixes_headings_and_key_paths() {
    let ws = TempWorkspace::new("mixed-heading-json");
    fs::write(ws.root().join("notes.md"), "# API Reference\n# API Guide\n")
        .expect("md should be created");
    fs::write(
        ws.root().join("config.json"),
        r#"{"api_key": "secret", "api_version": "v2"}"#,
    )
    .expect("json should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let names = symbol_names(engine.execute(CoreOperation::SearchSymbols {
        query: "api".to_string(),
        realm: None,
    }));

    assert!(
        !names.is_empty(),
        "expected matches for 'api', got none; names={names:?}"
    );

    let has_heading = names
        .iter()
        .any(|n| n.eq_ignore_ascii_case("API Reference") || n.eq_ignore_ascii_case("API Guide"));
    let has_key_path = names.iter().any(|n| n.contains("api_"));

    assert!(
        has_heading,
        "expected a heading result (API Reference or API Guide) for 'api', got: {names:?}"
    );
    assert!(
        has_key_path,
        "expected a key-path result (api_*) for 'api', got: {names:?}"
    );
}
