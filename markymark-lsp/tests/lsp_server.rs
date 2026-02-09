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
