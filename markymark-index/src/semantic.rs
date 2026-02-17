//! Semantic index built on Zig embedding kernels.
//!
//! This module is feature-gated behind `embeddings`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use crate::DocumentIndex;
use markymark_core::prelude::*;

/// Semantic metadata for a heading-level search entry.
#[derive(Debug, Clone)]
pub struct SemanticEntry {
    /// Document URI containing this entry.
    pub doc_uri: DocumentUri,
    /// Heading text used as semantic label.
    pub heading: String,
    /// Markdown heading level (1-6).
    pub heading_level: u8,
    /// Section start position.
    pub section_start: Position,
    /// Section end position.
    pub section_end: Position,
}

/// Semantic search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matched document URI.
    pub doc_uri: DocumentUri,
    /// Matched heading text.
    pub heading: String,
    /// Similarity score.
    pub score: f32,
    /// Source range for the matched heading/section.
    pub section_range: Range,
}

/// Pair of near-duplicate documents.
#[derive(Debug, Clone)]
pub struct DuplicateMatch {
    /// First URI in the pair.
    pub doc_uri_a: DocumentUri,
    /// Second URI in the pair.
    pub doc_uri_b: DocumentUri,
    /// Jaccard similarity over token hashes.
    pub similarity: f32,
}

/// Semantic index backed by [`ZigEmbeddingIndex`].
///
/// Stores entry metadata keyed by stable IDs and filters stale embedding IDs at
/// query time. This supports document replacement/removal even though the
/// current Zig embedding index API does not expose a delete operation.
pub struct SemanticIndex {
    provider: Arc<dyn EmbeddingProvider>,
    index: ZigEmbeddingIndex,
    entries_by_id: HashMap<String, SemanticEntry>,
    doc_to_ids: HashMap<DocumentUri, Vec<String>>,
    doc_token_sets: HashMap<DocumentUri, BTreeSet<u32>>,
}

impl SemanticIndex {
    /// Create a semantic index using the provided embedding backend.
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Result<Self, EmbedError> {
        let dims = provider.dimensions();
        if dims == 0 {
            return Err(EmbedError::InvalidInput(
                "embedding dimensions must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            provider,
            index: ZigEmbeddingIndex::new(dims)?,
            entries_by_id: HashMap::new(),
            doc_to_ids: HashMap::new(),
            doc_token_sets: HashMap::new(),
        })
    }

    /// Add (or replace) semantic entries for a document.
    ///
    /// If the document has headings, one semantic entry is generated per
    /// heading. If it has no headings, a single fallback entry based on the
    /// document file stem is created.
    pub fn add_document(
        &mut self,
        uri: DocumentUri,
        index: &DocumentIndex,
    ) -> Result<(), EmbedError> {
        self.remove_document(&uri);

        let mut ids = Vec::new();
        let mut token_set = BTreeSet::new();

        if index.headings().is_empty() {
            let fallback_heading = fallback_heading(&uri);
            let embedding = self.provider.embed(&fallback_heading)?;
            let id = format!("{}#fallback", uri.as_str());
            self.index.add(&id, &embedding)?;

            token_set.extend(token_hashes(&fallback_heading));
            self.entries_by_id.insert(
                id.clone(),
                SemanticEntry {
                    doc_uri: uri.clone(),
                    heading: fallback_heading,
                    heading_level: 1,
                    section_start: Position::new(0, 0),
                    section_end: Position::new(0, 0),
                },
            );
            ids.push(id);
        } else {
            for (i, heading) in index.headings().iter().enumerate() {
                let embedding_input = heading.text.to_string();
                let embedding = self.provider.embed(&embedding_input)?;
                let id = format!("{}#{}#{i}", uri.as_str(), heading.slug);
                self.index.add(&id, &embedding)?;

                token_set.extend(token_hashes(&embedding_input));
                self.entries_by_id.insert(
                    id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: heading.text.to_string(),
                        heading_level: heading.level,
                        section_start: heading.range.start,
                        section_end: heading.range.end,
                    },
                );
                ids.push(id);
            }
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

    /// Run semantic search over indexed entries.
    pub fn search(
        &self,
        query: &str,
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        if top_k == 0 || self.entries_by_id.is_empty() {
            return Ok(Vec::new());
        }

        let query_embedding = self.provider.embed(query)?;
        let score_floor = min_score.clamp(0.0, 1.0);

        let fetch_k = self.index.count().max(top_k);
        let raw = self.index.search(&query_embedding, fetch_k)?;

        let mut out = Vec::new();
        for candidate in raw {
            if candidate.score < score_floor {
                continue;
            }
            let Some(entry) = self.entries_by_id.get(&candidate.id) else {
                continue;
            };

            out.push(SearchResult {
                doc_uri: entry.doc_uri.clone(),
                heading: entry.heading.clone(),
                score: candidate.score,
                section_range: Range::new(entry.section_start, entry.section_end),
            });

            if out.len() as u32 >= top_k {
                break;
            }
        }

        Ok(out)
    }

    /// Detect near-duplicate document pairs using token-hash Jaccard similarity.
    pub fn detect_duplicates(&self, threshold: f32) -> Vec<DuplicateMatch> {
        let threshold = threshold.clamp(0.0, 1.0);
        if self.doc_token_sets.len() < 2 {
            return Vec::new();
        }

        let mut uris = self.doc_token_sets.keys().cloned().collect::<Vec<_>>();
        uris.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        let mut out = Vec::new();
        for i in 0..uris.len() {
            for j in (i + 1)..uris.len() {
                let a = &uris[i];
                let b = &uris[j];
                let Some(set_a) = self.doc_token_sets.get(a) else {
                    continue;
                };
                let Some(set_b) = self.doc_token_sets.get(b) else {
                    continue;
                };

                let similarity = jaccard_similarity(set_a, set_b);
                if similarity >= threshold {
                    out.push(DuplicateMatch {
                        doc_uri_a: a.clone(),
                        doc_uri_b: b.clone(),
                        similarity,
                    });
                }
            }
        }

        out.sort_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then_with(|| a.doc_uri_a.as_str().cmp(b.doc_uri_a.as_str()))
                .then_with(|| a.doc_uri_b.as_str().cmp(b.doc_uri_b.as_str()))
        });

        out
    }

    /// Number of active semantic entries.
    pub fn entry_count(&self) -> usize {
        self.entries_by_id.len()
    }
}

fn fallback_heading(uri: &DocumentUri) -> String {
    uri.to_file_path()
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(uri.as_str())
        .to_string()
}

fn token_hashes(text: &str) -> Vec<u32> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| fnv1a32(&token.to_ascii_lowercase()))
        .collect()
}

fn fnv1a32(text: &str) -> u32 {
    const OFFSET: u32 = 0x811c9dc5;
    const PRIME: u32 = 0x0100_0193;

    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn jaccard_similarity(a: &BTreeSet<u32>, b: &BTreeSet<u32>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }

    let intersection = a.intersection(b).count() as f32;
    let union = (a.len() + b.len()) as f32 - intersection;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}
