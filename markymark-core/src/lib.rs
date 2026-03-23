//! markymark-core: Core types and abstractions for the markymark LSP
//!
//! This crate provides the fundamental data structures and traits
//! used across all markymark crates.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub use error::{CoreError, CoreResult};

/// A 0-based position in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// 0-based line index.
    pub line: u32,

    /// 0-based character offset (UTF-16 code unit in LSP, but we treat as an opaque index here).
    pub character: u32,
}

impl Position {
    /// Create a new [`Position`].
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.line.cmp(&other.line) {
            Ordering::Equal => self.character.cmp(&other.character),
            o => o,
        }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A range in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,

    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    /// Create a new [`Range`].
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Returns true if `position` is within this range.
    ///
    /// Semantics follow LSP conventions: start is inclusive, end is exclusive.
    pub fn contains(&self, position: Position) -> bool {
        self.start <= position && position < self.end
    }
}

/// A document URI.
///
/// For now this is a small wrapper with basic support for `file://` URIs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentUri(String);

impl DocumentUri {
    /// Parse a URI string.
    ///
    /// This currently enforces that a scheme is present (e.g. `file://`).
    pub fn new(uri: &str) -> CoreResult<Self> {
        if uri.contains("://") {
            Ok(Self(uri.to_string()))
        } else {
            Err(CoreError::InvalidUri("URI is missing scheme".to_string()))
        }
    }

    /// Return the URI as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct a `file://` URI from a filesystem path.
    pub fn from_file_path(path: &Path) -> Self {
        let raw = path.to_string_lossy();
        let encoded = percent_encode_path(&raw);
        Self(format!("file://{}", encoded))
    }

    /// Convert a `file://` URI to a filesystem path.
    pub fn to_file_path(&self) -> Option<PathBuf> {
        let rest = self.0.strip_prefix("file://")?;
        let decoded = percent_decode(rest)?;
        Some(PathBuf::from(decoded))
    }
}

fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());

    for b in path.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }

    out
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }

            let hi = from_hex(bytes[i + 1])?;
            let lo = from_hex(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).ok()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub mod arena;
pub mod embeddings;
pub mod engine;
pub mod frontmatter;
pub mod graph_traits;
pub mod inference;
pub mod scanner;
pub mod sidecar;
pub mod structured;

pub use frontmatter::TypedFrontmatter;
pub use graph_traits::{EdgeKind, GraphNode};

pub mod prelude {
    //! Prelude module with common imports

    pub use crate::embeddings::{EmbedError, EmbeddingProvider};
    pub use crate::inference::{InferenceError, InferenceProvider};
    #[cfg(feature = "zig-kernels")]
    pub use crate::scanner::ZigScanBackend;
    pub use crate::scanner::{ScanAllResult, ScanBackend, ScanError};
    pub use crate::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
    pub use crate::{CoreError, CoreResult, DocumentUri, Position, Range};
}

pub mod error {
    //! Core error types

    use thiserror::Error;

    /// Core result type
    pub type CoreResult<T> = Result<T, CoreError>;

    /// Core error type
    #[derive(Error, Debug)]
    pub enum CoreError {
        /// Generic error message
        #[error("{0}")]
        Message(String),

        /// Invalid URI
        #[error("Invalid URI: {0}")]
        InvalidUri(String),

        /// Not implemented yet
        #[error("Not implemented: {0}")]
        NotImplemented(String),
    }
}
