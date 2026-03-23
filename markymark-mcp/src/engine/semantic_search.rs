use markymark_core::engine::CoreOperationResult;
use markymark_core::CoreError;

#[cfg(feature = "semantic-search")]
use super::search;
use super::{RuntimeEngine, DEFAULT_REALM};

impl RuntimeEngine {
    /// Handle the `SemanticSearch` operation: embed the query and search for
    /// semantically similar content within the specified realm.
    ///
    /// This method intentionally does NOT use `read_realm` — the three-phase
    /// lock protocol must clone the Arc and provider, then drop the read lock
    /// before the (potentially slow) embedding call in Phase 2.
    pub(super) async fn handle_semantic_search(
        &self,
        query: String,
        realm: Option<String>,
        top_k: u32,
        min_score: f32,
    ) -> CoreOperationResult {
        let realm_name = realm.unwrap_or_else(|| DEFAULT_REALM.to_string());

        // Validate realm existence regardless of semantic-search feature flag,
        // so that non-existent realm errors are consistent across feature configs.
        {
            let state = self.state.read().await;
            if !state.contains_key(&realm_name) {
                return CoreOperationResult::Error(CoreError::Message(format!(
                    "realm does not exist: {realm_name}"
                )));
            }
        }

        #[cfg(not(feature = "semantic-search"))]
        {
            let _ = (realm_name, query, top_k, min_score);
            CoreOperationResult::Error(CoreError::NotImplemented(
                "semantic-search feature is not enabled for markymark-mcp".to_string(),
            ))
        }

        #[cfg(feature = "semantic-search")]
        {
            // Early validation: reject empty query before touching the lock.
            if query.trim().is_empty() {
                return CoreOperationResult::Error(CoreError::Message(
                    "semantic query cannot be empty".to_string(),
                ));
            }

            // Phase 1: Clone the semantic Arc and embedding provider while
            // holding the read lock, then drop the lock.
            let (semantic_arc, provider) = {
                let state = self.state.read().await;
                // Realm was validated above; a missing entry here means a concurrent
                // remove raced with this operation.
                let realm_data = match state.get(&realm_name) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm_name}"
                        )));
                    }
                };
                let arc = match realm_data.index.semantic_index_arc() {
                    Some(arc) => arc,
                    None => {
                        return CoreOperationResult::SemanticMatches(Vec::new());
                    }
                };
                let provider = {
                    let guard = arc.lock().await;
                    guard.provider()
                };
                (arc, provider)
                // state (read guard) dropped here
            };

            // Phase 2: Embed the query outside any lock (slow: network / ONNX).
            let query_embedding = match provider.embed(&query).await {
                Ok(emb) => emb,
                Err(err) => {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "semantic search failed: {err}"
                    )));
                }
            };

            // Phase 3: In-memory index search inside the mutex (fast).
            search::handle_semantic_search_with_embedding(
                semantic_arc,
                query,
                &query_embedding,
                top_k,
                min_score,
            )
            .await
        }
    }
}
