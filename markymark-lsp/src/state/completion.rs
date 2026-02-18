//! Completion context detection and candidate generation.

use markymark_core::{DocumentUri, Position};

use super::ServerState;

/// Detected completion trigger context based on cursor position.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionContext {
    /// Inside `[[` — complete page names.
    WikiLink {
        /// The partial text typed after `[[`.
        partial: String,
    },
    /// Inside `[[page#` — complete headings in the target page.
    WikiLinkHeading {
        /// The target page name.
        target: String,
        /// The partial heading text typed after `#`.
        partial: String,
    },
    /// After `#` (not in a link context) — complete tag names.
    Tag {
        /// The partial text typed after `#`.
        partial: String,
    },
    /// Inside `((` — complete block IDs.
    BlockRef {
        /// The partial text typed after `((`.
        partial: String,
    },
    /// After `<` — complete XML tag names.
    XmlTag {
        /// The partial tag name typed after `<`.
        partial: String,
    },
}

/// A completion suggestion returned by [`ServerState::completion_at`].
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionCandidate {
    /// The completion label (displayed to the user).
    pub label: String,
    /// The kind of completion item.
    pub kind: CompletionCandidateKind,
    /// Optional detail text.
    pub detail: Option<String>,
}

/// The kind of a completion candidate.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionCandidateKind {
    /// A page name (for wiki link completion).
    Page,
    /// A heading (for heading completion).
    Heading,
    /// A tag name.
    Tag,
    /// A block reference ID.
    BlockRef,
    /// An XML tag name.
    XmlTag,
}

impl ServerState {
    /// Scans backward from the cursor to identify trigger patterns:
    /// - `[[partial` → [`CompletionContext::WikiLink`]
    /// - `[[target#partial` → [`CompletionContext::WikiLinkHeading`]
    /// - `#partial` (not inside `[[`) → [`CompletionContext::Tag`]
    /// - `((partial` → [`CompletionContext::BlockRef`]
    pub fn detect_completion_context(
        &self,
        uri: &DocumentUri,
        pos: Position,
    ) -> Option<CompletionContext> {
        let text = self.get_document_text(uri)?;
        let line = text.lines().nth(pos.line as usize)?;
        let col = pos.character as usize;
        if col > line.len() {
            return None;
        }
        let prefix = &line[..col];

        // Check for block ref: ((
        if let Some(open_idx) = prefix.rfind("((") {
            let after = &prefix[open_idx + 2..];
            if !after.contains("))") {
                return Some(CompletionContext::BlockRef {
                    partial: after.to_string(),
                });
            }
        }

        // Check for wiki link: [[
        if let Some(open_idx) = prefix.rfind("[[") {
            let after = &prefix[open_idx + 2..];
            if !after.contains("]]") {
                if let Some(hash_idx) = after.find('#') {
                    let target = &after[..hash_idx];
                    let partial = &after[hash_idx + 1..];
                    return Some(CompletionContext::WikiLinkHeading {
                        target: target.to_string(),
                        partial: partial.to_string(),
                    });
                } else {
                    return Some(CompletionContext::WikiLink {
                        partial: after.to_string(),
                    });
                }
            }
        }

        // Check for tag: # at word boundary (not inside [[)
        if let Some(hash_idx) = prefix.rfind('#') {
            if hash_idx == 0
                || prefix.as_bytes()[hash_idx - 1] == b' '
                || prefix.as_bytes()[hash_idx - 1] == b'\t'
            {
                let partial = &prefix[hash_idx + 1..];
                if partial
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    return Some(CompletionContext::Tag {
                        partial: partial.to_string(),
                    });
                }
            }
        }

        // Check for XML tag: < followed by alphanumeric/hyphen/underscore chars (not yet closed)
        if let Some(lt_idx) = prefix.rfind('<') {
            let after = &prefix[lt_idx + 1..];
            // Not a closing tag (</), not already closed (contains >)
            if !after.starts_with('/')
                && !after.contains('>')
                && after
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                return Some(CompletionContext::XmlTag {
                    partial: after.to_string(),
                });
            }
        }

        None
    }

    /// Get completion candidates at the given position.
    ///
    /// Combines context detection with realm/document data to produce
    /// a list of relevant completion suggestions.
    pub fn completion_at(&self, uri: &DocumentUri, pos: Position) -> Vec<CompletionCandidate> {
        let context = match self.detect_completion_context(uri, pos) {
            Some(ctx) => ctx,
            None => return Vec::new(),
        };

        let mut candidates = Vec::new();

        match context {
            CompletionContext::WikiLink { partial } => {
                let partial_lower = partial.to_lowercase();
                for (doc_uri, _index) in self.realm.iter_documents() {
                    if doc_uri == uri {
                        continue;
                    }
                    if let Some(path) = doc_uri.to_file_path() {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if partial_lower.is_empty()
                                || stem.to_lowercase().contains(&partial_lower)
                            {
                                candidates.push(CompletionCandidate {
                                    label: stem.to_string(),
                                    kind: CompletionCandidateKind::Page,
                                    detail: None,
                                });
                            }
                        }
                    }
                }
            }
            CompletionContext::WikiLinkHeading { target, partial } => {
                let partial_lower = partial.to_lowercase();
                let target_lower = target.to_lowercase();
                for (doc_uri, index) in self.realm.iter_documents() {
                    if let Some(path) = doc_uri.to_file_path() {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem.to_lowercase() == target_lower {
                                for heading in index.headings() {
                                    if partial_lower.is_empty()
                                        || heading.text.to_lowercase().contains(&partial_lower)
                                    {
                                        candidates.push(CompletionCandidate {
                                            label: heading.text.to_string(),
                                            kind: CompletionCandidateKind::Heading,
                                            detail: Some(format!("H{}", heading.level)),
                                        });
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            CompletionContext::Tag { partial } => {
                let partial_lower = partial.to_lowercase();
                for (tag_name, _count) in self.realm.tag_counts() {
                    if partial_lower.is_empty() || tag_name.to_lowercase().contains(&partial_lower)
                    {
                        candidates.push(CompletionCandidate {
                            label: tag_name.to_string(),
                            kind: CompletionCandidateKind::Tag,
                            detail: None,
                        });
                    }
                }
            }
            CompletionContext::BlockRef { partial } => {
                let partial_lower = partial.to_lowercase();
                for (_doc_uri, index) in self.realm.iter_documents() {
                    for block_id in index.block_ids() {
                        if partial_lower.is_empty()
                            || block_id.to_lowercase().contains(&partial_lower)
                        {
                            candidates.push(CompletionCandidate {
                                label: block_id.to_string(),
                                kind: CompletionCandidateKind::BlockRef,
                                detail: None,
                            });
                        }
                    }
                }
            }
            CompletionContext::XmlTag { partial } => {
                let partial_lower = partial.to_lowercase();
                // Collect unique XML tag names across all documents
                let mut seen = std::collections::HashSet::new();
                for (_doc_uri, index) in self.realm.iter_documents() {
                    for xt in index.xml_tags() {
                        if seen.insert(xt.tag_name.to_string())
                            && (partial_lower.is_empty()
                                || xt.tag_name.to_lowercase().contains(&partial_lower))
                        {
                            candidates.push(CompletionCandidate {
                                label: xt.tag_name.to_string(),
                                kind: CompletionCandidateKind::XmlTag,
                                detail: None,
                            });
                        }
                    }
                }
            }
        }

        candidates
    }
}
