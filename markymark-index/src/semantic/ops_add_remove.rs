use std::collections::BTreeSet;

use crate::DocumentIndex;
use markymark_core::prelude::*;

use super::helpers::{fallback_heading, token_hashes};
use super::{SemanticEntry, SemanticIndex};

struct EntryPlan {
    id: String,
    embedding_input: String,
    entry: SemanticEntry,
}

struct DocumentPlan {
    uri: DocumentUri,
    ids: Vec<String>,
    token_set: BTreeSet<u32>,
    entries: Vec<EntryPlan>,
}

fn build_document_plan(uri: &DocumentUri, index: &DocumentIndex) -> DocumentPlan {
    let mut ids = Vec::new();
    let mut entries = Vec::new();
    let mut token_set = BTreeSet::new();

    if index.headings().is_empty() {
        let heading = fallback_heading(uri);
        let id = format!("{}#fallback", uri.as_str());

        token_set.extend(token_hashes(&heading));
        entries.push(EntryPlan {
            id: id.clone(),
            embedding_input: heading.clone(),
            entry: SemanticEntry {
                doc_uri: uri.clone(),
                heading,
                heading_level: 1,
                section_start: Position::new(0, 0),
                section_end: Position::new(0, 0),
            },
        });
        ids.push(id);
    } else {
        for (i, heading) in index.headings().iter().enumerate() {
            let text = heading.text.to_string();
            if text.trim().is_empty() {
                continue;
            }

            let id = format!("{}#{}#{i}", uri.as_str(), heading.slug);
            token_set.extend(token_hashes(&text));

            entries.push(EntryPlan {
                id: id.clone(),
                embedding_input: text.clone(),
                entry: SemanticEntry {
                    doc_uri: uri.clone(),
                    heading: text,
                    heading_level: heading.level,
                    section_start: heading.range.start,
                    section_end: heading.range.end,
                },
            });
            ids.push(id);
        }
    }

    DocumentPlan {
        uri: uri.clone(),
        ids,
        token_set,
        entries,
    }
}

impl SemanticIndex {
    async fn embed_texts_with_batch_fallback(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        match self.provider.embed_batch(texts).await {
            Ok(embeddings) => {
                if embeddings.len() == texts.len() {
                    Ok(embeddings)
                } else {
                    Err(EmbedError::InternalError(format!(
                        "embed_batch cardinality mismatch: expected {}, got {}",
                        texts.len(),
                        embeddings.len()
                    )))
                }
            }
            Err(err) => {
                log::warn!(
                    "batch embedding failed for {} texts ({err}); falling back to sequential",
                    texts.len()
                );

                let mut out = Vec::with_capacity(texts.len());
                for text in texts {
                    out.push(self.provider.embed(text).await?);
                }
                Ok(out)
            }
        }
    }

