//! Scan backend trait for markdown element extraction.
//!
//! [`ScanBackend`] provides a transport-agnostic interface for scanning
//! markdown text for structural elements (headings, links, tags, block IDs)
//! and estimating token counts. Implementations include:
//!
//! - `TreeSitterScanBackend` (always available) — wraps `markymark-parser`
//! - `ZigScanBackend` (behind `zig-kernels` feature) — wraps `markymark-kernels`

use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by scan backend operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    /// Invalid or malformed input text.
    InvalidInput(String),
    /// Internal scanner failure.
    InternalError(String),
}

impl fmt::Display for ScanError {
    /// Formats a `ScanError` into a human-readable message.
    ///
    /// Produces `scan: invalid input: {msg}` for `ScanError::InvalidInput(msg)` and
    /// `scan: internal error: {msg}` for `ScanError::InternalError(msg)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::scanner::ScanError;
    ///
    /// let e = ScanError::InvalidInput("bad markdown".into());
    /// assert_eq!(format!("{}", e), "scan: invalid input: bad markdown");
    ///
    /// let e2 = ScanError::InternalError("parser failed".into());
    /// assert_eq!(format!("{}", e2), "scan: internal error: parser failed");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "scan: invalid input: {msg}"),
            Self::InternalError(msg) => write!(f, "scan: internal error: {msg}"),
        }
    }
}

impl std::error::Error for ScanError {}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A heading found by a scan backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingResult {
    /// Heading text content.
    pub text: String,
    /// Byte offset of the heading in the source text.
    pub offset: u32,
    /// Heading level (1–6).
    pub level: u8,
}

/// Link type discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLinkType {
    /// Standard markdown link `[text](url)`.
    Markdown,
    /// Wiki-style link `[[target]]` or `[[target|display]]`.
    Wiki,
}

/// A link found by a scan backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkResult {
    /// Byte offset of the link in the source text.
    pub offset: u32,
    /// Display text of the link.
    pub text: String,
    /// Link target / URL.
    pub target: String,
    /// Whether this is a markdown or wiki link.
    pub link_type: ScanLinkType,
}

/// A tag found by a scan backend (e.g. `#topic`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagResult {
    /// Tag name without the leading `#`.
    pub name: String,
    /// Byte offset of the `#` in the source text.
    pub offset: u32,
}

