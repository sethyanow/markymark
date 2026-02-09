//! LSP server: LanguageServer trait implementation using tower-lsp-server.

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService};

use crate::state::ServerState;

/// The LSP server backend.
pub struct Backend {
    /// The tower-lsp client for sending notifications.
    #[allow(dead_code)]
    client: Client,
    /// Shared server state behind a read-write lock.
    state: Arc<RwLock<ServerState>>,
}

impl Backend {
    /// Create a new Backend with the given tower-lsp client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ServerState::new())),
        }
    }

    /// Create a Backend with pre-existing state (for testing).
    pub fn with_state(client: Client, state: ServerState) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Get a reference to the shared state (for testing).
    pub fn state(&self) -> &Arc<RwLock<ServerState>> {
        &self.state
    }
}

/// Create an `LspService` and socket pair for the markymark LSP server.
pub fn create_service() -> (LspService<Backend>, tower_lsp_server::ClientSocket) {
    LspService::new(Backend::new)
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        // No-op
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            let mut state = self.state.write().await;
            state.open_document(doc_uri, params.text_document.text);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            if let Some(change) = params.content_changes.into_iter().last() {
                let mut state = self.state.write().await;
                state.change_document(&doc_uri, change.text);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            let mut state = self.state.write().await;
            state.close_document(&doc_uri);
        }
    }

    async fn goto_definition(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(None)
    }

    async fn hover(&self, _params: HoverParams) -> Result<Option<Hover>> {
        Ok(None)
    }

    async fn document_symbol(
        &self,
        _params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(None)
    }
}
