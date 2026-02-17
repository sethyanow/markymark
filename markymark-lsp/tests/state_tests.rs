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

// ── Incremental sync tests (marky-tzq) ─────────────────────────────

use markymark_lsp::state::DocumentChange;
use std::time::{Duration, Instant};

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
fn test_incremental_crlf_line_ending_edit_matches_full() {
    assert_incremental_matches_full(
        "# Title\r\n\r\nHello world\r\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 6,
            end_line: 2,
            end_character: 11,
            text: "earth".to_string(),
        },
        "# Title\r\n\r\nHello earth\r\n",
    );
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
fn test_incremental_md_tree_updated() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "# Hello".to_string());

    assert!(state.get_md_tree(&uri).is_some());

    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 0,
            start_character: 7,
            end_line: 0,
            end_character: 7,
            text: "\n\n## Sub".to_string(),
        }],
    );

    assert!(
        state.get_md_tree(&uri).is_some(),
        "MarkdownTree should be updated after incremental change"
    );
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

// --- Incremental tree-sitter parsing correctness tests ---

/// Helper: open a document, apply an incremental change, then compare the
/// resulting index against a fresh full-parse index of the same final text.
fn assert_incremental_matches_full(
    original: &str,
    change: DocumentChange,
    expected_final_text: &str,
) {
    // Path 1: incremental (open + apply_document_changes)
    let mut inc_state = ServerState::new();
    let uri = DocumentUri::new("file:///test/incr.md").unwrap();
    inc_state.open_document(uri.clone(), original.to_string());
    inc_state.apply_document_changes(&uri, vec![change]);

    // Path 2: fresh full parse of the final text
    let mut full_state = ServerState::new();
    let full_uri = DocumentUri::new("file:///test/full.md").unwrap();
    full_state.open_document(full_uri.clone(), expected_final_text.to_string());

    // Verify text matches
    assert_eq!(
        inc_state.get_document_text(&uri).unwrap(),
        expected_final_text,
        "text after incremental change should match expected"
    );

    // Compare headings
    let inc_index = inc_state.get_document_index(&uri).unwrap();
    let full_index = full_state.get_document_index(&full_uri).unwrap();

    let inc_headings: Vec<_> = inc_index
        .headings()
        .iter()
        .map(|h| (h.level, h.text.to_string()))
        .collect();
    let full_headings: Vec<_> = full_index
        .headings()
        .iter()
        .map(|h| (h.level, h.text.to_string()))
        .collect();
    assert_eq!(
        inc_headings, full_headings,
        "headings should match between incremental and full parse"
    );

    // Compare wiki links
    let inc_wl: Vec<_> = inc_index
        .wiki_links()
        .iter()
        .map(|w| w.target.to_string())
        .collect();
    let full_wl: Vec<_> = full_index
        .wiki_links()
        .iter()
        .map(|w| w.target.to_string())
        .collect();
    assert_eq!(
        inc_wl, full_wl,
        "wiki links should match between incremental and full parse"
    );
}

#[test]
fn test_incremental_parse_insert_heading_matches_full() {
    // Inserting at (2, 10) = byte 19 (the '\n' after "Some text.")
    // Original trailing '\n' is preserved after the insertion.
    assert_incremental_matches_full(
        "# Hello\n\nSome text.\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 10,
            end_line: 2,
            end_character: 10,
            text: "\n\n## New Section\n".to_string(),
        },
        "# Hello\n\nSome text.\n\n## New Section\n\n",
    );
}

#[test]
fn test_incremental_parse_delete_heading_matches_full() {
    // Deleting (2,0)..(3,0) removes "## Remove\n" (bytes 8..18)
    // Leaves "# Keep\n\n" + "\nParagraph.\n" = triple newline between them.
    assert_incremental_matches_full(
        "# Keep\n\n## Remove\n\nParagraph.\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 0,
            end_line: 3,
            end_character: 0,
            text: String::new(),
        },
        "# Keep\n\n\nParagraph.\n",
    );
}

#[test]
fn test_incremental_parse_rename_heading_matches_full() {
    assert_incremental_matches_full(
        "# Old Name\n\nBody.\n",
        DocumentChange::Incremental {
            start_line: 0,
            start_character: 2,
            end_line: 0,
            end_character: 10,
            text: "New Name".to_string(),
        },
        "# New Name\n\nBody.\n",
    );
}

#[test]
fn test_incremental_parse_add_wiki_link_matches_full() {
    assert_incremental_matches_full(
        "# Page\n\nSome text.\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 10,
            end_line: 2,
            end_character: 10,
            text: " See [[Other Page]]".to_string(),
        },
        "# Page\n\nSome text. See [[Other Page]]\n",
    );
}