/// A block ID found by a scan backend (e.g. `^block-id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIdResult {
    /// Block ID without the leading `^`.
    pub id: String,
    /// Byte offset of the `^` in the source text.
    pub offset: u32,
}

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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Dummy implementation for compile-time trait checks.
    struct DummyScanBackend;

    impl ScanBackend for DummyScanBackend {
        /// Extracts heading elements from the provided Markdown text.
        ///
        /// # Returns
        ///
        /// A `Vec<HeadingResult>` containing one entry per heading found; each entry includes the heading text, byte offset, and level.
        ///
        /// # Examples
        ///
        /// ```
        /// // Given an implementation of `ScanBackend` named `scanner`:
        /// // let scanner: &dyn ScanBackend = ...;
        /// // let headings = scanner.scan_headings("# Title\n\n## Subtitle").unwrap();
        /// // assert_eq!(headings.len(), 2);
        /// ```
        fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> {
            Ok(Vec::new())
        }

        /// Extracts all links from the given markdown text and returns their metadata.
        ///
        /// Returns a `Vec<LinkResult>` containing one entry per discovered link, or a
        /// `ScanError` if the input cannot be scanned.
        ///
        /// # Examples
        ///
        /// ```
        /// // A backend implementation may return zero or more LinkResult entries.
        /// struct Dummy;
        /// impl ScanBackend for Dummy {
        ///     fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> { Ok(Vec::new()) }
        ///     fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> { Ok(Vec::new()) }
        ///     fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> { Ok(Vec::new()) }
        ///     fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> { Ok(Vec::new()) }
        ///     fn estimate_tokens(&self, _text: &str) -> Result<u32, ScanError> { Ok(0) }
        /// }
        ///
        /// let backend = Dummy;
        /// let links = backend.scan_links("No links here").unwrap();
        /// assert!(links.is_empty());
        /// ```
        fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> {
            Ok(Vec::new())
        }

        /// Scans the provided markdown text and extracts tags (tokens beginning with `#`).
        ///
        /// Returns a `TagResult` for each tag found, where `name` excludes the leading `#` and `offset` is the byte offset of the `#` in the source.
        ///
        /// # Parameters
        ///
        /// - `text` — the markdown source to scan for tags.
        ///
        /// # Returns
        ///
        /// `Ok(Vec<TagResult>)` with one entry per tag found; `Err(ScanError)` if the input is invalid or the scanner encounters an internal error.
        ///
        /// # Examples
        ///
        /// ```
        /// // Example usage; a concrete backend must implement `ScanBackend`.
        /// struct DummyScanBackend;
        /// impl markymark_core::scanner::ScanBackend for DummyScanBackend {
        ///     fn scan_headings(&self, _text: &str) -> Result<Vec<markymark_core::scanner::HeadingResult>, markymark_core::scanner::ScanError> { Ok(Vec::new()) }
        ///     fn scan_links(&self, _text: &str) -> Result<Vec<markymark_core::scanner::LinkResult>, markymark_core::scanner::ScanError> { Ok(Vec::new()) }
        ///     fn scan_tags(&self, _text: &str) -> Result<Vec<markymark_core::scanner::TagResult>, markymark_core::scanner::ScanError> { Ok(Vec::new()) }
        ///     fn scan_block_ids(&self, _text: &str) -> Result<Vec<markymark_core::scanner::BlockIdResult>, markymark_core::scanner::ScanError> { Ok(Vec::new()) }
        ///     fn estimate_tokens(&self, _text: &str) -> Result<u32, markymark_core::scanner::ScanError> { Ok(0) }
        /// }
        ///
        /// let backend: &dyn markymark_core::scanner::ScanBackend = &DummyScanBackend;
        /// let result = backend.scan_tags("#tag1 #tag2");
        /// assert!(result.is_ok());
        /// ```
        fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> {
            Ok(Vec::new())
        }

        /// Finds block IDs (identifiers preceded by `^`) in the given markdown text.
        ///
        /// Returns a vector of `BlockIdResult` entries containing the block ID (without the leading `^`)
        /// and the byte offset at which the `^` was found.
        ///
        /// # Examples
        ///
        /// ```
        /// let backend = DummyScanBackend;
        /// let res = backend.scan_block_ids("para ^blockid").unwrap();
        /// assert_eq!(res.len(), 1);
        /// assert_eq!(res[0].id, "blockid");
        /// ```
        fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
            Ok(Vec::new())
        }

        /// Estimates the number of tokens in the provided text for downstream processing.
        ///
        /// # Returns
        ///
        /// `Ok(n)` with the estimated token count, `Err(ScanError)` if estimation fails.
        ///
        /// # Examples
        ///
        /// ```
        /// let backend = DummyScanBackend;
        /// let tokens = backend.estimate_tokens("hello world").unwrap();
        /// assert_eq!(tokens, 0);
        /// ```
        fn estimate_tokens(&self, _text: &str) -> Result<u32, ScanError> {
            Ok(0)
        }
    }

    #[test]
    fn test_scan_backend_trait_object() {
        // Verifies ScanBackend is object-safe (dyn-compatible).
        let backend: Box<dyn ScanBackend> = Box::new(DummyScanBackend);
        let result = backend.scan_headings("# Hello");
        assert!(result.is_ok());
    }

    #[test]
    fn test_scan_backend_send_sync() {
        // Verifies ScanBackend implementations are Send + Sync.
        /// Asserts at compile time that a type implements both `Send` and `Sync`.
///
/// This is a zero-cost helper used in tests to verify thread-safety bounds for a type; it has no runtime effect.
///
/// # Examples
///
/// ```
/// // Fails to compile if `T` is not `Send + Sync`.
/// fn assert_send_sync<T: Send + Sync>() {}
///
/// // Usage:
/// assert_send_sync::<i32>();
/// ```
fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DummyScanBackend>();

        // Also verify the trait object is Send + Sync.
        /// Asserts at compile time that a `dyn ScanBackend` trait object is `Send + Sync`.

///

/// This helper exists solely for compile-time checks; it takes a reference to a trait object

/// typed as `dyn ScanBackend + Send + Sync` and is a no-op at runtime.

///

/// # Examples

///

/// ```

/// use markymark_core::scanner::{ScanBackend, HeadingResult, LinkResult, TagResult, BlockIdResult, ScanError};

///

/// struct Dummy;

///

/// impl ScanBackend for Dummy {

///     fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> { Ok(vec![]) }

///     fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> { Ok(vec![]) }

///     fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> { Ok(vec![]) }

///     fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> { Ok(vec![]) }

///     fn estimate_tokens(&self, _text: &str) -> Result<u32, ScanError> { Ok(0) }

/// }

///

/// let dummy = Dummy;

/// // Compile-time assertion that `&dyn ScanBackend` can be used as `&dyn ScanBackend + Send + Sync`.

/// crate::scanner::assert_dyn_send_sync(&dummy as &(dyn ScanBackend + Send + Sync));

/// ```
fn assert_dyn_send_sync(_: &(dyn ScanBackend + Send + Sync)) {}
        let backend = DummyScanBackend;
        assert_dyn_send_sync(&backend);
    }

    #[test]
    fn test_scan_error_display() {
        let err = ScanError::InvalidInput("bad text".to_string());
        assert_eq!(err.to_string(), "scan: invalid input: bad text");

        let err = ScanError::InternalError("crash".to_string());
        assert_eq!(err.to_string(), "scan: internal error: crash");
    }

    #[test]
    fn test_heading_result_fields() {
        let h = HeadingResult {
            text: "Hello".to_string(),
            offset: 2,
            level: 1,
        };
        assert_eq!(h.text, "Hello");
        assert_eq!(h.offset, 2);
        assert_eq!(h.level, 1);
    }

    #[test]
    fn test_link_result_fields() {
        let l = LinkResult {
            offset: 0,
            text: "click".to_string(),
            target: "https://example.com".to_string(),
            link_type: ScanLinkType::Markdown,
        };
        assert_eq!(l.link_type, ScanLinkType::Markdown);

        let w = LinkResult {
            offset: 0,
            text: "Page".to_string(),
            target: "My Page".to_string(),
            link_type: ScanLinkType::Wiki,
        };
        assert_eq!(w.link_type, ScanLinkType::Wiki);
    }

    #[test]
    fn test_tag_result_fields() {
        let t = TagResult {
            name: "topic".to_string(),
            offset: 5,
        };
        assert_eq!(t.name, "topic");
    }

    #[test]
    fn test_block_id_result_fields() {
        let b = BlockIdResult {
            id: "my-block".to_string(),
            offset: 10,
        };
        assert_eq!(b.id, "my-block");
    }
}