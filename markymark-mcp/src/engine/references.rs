//! FindReferences and Rename operation handlers.

use markymark_core::engine::CoreOperationResult;
use markymark_core::{CoreError, DocumentUri, Range};
use markymark_index::{AnyDocumentIndex, RealmIndex};

use crate::rename_ops::{compare_ranges, rename_heading, rename_xml_tag};

/// Sort locations by URI then range for deterministic output.
fn sort_locations(locations: &mut [(DocumentUri, Range)]) {
    locations.sort_by(|(uri_a, range_a), (uri_b, range_b)| {
        uri_a
            .as_str()
            .cmp(uri_b.as_str())
            .then_with(|| compare_ranges(*range_a, *range_b))
    });
}

pub(crate) fn handle_find_references(
    realm: &RealmIndex,
    uri: &DocumentUri,
    position: Range,
) -> CoreOperationResult {
    let cursor = position.start;
    match realm.get_any_document(uri) {
        Some(AnyDocumentIndex::Markdown(index)) => {
            if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                let slug = &heading.slug;
                let mut locations = Vec::new();

                for (doc_uri, doc_index) in realm.iter_documents() {
                    for wl in doc_index.wiki_links() {
                        if wl.heading == Some(slug) {
                            locations.push((doc_uri.clone(), wl.range));
                        }
                    }
                    for ml in doc_index.markdown_links() {
                        if ml.anchor == Some(slug) {
                            locations.push((doc_uri.clone(), ml.range));
                        }
                    }
                }

                sort_locations(&mut locations);

                return CoreOperationResult::Locations(locations);
            }

            if let Some(xml_tag) = index.xml_tags().iter().find(|x| x.range.contains(cursor)) {
                let tag_name = &xml_tag.tag_name;
                let mut locations = Vec::new();

                for (doc_uri, doc_index) in realm.iter_documents() {
                    for xt in doc_index.xml_tags() {
                        if xt.tag_name == *tag_name {
                            locations.push((doc_uri.clone(), xt.range));
                        }
                    }
                }

                sort_locations(&mut locations);

                return CoreOperationResult::Locations(locations);
            }

            // Block ref forward: cursor on ((uuid)) → find all docs with same uuid
            if let Some(block_ref) = index.block_refs().iter().find(|r| r.range.contains(cursor)) {
                let target_uuid = block_ref.uuid;
                let mut locations = Vec::new();

                for (doc_uri, doc_index) in realm.iter_documents() {
                    for br in doc_index.block_refs() {
                        if br.uuid == target_uuid {
                            locations.push((doc_uri.clone(), br.range));
                        }
                    }
                }

                sort_locations(&mut locations);

                return CoreOperationResult::Locations(locations);
            }

            // Block-id inverse: cursor on ^block-id → find all ((uuid)) refs to it
            let block_hit = index.block_ids().find_map(|id| {
                let entry = index.block_by_id(id)?;
                if entry.range.contains(cursor) {
                    Some(id.to_string())
                } else {
                    None
                }
            });
            if let Some(target_id) = block_hit {
                let mut locations = Vec::new();

                for (doc_uri, doc_index) in realm.iter_documents() {
                    for br in doc_index.block_refs() {
                        if br.uuid == target_id {
                            locations.push((doc_uri.clone(), br.range));
                        }
                    }
                }

                sort_locations(&mut locations);

                return CoreOperationResult::Locations(locations);
            }

            CoreOperationResult::Error(CoreError::Message(
                "no referenceable symbol at position".to_string(),
            ))
        }
        Some(AnyDocumentIndex::Structured(index)) => {
            if index.find_key_at_position(cursor).is_some() {
                CoreOperationResult::Locations(Vec::new())
            } else {
                CoreOperationResult::Error(CoreError::Message(
                    "no referenceable symbol at position".to_string(),
                ))
            }
        }
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}

pub(crate) fn handle_rename(
    realm: &RealmIndex,
    uri: &DocumentUri,
    position: Range,
    new_name: &str,
) -> CoreOperationResult {
    let cursor = position.start;
    match realm.get_any_document(uri) {
        Some(AnyDocumentIndex::Markdown(index)) => {
            if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                return rename_heading(realm, uri, heading.clone(), new_name);
            }

            if let Some(xml_tag) = index.xml_tags().iter().find(|x| x.range.contains(cursor)) {
                return rename_xml_tag(realm, xml_tag.tag_name, new_name);
            }

            CoreOperationResult::Error(CoreError::Message(
                "no renameable symbol at position".to_string(),
            ))
        }
        Some(AnyDocumentIndex::Structured(_)) => CoreOperationResult::Error(CoreError::Message(
            "rename is not supported for structured documents".to_string(),
        )),
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}