    async fn apply_document_plans(&mut self, plans: Vec<DocumentPlan>) -> Result<(), EmbedError> {
        let total_entries = plans.iter().map(|plan| plan.entries.len()).sum::<usize>();
        let mut texts = Vec::with_capacity(total_entries);
        for plan in &plans {
            for entry in &plan.entries {
                texts.push(entry.embedding_input.as_str());
            }
        }
        let embeddings = self.embed_texts_with_batch_fallback(&texts).await?;
        let mut embedding_iter = embeddings.into_iter();

        let mut staged_zig_adds: Vec<(String, Vec<f32>)> = Vec::with_capacity(total_entries);
        let mut pending_entries: Vec<(String, SemanticEntry)> = Vec::with_capacity(total_entries);
        let mut staged_docs: Vec<(DocumentUri, Vec<String>, BTreeSet<u32>)> =
            Vec::with_capacity(plans.len());

        for plan in plans {
            for entry in plan.entries {
                let Some(embedding) = embedding_iter.next() else {
                    return Err(EmbedError::InternalError(
                        "embedding cardinality underflow while staging documents".to_string(),
                    ));
                };
                staged_zig_adds.push((entry.id.clone(), embedding));
                pending_entries.push((entry.id, entry.entry));
            }
            staged_docs.push((plan.uri, plan.ids, plan.token_set));
        }

        if embedding_iter.next().is_some() {
            return Err(EmbedError::InternalError(
                "embedding cardinality overflow while staging documents".to_string(),
            ));
        }

        for (id, embedding) in staged_zig_adds {
            self.index
                .add(&id, &embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
        }

        for (id, entry) in pending_entries {
            self.entries_by_id.insert(id, entry);
        }

        for (uri, ids, token_set) in staged_docs {
            self.doc_to_ids.insert(uri.clone(), ids);
            self.doc_token_sets.insert(uri, token_set);
        }

        Ok(())
    }

    /// Add (or replace) semantic entries for a document.
    ///
    /// If the document has headings, one semantic entry is generated per
    /// heading. If it has no headings, a single fallback entry based on the
    /// document file stem is created.
    ///
    /// On embed failure the previous state is restored (snapshot-then-rollback).
    pub async fn add_document(
        &mut self,
        uri: DocumentUri,
        index: &DocumentIndex,
    ) -> Result<(), EmbedError> {
        // Snapshot current state for rollback on failure.
        let prev_ids = self.doc_to_ids.get(&uri).cloned();
        let prev_entries: Vec<(String, SemanticEntry)> = prev_ids
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entries_by_id.get(id).cloned().map(|e| (id.clone(), e)))
                    .collect()
            })
            .unwrap_or_default();
        let prev_tokens = self.doc_token_sets.get(&uri).cloned();

        // Remove old entries (optimistic).
        self.remove_document(&uri);

        // Attempt new indexing — rollback on failure.
        if let Err(err) = self
            .apply_document_plans(vec![build_document_plan(&uri, index)])
            .await
        {
            // Rollback: restore previous state.
            for (id, entry) in prev_entries {
                self.entries_by_id.insert(id, entry);
            }
            if let Some(ids) = prev_ids {
                self.doc_to_ids.insert(uri.clone(), ids);
            }
            if let Some(tokens) = prev_tokens {
                self.doc_token_sets.insert(uri, tokens);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Add or replace semantic entries for multiple documents in one batch.
    ///
    /// This method batches embedding generation across all provided documents.
    /// On batch provider failure, it logs and falls back to sequential per-text
    /// embedding to preserve resilience.
    ///
    /// On embed failure ALL documents are rolled back to their previous state
    /// (snapshot-then-rollback).
    pub async fn add_documents(
        &mut self,
        docs: Vec<(DocumentUri, &DocumentIndex)>,
    ) -> Result<(), EmbedError> {
        if docs.is_empty() {
            return Ok(());
        }

        // Snapshot all documents for rollback.
        let snapshots: Vec<_> = docs
            .iter()
            .map(|(uri, _)| {
                let prev_ids = self.doc_to_ids.get(uri).cloned();
                let prev_entries: Vec<(String, SemanticEntry)> = prev_ids
                    .as_ref()
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|id| {
                                self.entries_by_id.get(id).cloned().map(|e| (id.clone(), e))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let prev_tokens = self.doc_token_sets.get(uri).cloned();
                (uri.clone(), prev_ids, prev_entries, prev_tokens)
            })
            .collect();

        // Remove all old entries and build plans.
        let mut plans = Vec::with_capacity(docs.len());
        for (uri, index) in docs {
            self.remove_document(&uri);
            plans.push(build_document_plan(&uri, index));
        }

        // Attempt batch indexing — rollback ALL on failure.
        if let Err(err) = self.apply_document_plans(plans).await {
            for (uri, prev_ids, prev_entries, prev_tokens) in snapshots {
                for (id, entry) in prev_entries {
                    self.entries_by_id.insert(id, entry);
                }
                if let Some(ids) = prev_ids {
                    self.doc_to_ids.insert(uri.clone(), ids);
                }
                if let Some(tokens) = prev_tokens {
                    self.doc_token_sets.insert(uri, tokens);
                }
            }
            return Err(err);
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
}
