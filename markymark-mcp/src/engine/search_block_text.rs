use markymark_core::engine::{BlockTextMatchResult, CoreOperationResult};
use markymark_core::CoreError;

use super::{helpers, RuntimeEngine};

impl RuntimeEngine {
    /// Handle the `SearchBlockText` operation: search for block-level text
    /// matches within the specified realm, with optional kind filtering.
    pub(super) async fn handle_search_block_text(
        &self,
        query: String,
        realm_name: Option<String>,
        kind_filter: Option<String>,
        limit: usize,
        include_text: bool,
    ) -> CoreOperationResult {
        // Reject empty/whitespace queries — they'd match everything.
        if query.trim().is_empty() {
            return CoreOperationResult::Error(CoreError::Message(
                "query must not be empty or whitespace-only".to_string(),
            ));
        }

        let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
            Ok(v) => v,
            Err(e) => return e,
        };

        // Parse kind filter string to BlockKind enum
        let block_kind_filter = kind_filter.as_deref().and_then(helpers::parse_block_kind);

        let (realm_matches, truncated) = realm_data.index.search_block_text(
            query.trim(),
            block_kind_filter,
            limit,
            include_text,
        );

        let matches = realm_matches
            .into_iter()
            .map(|m| BlockTextMatchResult {
                uri: m.uri,
                kind: helpers::block_kind_str(&m.kind).to_string(),
                range: m.range,
                parent_heading_slug: m.parent_heading_slug,
                block_id: m.block_id,
                text: m.text,
            })
            .collect();

        CoreOperationResult::BlockTextMatches {
            realm: realm_key,
            query,
            matches,
            truncated,
        }
    }
}
