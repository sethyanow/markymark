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
        self.remove_document(&uri);

        let mut staged_zig_adds: Vec<(String, Vec<f32>)> = Vec::new();
        let mut ids = Vec::new();
        let mut pending_entries = Vec::new();
        let mut token_set = std::collections::BTreeSet::new();

        if index.headings().is_empty() {
            let fallback_heading = fallback_heading(&uri);
            let embedding = self.provider.embed(&fallback_heading).await?;
            let id = format!("{}#fallback", uri.as_str());

            token_set.extend(token_hashes(&fallback_heading));
            staged_zig_adds.push((id.clone(), embedding));
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
                let embedding = self.provider.embed(&embedding_input).await?;
                let id = format!("{}#{}#{i}", uri.as_str(), heading.slug);

                token_set.extend(token_hashes(&embedding_input));
                staged_zig_adds.push((id.clone(), embedding));
                pending_entries.push((
                    id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: heading.text.to_string(),
                        heading_level: heading.level,
                        section_start: heading.range.start,
                        section_end: heading.range.end,
                    },
                ));
                ids.push(id);
            }
        }

        for (id, embedding) in staged_zig_adds {
            self.index
                .add(&id, &embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
        }

        for (id, entry) in pending_entries {
            self.entries_by_id.insert(id, entry);
        }
        self.doc_to_ids.insert(uri.clone(), ids);
        self.doc_token_sets.insert(uri, token_set);
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
