//! Tests for server state management (document lifecycle).

use markymark_core::DocumentUri;
use markymark_lsp::state::{DocumentChange, ServerState, SymbolAtPosition};

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
    let symbol = state.symbol_at_position(&uri, Position::new(1, 4));
    match symbol {
        Some(SymbolAtPosition::StructuredKey(info)) => {
            assert_eq!(info.key, "host");
            assert_eq!(info.path, "host");
        }
        Some(other) => panic!("expected StructuredKey, got {:?}", other),
        None => panic!("should find structured key at cursor position"),
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

// ── Incremental text edit tests ────────────────────────────────────────────────
// These tests verify that apply_document_changes correctly updates document text
// and rebuilds the index. They do not depend on any implementation internals.

#[test]
fn test_incremental_single_insert() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello\n\nWorld".to_string());

    // Insert " there" after "Hello" (line 0, char 7 = end of "# Hello")
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 7,
            end_line: 0,
            end_character: 7,
            text: " there".to_string(),
        }],
    );

    assert_eq!(
        state.get_document_text(&uri),
        Some("# Hello there\n\nWorld")
    );
}

#[test]
fn test_incremental_single_delete() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello World\n\nText".to_string());

    // Delete " World" (line 0, chars 7..13)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 7,
            end_line: 0,
            end_character: 13,
            text: String::new(),
        }],
    );

    assert_eq!(state.get_document_text(&uri), Some("# Hello\n\nText"));
}

#[test]
fn test_incremental_single_replace() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Old Title\n\nBody".to_string());

    // Replace "Old Title" with "New Title" (line 0, chars 2..11)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 2,
            end_line: 0,
            end_character: 11,
            text: "New Title".to_string(),
        }],
    );

    assert_eq!(state.get_document_text(&uri), Some("# New Title\n\nBody"));
    // Verify index was updated
    let index = state.get_document_index(&uri).unwrap();
    assert_eq!(index.headings()[0].text, "New Title");
}

#[test]
fn test_incremental_multiline_replace() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Title\n\nLine 1\nLine 2\nLine 3".to_string());

    // Replace "Line 1\nLine 2" with "Replaced" (line 2 char 0 to line 3 char 6)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 2,
            start_character: 0,
            end_line: 3,
            end_character: 6,
            text: "Replaced".to_string(),
        }],
    );

    assert_eq!(
        state.get_document_text(&uri),
        Some("# Title\n\nReplaced\nLine 3")
    );
}

#[test]
fn test_incremental_multiple_changes_in_sequence() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Title\n\nHello world".to_string());

    // Two changes applied in order:
    // 1) replace "world" (chars 6..11 on line 2) with "earth"
    //    text becomes: "# Title\n\nHello earth"
    // 2) insert "!" at end of line 2 (char 11 after "earth")
    //    text becomes: "# Title\n\nHello earth!"
    state.apply_document_changes(
        &uri,
        vec![
            DocumentChange::Incremental {
                start_line: 2,
                start_character: 6,
                end_line: 2,
                end_character: 11,
                text: "earth".to_string(),
            },
            DocumentChange::Incremental {
                start_line: 2,
                start_character: 11,
                end_line: 2,
                end_character: 11,
                text: "!".to_string(),
            },
        ],
    );

    assert_eq!(
        state.get_document_text(&uri),
        Some("# Title\n\nHello earth!")
    );
}

#[test]
fn test_incremental_full_replacement_fallback() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Old\n\nContent".to_string());

    // Full replacement (no range = full text swap)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Full(
            "# Brand New\n\nDifferent content".to_string(),
        )],
    );

    assert_eq!(
        state.get_document_text(&uri),
        Some("# Brand New\n\nDifferent content")
    );
    let index = state.get_document_index(&uri).unwrap();
    assert_eq!(index.headings()[0].text, "Brand New");
}

#[test]
fn test_incremental_utf16_accented_char() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    // "café" — é is U+00E9 (1 UTF-16 code unit, 2 UTF-8 bytes)
    state.open_document(uri.clone(), "# café\n\nText".to_string());

    // "# café" in UTF-16: # (1) + space (1) + c (1) + a (1) + f (1) + é (1) = 6 units
    // Insert "!" at end of heading (UTF-16 offset 6)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 6,
            end_line: 0,
            end_character: 6,
            text: "!".to_string(),
        }],
    );

    assert_eq!(state.get_document_text(&uri), Some("# café!\n\nText"));
}

#[test]
fn test_incremental_utf16_emoji_surrogate_pair() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    // 🎉 is U+1F389 (2 UTF-16 code units / surrogate pair, 4 UTF-8 bytes)
    state.open_document(uri.clone(), "# 🎉 Party\n\nText".to_string());

    // "# 🎉 Party" in UTF-16: # (1) + space (1) + 🎉 (2) + space (1) + P (1) + a (1) + r (1) + t (1) + y (1) = 10 units
    // "Party" starts at UTF-16 offset 5, ends at 10
    // Replace "Party" with "Time"
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 5,
            end_line: 0,
            end_character: 10,
            text: "Time".to_string(),
        }],
    );

    assert_eq!(state.get_document_text(&uri), Some("# 🎉 Time\n\nText"));
}

