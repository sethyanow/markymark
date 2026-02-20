//! Engine handler for `GetDiagnostics` operation.

use markymark_core::engine::{CoreDiagnostic, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri};
use markymark_index::compute_diagnostics;

use super::RealmData;

/// Compute diagnostics for all markdown documents in a realm.
///
/// Returns a `CoreOperationResult::Diagnostics` with only the files that have
/// at least one diagnostic. Files without issues are omitted.
pub(crate) fn handle_get_diagnostics_realm(
    realm_data: &RealmData,
    realm_name: &str,
) -> CoreOperationResult {
    let mut items: Vec<(DocumentUri, Vec<CoreDiagnostic>)> = Vec::new();

    for (uri, doc_index) in realm_data.index.iter_documents() {
        let diags = compute_diagnostics(doc_index, &realm_data.index, uri);
        if !diags.is_empty() {
            items.push((uri.clone(), diags));
        }
    }

    CoreOperationResult::Diagnostics {
        realm: realm_name.to_string(),
        items,
    }
}

/// Compute diagnostics for a single document.
///
/// For markdown documents, runs the full diagnostic suite. For structured
/// documents (JSON, YAML, TOML, etc.) returns empty diagnostics since the
/// diagnostic checks only apply to markdown. Returns `Error` only if the
/// document is not indexed at all.
pub(crate) fn handle_get_diagnostics_file(
    realm_data: &RealmData,
    realm_name: &str,
    uri: &DocumentUri,
) -> CoreOperationResult {
    let Some(any_doc) = realm_data.index.get_any_document(uri) else {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "document not indexed: {}",
            uri.as_str()
        )));
    };

    // Diagnostics only apply to markdown; structured docs get empty results.
    let items = match any_doc.as_markdown() {
        Some(doc_index) => {
            let diags = compute_diagnostics(doc_index, &realm_data.index, uri);
            if diags.is_empty() {
                vec![]
            } else {
                vec![(uri.clone(), diags)]
            }
        }
        None => vec![],
    };

    CoreOperationResult::Diagnostics {
        realm: realm_name.to_string(),
        items,
    }
}
