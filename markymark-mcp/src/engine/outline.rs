//! GetOutline operation handler.

use markymark_core::engine::CoreOperationResult;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::RealmIndex;

pub(crate) fn handle_get_outline(realm: &RealmIndex, uri: &DocumentUri) -> CoreOperationResult {
    match realm.get_any_document(uri) {
        Some(markymark_index::AnyDocumentIndex::Markdown(index)) => CoreOperationResult::Outline(
            index
                .headings()
                .iter()
                .map(|heading| heading.text.to_string())
                .collect(),
        ),
        Some(markymark_index::AnyDocumentIndex::Structured(index)) => CoreOperationResult::Outline(
            index
                .keys()
                .iter()
                .map(|k| {
                    let indent = "  ".repeat(k.depth);
                    format!("{indent}{}: {:?}", k.path, k.value_kind)
                })
                .collect(),
        ),
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}
