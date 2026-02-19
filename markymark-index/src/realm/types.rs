//! Cross-document resolved types for the realm index.
//!
//! These are owned copies of heading and block data that survive
//! removal of individual documents (unlike arena-backed document entries).

use markymark_core::prelude::*;

use crate::document::DocumentIndex;
use crate::structured_document::StructuredDocumentIndex;

/// Owned copy of heading data for cross-document lookups.
/// Survives removal of individual documents (unlike arena-backed document entries).
#[derive(Debug, Clone)]
pub struct ResolvedHeading {
    /// The heading text.
    pub text: String,
    /// URL-safe slug.
    pub slug: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range.
    pub range: Range,
}

/// Owned copy of block data for cross-document lookups.
#[derive(Debug, Clone)]
pub struct ResolvedBlock {
    /// Block identifier.
    pub id: String,
    /// Source range.
    pub range: Range,
}

/// Either a markdown or structured document index.
pub enum AnyDocumentIndex {
    /// Markdown document indexed from tree-sitter AST.
    Markdown(DocumentIndex),
    /// Structured document (JSON, YAML, TOML, etc.) indexed from key entries.
    Structured(StructuredDocumentIndex),
}

impl AnyDocumentIndex {
    /// Returns the markdown index if this is a markdown document.
    pub fn as_markdown(&self) -> Option<&DocumentIndex> {
        match self {
            Self::Markdown(idx) => Some(idx),
            Self::Structured(_) => None,
        }
    }

    /// Returns the structured index if this is a structured document.
    pub fn as_structured(&self) -> Option<&StructuredDocumentIndex> {
        match self {
            Self::Markdown(_) => None,
            Self::Structured(idx) => Some(idx),
        }
    }

    /// Whether this is a markdown document.
    pub fn is_markdown(&self) -> bool {
        matches!(self, Self::Markdown(_))
    }

    /// Whether this is a structured document.
    pub fn is_structured(&self) -> bool {
        matches!(self, Self::Structured(_))
    }
}

impl std::fmt::Debug for AnyDocumentIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(_) => f.debug_tuple("Markdown").field(&"DocumentIndex").finish(),
            Self::Structured(idx) => f.debug_tuple("Structured").field(idx).finish(),
        }
    }
}
