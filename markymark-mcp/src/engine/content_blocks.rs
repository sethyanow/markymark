//! GetContentBlocks operation handler.

use markymark_core::engine::{ContentBlockResult, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri};
use markymark_index::RealmIndex;

/// Convert a `BlockKind` to its wire string.
fn block_kind_to_str(kind: markymark_index::document::BlockKind) -> &'static str {
    use markymark_index::document::BlockKind;
    match kind {
        BlockKind::Paragraph => "paragraph",
        BlockKind::ListItem => "list_item",
        BlockKind::CodeBlock => "code_block",
        BlockKind::BlockQuote => "blockquote",
        BlockKind::ThematicBreak => "thematic_break",
        BlockKind::Table => "table",
    }
}

pub(crate) fn handle_get_content_blocks(
    realm: &RealmIndex,
    uri: &DocumentUri,
    kind_filter: Option<String>,
    heading_filter: Option<String>,
    block_id: Option<String>,
    include_text: bool,
) -> CoreOperationResult {
    // Only markdown documents have content blocks.
    let index = match realm.get_any_document(uri) {
        Some(markymark_index::AnyDocumentIndex::Markdown(idx)) => idx,
        Some(_structured) => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "get-content-blocks is only supported for markdown documents, not: {}",
                uri.as_str()
            )));
        }
        None => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "document is not indexed: {}",
                uri.as_str()
            )));
        }
    };

    let headings = index.headings();
    let content_blocks = index.content_blocks();

    let blocks: Vec<ContentBlockResult> = content_blocks
        .iter()
        .filter(|block| {
            // Kind filter
            if let Some(ref kind_str) = kind_filter {
                if block_kind_to_str(block.kind) != kind_str.as_str() {
                    return false;
                }
            }

            // Heading filter: match against parent heading slug.
            if let Some(ref heading_slug) = heading_filter {
                match block.parent_heading {
                    Some(hi) => {
                        // Bounds-check before slug lookup (defensive).
                        let slug = headings.get(hi).map(|h| h.slug).unwrap_or("");
                        if slug != heading_slug.as_str() {
                            return false;
                        }
                    }
                    // Block has no parent heading: excluded when heading filter is active.
                    None => return false,
                }
            }

            // Block ID filter.
            if let Some(ref bid) = block_id {
                match block.block_id {
                    Some(id) if id == bid.as_str() => {}
                    _ => return false,
                }
            }

            true
        })
        .map(|block| {
            let parent_heading_slug = block
                .parent_heading
                .and_then(|hi| headings.get(hi).map(|h| h.slug.to_string()));
            let text = if include_text {
                Some(index.block_text(block).to_string())
            } else {
                None
            };

            ContentBlockResult {
                kind: block_kind_to_str(block.kind).to_string(),
                range: block.range,
                parent_heading_index: block.parent_heading,
                parent_heading_slug,
                block_id: block.block_id.map(|s| s.to_string()),
                text,
            }
        })
        .collect();

    CoreOperationResult::ContentBlocks {
        uri: uri.clone(),
        blocks,
    }
}
