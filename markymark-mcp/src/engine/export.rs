//! ExportIndex operation handler.

use markymark_core::engine::{ContentBlockResult, CoreOperationResult};
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::document::{FrontmatterValueEntry, PropertyValueEntry};

/// Convert a `FrontmatterValueEntry` to a `Vec<String>` for the wire DTO.
fn fm_entry_to_strings(value: &FrontmatterValueEntry<'_>) -> Vec<String> {
    match value {
        FrontmatterValueEntry::String(s) => vec![s.to_string()],
        FrontmatterValueEntry::Integer(n) => vec![n.to_string()],
        FrontmatterValueEntry::Float(f) => vec![f.to_string()],
        FrontmatterValueEntry::Boolean(b) => vec![b.to_string()],
        FrontmatterValueEntry::List(items) => items.iter().flat_map(fm_entry_to_strings).collect(),
        FrontmatterValueEntry::Map(entries) => entries
            .iter()
            .map(|(k, v)| {
                let vs = fm_entry_to_strings(v).join(", ");
                format!("{k}: {vs}")
            })
            .collect(),
        FrontmatterValueEntry::Null => Vec::new(),
    }
}
use markymark_index::RealmIndex;

pub(crate) fn handle_export_index(
    realm: &RealmIndex,
    uri: &DocumentUri,
    include_blocks: bool,
) -> CoreOperationResult {
    match realm.get_any_document(uri) {
        Some(markymark_index::AnyDocumentIndex::Markdown(index)) => {
            let headings = index
                .headings()
                .iter()
                .map(|h| (h.text.to_string(), h.level, h.range))
                .collect();

            let xml_tags = index
                .xml_tags()
                .iter()
                .map(|x| (x.tag_name.to_string(), x.range))
                .collect();

            let wiki_links = index
                .wiki_links()
                .iter()
                .map(|wl| {
                    (
                        wl.target.to_string(),
                        wl.heading.map(|h| h.to_string()),
                        wl.range,
                    )
                })
                .collect();

            let markdown_links = index
                .markdown_links()
                .iter()
                .map(|ml| (ml.text.to_string(), ml.url.to_string(), ml.range))
                .collect();

            let frontmatter = index
                .frontmatter()
                .iter()
                .map(|e| {
                    let values = fm_entry_to_strings(&e.value);
                    (e.key.to_string(), values)
                })
                .collect();

            let properties = index
                .properties()
                .iter()
                .map(|e| {
                    let value = match &e.value {
                        PropertyValueEntry::String(s) => vec![s.to_string()],
                        PropertyValueEntry::PageRef(s) => vec![s.to_string()],
                        PropertyValueEntry::List(items) => {
                            items.iter().map(|s| s.to_string()).collect()
                        }
                    };
                    (e.key.to_string(), value)
                })
                .collect();

            let content_blocks = if include_blocks {
                let heading_list = index.headings();
                Some(
                    index
                        .content_blocks()
                        .iter()
                        .map(|b| {
                            let parent_slug = b
                                .parent_heading
                                .and_then(|idx| heading_list.get(idx).map(|h| h.slug.to_string()));
                            ContentBlockResult {
                                kind: super::helpers::block_kind_str(&b.kind).to_string(),
                                range: b.range,
                                parent_heading_index: b.parent_heading,
                                parent_heading_slug: parent_slug,
                                block_id: b.block_id.map(|s| s.to_string()),
                                text: Some(index.block_text(b).to_string()),
                            }
                        })
                        .collect(),
                )
            } else {
                None
            };

            CoreOperationResult::DocumentExport {
                uri: uri.clone(),
                document_kind: Some(DocumentKind::Markdown),
                headings,
                xml_tags,
                wiki_links,
                markdown_links,
                frontmatter,
                properties,
                content_blocks,
            }
        }
        Some(markymark_index::AnyDocumentIndex::Structured(index)) => {
            // For structured docs, export key paths as "headings"
            // with depth as level and key range as position.
            let headings = index
                .keys()
                .iter()
                .map(|k| {
                    (
                        k.path.clone(),
                        (k.depth as u8).saturating_add(1),
                        k.key_range,
                    )
                })
                .collect();

            CoreOperationResult::DocumentExport {
                uri: uri.clone(),
                document_kind: Some(index.kind()),
                headings,
                xml_tags: Vec::new(),
                wiki_links: Vec::new(),
                markdown_links: Vec::new(),
                frontmatter: Vec::new(),
                properties: Vec::new(),
                content_blocks: None,
            }
        }
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}
