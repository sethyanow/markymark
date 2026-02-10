//! Workspace symbol tests (workspace/symbol handler).

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

/// Helper: create a Backend pre-loaded with multiple test documents.
async fn setup_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    {
        let mut state = backend.state().write().await;

        let uri_a = DocumentUri::new("file:///workspace/notes.md").unwrap();
        state.open_document(
            uri_a,
            concat!(
                "# Introduction\n",
                "\n",
                "## Details\n",
                "\n",
                "Some content with #rust and #programming tags.\n",
            )
            .to_string(),
        );

        let uri_b = DocumentUri::new("file:///workspace/guide.md").unwrap();
        state.open_document(
            uri_b,
            concat!(
                "# Getting Started\n",
                "\n",
                "## Advanced Topics\n",
                "\n",
                "More content with #rust tag.\n",
            )
            .to_string(),
        );
    }

    (service, socket)
}

fn make_params(query: &str) -> WorkspaceSymbolParams {
    WorkspaceSymbolParams {
        query: query.to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

/// Extract symbol names from a WorkspaceSymbolResponse.
fn symbol_names(response: &WorkspaceSymbolResponse) -> Vec<String> {
    match response {
        WorkspaceSymbolResponse::Flat(symbols) => symbols.iter().map(|s| s.name.clone()).collect(),
        WorkspaceSymbolResponse::Nested(symbols) => {
            symbols.iter().map(|s| s.name.clone()).collect()
        }
    }
}

#[tokio::test]
async fn test_workspace_symbol_empty_query_returns_all() {
    // Empty query should return all headings from all documents.
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("")).await.unwrap();
    assert!(
        result.is_some(),
        "empty query should return all symbols, not None"
    );

    let names = symbol_names(result.as_ref().unwrap());
    // Should contain headings from both docs
    assert!(
        names.iter().any(|n| n == "Introduction"),
        "should include 'Introduction' from notes.md; got: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "Getting Started"),
        "should include 'Getting Started' from guide.md; got: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "Details"),
        "should include 'Details' from notes.md; got: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "Advanced Topics"),
        "should include 'Advanced Topics' from guide.md; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_workspace_symbol_filters_by_query() {
    // Query "intro" should match "Introduction" but not "Details".
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("intro")).await.unwrap();
    assert!(result.is_some(), "query 'intro' should return matches");

    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n.to_lowercase().contains("intro")),
        "should match 'Introduction'; got: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n == "Details"),
        "should NOT match 'Details'; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_workspace_symbol_case_insensitive() {
    // Query "INTRO" should match "Introduction" (case-insensitive).
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("INTRO")).await.unwrap();
    assert!(
        result.is_some(),
        "case-insensitive query 'INTRO' should return matches"
    );

    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n == "Introduction"),
        "should match 'Introduction' case-insensitively; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_workspace_symbol_includes_tags() {
    // Tags should appear in workspace symbol results.
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("rust")).await.unwrap();
    assert!(result.is_some(), "query 'rust' should match tag #rust");

    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n.contains("rust")),
        "should include tag 'rust' in results; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_workspace_symbol_returns_correct_locations() {
    // Verify returned symbols have correct URIs and valid ranges.
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("Introduction")).await.unwrap();
    assert!(result.is_some(), "should find 'Introduction'");

    match result.unwrap() {
        WorkspaceSymbolResponse::Flat(symbols) => {
            let sym = symbols
                .iter()
                .find(|s| s.name == "Introduction")
                .expect("should find 'Introduction' symbol");
            assert_eq!(
                sym.location.uri.as_str(),
                "file:///workspace/notes.md",
                "Introduction should be in notes.md"
            );
            // Line 0: "# Introduction"
            assert_eq!(
                sym.location.range.start.line, 0,
                "Introduction heading should be on line 0"
            );
        }
        WorkspaceSymbolResponse::Nested(symbols) => {
            let sym = symbols
                .iter()
                .find(|s| s.name == "Introduction")
                .expect("should find 'Introduction' symbol");
            match &sym.location {
                OneOf::Left(loc) => {
                    assert_eq!(
                        loc.uri.as_str(),
                        "file:///workspace/notes.md",
                        "Introduction should be in notes.md"
                    );
                    assert_eq!(
                        loc.range.start.line, 0,
                        "Introduction heading should be on line 0"
                    );
                }
                OneOf::Right(ws_loc) => {
                    assert_eq!(
                        ws_loc.uri.as_str(),
                        "file:///workspace/notes.md",
                        "Introduction should be in notes.md"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn test_workspace_symbol_no_documents() {
    // Empty state should return None or empty.
    let (service, _socket) = create_service();
    let backend = service.inner();

    let result = backend.symbol(make_params("")).await.unwrap();
    let is_empty = match &result {
        None => true,
        Some(WorkspaceSymbolResponse::Flat(v)) => v.is_empty(),
        Some(WorkspaceSymbolResponse::Nested(v)) => v.is_empty(),
    };
    assert!(
        is_empty,
        "workspace symbol on empty state should return None or empty"
    );
}

#[tokio::test]
async fn test_workspace_symbol_cross_document() {
    // Symbols from multiple documents should appear in results.
    let (service, _socket) = setup_workspace().await;
    let backend = service.inner();

    let result = backend.symbol(make_params("")).await.unwrap();
    assert!(result.is_some(), "should return symbols");

    // Collect URIs from all returned symbols
    let uris: Vec<String> = match result.unwrap() {
        WorkspaceSymbolResponse::Flat(symbols) => symbols
            .iter()
            .map(|s| s.location.uri.as_str().to_string())
            .collect(),
        WorkspaceSymbolResponse::Nested(symbols) => symbols
            .iter()
            .map(|s| match &s.location {
                OneOf::Left(loc) => loc.uri.as_str().to_string(),
                OneOf::Right(ws_loc) => ws_loc.uri.as_str().to_string(),
            })
            .collect(),
    };

    assert!(
        uris.iter().any(|u| u == "file:///workspace/notes.md"),
        "should include symbols from notes.md; got URIs: {:?}",
        uris
    );
    assert!(
        uris.iter().any(|u| u == "file:///workspace/guide.md"),
        "should include symbols from guide.md; got URIs: {:?}",
        uris
    );
}

// =======================================================================
// Acceptance tests: document lifecycle -> workspace/symbol
// =======================================================================

#[tokio::test]
async fn test_acceptance_new_documents_appear_in_workspace_symbols() {
    // Open 3 documents, verify workspace/symbol returns headings from all 3.
    let (service, _socket) = create_service();
    let backend = service.inner();

    {
        let mut state = backend.state().write().await;
        state.open_document(
            DocumentUri::new("file:///ws/alpha.md").unwrap(),
            "# Alpha Title\n".to_string(),
        );
        state.open_document(
            DocumentUri::new("file:///ws/beta.md").unwrap(),
            "# Beta Title\n".to_string(),
        );
        state.open_document(
            DocumentUri::new("file:///ws/gamma.md").unwrap(),
            "# Gamma Title\n".to_string(),
        );
    }

    let result = backend.symbol(make_params("")).await.unwrap();
    assert!(result.is_some(), "should return symbols from all 3 docs");
    let names = symbol_names(result.as_ref().unwrap());

    assert!(
        names.iter().any(|n| n == "Alpha Title"),
        "should include heading from alpha.md; got: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "Beta Title"),
        "should include heading from beta.md; got: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "Gamma Title"),
        "should include heading from gamma.md; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_acceptance_closed_document_removed_from_workspace_symbols() {
    // Open 2 docs, close 1, verify workspace/symbol only returns headings
    // from the remaining doc.
    let (service, _socket) = create_service();
    let backend = service.inner();

    let uri_keep = DocumentUri::new("file:///ws/keep.md").unwrap();
    let uri_close = DocumentUri::new("file:///ws/close-me.md").unwrap();

    {
        let mut state = backend.state().write().await;
        state.open_document(uri_keep.clone(), "# Keeper\n".to_string());
        state.open_document(uri_close.clone(), "# Temporary\n".to_string());
    }

    // Verify both are present initially
    let result = backend.symbol(make_params("")).await.unwrap();
    let names = symbol_names(result.as_ref().unwrap());
    assert!(names.iter().any(|n| n == "Keeper"));
    assert!(names.iter().any(|n| n == "Temporary"));

    // Close one document
    {
        let mut state = backend.state().write().await;
        state.close_document(&uri_close);
    }

    // Re-query: only Keeper should remain
    let result = backend.symbol(make_params("")).await.unwrap();
    assert!(
        result.is_some(),
        "should still return symbols from remaining doc"
    );
    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n == "Keeper"),
        "Keeper should still be present; got: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n == "Temporary"),
        "Temporary should be gone after close; got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_acceptance_changed_document_reflects_in_workspace_symbols() {
    // Open doc with "Old Title", change to "New Title", verify workspace/symbol
    // reflects the change (shows New Title, not Old Title).
    let (service, _socket) = create_service();
    let backend = service.inner();

    let uri = DocumentUri::new("file:///ws/mutable.md").unwrap();

    {
        let mut state = backend.state().write().await;
        state.open_document(uri.clone(), "# Old Title\n".to_string());
    }

    // Verify original heading
    let result = backend.symbol(make_params("")).await.unwrap();
    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n == "Old Title"),
        "should initially have 'Old Title'; got: {:?}",
        names
    );

    // Change the document
    {
        let mut state = backend.state().write().await;
        state.change_document(&uri, "# New Title\n".to_string());
    }

    // Re-query: should reflect the change
    let result = backend.symbol(make_params("")).await.unwrap();
    assert!(result.is_some(), "should return symbols after change");
    let names = symbol_names(result.as_ref().unwrap());
    assert!(
        names.iter().any(|n| n == "New Title"),
        "should show 'New Title' after change; got: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n == "Old Title"),
        "should NOT show 'Old Title' after change; got: {:?}",
        names
    );
}
