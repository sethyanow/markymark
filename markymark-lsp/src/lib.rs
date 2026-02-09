//! markymark-lsp: LSP server implementation using tower-lsp-server
//!
//! Provides Language Server Protocol support for markdown documents.
//!
//! ## Architecture
//!
//! - [`convert`]: Type conversions between `lsp_types` and `markymark_core` types
//! - [`state`]: Document store, parsing, and indexing (transport-agnostic)
//! - [`server`]: `LanguageServer` trait implementation delegating to state + convert

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod convert;
pub mod server;
pub mod state;
