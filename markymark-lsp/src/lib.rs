//! markymark-lsp: LSP server implementation using tower-lsp-server
//!
//! Provides Language Server Protocol support for markdown documents.
//!
//! ## Architecture
//!
//! - [`convert`]: Type conversions between `lsp_types` and `markymark_core` types
//! - [`diagnostics`]: Diagnostic generation (broken links, duplicate headings)
//! - [`incremental`]: Incremental re-indexing after document edits
//! - [`state`]: Document store, parsing, and indexing (transport-agnostic)
//! - [`server`]: `LanguageServer` trait implementation delegating to state + convert

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod convert;
pub mod diagnostics;
pub mod incremental;
pub mod server;
pub mod state;
mod symbols;

/// Run the LSP server over stdio transport.
///
/// Creates an LSP service and runs it on stdin/stdout until shutdown.
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = server::create_service();
    tower_lsp_server::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
