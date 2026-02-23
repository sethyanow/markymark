//! Scan backend trait for markdown element extraction.
//!
//! [`ScanBackend`] provides a transport-agnostic interface for scanning
//! markdown text for structural elements (headings, links, tags, block IDs)
//! and estimating token counts. Implementations include:
//!
//! - `TreeSitterScanBackend` (always available) — wraps `markymark-parser`
//! - `ZigScanBackend` (behind `zig-kernels` feature) — wraps `markymark-kernels`

mod types;

#[cfg(feature = "zig-kernels")]
mod md4c;

#[cfg(test)]
mod tests;

// Re-export all public types from types module.
pub use types::*;

// Re-export backend implementations.
#[cfg(feature = "zig-kernels")]
pub use md4c::{Md4cScanBackend, ZigScanBackend};

// ---------------------------------------------------------------------------
// ScanBackend trait
// ---------------------------------------------------------------------------

/// Transport-agnostic interface for scanning markdown text.
///
/// Implementations must be stateless (`&self` methods only), `Send + Sync`,
/// and object-safe (no generics, no `Self` in return position).
pub trait ScanBackend: Send + Sync {
    /// Scan text for ATX headings.
    fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError>;

    /// Scan text for markdown and wiki links.
    fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError>;

    /// Scan text for `#tag` patterns.
    fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError>;

    /// Scan text for `^block-id` patterns.
    fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError>;

    /// Estimate the approximate BPE token count for the given text.
    fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError>;

    /// Scan text for inline code spans (`` `code` ``).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_code_spans(&self, _text: &str) -> Result<Vec<CodeSpanResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for task list items (e.g. `- [x] Done`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_tasks(&self, _text: &str) -> Result<Vec<TaskResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for embed references (e.g. `![[target]]`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_embeds(&self, _text: &str) -> Result<Vec<EmbedResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for callout blockquotes (e.g. `> [!note] Title`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_callouts(&self, _text: &str) -> Result<Vec<CalloutResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for block references (e.g. `((uuid))`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_block_refs(&self, _text: &str) -> Result<Vec<BlockRefResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for query blocks (e.g. `{{query ...}}`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_query_blocks(&self, _text: &str) -> Result<Vec<QueryBlockResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for link definitions (e.g. `[label]: url "title"`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_link_definitions(&self, _text: &str) -> Result<Vec<LinkDefinitionResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for Logseq-style properties (e.g. `tags:: project`).
    ///
    /// Default returns empty (backward compat for backends that don't extract).
    fn scan_properties(&self, _text: &str) -> Result<Vec<PropertyResult>, ScanError> {
        Ok(Vec::new())
    }

    /// Scan text for headings, links, code spans, tasks, embeds, callouts, and
    /// block refs in a single pass.
    ///
    /// The default implementation calls each scan method separately. Backends
    /// that parse once internally (e.g., [`Md4cScanBackend`]) should override
    /// this to avoid multiple parses.
    fn scan_all(&self, text: &str) -> Result<ScanAllResult, ScanError> {
        Ok(ScanAllResult {
            headings: self.scan_headings(text)?,
            links: self.scan_links(text)?,
            code_spans: self.scan_code_spans(text)?,
            tasks: self.scan_tasks(text)?,
            embeds: self.scan_embeds(text)?,
            callouts: self.scan_callouts(text)?,
            block_refs: self.scan_block_refs(text)?,
            query_blocks: self.scan_query_blocks(text)?,
            link_definitions: self.scan_link_definitions(text)?,
            properties: self.scan_properties(text)?,
        })
    }
}
