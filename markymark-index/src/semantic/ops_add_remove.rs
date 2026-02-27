use crate::DocumentIndex;
use markymark_core::prelude::*;

use super::helpers::{fallback_heading, token_hashes};
use super::{SemanticEntry, SemanticIndex};

impl SemanticIndex {
    /// Add (or replace) semantic entries for a document.
    ///
    /// If the document has headings, one semantic entry is generated per
    /// heading. If it has no headings, a single fallback entry based on the
    /// document file stem is created.
    pub async fn add_document(
        &mut self,
        uri: DocumentUri,
        index: &DocumentIndex,
    ) -> Result<(), EmbedError> {
        self.add_documents(vec![(uri, index)]).await
    }

    /// Add (or replace) semantic entries for many documents using batch embedding.
    pub async fn add_documents(
        &mut self,
        docs: Vec<(DocumentUri, &DocumentIndex)>,
    ) -> Result<(), EmbedError> {
        let mut docs_state = Vec::with_capacity(docs.len());
        let mut pending_entries = Vec::new();
        let mut pending_embeddings: Vec<(String, String)> = Vec::new();

        for (uri, index) in docs {
            let mut ids = Vec::new();
            let mut token_set = std::collections::BTreeSet::new();

            if index.headings().is_empty() {
                let fallback_heading = fallback_heading(&uri);
                let id = format!("{}#fallback", uri.as_str());

                token_set.extend(token_hashes(&fallback_heading));
                pending_embeddings.push((id.clone(), fallback_heading.clone()));
                pending_entries.push((
                    id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: fallback_heading,
                        heading_level: 1,
                        section_start: Position::new(0, 0),
                        section_end: Position::new(0, 0),
                    },
                ));
                ids.push(id);
            } else {
                for (i, heading) in index.headings().iter().enumerate() {
                    let embedding_input = heading.text.to_string();
                    if embedding_input.trim().is_empty() {
                        continue;
                    }

                    let id = format!("{}#{}#{i}", uri.as_str(), heading.slug);

                    token_set.extend(token_hashes(&embedding_input));
                    pending_embeddings.push((id.clone(), embedding_input.clone()));
                    pending_entries.push((
                        id.clone(),
                        SemanticEntry {
                            doc_uri: uri.clone(),
                            heading: embedding_input,
                            heading_level: heading.level,
                            section_start: heading.range.start,
                            section_end: heading.range.end,
                        },
                    ));
                    ids.push(id);
                }
            }

            docs_state.push((uri, ids, token_set));
        }

        let mut staged_zig_adds: Vec<(String, Vec<f32>)> =
            Vec::with_capacity(pending_embeddings.len());
        if !pending_embeddings.is_empty() {
            let inputs: Vec<&str> = pending_embeddings
                .iter()
                .map(|(_, text)| text.as_str())
                .collect();
            let embeddings = self.embed_with_batch_fallback(&inputs).await?;
            if embeddings.len() != pending_embeddings.len() {
                return Err(EmbedError::InternalError(format!(
                    "embed_batch returned {} vectors for {} texts",
                    embeddings.len(),
                    pending_embeddings.len()
                )));
            }

            for ((id, _), embedding) in pending_embeddings.into_iter().zip(embeddings.into_iter()) {
                staged_zig_adds.push((id, embedding));
            }
        }

        // Commit after all embedding calls succeed.
        for (uri, _, _) in &docs_state {
            self.remove_document(uri);
        }

        for (id, embedding) in staged_zig_adds {
            self.index
                .add(&id, &embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
        }

        for (id, entry) in pending_entries {
            self.entries_by_id.insert(id, entry);
        }

        for (uri, ids, token_set) in docs_state {
            self.doc_to_ids.insert(uri.clone(), ids);
            self.doc_token_sets.insert(uri, token_set);
        }

        Ok(())
    }

    /// Remove semantic metadata for a document.
    ///
    /// This removes in-memory metadata and duplicate-detection tokens. The
    /// underlying embedding vectors remain in the Zig index and are filtered out
    /// at query time by ID.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        if let Some(ids) = self.doc_to_ids.remove(uri) {
            for id in ids {
                self.entries_by_id.remove(&id);
            }
        }
        self.doc_token_sets.remove(uri);
    }

    async fn embed_with_batch_fallback(
        &self,
        inputs: &[&str],
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match self.provider.embed_batch(inputs).await {
            Ok(embeddings) => Ok(embeddings),
            Err(batch_err) => {
                log::warn!(
                    "batch embedding failed for {} inputs, falling back to sequential: {batch_err}",
                    inputs.len()
                );
                let mut embeddings = Vec::with_capacity(inputs.len());
                for input in inputs {
                    embeddings.push(self.provider.embed(input).await?);
                }
                Ok(embeddings)
            }
        }
    }
}
