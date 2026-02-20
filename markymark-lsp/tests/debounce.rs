//! Integration tests for did_change debounce behavior.
//!
//! Verifies that rapid did_change events are coalesced: only the final
//! reparse fires, not one per keystroke.

use markymark_core::DocumentUri;
use markymark_lsp::server::{create_service, DEBOUNCE_MS};
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
    tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS * 5)).await;

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
    tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS * 5)).await;

    // State must reflect the fresh open, NOT the stale buffered change.
    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Fresh Open"),
        "stale debounce must be cancelled by did_close; fresh content must persist"
    );
}

/// Closing a document while a debounce is pending must cancel the task;
/// the document must be absent from the index afterwards (not updated with stale change).
///
/// T2-9: close-during-debounce should cancel pending task and leave index unchanged.
#[tokio::test]
async fn test_close_during_debounce_index_unchanged() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let (uri, doc_uri) = doc_uri();

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

    // Fire a change — starts the debounce timer.
    backend
        .did_change(full_change(uri.clone(), 1, "# Stale Change"))
        .await;

    // Close immediately while debounce is still pending.
    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })
        .await;

    // Wait well past the debounce delay.
    tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS * 5)).await;

    // Document must be absent: close removed it and the debounce was cancelled,
    // so the stale change was never applied.
    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        None,
        "closed document must not appear in index even if a debounce was pending"
    );
}

/// A did_change with an empty content_changes list must be a no-op:
/// no debounce is scheduled and the document state is unchanged.
///
/// T3-1: empty change batch path.
#[tokio::test]
async fn test_empty_change_batch_is_noop() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let (uri, doc_uri) = doc_uri();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 0,
                text: "# Stable".to_string(),
            },
        })
        .await;

    // Send a did_change with no content changes.
    backend
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 1,
            },
            content_changes: vec![],
        })
        .await;

    // Wait past the debounce window — no task should have been scheduled.
    tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS * 5)).await;

    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Stable"),
        "empty change batch must be ignored; document state must remain unchanged"
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
    tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS * 5)).await;

    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Updated"),
        "single change should apply after debounce"
    );
}

/// Deterministic regression test for the close/reopen race condition (marky-aemm).
///
/// Exercises the exact race window: a debounce task drains pending_changes and
/// captures the generation counter, then a close/reopen happens before the task
/// can apply the changes. The generation counter must detect the mismatch and
/// discard the stale batch.
///
/// Uses Backend test helpers (drain_pending / try_apply_drained) to avoid
/// timing-dependent reproduction.
#[tokio::test]
async fn test_close_reopen_during_debounce_drain_window() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let (uri, doc_uri) = doc_uri();

    // 1. Open with initial content.
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

    // 2. Fire a change — this buffers pending_changes and starts a debounce timer.
    backend
        .did_change(full_change(uri.clone(), 1, "# Stale Change"))
        .await;

    // 3. Simulate the debounce task waking up and draining pending_changes.
    //    This captures the current generation and removes the abort handle,
    //    mirroring the real debounce task's drain step.
    let (drained_batches, captured_gen) = backend
        .drain_pending(&doc_uri)
        .expect("pending changes should exist after did_change");
    assert!(
        !drained_batches.is_empty(),
        "drained batches must not be empty"
    );

    // 4. Close the document — bumps generation, clears any remaining pending state.
    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })
        .await;

    // 5. Reopen with fresh content — bumps generation again.
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 0,
                text: "# Fresh Content".to_string(),
            },
        })
        .await;

    // 6. Now the debounce task tries to apply the drained stale batches.
    //    The generation counter should have changed (close + reopen = +2),
    //    so try_apply_drained must detect the mismatch and discard the batch.
    let applied = backend
        .try_apply_drained(&doc_uri, drained_batches, captured_gen)
        .await;
    assert!(
        !applied,
        "stale batch must be discarded when generation changed (close/reopen happened)"
    );

    // 7. Verify the document still has the fresh content from the reopen.
    let state = backend.state().read().await;
    assert_eq!(
        state.get_document_text(&doc_uri),
        Some("# Fresh Content"),
        "fresh content must persist; stale batch must not overwrite it"
    );
}

/// Closing a document must clean up its generation entry to prevent unbounded
/// growth of the document_generations HashMap (marky-jwsk).
///
/// RED: Without cleanup, each unique URI leaves a permanent entry after close.
#[tokio::test]
async fn test_close_cleans_up_generation_entries() {
    let (service, _socket) = create_service();
    let backend = service.inner();

    // Open and close 100 unique URIs.
    for i in 0..100 {
        let raw = format!("file:///test/cleanup_{i}.md");
        let uri = Uri::from_str(&raw).unwrap();

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 0,
                    text: format!("# Doc {i}"),
                },
            })
            .await;

        backend
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
            })
            .await;
    }

    // After closing all documents, generation entries should be cleaned up.
    let gen_count = backend.document_generations_count();
    assert_eq!(
        gen_count, 0,
        "document_generations should be empty after closing all documents, \
         but has {gen_count} entries (unbounded growth bug)"
    );
}

/// Generation entries for currently-open documents must be retained.
#[tokio::test]
async fn test_open_documents_retain_generation_entries() {
    let (service, _socket) = create_service();
    let backend = service.inner();

    // Open 5 documents.
    for i in 0..5 {
        let raw = format!("file:///test/retain_{i}.md");
        let uri = Uri::from_str(&raw).unwrap();

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 0,
                    text: format!("# Doc {i}"),
                },
            })
            .await;
    }

    // All 5 should have generation entries.
    assert_eq!(
        backend.document_generations_count(),
        5,
        "each open document should have a generation entry"
    );

    // Close 3 of them.
    for i in 0..3 {
        let raw = format!("file:///test/retain_{i}.md");
        let uri = Uri::from_str(&raw).unwrap();

        backend
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
            })
            .await;
    }

    // Only 2 should remain.
    assert_eq!(
        backend.document_generations_count(),
        2,
        "only open documents should retain generation entries"
    );
}
