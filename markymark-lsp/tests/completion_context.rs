//! Completion context detection tests.

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

#[test]
fn test_detect_completion_context_xml_tag() {
    // Text ending with `<ag` should detect XmlTag context with partial "ag".
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "Content <ag".to_string());

    let ctx = state.detect_completion_context(&uri, Position::new(0, 11));
    assert_eq!(
        ctx,
        Some(CompletionContext::XmlTag {
            partial: "ag".to_string()
        }),
        "should detect XML tag context with partial 'ag'"
    );
}

#[test]
fn test_detect_completion_context_xml_tag_empty() {
    // Text ending with `<` at a word boundary should detect XmlTag with empty partial.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "Content <".to_string());

    let ctx = state.detect_completion_context(&uri, Position::new(0, 9));
    assert_eq!(
        ctx,
        Some(CompletionContext::XmlTag {
            partial: String::new()
        }),
        "should detect XML tag context with empty partial"
    );
}

#[test]
fn test_detect_completion_context_xml_tag_not_in_closed_tag() {
    // A closed tag `<agent>` should NOT trigger XML tag completion.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "<agent> text".to_string());

    let ctx = state.detect_completion_context(&uri, Position::new(0, 12));
    assert_eq!(ctx, None, "closed XML tag should not trigger completion");
}

// ---------------------------------------------------------------------------
// UTF-16 / byte-offset mismatch regression tests (marky-9cw)
// ---------------------------------------------------------------------------

#[test]
fn test_detect_completion_context_utf16_wiki_link_after_multibyte() {
    // "café [[no" — é (U+00E9) is 2 bytes in UTF-8 but 1 UTF-16 code unit.
    // Total: 11 bytes, 10 UTF-16 units. Cursor at character=10 (UTF-16) = byte 11.
    // Bug: using character=10 as byte index gives "café [[n" (missing 'o').
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "caf\u{00E9} [[no".to_string());

    // LSP position: line 0, character 10 (UTF-16 code units)
    let ctx = state.detect_completion_context(&uri, Position::new(0, 10));
    assert_eq!(
        ctx,
        Some(CompletionContext::WikiLink {
            partial: "no".to_string()
        }),
        "UTF-16 offset should be correctly converted to byte offset; \
         'no' is the full partial after '[[', not 'n'"
    );
}

#[test]
fn test_detect_completion_context_utf16_cursor_after_multibyte_no_panic() {
    // "café" — é (U+00E9) is 2 bytes in UTF-8 (0xC3 0xA9), 1 UTF-16 code unit.
    // UTF-8: [63, 61, 66, C3, A9] = 5 bytes. UTF-16: 4 code units.
    // Cursor at character=4 (UTF-16) = byte offset 5 (end of string).
    // Bug: col=4, line[..4] slices at byte 4 which is inside the é (0xC3) — PANICS.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "caf\u{00E9}".to_string());

    // Should NOT panic; cursor is at end of "café", no trigger => None
    let ctx = state.detect_completion_context(&uri, Position::new(0, 4));
    assert_eq!(
        ctx, None,
        "cursor after multi-byte char should not panic and should return None"
    );
}

#[test]
fn test_detect_completion_context_utf16_emoji_wiki_link() {
    // "🎉 [[yes" — 🎉 (U+1F389) is 4 bytes in UTF-8, 2 UTF-16 code units.
    // UTF-8 bytes: [F0,9F,8E,89, 20, 5B,5B, 79,65,73] = 10 bytes
    // UTF-16 units: [D83C,DF89, 0020, 005B,005B, 0079,0065,0073] = 8 units
    // Cursor at character=8 (UTF-16) = byte 10.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "\u{1F389} [[yes".to_string());

    let ctx = state.detect_completion_context(&uri, Position::new(0, 8));
    assert_eq!(
        ctx,
        Some(CompletionContext::WikiLink {
            partial: "yes".to_string()
        }),
        "emoji (surrogate pair = 2 UTF-16 units) should not break wiki link detection"
    );
}
