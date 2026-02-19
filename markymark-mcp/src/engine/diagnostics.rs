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

/// Compute diagnostics for a single markdown document.
///
/// Returns `Error` if the document is not indexed in the given realm.
pub(crate) fn handle_get_diagnostics_file(
    realm_data: &RealmData,
    realm_name: &str,
    uri: &DocumentUri,
) -> CoreOperationResult {
    let Some(doc_index) = realm_data.index.get_document(uri) else {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "document not indexed: {}",
            uri.as_str()
        )));
    };

    let diags = compute_diagnostics(doc_index, &realm_data.index, uri);
    let items = if diags.is_empty() {
        vec![]
    } else {
        vec![(uri.clone(), diags)]
    };

    CoreOperationResult::Diagnostics {
        realm: realm_name.to_string(),
        items,
    }
}
