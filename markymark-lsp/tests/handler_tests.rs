//! LSP handler integration tests (goto_definition, references, hover, document_symbol).

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

// =======================================================================
// goto_definition
// =======================================================================

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

// =======================================================================
// references
// =======================================================================

#[tokio::test]
async fn test_references_for_heading() {
    // Cursor on "## Introduction" -> should return all wiki links referencing "introduction"
    // Line 2: "## Introduction"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 5), // on "Introduction"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for heading with incoming links should return locations"
    );
    let locs = result.unwrap();
    // [[#introduction]] on line 6 and [intro](#introduction) on line 8
    assert!(
        locs.len() >= 2,
        "should find at least 2 references to introduction: found {}",
        locs.len()
    );
}

#[tokio::test]
async fn test_references_for_heading_across_docs() {
    // Heading in other-page.md referenced from main.md via [[other-page#details]]
    // Cursor on "## Details" in other-page.md -> line 2 of other-page.md
    let (service, _socket, _uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_other.clone(),
            },
            position: Position::new(2, 5), // on "Details"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for heading referenced from another doc should return locations"
    );
    let locs = result.unwrap();
    // main.md has [[other-page#details]] on line 6
    assert!(
        !locs.is_empty(),
        "should find at least 1 cross-document reference to details"
    );
    // Verify at least one reference points to main.md
    let main_uri_str = "file:///workspace/main.md";
    assert!(
        locs.iter().any(|l| l.uri.as_str() == main_uri_str),
        "should include a reference from main.md"
    );
}

#[tokio::test]
async fn test_references_for_heading_no_refs() {
    // Cursor on "# Other Page" heading which has no incoming references
    // Line 0 of other-page.md: "# Other Page"
    let (service, _socket, _uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_other.clone(),
            },
            position: Position::new(0, 5), // on "Other Page"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    // No wiki links reference the "other-page" heading slug specifically,
    // so either None or empty list is acceptable.
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references for heading with no incoming links should be empty or None"
    );
}

// =======================================================================
// hover
// =======================================================================

#[tokio::test]
async fn test_hover_on_heading() {
    // Cursor on heading -> should return markdown with heading info
    // Line 2: "## Introduction"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 5), // on "Introduction"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on heading should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("Introduction"),
                "hover content should mention the heading text"
            );
        }
        HoverContents::Scalar(MarkedString::String(s)) => {
            assert!(s.contains("Introduction"));
        }
        HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
            assert!(ls.value.contains("Introduction"));
        }
        HoverContents::Array(arr) => {
            let text: String = arr
                .iter()
                .map(|m| match m {
                    MarkedString::String(s) => s.clone(),
                    MarkedString::LanguageString(ls) => ls.value.clone(),
                })
                .collect();
            assert!(text.contains("Introduction"));
        }
    }
}

#[tokio::test]
async fn test_hover_on_wiki_link() {
    // Cursor on [[other-page]] -> should return info about target
    // Line 4: "See [[other-page]] for details."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 8), // inside "other-page"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link should return hover info about the target"
    );
}

#[tokio::test]
async fn test_hover_on_plain_text_returns_none() {
    // Cursor on plain text -> None
    // Line 4: "See [[other-page]] for details." -- on "for"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(result.is_none(), "hover on plain text should return None");
}

// =======================================================================
// document_symbol
// =======================================================================

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

// =======================================================================
// Additional edge case tests
// =======================================================================

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

#[tokio::test]
async fn test_references_on_plain_text_returns_none() {
    // Cursor on plain text should not return references
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references on plain text should return empty or None"
    );
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

#[tokio::test]
async fn test_hover_on_wiki_link_with_heading() {
    // Cursor on [[other-page#details]] -> should return hover info about the heading target
    // Line 6: "Also check [[other-page#details]] and [[#introduction]]."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(6, 20), // inside "other-page#details"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link with heading fragment should return info"
    );
}