#[test]
fn test_incremental_edit_at_document_start() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "Hello".to_string());

    // Insert "# " at the very beginning
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 0,
            text: "# ".to_string(),
        }],
    );

    assert_eq!(state.get_document_text(&uri), Some("# Hello"));
}

#[test]
fn test_incremental_edit_at_document_end() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Title".to_string());

    // Append new paragraph at end (line 0, char 7 = end of "# Title")
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 7,
            end_line: 0,
            end_character: 7,
            text: "\n\nNew paragraph".to_string(),
        }],
    );

    assert_eq!(
        state.get_document_text(&uri),
        Some("# Title\n\nNew paragraph")
    );
}

#[test]
fn test_incremental_index_updated_after_changes() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# One\n\nSome text".to_string());

    // Add a second heading after "Some text" (line 2, char 9)
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 2,
            start_character: 9,
            end_line: 2,
            end_character: 9,
            text: "\n\n## Two".to_string(),
        }],
    );

    let index = state.get_document_index(&uri).unwrap();
    assert_eq!(index.headings().len(), 2);
    assert_eq!(index.headings()[0].text, "One");
    assert_eq!(index.headings()[1].text, "Two");
}

#[test]
fn test_incremental_no_change_for_unknown_uri() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/unknown.md").unwrap();

    // Should not panic — gracefully handles missing document
    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 5,
            text: "hello".to_string(),
        }],
    );

    assert!(state.get_document_text(&uri).is_none());
}

// ── Engine parity tests (marky-n78f) ─────────────────────────────────────────
// These tests verify that open_document produces correct index data. They pass
// now (via from_scan) and must continue to pass after migration (via from_blob).

#[test]
fn test_engine_parity_headings() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/headings.md").unwrap();
    state.open_document(uri.clone(), "# Foo\n## Bar\n### Baz\n".to_string());

    let index = state.get_document_index(&uri).unwrap();
    let headings = index.headings();
    assert_eq!(headings.len(), 3);

    assert_eq!(headings[0].text, "Foo");
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[0].slug, "foo");

    assert_eq!(headings[1].text, "Bar");
    assert_eq!(headings[1].level, 2);
    assert_eq!(headings[1].slug, "bar");

    assert_eq!(headings[2].text, "Baz");
    assert_eq!(headings[2].level, 3);
    assert_eq!(headings[2].slug, "baz");
}

#[test]
fn test_engine_parity_wiki_links() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/wiki.md").unwrap();
    state.open_document(
        uri.clone(),
        "# Page\n\nSee [[My Target]] and [[Other Doc]].\n".to_string(),
    );

    let index = state.get_document_index(&uri).unwrap();
    let links = index.wiki_links();
    assert_eq!(links.len(), 2, "should find exactly 2 wiki links");
    assert_eq!(links[0].target, "My Target");
    assert_eq!(links[1].target, "Other Doc");
}

#[test]
fn test_engine_parity_tags() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/tags.md").unwrap();
    state.open_document(
        uri.clone(),
        "# Note\n\nSome text #rust #testing here.\n".to_string(),
    );

    let index = state.get_document_index(&uri).unwrap();
    let tags: Vec<_> = index.tags().iter().collect();
    assert_eq!(tags.len(), 2, "should find exactly 2 tags");
    let tag_names: Vec<&str> = tags.iter().map(|t| t.name).collect();
    assert!(tag_names.contains(&"rust"), "should contain tag 'rust'");
    assert!(
        tag_names.contains(&"testing"),
        "should contain tag 'testing'"
    );
}

#[test]
fn test_engine_parity_block_ids() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/blocks.md").unwrap();
    state.open_document(
        uri.clone(),
        "# Doc\n\nSome block content. ^myblock\n".to_string(),
    );

    let index = state.get_document_index(&uri).unwrap();
    let block = index.block_by_id("myblock");
    assert!(block.is_some(), "block ^myblock should be indexed");
    assert_eq!(block.unwrap().id, "myblock");
}

#[test]
fn test_engine_lifecycle_open_and_close() {
    // Open a document → index is available. Close → index gone. Engine lifecycle.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/lifecycle.md").unwrap();

    state.open_document(uri.clone(), "# Title\n\n#tag [[link]]\n".to_string());
    assert_eq!(state.document_count(), 1);
    let index = state.get_document_index(&uri);
    assert!(index.is_some(), "index should exist after open");
    assert!(
        !index.unwrap().headings().is_empty(),
        "headings should be indexed"
    );

    state.close_document(&uri);
    assert_eq!(state.document_count(), 0);
    assert!(
        state.get_document_index(&uri).is_none(),
        "index should be gone after close"
    );
}
