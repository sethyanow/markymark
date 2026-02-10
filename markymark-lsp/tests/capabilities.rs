//! Tests for server initialization capabilities.

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
async fn test_capabilities_workspace_symbol_provider() {
    let caps = get_capabilities().await;
    assert!(
        caps.workspace_symbol_provider.is_some(),
        "server should declare workspace symbol provider capability"
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

#[tokio::test]
async fn test_capabilities_completion_provider() {
    // Verify ServerCapabilities includes completion_provider.
    let caps = get_capabilities().await;
    assert!(
        caps.completion_provider.is_some(),
        "server should declare completion provider capability"
    );
}
