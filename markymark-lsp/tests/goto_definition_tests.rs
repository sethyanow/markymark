//! LSP goto_definition handler integration tests.

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
async fn test_goto_definition_wiki_link_to_document() {
    // Cursor on [[other-page]] -> should navigate to other-page.md
    // Line 4: "See [[other-page]] for details."
    // The wiki link text starts at char 4 ("[[") -- place cursor inside the link
    let (service, _socket, uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 8), // inside "other-page"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "goto_definition on [[other-page]] should return a location"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(
                loc.uri.as_str(),
                uri_other.as_str(),
                "should navigate to other-page.md"
            );
        }
        GotoDefinitionResponse::Array(locs) => {
            assert!(!locs.is_empty(), "should have at least one location");
            assert_eq!(locs[0].uri.as_str(), uri_other.as_str());
        }
        GotoDefinitionResponse::Link(links) => {
            assert!(!links.is_empty(), "should have at least one link");
            assert_eq!(links[0].target_uri.as_str(), uri_other.as_str());
        }
    }
}

#[tokio::test]
async fn test_goto_definition_wiki_link_to_heading() {
    // Cursor on [[other-page#details]] -> should navigate to heading in other doc
    // Line 6: "Also check [[other-page#details]] and [[#introduction]]."
    let (service, _socket, uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(6, 20), // inside "other-page#details"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "goto_definition on [[other-page#details]] should return a location"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(
                loc.uri.as_str(),
                uri_other.as_str(),
                "should navigate to other-page.md"
            );
            // The Details heading is on line 2 of other-page.md
            assert_eq!(
                loc.range.start.line, 2,
                "should point to the Details heading"
            );
        }
        GotoDefinitionResponse::Array(locs) => {
            assert!(!locs.is_empty());
            assert_eq!(locs[0].uri.as_str(), uri_other.as_str());
            assert_eq!(locs[0].range.start.line, 2);
        }
        GotoDefinitionResponse::Link(links) => {
            assert!(!links.is_empty());
            assert_eq!(links[0].target_uri.as_str(), uri_other.as_str());
        }
    }
}

#[tokio::test]
async fn test_goto_definition_wiki_link_same_page() {
    // Cursor on [[#introduction]] -> should navigate to heading in same doc
    // Line 6: "Also check [[other-page#details]] and [[#introduction]]."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(6, 45), // inside "#introduction"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "goto_definition on [[#introduction]] should return a location"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(
                loc.uri.as_str(),
                uri_main.as_str(),
                "should navigate within same document"
            );
            // Introduction heading is on line 2
            assert_eq!(
                loc.range.start.line, 2,
                "should point to Introduction heading"
            );
        }
        GotoDefinitionResponse::Array(locs) => {
            assert!(!locs.is_empty());
            assert_eq!(locs[0].uri.as_str(), uri_main.as_str());
            assert_eq!(locs[0].range.start.line, 2);
        }
        GotoDefinitionResponse::Link(links) => {
            assert!(!links.is_empty());
            assert_eq!(links[0].target_uri.as_str(), uri_main.as_str());
        }
    }
}

#[tokio::test]
async fn test_goto_definition_markdown_link_anchor() {
    // Cursor on [intro](#introduction) -> should navigate to heading
    // Line 8: "A markdown link: [intro](#introduction)"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(8, 30), // inside "#introduction" part of markdown link
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "goto_definition on markdown anchor link should return a location"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(loc.uri.as_str(), uri_main.as_str());
            assert_eq!(
                loc.range.start.line, 2,
                "should point to Introduction heading"
            );
        }
        GotoDefinitionResponse::Array(locs) => {
            assert!(!locs.is_empty());
            assert_eq!(locs[0].uri.as_str(), uri_main.as_str());
            assert_eq!(locs[0].range.start.line, 2);
        }
        GotoDefinitionResponse::Link(links) => {
            assert!(!links.is_empty());
            assert_eq!(links[0].target_uri.as_str(), uri_main.as_str());
        }
    }
}

#[tokio::test]
async fn test_goto_definition_on_heading_returns_none() {
    // Cursor on a heading text itself -> should return None
    // Line 0: "# Main Document"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(0, 5), // on "Main Document" text
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "goto_definition on a heading should return None (headings are not links)"
    );
}

#[tokio::test]
async fn test_goto_definition_on_plain_text_returns_none() {
    // Cursor on plain paragraph text -> should return None
    // Line 4: "See [[other-page]] for details."  -- place cursor on "for"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "goto_definition on plain text should return None"
    );
}

#[tokio::test]
async fn test_goto_definition_wiki_link_nonexistent_target() {
    // Cursor on a wiki link to a page that doesn't exist
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let uri_orphan: Uri = "file:///workspace/orphan.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/orphan.md").unwrap();
        state.open_document(core_uri, "See [[nonexistent-page]] here.\n".to_string());
    }

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_orphan.clone(),
            },
            position: Position::new(0, 10), // inside "nonexistent-page"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "goto_definition on a wiki link to nonexistent page should return None"
    );
}
