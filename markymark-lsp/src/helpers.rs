//! Free helper functions extracted from `server.rs` to keep it under 1000 lines.

use std::collections::HashMap;

use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Location;

use crate::state::{ServerState, StructuredKeyInfo};
use markymark_core::{DocumentUri, Range as CoreRange};
use markymark_index::resolution::ResolvedTarget;
use markymark_index::DocumentIndex;

/// Convert a `ResolvedTarget` to an `ls_types::Location`, looking up heading/block ranges.
pub(crate) fn resolved_target_to_location(
    state: &ServerState,
    target: &ResolvedTarget,
) -> Result<Option<Location>> {
    let zero_range = CoreRange::new(
        markymark_core::Position::new(0, 0),
        markymark_core::Position::new(0, 0),
    );

    match target {
        ResolvedTarget::Document(uri) => crate::convert::to_lsp_location(uri, zero_range)
            .map(Some)
            .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error()),
        ResolvedTarget::Heading { uri, slug, .. } => {
            let range = state
                .get_document_index(uri)
                .and_then(|idx| idx.heading_by_slug(slug))
                .map(|h| h.range)
                .unwrap_or(zero_range);
            crate::convert::to_lsp_location(uri, range)
                .map(Some)
                .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error())
        }
        ResolvedTarget::Block { uri, id } => {
            let range = state
                .get_document_index(uri)
                .and_then(|idx| idx.block_by_id(id))
                .map(|b| b.range)
                .unwrap_or(zero_range);
            crate::convert::to_lsp_location(uri, range)
                .map(Some)
                .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error())
        }
        ResolvedTarget::KeyPath { uri, range, .. } => crate::convert::to_lsp_location(uri, *range)
            .map(Some)
            .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error()),
    }
}

/// Iterate over all `(DocumentUri, DocumentIndex)` pairs in the realm.
pub(crate) fn iter_realm_documents(
    state: &ServerState,
) -> impl Iterator<Item = (&DocumentUri, &DocumentIndex)> {
    state.realm().iter_documents()
}

#[derive(Debug, Default)]
pub(crate) struct XmlHoverStats {
    pub(crate) occurrences: usize,
    pub(crate) document_count: usize,
    pub(crate) attribute_counts: Vec<(String, usize)>,
}

pub(crate) fn xml_hover_stats(state: &ServerState, tag_name: &str) -> XmlHoverStats {
    let mut occurrences = 0usize;
    let mut document_count = 0usize;
    let mut attribute_counts: HashMap<String, usize> = HashMap::new();

    for (_uri, index) in iter_realm_documents(state) {
        let mut has_tag_in_document = false;
        for tag in index.xml_tags() {
            if tag.tag_name != tag_name {
                continue;
            }
            has_tag_in_document = true;
            occurrences += 1;
            for attr_name in tag.attributes.keys() {
                *attribute_counts
                    .entry((*attr_name).to_string())
                    .or_insert(0) += 1;
            }
        }

        if has_tag_in_document {
            document_count += 1;
        }
    }

    let mut attribute_counts: Vec<(String, usize)> = attribute_counts.into_iter().collect();
    attribute_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    XmlHoverStats {
        occurrences,
        document_count,
        attribute_counts,
    }
}

/// Build hover markdown for a structured document key.
pub(crate) fn structured_key_hover_markdown(info: &StructuredKeyInfo) -> String {
    let mut lines = Vec::new();
    lines.push(format!("**Key:** `{}`", info.path));
    lines.push(format!("**Type:** {:?}", info.value_kind));
    lines.push(format!("**Depth:** {}", info.depth));
    lines.push(format!("**Format:** {:?}", info.document_kind));
    lines.join("\n\n")
}
