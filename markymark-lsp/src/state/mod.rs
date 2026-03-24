//! Server state: document store, parsing, and indexing.

pub mod completion;
pub mod navigation;
pub mod rename;

use std::collections::HashMap;

use markymark_core::structured::DocumentKind;
use markymark_core::DocumentUri;
use markymark_index::{
    mask_frontmatter, parse_frontmatter_owned, AnyDocumentIndex, DocumentIndex, RealmIndex,
    StructuredDocumentIndex,
};
use markymark_kernels::engine::{DocumentEngine, EditRange};
use markymark_parser::structured::parse_structured;

pub use crate::diagnostics::{DiagnosticSeverity, MarkyDiagnostic};
pub use completion::{CompletionCandidate, CompletionCandidateKind, CompletionContext};
pub use navigation::{StructuredKeyInfo, SymbolAtPosition};
pub use rename::{PrepareRenameResult, RenameEdit};

/// A content change event representing either a full document replacement
/// or an incremental text edit with LSP positions.
#[derive(Debug, Clone)]
pub enum DocumentChange {
    /// Full document text replacement.
    Full(String),
    /// Incremental edit specified with LSP line/character positions.
    /// Character offsets are in UTF-16 code units (as per LSP spec).
    Incremental {
        /// Start line (0-based).
        start_line: u32,
        /// Start character offset (UTF-16 code units).
        start_character: u32,
        /// End line (0-based).
        end_line: u32,
        /// End character offset (UTF-16 code units).
        end_character: u32,
        /// The replacement text.
        text: String,
    },
}

/// Byte bounds computed from a LSP incremental-change range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IncrementalByteBounds {
    /// Byte offset of the start of the change.
    start_byte: usize,
    /// Byte offset of the old end of the change.
    old_end_byte: usize,
    /// True if the start position was clamped to the text length.
    start_clamped: bool,
    /// True if the end position was clamped to the text length.
    end_clamped: bool,
    /// True when the raw end position was before the raw start.
    end_before_start: bool,
}

