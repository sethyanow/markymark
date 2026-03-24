use markymark_core::engine::{ContentBlockResult, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri};

use super::{helpers, RuntimeEngine};

impl RuntimeEngine {
    /// Handle the `GetContentBlocks` operation: look up the document in the
    /// specified realm and return content blocks filtered by kind, heading,
    /// and/or block ID.
    pub(super) async fn handle_get_content_blocks(
        &self,
        uri: DocumentUri,
        realm_name: Option<String>,
        kind_filter: Option<String>,
        heading_filter: Option<String>,
        block_id: Option<String>,
        include_text: bool,
    ) -> CoreOperationResult {
        let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
            Ok(v) => v,
            Err(e) => return e,
        };

        let Some(doc) = realm_data.index.get_document(&uri) else {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "document not found in realm \"{realm_key}\": {}",
                uri.as_str()
            )));
        };

        let headings = doc.headings();
        let content_blocks = doc.content_blocks();

        let blocks: Vec<ContentBlockResult> = content_blocks
            .iter()
            .filter(|b| {
                if let Some(ref kind) = kind_filter {
                    if helpers::block_kind_str(&b.kind) != kind.as_str() {
                        return false;
                    }
                }
                if let Some(ref heading_slug) = heading_filter {
                    let matches = b.parent_heading.is_some_and(|idx| {
                        headings
                            .get(idx)
                            .is_some_and(|h| h.slug == heading_slug.as_str())
                    });
                    if !matches {
                        return false;
                    }
                }
                if let Some(ref bid) = block_id {
                    if b.block_id != Some(bid.as_str()) {
                        return false;
                    }
                }
                true
            })
            .map(|b| {
                let parent_slug = b
                    .parent_heading
                    .and_then(|idx| headings.get(idx).map(|h| h.slug.to_string()));
                let text = if include_text {
                    Some(doc.block_text(b).to_string())
                } else {
                    None
                };
                ContentBlockResult {
                    kind: helpers::block_kind_str(&b.kind).to_string(),
                    range: b.range,
                    parent_heading_index: b.parent_heading,
                    parent_heading_slug: parent_slug,
                    block_id: b.block_id.map(|s| s.to_string()),
                    text,
                }
            })
            .collect();

        CoreOperationResult::ContentBlocks { uri, blocks }
    }
}
