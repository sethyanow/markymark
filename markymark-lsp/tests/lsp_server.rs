//! Comprehensive tests for the markymark-lsp crate.
//!
//! These tests drive the architecture by testing the convert, state, and server
//! modules directly without requiring the tower-lsp transport layer.

use markymark_core::{DocumentUri, Position, Range};
use markymark_lsp::convert;
use markymark_lsp::state::ServerState;
use tower_lsp_server::ls_types;

// ---------------------------------------------------------------------------
// Type conversion: Position
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_position_zero() {
    let lsp_pos = ls_types::Position::new(0, 0);
    let core_pos = convert::from_lsp_position(lsp_pos);
    assert_eq!(core_pos.line, 0);
    assert_eq!(core_pos.character, 0);
}

#[test]
fn test_from_lsp_position_nonzero() {
    let lsp_pos = ls_types::Position::new(42, 17);
    let core_pos = convert::from_lsp_position(lsp_pos);
    assert_eq!(core_pos.line, 42);
    assert_eq!(core_pos.character, 17);
}

#[test]
fn test_to_lsp_position() {
    let core_pos = Position::new(10, 5);
    let lsp_pos = convert::to_lsp_position(core_pos);
    assert_eq!(lsp_pos.line, 10);
    assert_eq!(lsp_pos.character, 5);
}

#[test]
fn test_position_roundtrip_lsp_to_core_to_lsp() {
    let original = ls_types::Position::new(99, 55);
    let core = convert::from_lsp_position(original);
    let roundtrip = convert::to_lsp_position(core);
    assert_eq!(roundtrip.line, original.line);
    assert_eq!(roundtrip.character, original.character);
}

#[test]
fn test_position_roundtrip_core_to_lsp_to_core() {
    let original = Position::new(7, 23);
    let lsp = convert::to_lsp_position(original);
    let roundtrip = convert::from_lsp_position(lsp);
    assert_eq!(roundtrip.line, original.line);
    assert_eq!(roundtrip.character, original.character);
}

// ---------------------------------------------------------------------------
// Type conversion: Range
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_range() {
    let lsp_range = ls_types::Range::new(
        ls_types::Position::new(1, 0),
        ls_types::Position::new(1, 10),
    );
    let core_range = convert::from_lsp_range(lsp_range);
    assert_eq!(core_range.start.line, 1);
    assert_eq!(core_range.start.character, 0);
    assert_eq!(core_range.end.line, 1);
    assert_eq!(core_range.end.character, 10);
}

#[test]
fn test_to_lsp_range() {
    let core_range = Range::new(Position::new(3, 5), Position::new(3, 15));
    let lsp_range = convert::to_lsp_range(core_range);
    assert_eq!(lsp_range.start.line, 3);
    assert_eq!(lsp_range.start.character, 5);
    assert_eq!(lsp_range.end.line, 3);
    assert_eq!(lsp_range.end.character, 15);
}

#[test]
fn test_range_roundtrip() {
    let original = ls_types::Range::new(
        ls_types::Position::new(5, 3),
        ls_types::Position::new(8, 20),
    );
    let core = convert::from_lsp_range(original);
    let roundtrip = convert::to_lsp_range(core);
    assert_eq!(roundtrip.start.line, original.start.line);
    assert_eq!(roundtrip.start.character, original.start.character);
    assert_eq!(roundtrip.end.line, original.end.line);
    assert_eq!(roundtrip.end.character, original.end.character);
}

// ---------------------------------------------------------------------------
// Type conversion: URI
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_uri_file() {
    let uri: ls_types::Uri = "file:///home/user/notes/readme.md".parse().unwrap();
    let doc_uri = convert::from_lsp_uri(&uri).expect("should convert file URI");
    assert_eq!(doc_uri.as_str(), "file:///home/user/notes/readme.md");
}

#[test]
fn test_to_lsp_uri_file() {
    let doc_uri = DocumentUri::new("file:///tmp/test.md").unwrap();
    let uri = convert::to_lsp_uri(&doc_uri).expect("should convert to URI");
    assert_eq!(uri.as_str(), "file:///tmp/test.md");
}