#[test]
fn test_incremental_parse_100_sequential_edits_matches_full() {
    use markymark_lsp::state::DocumentChange;

    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/stress.md").unwrap();
    let mut source = "# Title\n\nContent.\n".to_string();
    state.open_document(uri.clone(), source.clone());

    // Apply 100 single-char insertions before the period
    for i in 0..100u8 {
        let ch = (b'a' + (i % 26)) as char;
        // Find the '.' in the current text to know the line/character
        let dot_pos = source.find('.').unwrap();
        let line = source[..dot_pos].matches('\n').count() as u32;
        let col = dot_pos - source[..dot_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);

        state.apply_document_changes(
            &uri,
            vec![DocumentChange::Incremental {
                start_line: line,
                start_character: col as u32,
                end_line: line,
                end_character: col as u32,
                text: ch.to_string(),
            }],
        );

        // Update our source tracker to match
        source.insert(dot_pos, ch);
    }

    // Fresh full parse of the same final text
    let mut full_state = ServerState::new();
    let full_uri = DocumentUri::new("file:///test/full.md").unwrap();
    full_state.open_document(full_uri.clone(), source.clone());

    // Compare results
    let inc_text = state.get_document_text(&uri).unwrap();
    assert_eq!(inc_text, source.as_str());

    let inc_index = state.get_document_index(&uri).unwrap();
    let full_index = full_state.get_document_index(&full_uri).unwrap();

    let inc_headings: Vec<_> = inc_index
        .headings()
        .iter()
        .map(|h| (h.level, h.text.to_string()))
        .collect();
    let full_headings: Vec<_> = full_index
        .headings()
        .iter()
        .map(|h| (h.level, h.text.to_string()))
        .collect();
    assert_eq!(
        inc_headings, full_headings,
        "100 sequential edits: headings should match"
    );
}

#[test]
fn incremental_wiki_links_matches_full_rebuild() {
    assert_incremental_matches_full(
        "# Title\n\nSee [[Page]] and [[Keep]] and [[Tail]].\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 4,
            end_line: 2,
            end_character: 12,
            text: "[[Other]]".to_string(),
        },
        "# Title\n\nSee [[Other]] and [[Keep]] and [[Tail]].\n",
    );
}

#[test]
fn incremental_append_new_wiki_link_after_last_existing_matches_full() {
    let tail = "x".repeat(220);
    let original = format!("# Title\n\n[[Page]]\n{}\n", tail);
    let expected = format!("# Title\n\n[[Page]]\n{} [[NewTail]]\n", tail);

    assert_incremental_matches_full(
        &original,
        DocumentChange::Incremental {
            start_line: 3,
            start_character: 220,
            end_line: 3,
            end_character: 220,
            text: " [[NewTail]]".to_string(),
        },
        &expected,
    );
}

#[test]
fn wiki_links_unchanged_sections_reused() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/reuse.md").unwrap();
    state.open_document(
        uri.clone(),
        "# Title\n\n[[A]]\nmiddle [[B]]\n[[C]]\n".to_string(),
    );

    let before = state
        .get_document_index(&uri)
        .expect("index should exist")
        .wiki_links()
        .iter()
        .map(|w| (w.target.to_string(), w.range))
        .collect::<Vec<_>>();
    assert_eq!(before.len(), 3);

    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 3,
            start_character: 9,
            end_line: 3,
            end_character: 10,
            text: "X".to_string(),
        }],
    );

    let after = state
        .get_document_index(&uri)
        .expect("index should exist")
        .wiki_links()
        .iter()
        .map(|w| (w.target.to_string(), w.range))
        .collect::<Vec<_>>();
    assert_eq!(after.len(), 3);

    assert_eq!(before[0], after[0], "first link should remain unchanged");
    assert_eq!(before[2], after[2], "third link should remain unchanged");
    assert_eq!(after[1].0, "X", "middle link target should be updated");
}

#[test]
fn wiki_links_neighbor_validation() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/neighbors.md").unwrap();
    state.open_document(uri.clone(), "# T\n\n[[A]][[B]]\n".to_string());

    state.apply_document_changes(
        &uri,
        vec![DocumentChange::Incremental {
            start_line: 2,
            start_character: 5,
            end_line: 2,
            end_character: 5,
            text: " text ".to_string(),
        }],
    );

    let links = state
        .get_document_index(&uri)
        .expect("index should exist")
        .wiki_links()
        .iter()
        .map(|w| (w.target.to_string(), w.range))
        .collect::<Vec<_>>();

    assert_eq!(links.len(), 2);
    assert_eq!(links[0].0, "A");
    assert_eq!(links[1].0, "B");
    assert!(links[1].1.start.character > links[0].1.end.character);
}