/// Compute byte offsets from LSP line/character positions.
fn incremental_byte_bounds(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> IncrementalByteBounds {
    let raw_start_byte =
        crate::convert::lsp_position_to_byte_offset(text, start_line, start_character);
    let raw_end_byte = crate::convert::lsp_position_to_byte_offset(text, end_line, end_character);

    let end_before_start = raw_end_byte < raw_start_byte;
    let start_byte = raw_start_byte.min(text.len());
    let old_end_byte = raw_end_byte.min(text.len()).max(start_byte);

    IncrementalByteBounds {
        start_byte,
        old_end_byte,
        start_clamped: position_was_clamped(text, start_line, start_character),
        end_clamped: position_was_clamped(text, end_line, end_character),
        end_before_start,
    }
}

/// Returns true if an LSP position was clamped (beyond the actual text).
fn position_was_clamped(text: &str, line: u32, character: u32) -> bool {
    let target_line = line as usize;
    let target_character = character as usize;
    let Some(line_text) = text.split('\n').nth(target_line) else {
        return true;
    };
    let content = line_text.strip_suffix('\r').unwrap_or(line_text);
    target_character > content.encode_utf16().count()
}

/// Per-document engine state: the Zig engine plus the content hash from its
/// last successful index build. Used to short-circuit blob serialization when
/// the document's structural content hasn't changed.
struct EngineState {
    engine: DocumentEngine,
    last_hash: u64,
}

/// The internal state of the LSP server.
///
/// Manages document text storage, per-document Zig DocumentEngine instances,
/// and the realm index for cross-document lookups.
pub struct ServerState {
    /// Raw document text keyed by URI string.
    documents: HashMap<String, String>,
    /// The realm index for cross-document lookups.
    realm: RealmIndex,
    /// Per-document stateful Zig document engines (keyed by URI string).
    /// Wrapped in `Mutex` so `ServerState: Sync` is derived soundly.
    /// `DocumentEngine` is `Send` but not `Sync` (get_result can mutate Zig-side
    /// cached state), so we gate shared access through a mutex even though
    /// all call-sites already hold `&mut self` and the mutex is uncontested.
    /// INVARIANT: all maps use `uri.as_str().to_string()` as the key.
    engines: HashMap<String, std::sync::Mutex<EngineState>>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    fn should_force_engine_update_fail_for_tests(uri_str: &str) -> bool {
        cfg!(debug_assertions) && uri_str.contains("__marky_test_force_update_fail__")
    }

    fn should_force_engine_result_conversion_fail_for_tests(uri_str: &str) -> bool {
        cfg!(debug_assertions) && uri_str.contains("__marky_test_force_conversion_fail__")
    }

    fn index_from_engine_result(
        uri_str: &str,
        engine: &DocumentEngine,
        frontmatter: Vec<markymark_index::FrontmatterOwnedEntry>,
        aliases: Vec<String>,
    ) -> Result<DocumentIndex, String> {
        let result = engine
            .get_result()
            .map_err(|e| format!("get_result failed: {e:?}"))?;
        if Self::should_force_engine_result_conversion_fail_for_tests(uri_str) {
            return Err("forced engine result conversion failure (test hook, uri)".to_string());
        }
        let extraction = result
            .to_extraction()
            .map_err(|e| format!("to_extraction failed: {e:?}"))?;
        Ok(DocumentIndex::from_engine_result_with_frontmatter(
            &extraction,
            frontmatter,
            aliases,
        ))
    }

    /// Create a new empty server state.
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            realm: RealmIndex::default(),
            engines: HashMap::new(),
        }
    }

    /// Build a [`DocumentIndex`] from raw text via an ephemeral engine.
    ///
    /// This is the last-resort fallback when the persistent engine fails and
    /// no stale index is cached. Panics on failure — acceptable because it's
    /// the end of the fallback chain.
    fn fallback_scan_with_frontmatter(text: &str) -> DocumentIndex {
        DocumentIndex::from_text(text)
    }

    /// Build a markdown document index via the stateful Zig DocumentEngine.
    ///
    /// If an engine for the URI exists, updates it; otherwise creates one.
    /// On update/create/result-conversion failures, tries stale-state fallback
    /// first. When no stale state exists, falls back to scan path.
    fn build_markdown_index_via_engine(
        &mut self,
        uri: &DocumentUri,
        text: &str,
        edit_range: Option<EditRange>,
    ) -> Option<DocumentIndex> {
        let uri_str = uri.as_str();
        let has_stale_index = self.realm.get_document(uri).is_some();

        // Parse frontmatter and mask it so md4c doesn't misparse `---`
        // delimiters as setext headings. Masking preserves byte offsets.
        let (fm, aliases) = parse_frontmatter_owned(text);
        let masked = mask_frontmatter(text);

        if let Some(engine_mutex) = self.engines.get(uri_str) {
            // Engine exists — update it. The mutex is uncontested here because
            // build_markdown_index_via_engine takes &mut self.
            let mut engine_state = match engine_mutex.lock() {
                Ok(guard) => guard,
                Err(_poisoned) => {
                    log::warn!(
                        target: "markymark_lsp",
                        "engine mutex poisoned for {}",
                        uri_str
                    );
                    return if has_stale_index {
                        None
                    } else {
                        Some(Self::fallback_scan_with_frontmatter(text))
                    };
                }
            };
            let update_result = if Self::should_force_engine_update_fail_for_tests(uri_str) {
                Err("forced engine update failure (test hook, uri)".to_string())
            } else {
                engine_state
                    .engine
                    .update(&masked, edit_range)
                    .map_err(|e| format!("engine update failed: {e:?}"))
            };

            match update_result {
                Ok(()) => {
                    // Content hash short-circuit: skip blob pipeline when
                    // document structure hasn't changed.
                    let new_hash = engine_state.engine.content_hash();
                    if new_hash == engine_state.last_hash {
                        return None;
                    }

                    match Self::index_from_engine_result(
                        uri_str,
                        &engine_state.engine,
                        fm.clone(),
                        aliases.clone(),
                    ) {
                        Ok(index) => {
                            engine_state.last_hash = new_hash;
                            return Some(index);
                        }
                        Err(e) => {
                            // Don't update last_hash — index build failed.
                            // Next call will retry the full pipeline.
                            log::warn!(
                                target: "markymark_lsp",
                                "engine result conversion failed for {}: {}, trying stale fallback",
                                uri_str, e
                            );
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        target: "markymark_lsp",
                        "{} for {}, trying stale engine snapshot",
                        e, uri_str
                    );
                }
            }

            match Self::index_from_engine_result(
                uri_str,
                &engine_state.engine,
                fm.clone(),
                aliases.clone(),
            ) {
                Ok(index) => return Some(index),
                Err(e) => {
                    log::warn!(
                        target: "markymark_lsp",
                        "stale engine snapshot failed for {}: {}",
                        uri_str, e
                    );
                }
            }
        } else {
            // No engine yet — create one with masked text
            match DocumentEngine::new(&masked) {
                Ok(engine) => {
                    let built = Self::index_from_engine_result(uri_str, &engine, fm, aliases);
                    let initial_hash = engine.content_hash();
                    // Engine creation succeeded. Keep it even if current conversion failed.
                    self.engines.insert(
                        uri_str.to_string(),
                        std::sync::Mutex::new(EngineState {
                            engine,
                            last_hash: initial_hash,
                        }),
                    );
                    match built {
                        Ok(index) => return Some(index),
                        Err(e) => {
                            log::warn!(
                                target: "markymark_lsp",
                                "engine result conversion failed (new engine) for {}: {}, using fallback",
                                uri_str, e
                            );
                        }
                    }
                }
                Err(e) => log::warn!(
                    target: "markymark_lsp",
                    "engine create failed for {}: {:?}",
                    uri_str, e
                ),
            }
        }

        if has_stale_index {
            None
        } else {
            // Secondary fallback: ephemeral engine via from_text. Never panics.
            Some(Self::fallback_scan_with_frontmatter(text))
        }
    }

    /// Detect document kind from URI file extension.
    fn document_kind_from_uri(uri: &DocumentUri) -> Option<DocumentKind> {
        uri.to_file_path()
            .as_deref()
            .and_then(DocumentKind::from_path)
    }

    /// Handle a document being opened: store text, parse, and index.
    pub async fn open_document(&mut self, uri: DocumentUri, text: String) {
        let kind = Self::document_kind_from_uri(&uri);
        self.documents
            .insert(uri.as_str().to_string(), text.clone());

        match kind {
            Some(DocumentKind::Markdown) | None => {
                let index = self
                    .build_markdown_index_via_engine(&uri, &text, None)
                    .unwrap_or_else(|| Self::fallback_scan_with_frontmatter(&text));
                self.realm.add_document(uri, index).await;
            }
            Some(kind) => {
                if let Ok(ast) = parse_structured(&text, kind) {
                    self.realm
                        .add_structured_document(uri, StructuredDocumentIndex::from_ast(ast));
                }
            }
        }
    }

    /// Handle a document being changed: apply changes, re-parse, re-index.
    pub async fn change_document(&mut self, uri: &DocumentUri, text: String) {
        let kind = Self::document_kind_from_uri(uri);
        self.documents
            .insert(uri.as_str().to_string(), text.clone());

        match kind {
            Some(DocumentKind::Markdown) | None => {
                if let Some(index) = self.build_markdown_index_via_engine(uri, &text, None) {
                    self.realm.update_document(uri.clone(), index).await;
                }
            }
            Some(kind) => {
                self.realm.remove_document(uri).await;
                if let Ok(ast) = parse_structured(&text, kind) {
                    self.realm.add_structured_document(
                        uri.clone(),
                        StructuredDocumentIndex::from_ast(ast),
                    );
                }
            }
        }
    }

    /// Apply a sequence of content changes to a document, then re-parse and re-index.
    ///
    /// Changes are applied in order. Each change operates on the text as modified
    /// by the previous change (per LSP spec). Supports both incremental edits
    /// (with position range in UTF-16 code units) and full-text replacements.
    pub async fn apply_document_changes(
        &mut self,
        uri: &DocumentUri,
        changes: Vec<DocumentChange>,
    ) {
        if changes.is_empty() {
            return;
        }

        // Phase 1: Apply text edits and accumulate edit range bounding box
        let mut accumulated: Option<(usize, usize, usize)> = None;

        let final_text = {
            let Some(text) = self.documents.get_mut(uri.as_str()) else {
                return;
            };

            for change in changes {
                match change {
                    DocumentChange::Full(new_text) => {
                        *text = new_text;
                        // Full replacement invalidates any accumulated range
                        accumulated = None;
                    }
                    DocumentChange::Incremental {
                        start_line,
                        start_character,
                        end_line,
                        end_character,
                        text: new_text,
                    } => {
                        let bounds = incremental_byte_bounds(
                            text,
                            start_line,
                            start_character,
                            end_line,
                            end_character,
                        );

                        if bounds.end_before_start {
                            log::warn!(
                                target: "markymark_lsp",
                                "skipping invalid incremental edit for {} \
                                 (old_end < start: start={}:{}, end={}:{})",
                                uri.as_str(),
                                start_line,
                                start_character,
                                end_line,
                                end_character
                            );
                            continue;
                        }

                        if bounds.start_clamped || bounds.end_clamped {
                            log::warn!(
                                target: "markymark_lsp",
                                "clamped incremental edit range for {} \
                                 (start={start_line}:{start_character}, end={end_line}:{end_character}, text_len_bytes={})",
                                uri.as_str(),
                                text.len()
                            );
                        }

                        text.replace_range(bounds.start_byte..bounds.old_end_byte, &new_text);

                        // Accumulate bounding box from applied edits
                        let old_len = bounds.old_end_byte - bounds.start_byte;
                        accumulated = Some(match accumulated {
                            Some((min_start, total_old, total_new)) => (
                                min_start.min(bounds.start_byte),
                                total_old + old_len,
                                total_new + new_text.len(),
                            ),
                            None => (bounds.start_byte, old_len, new_text.len()),
                        });
                    }
                }
            }
            text.clone()
        };

        // Convert accumulated bounding box to EditRange
        let edit_range = accumulated.map(|(start, old_len, new_len)| EditRange {
            offset: start as u32,
            old_len: old_len as u32,
            new_len: new_len as u32,
        });

        // Phase 2: Re-index with the final text via the engine pipeline
        let kind = Self::document_kind_from_uri(uri);

        match kind {
            Some(DocumentKind::Markdown) | None => {
                if let Some(index) =
                    self.build_markdown_index_via_engine(uri, &final_text, edit_range)
                {
                    self.realm.update_document(uri.clone(), index).await;
                }
            }
            Some(kind) => {
                self.realm.remove_document(uri).await;
                if let Ok(ast) = parse_structured(&final_text, kind) {
                    self.realm.add_structured_document(
                        uri.clone(),
                        StructuredDocumentIndex::from_ast(ast),
                    );
                }
            }
        }
    }

    /// Handle a document being closed: remove from store and index.
    pub async fn close_document(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri.as_str());
        self.engines.remove(uri.as_str());
        self.realm.remove_document(uri).await;
    }

    /// Get the stored text for a document.
    pub fn get_document_text(&self, uri: &DocumentUri) -> Option<&str> {
        self.documents.get(uri.as_str()).map(|s| s.as_str())
    }

    /// Get the markdown document index for a URI.
    pub fn get_document_index(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.realm.get_document(uri)
    }

    /// Get the any-type document index for a URI.
    pub fn get_any_document_index(&self, uri: &DocumentUri) -> Option<&AnyDocumentIndex> {
        self.realm.get_any_document(uri)
    }

    /// Get the structured document index for a URI.
    pub fn get_structured_document_index(
        &self,
        uri: &DocumentUri,
    ) -> Option<&StructuredDocumentIndex> {
        self.realm.get_structured_document(uri)
    }

    /// Get a reference to the realm index.
    pub fn realm(&self) -> &RealmIndex {
        &self.realm
    }

    /// Get the number of open documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Compute diagnostics for a document.
    ///
    /// Checks for:
    /// - Broken wiki links (target page or heading doesn't exist)
    /// - Broken markdown link anchors (heading slug doesn't exist in current doc)
    /// - Duplicate heading slugs within the same document
    pub fn compute_diagnostics(&self, uri: &DocumentUri) -> Vec<MarkyDiagnostic> {
        let index = match self.realm.get_document(uri) {
            Some(idx) => idx,
            None => return Vec::new(),
        };
        crate::diagnostics::compute_diagnostics(index, &self.realm, uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_index_returns_none_for_unchanged_content() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_unchanged.md").unwrap();
        let text = "# Hello\n";

        // First call — always returns Some (new engine, no previous hash)
        let result1 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(result1.is_some(), "first call should return Some");

        // Second call with same text — hash unchanged, should return None
        let result2 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(
            result2.is_none(),
            "second call with unchanged text should return None"
        );
    }

    #[test]
    fn test_build_index_returns_some_for_changed_content() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_changed.md").unwrap();

        let result1 = state.build_markdown_index_via_engine(&uri, "# Hello\n", None);
        assert!(result1.is_some(), "first call should return Some");

        let result2 =
            state.build_markdown_index_via_engine(&uri, "# Hello\n## World\n", None);
        assert!(
            result2.is_some(),
            "changed content should return Some"
        );
    }

    #[test]
    fn test_build_index_first_call_always_returns_some() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/first_call.md").unwrap();

        let result = state.build_markdown_index_via_engine(&uri, "# First\n", None);
        assert!(
            result.is_some(),
            "first call for any URI should return Some"
        );
    }

    #[test]
    fn test_build_index_whitespace_change_not_short_circuited() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_whitespace.md").unwrap();

        let r1 = state.build_markdown_index_via_engine(&uri, "# Hello\n", None);
        assert!(r1.is_some());

        // Add trailing whitespace — different bytes, different hash, no short-circuit
        // This is conservative: structure hasn't changed, but hash is byte-level
        let r2 = state.build_markdown_index_via_engine(&uri, "# Hello \n", None);
        assert!(
            r2.is_some(),
            "whitespace-only change should return Some (hash is byte-level, not structural)"
        );
    }

    #[test]
    fn test_build_index_close_reopen_resets_hash() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_close_reopen.md").unwrap();
        let text = "# Hello\n";

        // First call — returns Some, stores engine + hash
        let r1 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(r1.is_some());

        // Second call — short-circuits
        let r2 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(r2.is_none());

        // Close removes engine state
        state.engines.remove(uri.as_str());

        // Reopen with same content — new engine, no previous hash, returns Some
        let r3 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(
            r3.is_some(),
            "after close+reopen, first call should return Some even for same content"
        );
    }

    #[test]
    fn test_build_index_multibyte_utf8_short_circuits() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_utf8.md").unwrap();
        let text = "# 日本語のヘッダー\n\n本文テキスト\n";

        let r1 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(r1.is_some(), "first call should return Some");

        let r2 = state.build_markdown_index_via_engine(&uri, text, None);
        assert!(r2.is_none(), "same multi-byte content should short-circuit");

        // Change one character in the heading
        let r3 = state
            .build_markdown_index_via_engine(&uri, "# 日本語のヘッダ\n\n本文テキスト\n", None);
        assert!(r3.is_some(), "changed multi-byte content should return Some");
    }

    #[test]
    fn test_build_index_empty_content_short_circuits() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_empty.md").unwrap();

        // Empty content — hash is 0 per Zig engine behavior
        let r1 = state.build_markdown_index_via_engine(&uri, "", None);
        assert!(r1.is_some(), "first call with empty content should return Some");

        // Second call with same empty content — hash still 0, should short-circuit
        let r2 = state.build_markdown_index_via_engine(&uri, "", None);
        assert!(
            r2.is_none(),
            "second call with same empty content should return None"
        );
    }

    #[test]
    fn test_build_index_returns_some_for_reverted_content() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/hash_revert.md").unwrap();

        let r1 = state.build_markdown_index_via_engine(&uri, "# Hello\n", None);
        assert!(r1.is_some(), "first call should return Some");

        let r2 = state.build_markdown_index_via_engine(&uri, "# World\n", None);
        assert!(r2.is_some(), "changed content should return Some");

        // Revert to original — hash differs from last (World), so should return Some
        let r3 = state.build_markdown_index_via_engine(&uri, "# Hello\n", None);
        assert!(
            r3.is_some(),
            "reverted content should return Some (different from last hash)"
        );
    }

    #[tokio::test]
    async fn test_apply_incremental_changes_threads_edit_range() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/edit_range_threading.md").unwrap();

        // Multi-heading doc: two headings, body after each
        let text = "# First\n\nBody\n\n# Second\n\nMore body\n".to_string();
        state.open_document(uri.clone(), text).await;

        // Apply incremental change at the end (after both headings):
        // insert " appended" at end of "More body" on line 6, char 9
        let changes = vec![DocumentChange::Incremental {
            start_line: 6,
            start_character: 9,
            end_line: 6,
            end_character: 9,
            text: " appended".to_string(),
        }];

        state.apply_document_changes(&uri, changes).await;

        // With edit range threaded, headings before the edit should reuse slugs
        let engine_state = state.engines.get(uri.as_str()).unwrap().lock().unwrap();
        assert!(
            engine_state.engine.slug_reuse_count() > 0,
            "incremental edit at end should reuse slugs for headings before edit range, \
             got slug_reuse_count={}",
            engine_state.engine.slug_reuse_count()
        );
    }

    #[tokio::test]
    async fn test_apply_full_change_passes_none() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/full_change_none.md").unwrap();

        // Open with two headings
        let text = "# First\n\n# Second\n".to_string();
        state.open_document(uri.clone(), text).await;

        // Apply a Full change — should pass None to engine (no incremental info)
        let changes = vec![DocumentChange::Full(
            "# First\n\n# Second\n\nNew paragraph\n".to_string(),
        )];

        state.apply_document_changes(&uri, changes).await;

        // Full change → no edit range → no slug reuse
        let engine_state = state.engines.get(uri.as_str()).unwrap().lock().unwrap();
        assert_eq!(
            engine_state.engine.slug_reuse_count(),
            0,
            "Full change should pass None to engine, so no slug reuse"
        );
    }

    #[tokio::test]
    async fn test_apply_mixed_full_then_incremental_invalidates_range() {
        let mut state = ServerState::new();
        let uri = DocumentUri::new("file:///test/mixed_changes.md").unwrap();

        // Open with two headings
        let text = "# First\n\n# Second\n\nBody\n".to_string();
        state.open_document(uri.clone(), text).await;

        // Mixed sequence: Full replacement first, then incremental edit
        // The Full change should invalidate any accumulated range
        let changes = vec![
            DocumentChange::Full("# First\n\n# Second\n\nBody\n".to_string()),
            DocumentChange::Incremental {
                start_line: 4,
                start_character: 4,
                end_line: 4,
                end_character: 4,
                text: " extra".to_string(),
            },
        ];

        state.apply_document_changes(&uri, changes).await;

        // After Full change invalidates range, subsequent Incremental should still
        // accumulate from scratch. But the key behavior: a Full anywhere in the
        // sequence resets the accumulated range.
        // For now, after implementation this tests that the incremental after Full
        // DOES produce a range (not None from the Full).
        // Pre-implementation: slug_reuse_count will be 0 regardless (None always passed)
        let engine_state = state.engines.get(uri.as_str()).unwrap().lock().unwrap();
        let reuse = engine_state.engine.slug_reuse_count();
        // After GREEN: reuse > 0 (Incremental after Full builds fresh range)
        // Currently: reuse == 0 (None always passed) — this test establishes the behavior
        // We assert the post-implementation expectation
        assert!(
            reuse > 0,
            "Incremental after Full should still thread edit range, got slug_reuse_count={}",
            reuse
        );
    }
}