#[test]
fn test_uri_roundtrip() {
    let original: ls_types::Uri = "file:///workspace/docs/index.md".parse().unwrap();
    let doc_uri = convert::from_lsp_uri(&original).expect("from_lsp_uri");
    let roundtrip = convert::to_lsp_uri(&doc_uri).expect("to_lsp_uri");
    assert_eq!(roundtrip.as_str(), original.as_str());
}

// ---------------------------------------------------------------------------
// Server state: document lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_state_new_is_empty() {
    let state = ServerState::new();
    assert_eq!(state.document_count(), 0);
}

#[test]
fn test_state_open_document() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello\n\nWorld".to_string());

    assert_eq!(state.document_count(), 1);
    assert_eq!(state.get_document_text(&uri), Some("# Hello\n\nWorld"));
}

#[test]
fn test_state_open_document_creates_index() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Title\n\n## Section".to_string());

    let index = state.get_document_index(&uri);
    assert!(index.is_some(), "opening a document should create an index");
    let index = index.unwrap();
    assert_eq!(index.headings().len(), 2);
    assert_eq!(index.headings()[0].text, "Title");
    assert_eq!(index.headings()[1].text, "Section");
}

#[test]
fn test_state_change_document() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Old Title".to_string());

    state.change_document(&uri, "# New Title\n\n## Added".to_string());

    assert_eq!(
        state.get_document_text(&uri),
        Some("# New Title\n\n## Added")
    );
    let index = state.get_document_index(&uri).unwrap();
    assert_eq!(index.headings().len(), 2);
    assert_eq!(index.headings()[0].text, "New Title");
}

#[test]
fn test_state_close_document() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello".to_string());
    assert_eq!(state.document_count(), 1);

    state.close_document(&uri);
    assert_eq!(state.document_count(), 0);
    assert!(state.get_document_text(&uri).is_none());
    assert!(state.get_document_index(&uri).is_none());
}

#[test]
fn test_state_multiple_documents() {
    let mut state = ServerState::new();
    let uri_a = DocumentUri::new("file:///test/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///test/b.md").unwrap();

    state.open_document(uri_a.clone(), "# Doc A".to_string());
    state.open_document(uri_b.clone(), "# Doc B".to_string());

    assert_eq!(state.document_count(), 2);
    assert_eq!(state.get_document_text(&uri_a), Some("# Doc A"));
    assert_eq!(state.get_document_text(&uri_b), Some("# Doc B"));
}

#[test]
fn test_state_realm_cross_document_lookup() {
    let mut state = ServerState::new();
    let uri_a = DocumentUri::new("file:///test/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///test/b.md").unwrap();

    state.open_document(uri_a.clone(), "# Shared Heading".to_string());
    state.open_document(uri_b.clone(), "# Other\n\n## Shared Heading".to_string());

    let results = state.realm().lookup_heading("shared-heading");
    assert_eq!(
        results.len(),
        2,
        "heading should be found in both documents"
    );
}

#[test]
fn test_state_wiki_links_indexed() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(
        uri.clone(),
        "See [[other-page]] and [[another]]".to_string(),
    );

    let index = state.get_document_index(&uri).unwrap();
    let links = index.wiki_links();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].target, "other-page");
    assert_eq!(links[1].target, "another");
}

// ---------------------------------------------------------------------------
// Navigation: resolution through ServerState
// ---------------------------------------------------------------------------

mod navigation {
    use super::*;
    use markymark_index::resolution::{resolve_markdown_link, resolve_wiki_link, ResolvedTarget};

    /// Helper: build a ServerState with multiple documents.
    fn setup_workspace() -> (ServerState, DocumentUri, DocumentUri) {
        let mut state = ServerState::new();
        let uri_main = DocumentUri::new("file:///workspace/main.md").unwrap();
        let uri_other = DocumentUri::new("file:///workspace/other-page.md").unwrap();

        state.open_document(
            uri_main.clone(),
            concat!(
                "# Main Document\n",
                "\n",
                "## Introduction\n",
                "\n",
                "See [[other-page]] for details.\n",
                "\n",
                "Also check [[other-page#details]] and [[#introduction]].\n",
                "\n",
                "A markdown link: [intro](#introduction)\n",
            )
            .to_string(),
        );

        state.open_document(
            uri_other.clone(),
            concat!(
                "# Other Page\n",
                "\n",
                "## Details\n",
                "\n",
                "Some detailed content here.\n",
            )
            .to_string(),
        );

        (state, uri_main, uri_other)
    }

