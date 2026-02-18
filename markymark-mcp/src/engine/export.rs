//! ExportIndex operation handler.

use markymark_core::engine::CoreOperationResult;
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::document::{FrontmatterValueEntry, PropertyValueEntry};
use markymark_index::RealmIndex;

pub(crate) fn handle_export_index(realm: &RealmIndex, uri: &DocumentUri) -> CoreOperationResult {
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
                    let values = match &e.value {
                        FrontmatterValueEntry::String(s) => vec![s.to_string()],
                        FrontmatterValueEntry::List(items) => {
                            items.iter().map(|s| s.to_string()).collect()
                        }
                    };
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

            CoreOperationResult::DocumentExport {
                uri: uri.clone(),
                document_kind: Some(DocumentKind::Markdown),
                headings,
                xml_tags,
                wiki_links,
                markdown_links,
                frontmatter,
                properties,
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
            }
        }
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}
