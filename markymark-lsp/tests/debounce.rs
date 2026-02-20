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

/// Closing a document while a debounce is pending must cancel the debounce.
///
/// Without the fix, the stale debounce task fires after did_close and re-applies
/// buffered changes to a document that may have been re-opened with fresh content.
///
/// RED: Without did_close cancellation, the stale debounce fires, applies
/// "# Stale Change" over the freshly opened "# Fresh Open".
#[tokio::test]
async fn test_did_close_cancels_pending_debounce() {
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

    // Fire a change — this starts the debounce timer and buffers the change.
    backend
        .did_change(full_change(uri.clone(), 1, "# Stale Change"))
        .await;

    // Close the document immediately while debounce is still pending.
    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })
        .await;

    // Re-open with fresh content (typical editor open-after-close).
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 0,
                text: "# Fresh Open".to_string(),
            },
        })
        .await;

    // Wait well past the debounce delay.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // State must reflect the fresh open, NOT the stale buffered change.
    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Fresh Open"),
        "stale debounce must be cancelled by did_close; fresh content must persist"
    );
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