    #[test]
    fn test_resolve_wiki_link_to_document() {
        let (state, uri_main, uri_other) = setup_workspace();
        let result = resolve_wiki_link(state.realm(), &uri_main, "other-page", None);
        assert!(result.is_some(), "wiki link to other-page should resolve");
        match result.unwrap() {
            ResolvedTarget::Document(uri) => {
                assert_eq!(uri.as_str(), uri_other.as_str());
            }
            other => panic!("expected Document, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_wiki_link_to_heading_in_other_doc() {
        let (state, uri_main, uri_other) = setup_workspace();
        let result = resolve_wiki_link(state.realm(), &uri_main, "other-page", Some("details"));
        assert!(
            result.is_some(),
            "wiki link to other-page#details should resolve"
        );
        match result.unwrap() {
            ResolvedTarget::Heading { uri, slug, text } => {
                assert_eq!(uri.as_str(), uri_other.as_str());
                assert_eq!(slug, "details");
                assert_eq!(text, "Details");
            }
            other => panic!("expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_wiki_link_same_page_heading() {
        let (state, uri_main, _uri_other) = setup_workspace();
        let result = resolve_wiki_link(state.realm(), &uri_main, "", Some("introduction"));
        assert!(result.is_some(), "same-page heading link should resolve");
        match result.unwrap() {
            ResolvedTarget::Heading { uri, slug, text } => {
                assert_eq!(uri.as_str(), uri_main.as_str());
                assert_eq!(slug, "introduction");
                assert_eq!(text, "Introduction");
            }
            other => panic!("expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_markdown_link_same_page_anchor() {
        let (state, uri_main, _uri_other) = setup_workspace();
        let result = resolve_markdown_link(state.realm(), &uri_main, "", Some("introduction"));
        assert!(result.is_some(), "markdown anchor link should resolve");
        match result.unwrap() {
            ResolvedTarget::Heading { uri, slug, .. } => {
                assert_eq!(uri.as_str(), uri_main.as_str());
                assert_eq!(slug, "introduction");
            }
            other => panic!("expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_wiki_link_not_found() {
        let (state, uri_main, _) = setup_workspace();
        let result = resolve_wiki_link(state.realm(), &uri_main, "nonexistent-page", None);
        assert!(
            result.is_none(),
            "link to nonexistent page should return None"
        );
    }

    #[test]
    fn test_references_heading_found_by_wiki_links() {
        // When we ask for references to a heading, we should find all wiki links
        // that reference that heading's slug.
        let (state, uri_main, _) = setup_workspace();
        let index = state.get_document_index(&uri_main).unwrap();

        // The main doc has wiki links: [[other-page]], [[other-page#details]], [[#introduction]]
        let wiki_links = index.wiki_links();
        let heading_refs: Vec<_> = wiki_links
            .iter()
            .filter(|wl| wl.heading.as_deref() == Some("introduction"))
            .collect();
        assert_eq!(
            heading_refs.len(),
            1,
            "should find one wiki link referencing #introduction"
        );
    }

    #[test]
    fn test_hover_heading_returns_info() {
        // Hovering on a heading should return the heading text and level.
        let (state, uri_main, _) = setup_workspace();
        let index = state.get_document_index(&uri_main).unwrap();

        let heading = index.heading_by_slug("introduction");
        assert!(heading.is_some());
        let heading = heading.unwrap();
        assert_eq!(heading.text, "Introduction");
        assert_eq!(heading.level, 2);
    }

    #[test]
    fn test_document_symbols_returns_heading_hierarchy() {
        // Document symbols should return headings as a hierarchy.
        let (state, uri_main, _) = setup_workspace();
        let index = state.get_document_index(&uri_main).unwrap();
        let outline = index.outline();

        // Root has one h1 child
        assert_eq!(outline.children.len(), 1);
        let h1 = &outline.children[0];
        assert_eq!(h1.heading.as_ref().unwrap().text, "Main Document");

        // h1 has one h2 child: Introduction
        assert_eq!(h1.children.len(), 1);
        assert_eq!(
            h1.children[0].heading.as_ref().unwrap().text,
            "Introduction"
        );
    }
}

// ---------------------------------------------------------------------------
// Server initialization capabilities
// ---------------------------------------------------------------------------

mod capabilities {
    use markymark_lsp::server::create_service;
    use tower_lsp_server::ls_types::*;
    use tower_lsp_server::LanguageServer;

    /// Helper: create a Backend and call initialize to get capabilities.
    async fn get_capabilities() -> ServerCapabilities {
        let (service, _socket) = create_service();
        let backend = service.inner();
        let result = backend
            .initialize(InitializeParams::default())
            .await
            .expect("initialize should succeed");
        result.capabilities
    }

    #[tokio::test]
    async fn test_capabilities_text_document_sync() {
        let caps = get_capabilities().await;
        assert!(
            caps.text_document_sync.is_some(),
            "server should declare text document sync capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_definition_provider() {
        let caps = get_capabilities().await;
        assert!(
            caps.definition_provider.is_some(),
            "server should declare definition provider capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_references_provider() {
        let caps = get_capabilities().await;
        assert!(
            caps.references_provider.is_some(),
            "server should declare references provider capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_hover_provider() {
        let caps = get_capabilities().await;
        assert!(
            caps.hover_provider.is_some(),
            "server should declare hover provider capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_document_symbol_provider() {
        let caps = get_capabilities().await;
        assert!(
            caps.document_symbol_provider.is_some(),
            "server should declare document symbol provider capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_workspace_symbol_provider() {
        let caps = get_capabilities().await;
        assert!(
            caps.workspace_symbol_provider.is_some(),
            "server should declare workspace symbol provider capability"
        );
    }

    #[tokio::test]
    async fn test_capabilities_sync_kind_is_full() {
        // We use full sync for simplicity in v1 (not incremental).
        let caps = get_capabilities().await;
        match caps.text_document_sync {
            Some(TextDocumentSyncCapability::Options(opts)) => {
                assert_eq!(
                    opts.change,
                    Some(TextDocumentSyncKind::FULL),
                    "should use FULL text document sync"
                );
                assert_eq!(opts.open_close, Some(true), "should support open/close");
            }
            Some(TextDocumentSyncCapability::Kind(kind)) => {
                assert_eq!(kind, TextDocumentSyncKind::FULL);
            }
            None => panic!("text_document_sync should be Some"),
        }
    }
}

// ---------------------------------------------------------------------------
// Handler tests: LSP navigation methods on Backend (Phase 4.2)
// ---------------------------------------------------------------------------

mod handler_tests {
    use markymark_core::DocumentUri;
    use markymark_lsp::server::create_service;
    use tower_lsp_server::ls_types::*;
    use tower_lsp_server::LanguageServer;

    /// Helper: create a Backend pre-loaded with test documents.
    ///
    /// Returns the service, main URI, and other-page URI.
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
        // Cursor on [[other-page]] → should navigate to other-page.md
        // Line 4: "See [[other-page]] for details."
        // The wiki link text starts at char 4 ("[[") — place cursor inside the link
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
        // Cursor on [[other-page#details]] → should navigate to heading in other doc
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
        // Cursor on [[#introduction]] → should navigate to heading in same doc
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
        // Cursor on [intro](#introduction) → should navigate to heading
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
        // Cursor on a heading text itself → should return None
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
        // Cursor on plain paragraph text → should return None
        // Line 4: "See [[other-page]] for details."  — place cursor on "for"
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
        // Cursor on "## Introduction" → should return all wiki links referencing "introduction"
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
        // Cursor on "## Details" in other-page.md → line 2 of other-page.md
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
        // Cursor on heading → should return markdown with heading info
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
        // Cursor on [[other-page]] → should return info about target
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
        // Cursor on plain text → None
        // Line 4: "See [[other-page]] for details." — on "for"
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
        // Document with H1>H2 hierarchy → nested DocumentSymbol array
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
        // Empty document → empty or None
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
        // Document with multiple H1s → flat list of top-level symbols
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
        // Cursor on [[other-page#details]] → should return hover info about the heading target
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
}

// ---------------------------------------------------------------------------
// Workspace symbol tests (Phase: workspace/symbol handler)
// ---------------------------------------------------------------------------

mod workspace_symbol_tests {
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
            WorkspaceSymbolResponse::Flat(symbols) => {
                symbols.iter().map(|s| s.name.clone()).collect()
            }
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
    // Acceptance tests: document lifecycle → workspace/symbol
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
}

// ---------------------------------------------------------------------------
// Completion tests (Phase: textDocument/completion handler — feature-009)
// ---------------------------------------------------------------------------

mod completion_context_tests {
    use markymark_core::{DocumentUri, Position};
    use markymark_lsp::state::{CompletionContext, ServerState};

    #[test]
    fn test_detect_completion_context_wiki_link() {
        // Text ending with `[[no` should detect WikiLink context with partial "no".
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "Check [[no".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 10));
        assert_eq!(
            ctx,
            Some(CompletionContext::WikiLink {
                partial: "no".to_string()
            }),
            "should detect wiki link context with partial 'no'"
        );
    }

    #[test]
    fn test_detect_completion_context_wiki_link_empty() {
        // Text ending with `[[` should detect WikiLink context with empty partial.
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "Check [[".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 8));
        assert_eq!(
            ctx,
            Some(CompletionContext::WikiLink {
                partial: String::new()
            }),
            "should detect wiki link context with empty partial"
        );
    }

    #[test]
    fn test_detect_completion_context_wiki_link_heading() {
        // Text `[[MyPage#int` should detect WikiLinkHeading context.
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "See [[MyPage#int".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 16));
        assert_eq!(
            ctx,
            Some(CompletionContext::WikiLinkHeading {
                target: "MyPage".to_string(),
                partial: "int".to_string(),
            }),
            "should detect wiki link heading context"
        );
    }

    #[test]
    fn test_detect_completion_context_tag() {
        // Text `Tags: #pro` should detect Tag context (not inside [[).
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "Tags: #pro".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 10));
        assert_eq!(
            ctx,
            Some(CompletionContext::Tag {
                partial: "pro".to_string()
            }),
            "should detect tag context with partial 'pro'"
        );
    }

    #[test]
    fn test_detect_completion_context_block_ref() {
        // Text `Ref ((abc` should detect BlockRef context.
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "Ref ((abc".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 9));
        assert_eq!(
            ctx,
            Some(CompletionContext::BlockRef {
                partial: "abc".to_string()
            }),
            "should detect block ref context with partial 'abc'"
        );
    }

    #[test]
    fn test_detect_completion_context_none() {
        // Plain text with no trigger characters should return None.
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "Hello world".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 11));
        assert_eq!(
            ctx, None,
            "plain text should not trigger any completion context"
        );
    }
}

mod completion_result_tests {
    use markymark_core::{DocumentUri, Position};
    use markymark_lsp::state::{CompletionCandidateKind, ServerState};

