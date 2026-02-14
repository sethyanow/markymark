//! Tests for navigation resolution through ServerState.

use markymark_core::DocumentUri;
use markymark_index::resolution::{resolve_markdown_link, resolve_wiki_link, ResolvedTarget};
use markymark_lsp::state::ServerState;

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
        .filter(|wl| wl.heading == Some("introduction"))
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
