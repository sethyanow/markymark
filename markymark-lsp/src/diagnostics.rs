//! Diagnostic computation for markdown documents.
//!
//! This module wraps the shared implementation in `markymark-index` and
//! re-exports the LSP-facing types for use within the LSP server.

pub use markymark_core::engine::CoreDiagnostic as MarkyDiagnostic;
pub use markymark_core::engine::DiagnosticSeverity;

use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex};

/// Compute diagnostics for a document given its index and realm.
///
/// Delegates to the shared implementation in `markymark_index::diagnostics`.
pub fn compute_diagnostics(
    index: &DocumentIndex,
    realm: &RealmIndex,
    uri: &DocumentUri,
) -> Vec<MarkyDiagnostic> {
    markymark_index::compute_diagnostics(index, realm, uri)
}