    #[test]
    fn test_wiki_link_completion_returns_page_names() {
        // Open 3 documents, complete inside `[[` → returns all 3 page names.
        let mut state = ServerState::new();
        let uri_notes = DocumentUri::new("file:///test/notes.md").unwrap();
        let uri_readme = DocumentUri::new("file:///test/readme.md").unwrap();
        let uri_todo = DocumentUri::new("file:///test/todo.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(uri_notes, "# Notes\n".to_string());
        state.open_document(uri_readme, "# Readme\n".to_string());
        state.open_document(uri_todo, "# Todo\n".to_string());
        // The editing document triggers completion
        state.open_document(uri_editor.clone(), "Link to [[".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 10));
        assert!(
            !candidates.is_empty(),
            "wiki link completion should return page names"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"notes"),
            "should include 'notes' in completions; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"readme"),
            "should include 'readme' in completions; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"todo"),
            "should include 'todo' in completions; got: {:?}",
            labels
        );

        // All should be Page kind
        assert!(
            candidates
                .iter()
                .all(|c| c.kind == CompletionCandidateKind::Page),
            "all wiki link completions should be Page kind"
        );
    }

    #[test]
    fn test_wiki_link_completion_filters_by_partial() {
        // Open 3 documents, complete `[[no` → returns only "notes".
        let mut state = ServerState::new();
        let uri_notes = DocumentUri::new("file:///test/notes.md").unwrap();
        let uri_readme = DocumentUri::new("file:///test/readme.md").unwrap();
        let uri_todo = DocumentUri::new("file:///test/todo.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(uri_notes, "# Notes\n".to_string());
        state.open_document(uri_readme, "# Readme\n".to_string());
        state.open_document(uri_todo, "# Todo\n".to_string());
        state.open_document(uri_editor.clone(), "Link to [[no".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 12));
        assert!(
            !candidates.is_empty(),
            "wiki link completion with partial 'no' should return matches"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"notes"),
            "should include 'notes'; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"readme"),
            "should NOT include 'readme'; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"todo"),
            "should NOT include 'todo'; got: {:?}",
            labels
        );
    }

    #[test]
    fn test_heading_completion_returns_target_headings() {
        // Open a target document with headings, complete `[[target#` → returns headings.
        let mut state = ServerState::new();
        let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_target,
            "# Introduction\n\n## Getting Started\n\n## Advanced Topics\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "See [[target#".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
        assert!(
            !candidates.is_empty(),
            "heading completion should return headings from target document"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"Introduction"),
            "should include 'Introduction'; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"Getting Started"),
            "should include 'Getting Started'; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"Advanced Topics"),
            "should include 'Advanced Topics'; got: {:?}",
            labels
        );

        // All should be Heading kind
        assert!(
            candidates
                .iter()
                .all(|c| c.kind == CompletionCandidateKind::Heading),
            "all heading completions should be Heading kind"
        );
    }

    #[test]
    fn test_heading_completion_filters_by_partial() {
        // Complete `[[target#int` → returns only headings containing "int".
        let mut state = ServerState::new();
        let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_target,
            "# Introduction\n\n## Getting Started\n\n## Advanced Topics\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "See [[target#int".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 16));
        assert!(
            !candidates.is_empty(),
            "heading completion with partial 'int' should return matches"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.iter().any(|l| l.to_lowercase().contains("int")),
            "should include heading matching 'int'; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"Getting Started"),
            "should NOT include 'Getting Started'; got: {:?}",
            labels
        );
    }

    #[test]
    fn test_tag_completion_returns_tags() {
        // Open a document with tags, complete `#` → returns available tags.
        let mut state = ServerState::new();
        let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_source,
            "Some text with #rust and #programming tags.\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "Tags: #".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 7));
        assert!(
            !candidates.is_empty(),
            "tag completion should return available tags"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"rust"),
            "should include 'rust'; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"programming"),
            "should include 'programming'; got: {:?}",
            labels
        );

        // All should be Tag kind
        assert!(
            candidates
                .iter()
                .all(|c| c.kind == CompletionCandidateKind::Tag),
            "all tag completions should be Tag kind"
        );
    }

    #[test]
    fn test_tag_completion_filters_by_partial() {
        // Complete `#pro` → returns only matching tags.
        let mut state = ServerState::new();
        let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_source,
            "Some text with #rust and #programming tags.\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "Tags: #pro".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 10));
        assert!(
            !candidates.is_empty(),
            "tag completion with partial 'pro' should return matches"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"programming"),
            "should include 'programming'; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"rust"),
            "should NOT include 'rust'; got: {:?}",
            labels
        );
    }

    #[test]
    fn test_block_ref_completion_returns_block_ids() {
        // Open a document with block IDs, complete `((` → returns block IDs.
        let mut state = ServerState::new();
        let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_source,
            "Some paragraph ^abc123\n\nAnother paragraph ^def456\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "Ref ((".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 6));
        assert!(
            !candidates.is_empty(),
            "block ref completion should return block IDs"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"abc123"),
            "should include 'abc123'; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"def456"),
            "should include 'def456'; got: {:?}",
            labels
        );

        // All should be BlockRef kind
        assert!(
            candidates
                .iter()
                .all(|c| c.kind == CompletionCandidateKind::BlockRef),
            "all block ref completions should be BlockRef kind"
        );
    }

    #[test]
    fn test_block_ref_completion_filters_by_partial() {
        // Complete `((ab` → returns only matching block IDs.
        let mut state = ServerState::new();
        let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(
            uri_source,
            "Some paragraph ^abc123\n\nAnother paragraph ^def456\n".to_string(),
        );
        state.open_document(uri_editor.clone(), "Ref ((ab".to_string());

        let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
        assert!(
            !candidates.is_empty(),
            "block ref completion with partial 'ab' should return matches"
        );

        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"abc123"),
            "should include 'abc123'; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"def456"),
            "should NOT include 'def456'; got: {:?}",
            labels
        );
    }
}

