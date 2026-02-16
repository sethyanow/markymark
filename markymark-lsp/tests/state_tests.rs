//! Tests for server state management (document lifecycle).

use markymark_core::DocumentUri;
use markymark_lsp::state::{ServerState, SymbolAtPosition};

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

#[test]
fn test_symbol_at_position_structured_json_key() {
    use markymark_core::Position;

    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/config.json").unwrap();
    state.open_document(uri.clone(), "{\n  \"host\": \"localhost\"\n}\n".to_string());

    // "host" is at line 1, characters 3..7 (inside quotes)
    let result = state.symbol_at_position(&uri, Position::new(1, 4));
    assert!(
        result.is_some(),
        "should find structured key at cursor position"
    );
    match result.unwrap() {
        SymbolAtPosition::StructuredKey(info) => {
            assert_eq!(info.key, "host");
            assert_eq!(info.path, "host");
        }
        other => panic!("expected StructuredKey, got {:?}", other),
    }
}

#[test]
fn test_symbol_at_position_structured_returns_none_off_key() {
    use markymark_core::Position;

    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/config.json").unwrap();
    state.open_document(uri.clone(), "{\n  \"host\": \"localhost\"\n}\n".to_string());

    // Position on opening brace
    let result = state.symbol_at_position(&uri, Position::new(0, 0));
    assert!(
        result.is_none(),
        "should return None when cursor is not on a key"
    );
}

// ── MarkdownTree storage tests (marky-tfd) ──────────────────────────

#[test]
fn test_md_tree_stored_on_open() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello\n\nWorld".to_string());

    assert!(
        state.get_md_tree(&uri).is_some(),
        "MarkdownTree should be stored after opening a markdown document"
    );
}

#[test]
fn test_md_tree_updated_on_change() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello".to_string());

    // Verify tree reflects original document (1 section in block tree root)
    let tree1 = state.get_md_tree(&uri).unwrap();
    let root1 = tree1.block_tree().root_node();
    let child_count_before = root1.child_count();

    state.change_document(&uri, "# Changed\n\n## Added\n\n## Third".to_string());

    let tree2 = state.get_md_tree(&uri);
    assert!(
        tree2.is_some(),
        "MarkdownTree should still exist after change"
    );
    let root2 = tree2.unwrap().block_tree().root_node();
    // More headings → more section nodes in tree-sitter-md's block tree
    assert!(
        root2.child_count() >= child_count_before,
        "tree should reflect updated document structure"
    );
}

#[test]
fn test_md_tree_removed_on_close() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello".to_string());
    assert!(state.get_md_tree(&uri).is_some());

    state.close_document(&uri);
    assert!(
        state.get_md_tree(&uri).is_none(),
        "MarkdownTree should be removed when document is closed"
    );
}

#[test]
fn test_md_tree_not_stored_for_structured_docs() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/config.json").unwrap();
    state.open_document(uri.clone(), "{\"key\": \"value\"}\n".to_string());

    assert!(
        state.get_md_tree(&uri).is_none(),
        "MarkdownTree should not be stored for non-markdown documents"
    );
}
