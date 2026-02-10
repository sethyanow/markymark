//! LSP document_symbol handler integration tests.

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

/// Helper: create a Backend pre-loaded with test documents.
///
/// Returns the service, socket, main URI, and other-page URI.
async fn setup_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_main: Uri = "file:///workspace/main.md".parse().unwrap();
    let uri_other: Uri = "file:///workspace/other-page.md".parse().unwrap();

    let main_text = concat!(
        "# Main Document\n",
        "\n",
        "## Introduction\n",
        "\n",
        "See [[other-page]] for details.\n",
        "\n",
        "Also check [[other-page#details]] and [[#introduction]].\n",
        "\n",
        "A markdown link: [intro](#introduction)\n",
    );

    let other_text = concat!(
        "# Other Page\n",
        "\n",
        "## Details\n",
        "\n",
        "Some detailed content here.\n",
    );

    // Populate state via the Backend's state handle
    {
        let mut state = backend.state().write().await;
        let core_main = DocumentUri::new("file:///workspace/main.md").unwrap();
        let core_other = DocumentUri::new("file:///workspace/other-page.md").unwrap();
        state.open_document(core_main, main_text.to_string());
        state.open_document(core_other, other_text.to_string());
    }

    (service, socket, uri_main, uri_other)
}

#[tokio::test]
async fn test_document_symbol_returns_heading_hierarchy() {
    // Document with H1>H2 hierarchy -> nested DocumentSymbol array
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: uri_main.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for a document with headings"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            // Should have at least the H1 "Main Document"
            assert!(
                !symbols.is_empty(),
                "should return at least one top-level symbol"
            );
            let h1 = &symbols[0];
            assert_eq!(h1.name, "Main Document");
            assert_eq!(h1.kind, SymbolKind::STRING);
            // H1 should have H2 "Introduction" as a child
            assert!(
                h1.children.as_ref().is_some_and(|c| !c.is_empty()),
                "H1 should have child symbols for nested headings"
            );
            let children = h1.children.as_ref().unwrap();
            assert_eq!(children[0].name, "Introduction");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            // Flat response is also acceptable; just verify headings present
            assert!(
                symbols.len() >= 2,
                "flat response should include at least 2 heading symbols"
            );
        }
    }
}

#[tokio::test]
async fn test_document_symbol_empty_document() {
    // Empty document -> empty or None
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    // Add an empty document
    let empty_uri: Uri = "file:///workspace/empty.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/empty.md").unwrap();
        state.open_document(core_uri, String::new());
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: empty_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    // Empty doc should return None or empty list
    let is_empty = match &result {
        None => true,
        Some(DocumentSymbolResponse::Nested(s)) => s.is_empty(),
        Some(DocumentSymbolResponse::Flat(s)) => s.is_empty(),
    };
    assert!(
        is_empty,
        "document_symbol for empty document should return empty or None"
    );
}

#[tokio::test]
async fn test_document_symbol_flat_headings() {
    // Document with multiple H1s -> flat list of top-level symbols
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let flat_uri: Uri = "file:///workspace/flat.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/flat.md").unwrap();
        state.open_document(core_uri, "# First\n\n# Second\n\n# Third\n".to_string());
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: flat_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for a document with headings"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            assert_eq!(symbols.len(), 3, "should have 3 top-level H1 symbols");
            assert_eq!(symbols[0].name, "First");
            assert_eq!(symbols[1].name, "Second");
            assert_eq!(symbols[2].name, "Third");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            assert_eq!(
                symbols.len(),
                3,
                "should have 3 heading symbols in flat mode"
            );
        }
    }
}

#[tokio::test]
async fn test_document_symbol_other_page() {
    // Verify symbols for other-page.md (H1 "Other Page" > H2 "Details")
    let (service, _socket, _, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: uri_other.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for other-page.md"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            assert_eq!(symbols.len(), 1, "should have 1 top-level H1");
            assert_eq!(symbols[0].name, "Other Page");
            let children = symbols[0].children.as_ref().unwrap();
            assert_eq!(children.len(), 1, "H1 should have 1 child H2");
            assert_eq!(children[0].name, "Details");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            assert!(symbols.len() >= 2, "should have at least 2 symbols");
        }
    }
}