mod completion_capability_tests {
    use markymark_lsp::server::create_service;
    use tower_lsp_server::ls_types::*;
    use tower_lsp_server::LanguageServer;

    #[tokio::test]
    async fn test_capabilities_completion_provider() {
        // Verify ServerCapabilities includes completion_provider.
        let (service, _socket) = create_service();
        let backend = service.inner();
        let result = backend
            .initialize(InitializeParams::default())
            .await
            .expect("initialize should succeed");
        let caps = result.capabilities;

        assert!(
            caps.completion_provider.is_some(),
            "server should declare completion provider capability"
        );
    }
}

mod completion_acceptance_tests {
    use markymark_core::{DocumentUri, Position};
    use markymark_lsp::state::ServerState;

    #[test]
    fn test_acceptance_completion_updates_after_document_change() {
        // Open a doc, get heading completions, change doc (add heading),
        // get completions again → new heading appears.
        let mut state = ServerState::new();
        let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

        state.open_document(uri_target.clone(), "# Original Heading\n".to_string());
        state.open_document(uri_editor.clone(), "See [[target#".to_string());

        // First completion: should include "Original Heading"
        let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
        assert!(
            !candidates.is_empty(),
            "should return heading completions from target document"
        );
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"Original Heading"),
            "should include 'Original Heading'; got: {:?}",
            labels
        );

        // Change the target document: add a new heading
        state.change_document(
            &uri_target,
            "# Original Heading\n\n## Added Later\n".to_string(),
        );

        // Second completion: should now include both headings
        let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"Original Heading"),
            "should still include 'Original Heading'; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"Added Later"),
            "should include newly added 'Added Later'; got: {:?}",
            labels
        );
    }

    #[test]
    fn test_acceptance_wiki_link_completion_excludes_current_document() {
        // Wiki link completion should NOT suggest the current document.
        // You shouldn't get a suggestion to link to yourself.
        let mut state = ServerState::new();
        let uri_a = DocumentUri::new("file:///test/alpha.md").unwrap();
        let uri_b = DocumentUri::new("file:///test/beta.md").unwrap();

        state.open_document(uri_a.clone(), "# Alpha\n\nLink: [[".to_string());
        state.open_document(uri_b, "# Beta\n".to_string());

        let candidates = state.completion_at(&uri_a, Position::new(2, 8));
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();

        assert!(
            labels.contains(&"beta"),
            "should include 'beta' from the other document; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"alpha"),
            "should NOT include 'alpha' (current document) in its own completions; got: {:?}",
            labels
        );
    }

    #[test]
    fn test_acceptance_tag_not_triggered_inside_wiki_link() {
        // A `#` inside `[[Page#heading` should be detected as WikiLinkHeading,
        // NOT as a Tag context. The wiki link context takes priority.
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/doc.md").unwrap();
        state.open_document(uri.clone(), "See [[Page#heading".to_string());

        let ctx = state.detect_completion_context(&uri, Position::new(0, 18));
        assert_eq!(
            ctx,
            Some(markymark_lsp::state::CompletionContext::WikiLinkHeading {
                target: "Page".to_string(),
                partial: "heading".to_string(),
            }),
            "# inside [[ should be WikiLinkHeading, not Tag"
        );
    }

    #[test]
    fn test_acceptance_closed_document_removed_from_completions() {
        // Open 2 docs, verify wiki link completion returns both,
        // close one, verify it no longer appears in completions.
        let mut state = ServerState::new();
        let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();
        let uri_keep = DocumentUri::new("file:///test/keep.md").unwrap();
        let uri_close = DocumentUri::new("file:///test/close-me.md").unwrap();

        state.open_document(uri_editor.clone(), "Link: [[".to_string());
        state.open_document(uri_keep.clone(), "# Keep\n".to_string());
        state.open_document(uri_close.clone(), "# Close Me\n".to_string());

        // Both should appear initially
        let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"keep"),
            "should include 'keep' initially; got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"close-me"),
            "should include 'close-me' initially; got: {:?}",
            labels
        );

        // Close one document
        state.close_document(&uri_close);

        // Only 'keep' should remain
        let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"keep"),
            "should still include 'keep' after close; got: {:?}",
            labels
        );
        assert!(
            !labels.contains(&"close-me"),
            "should NOT include 'close-me' after close; got: {:?}",
            labels
        );
    }
}
