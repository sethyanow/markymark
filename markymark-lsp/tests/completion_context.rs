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
