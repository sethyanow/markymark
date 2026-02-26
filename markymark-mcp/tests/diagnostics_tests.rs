//! Integration tests for `get_diagnostics` via `RuntimeEngine`.

mod common;

use common::TempWorkspace;
use markymark_core::engine::{CoreDiagnostic, CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

/// Helper to extract `CoreOperationResult::Diagnostics` items.
fn extract_diagnostics(result: CoreOperationResult) -> Vec<(String, Vec<CoreDiagnostic>)> {
    match result {
        CoreOperationResult::Diagnostics { items, .. } => items
            .into_iter()
            .map(|(uri, diags)| (uri.as_str().to_string(), diags))
            .collect(),
        other => panic!("expected Diagnostics, got {other:?}"),
    }
}

// ---- Tests ----

#[tokio::test]
async fn get_diagnostics_realm_wide_finds_broken_wiki_link() {
    let ws = TempWorkspace::new("broken-wiki");
    ws.write("notes.md", "# Notes\n\n[[missing-page]]\n");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("engine should build");

    let result = engine
        .execute(CoreOperation::GetDiagnostics {
            uri: None,
            realm: None,
        })
        .await;

    let items = extract_diagnostics(result);
    let diags: Vec<_> = items.into_iter().flat_map(|(_, d)| d).collect();

    assert!(
        diags.iter().any(|d| d.message.contains("missing-page")),
        "expected a broken-link diagnostic for [[missing-page]], got: {diags:?}"
    );
}

#[tokio::test]
async fn get_diagnostics_single_file_finds_duplicate_heading() {
    let ws = TempWorkspace::new("dup-heading");
    ws.write(
        "doc.md",
        "# Overview\n\nSome text.\n\n# Overview\n\nMore text.\n",
    );
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("engine should build");

    // Get a file:// URI for the file
    let file_path = ws.root().join("doc.md");
    let uri = markymark_core::DocumentUri::from_file_path(&file_path);

    let result = engine
        .execute(CoreOperation::GetDiagnostics {
            uri: Some(uri),
            realm: None,
        })
        .await;

    let items = extract_diagnostics(result);
    let diags: Vec<_> = items.into_iter().flat_map(|(_, d)| d).collect();

    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Duplicate") || d.message.contains("duplicate")),
        "expected a duplicate-heading diagnostic, got: {diags:?}"
    );
}

#[tokio::test]
async fn get_diagnostics_clean_workspace_returns_empty() {
    let ws = TempWorkspace::new("clean");
    ws.write("a.md", "# Hello\n\nNo broken links here.\n");
    ws.write("b.md", "# World\n\nSee [[a]].\n");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("engine should build");

    let result = engine
        .execute(CoreOperation::GetDiagnostics {
            uri: None,
            realm: None,
        })
        .await;

    let items = extract_diagnostics(result);
    let all_diags: Vec<_> = items.into_iter().flat_map(|(_, d)| d).collect();

    assert!(
        all_diags.is_empty(),
        "clean workspace should have no diagnostics, got: {all_diags:?}"
    );
}

#[tokio::test]
async fn get_diagnostics_missing_realm_returns_error() {
    let ws = TempWorkspace::new("no-realm");
    ws.write("x.md", "# X\n");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("engine should build");

    let result = engine
        .execute(CoreOperation::GetDiagnostics {
            uri: None,
            realm: Some("nonexistent".to_string()),
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "nonexistent realm should return Error, got {result:?}"
    );
}

#[tokio::test]
async fn get_diagnostics_structured_doc_returns_empty_not_error() {
    let ws = TempWorkspace::new("structured-doc");
    ws.write("data.json", r#"{"key": "value"}"#);
    ws.write("notes.md", "# Notes\n\nSome text.\n");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("engine should build");

    // Get a file:// URI for the JSON file
    let file_path = ws.root().join("data.json");
    let uri = markymark_core::DocumentUri::from_file_path(&file_path);

    let result = engine
        .execute(CoreOperation::GetDiagnostics {
            uri: Some(uri),
            realm: None,
        })
        .await;

    // Should return empty diagnostics, NOT an error
    let items = extract_diagnostics(result);
    let all_diags: Vec<_> = items.into_iter().flat_map(|(_, d)| d).collect();
    assert!(
        all_diags.is_empty(),
        "structured doc should have empty diagnostics, not an error; got: {all_diags:?}"
    );
}
