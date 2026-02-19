//! Integration tests for did_change debounce behavior.
//!
//! Verifies that rapid did_change events are coalesced: only the final
//! reparse fires, not one per keystroke.

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use std::str::FromStr;
use std::time::Duration;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

fn doc_uri() -> (Uri, DocumentUri) {
    let raw = "file:///test/debounce.md";
    let uri = Uri::from_str(raw).unwrap();
    let doc_uri = DocumentUri::new(raw).unwrap();
    (uri, doc_uri)
}

fn full_change(uri: Uri, version: i32, text: &str) -> DidChangeTextDocumentParams {
    DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier { uri, version },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }],
    }
}

/// Rapid did_change calls must NOT update the index until the debounce fires.
///
/// RED: Without debounce the index is updated synchronously on every call,
/// so the "immediately after" assertion fails.
#[tokio::test]
async fn test_debounce_defers_reparse_until_pause() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let (uri, doc_uri) = doc_uri();

    // Open with initial content.
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 0,
                text: "# Original".to_string(),
            },
        })
        .await;

    // Confirm initial state.
    {
        let state = backend.state().read().await;
        assert_eq!(
            state.get_document_text(&doc_uri),
            Some("# Original"),
            "initial text should be set after did_open"
        );
    }

    // Fire three rapid full-document changes with no delay.
    for i in 1..=3 {
        backend
            .did_change(full_change(uri.clone(), i, &format!("# Change {i}")))
            .await;
    }

    // Immediately after: state must still reflect the original text
    // (debounce has not fired yet).
    {
        let state = backend.state().read().await;
        assert_eq!(
            state.get_document_text(&doc_uri),
            Some("# Original"),
            "state should be stale immediately after rapid changes (debounce pending)"
        );
    }

    // Wait longer than the debounce delay (75 ms), then check final state.
    tokio::time::sleep(Duration::from_millis(200)).await;

    {
        let state = backend.state().read().await;
        assert_eq!(
            state.get_document_text(&doc_uri),
            Some("# Change 3"),
            "state should reflect final change after debounce fires"
        );
    }
}

/// A single did_change with no following changes must eventually apply.
#[tokio::test]
async fn test_single_change_applies_after_debounce() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let (uri, doc_uri) = doc_uri();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 0,
                text: "# Start".to_string(),
            },
        })
        .await;

    backend
        .did_change(full_change(uri.clone(), 1, "# Updated"))
        .await;

    // Wait for debounce.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Updated"),
        "single change should apply after debounce"
    );
}
