//! Server state: document store, parsing, and indexing.

pub mod completion;
pub mod navigation;
pub mod rename;

use std::collections::HashMap;

use markymark_core::scanner::Md4cScanBackend;
use markymark_core::structured::DocumentKind;
use markymark_core::DocumentUri;
use markymark_index::{
    mask_frontmatter, parse_frontmatter_owned, AnyDocumentIndex, DocumentIndex, RealmIndex,
    StructuredDocumentIndex,
};
use markymark_kernels::engine::DocumentEngine;
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
    engines: HashMap<String, std::sync::Mutex<DocumentEngine>>,
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

    /// Build a [`DocumentIndex`] from raw text via the scan fallback path,
    /// parsing and masking frontmatter so md4c doesn't misparse `---`
    /// delimiters as setext headings.
    fn fallback_scan_with_frontmatter(text: &str) -> DocumentIndex {
        let (fm, aliases) = parse_frontmatter_owned(text);
        let masked = mask_frontmatter(text);
        DocumentIndex::from_scan_with_frontmatter(&masked, &Md4cScanBackend, fm, aliases)
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
            let mut engine = match engine_mutex.lock() {
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
                engine
                    .update(&masked)
                    .map_err(|e| format!("engine update failed: {e:?}"))
            };

            match update_result {
                Ok(()) => {
                    match Self::index_from_engine_result(
                        uri_str,
                        &engine,
                        fm.clone(),
                        aliases.clone(),
                    ) {
                        Ok(index) => return Some(index),
                        Err(e) => {
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

            match Self::index_from_engine_result(uri_str, &engine, fm.clone(), aliases.clone()) {
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
                    // Engine creation succeeded. Keep it even if current conversion failed.
                    self.engines
                        .insert(uri_str.to_string(), std::sync::Mutex::new(engine));
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
            // Secondary fallback: from_scan with Md4cScanBackend. Never panics.
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
                    .build_markdown_index_via_engine(&uri, &text)
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
                if let Some(index) = self.build_markdown_index_via_engine(uri, &text) {
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

        // Phase 1: Apply text edits to the stored document text
        let final_text = {
            let Some(text) = self.documents.get_mut(uri.as_str()) else {
                return;
            };

            for change in changes {
                match change {
                    DocumentChange::Full(new_text) => {
                        *text = new_text;
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
                    }
                }
            }
            text.clone()
        };

        // Phase 2: Re-index with the final text via the engine pipeline
        let kind = Self::document_kind_from_uri(uri);

        match kind {
            Some(DocumentKind::Markdown) | None => {
                if let Some(index) = self.build_markdown_index_via_engine(uri, &final_text) {
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