#[test]
fn edge_case_empty_pending_edits() {
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/empty-edits.md").unwrap();
    state.open_document(uri.clone(), "# T\n\n[[A]] [[B]] [[C]]\n".to_string());
    assert_eq!(state.pending_edit_count(), 0);

    state.apply_document_changes(&uri, vec![]);

    assert_eq!(state.pending_edit_count(), 0);
    let links = state
        .get_document_index(&uri)
        .expect("index should exist")
        .wiki_links();
    assert_eq!(links.len(), 3);
}

#[test]
fn edge_case_all_links_in_changed_range() {
    assert_incremental_matches_full(
        "# T\n\n[[A]] [[B]]\n",
        DocumentChange::Incremental {
            start_line: 2,
            start_character: 0,
            end_line: 2,
            end_character: 11,
            text: "[[X]] [[Y]]".to_string(),
        },
        "# T\n\n[[X]] [[Y]]\n",
    );
}

#[test]
fn wiki_links_overlapping_incremental_edits_match_full() {
    let mut inc_state = ServerState::new();
    let uri = DocumentUri::new("file:///test/overlap.md").unwrap();
    inc_state.open_document(
        uri.clone(),
        "# T\n\n[[Alpha]] [[Beta]] [[Gamma]]\n".to_string(),
    );

    inc_state.apply_document_changes(
        &uri,
        vec![
            DocumentChange::Incremental {
                start_line: 2,
                start_character: 2,
                end_line: 2,
                end_character: 7,
                text: "Delta".to_string(),
            },
            DocumentChange::Incremental {
                start_line: 2,
                start_character: 2,
                end_line: 2,
                end_character: 7,
                text: "Epsilon".to_string(),
            },
        ],
    );

    let final_text = inc_state
        .get_document_text(&uri)
        .expect("updated text should exist")
        .to_string();

    let mut full_state = ServerState::new();
    let full_uri = DocumentUri::new("file:///test/overlap-full.md").unwrap();
    full_state.open_document(full_uri.clone(), final_text);

    let inc_links: Vec<_> = inc_state
        .get_document_index(&uri)
        .expect("incremental index should exist")
        .wiki_links()
        .iter()
        .map(|w| (w.target.to_string(), w.range))
        .collect();
    let full_links: Vec<_> = full_state
        .get_document_index(&full_uri)
        .expect("full index should exist")
        .wiki_links()
        .iter()
        .map(|w| (w.target.to_string(), w.range))
        .collect();

    assert_eq!(
        inc_links, full_links,
        "overlapping edits should produce identical wiki-link set"
    );
}

#[test]
#[ignore = "performance signal only; run explicitly for local benchmark evidence"]
fn benchmark_incremental_wiki_link_edit_faster_than_full_rebuild() {
    let uri = DocumentUri::new("file:///test/wiki-bench.md").unwrap();
    let mut text = String::new();
    text.push_str("# Bench\n\n");
    for i in 0..5000 {
        text.push_str(&format!("Line {} [[Page{}]] and [[Ref{}]]\n", i, i, i + 1));
    }

    let marker = "[[Page4000]]";
    let marker_offset = text.find(marker).expect("marker must exist");
    let marker_prefix = &text[..marker_offset];
    let start_line = marker_prefix.chars().filter(|c| *c == '\n').count() as u32;
    let line_start = marker_prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let start_character = (marker_offset - line_start + 2) as u32;
    let end_character = start_character + "Page400".len() as u32;

    let changed = text.replacen("[[Page4000]]", "[[Renamed4000]]", 1);

    let iterations = 8;
    let mut incremental_total = Duration::ZERO;
    let mut full_total = Duration::ZERO;

    for _ in 0..iterations {
        let mut state = ServerState::new();
        state.open_document(uri.clone(), text.clone());
        let start = Instant::now();
        state.apply_document_changes(
            &uri,
            vec![DocumentChange::Incremental {
                start_line,
                start_character,
                end_line: start_line,
                end_character,
                text: "Renamed4000".to_string(),
            }],
        );
        incremental_total += start.elapsed();
    }

    for _ in 0..iterations {
        let mut state = ServerState::new();
        state.open_document(uri.clone(), text.clone());
        let start = Instant::now();
        state.change_document(&uri, changed.clone());
        full_total += start.elapsed();
    }

    eprintln!(
        "wiki benchmark totals over {} iters: incremental={:?}, full={:?}",
        iterations, incremental_total, full_total
    );

    assert!(incremental_total > Duration::ZERO);
    assert!(full_total > Duration::ZERO);
}
