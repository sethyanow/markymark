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
    /// Matched heading level.
    pub heading_level: u8,
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
