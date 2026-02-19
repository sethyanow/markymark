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
// ZigScanBackend implementation (behind zig-kernels feature)
// ---------------------------------------------------------------------------

/// Zig SIMD-accelerated scan backend.
///
/// This backend delegates to the `markymark-kernels` FFI functions for
/// SIMD-accelerated markdown element extraction.
#[cfg(feature = "zig-kernels")]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZigScanBackend;

#[cfg(feature = "zig-kernels")]
impl ScanBackend for ZigScanBackend {
    fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
        markymark_kernels::scan::scan_headings(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|h| HeadingResult {
                        text: h.text,
                        offset: h.offset,
                        level: h.level,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
        markymark_kernels::scan::scan_links(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|l| LinkResult {
                        offset: l.offset,
                        text: l.text,
                        target: l.target,
                        link_type: match l.link_type {
                            markymark_kernels::scan::LinkType::Markdown => ScanLinkType::Markdown,
                            markymark_kernels::scan::LinkType::Wiki => ScanLinkType::Wiki,
                        },
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError> {
        markymark_kernels::scan::scan_tags(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|t| TagResult {
                        name: t.name,
                        offset: t.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
        markymark_kernels::scan::scan_block_ids(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|b| BlockIdResult {
                        id: b.id,
                        offset: b.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError> {
        Ok(markymark_kernels::tokens::estimate_tokens(text))
    }
}

// ---------------------------------------------------------------------------
// Md4cScanBackend implementation (behind zig-kernels feature)
// ---------------------------------------------------------------------------

/// md4c-based scan backend using single-pass streaming extraction.
///
/// Uses the Zig md4c ExtractionRenderer via FFI for heading and link
/// extraction. Delegates tags, block IDs, and token estimation to the
/// same Zig SIMD kernels used by [`ZigScanBackend`].
#[cfg(feature = "zig-kernels")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Md4cScanBackend;

#[cfg(feature = "zig-kernels")]
impl ScanBackend for Md4cScanBackend {
    fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .headings
                    .into_iter()
                    .map(|h| HeadingResult {
                        text: h.text,
                        offset: h.source_offset,
                        level: h.level,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .links
                    .into_iter()
                    .map(|l| LinkResult {
                        offset: l.source_offset,
                        text: l.text,
                        target: l.target,
                        link_type: if l.is_wiki {
                            ScanLinkType::Wiki
                        } else {
                            ScanLinkType::Markdown
                        },
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError> {
        markymark_kernels::scan::scan_tags(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|t| TagResult {
                        name: t.name,
                        offset: t.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
        markymark_kernels::scan::scan_block_ids(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|b| BlockIdResult {
                        id: b.id,
                        offset: b.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError> {
        Ok(markymark_kernels::tokens::estimate_tokens(text))
    }
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
        fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> {
            Ok(Vec::new())
        }

        fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> {
            Ok(Vec::new())
        }

        fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> {
            Ok(Vec::new())
        }

        fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
            Ok(Vec::new())
        }

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
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DummyScanBackend>();

        // Also verify the trait object is Send + Sync.
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

    // --- Md4cScanBackend tests (feature-gated) ---

    #[cfg(feature = "zig-kernels")]
    mod md4c_tests {
        use super::super::*;

        #[test]
        fn test_md4c_scan_backend_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<Md4cScanBackend>();
        }

        #[test]
        fn test_md4c_scan_backend_trait_object() {
            let backend: Box<dyn ScanBackend> = Box::new(Md4cScanBackend);
            let result = backend.scan_headings("# Hello");
            assert!(result.is_ok());
        }

        #[test]
        fn test_md4c_scan_headings_basic() {
            let backend = Md4cScanBackend;
            let results = backend.scan_headings("# Hello\n").unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].text, "Hello");
            assert_eq!(results[0].level, 1);
            assert_eq!(results[0].offset, 0);
        }

        #[test]
        fn test_md4c_scan_links_markdown() {
            let backend = Md4cScanBackend;
            let results = backend
                .scan_links("[click](https://example.com)\n")
                .unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].text, "click");
            assert_eq!(results[0].target, "https://example.com");
            assert_eq!(results[0].link_type, ScanLinkType::Markdown);
        }

        #[test]
        fn test_md4c_scan_links_wiki() {
            let backend = Md4cScanBackend;
            let results = backend.scan_links("[[Target]]\n").unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].link_type, ScanLinkType::Wiki);
            assert_eq!(results[0].target, "Target");
        }

        #[test]
        fn test_md4c_scan_empty_input() {
            let backend = Md4cScanBackend;
            assert!(backend.scan_headings("").unwrap().is_empty());
            assert!(backend.scan_links("").unwrap().is_empty());
            assert!(backend.scan_tags("").unwrap().is_empty());
            assert!(backend.scan_block_ids("").unwrap().is_empty());
            assert_eq!(backend.estimate_tokens("").unwrap(), 0);
        }

        #[test]
        fn test_md4c_scan_entity_decoded() {
            // md4c ExtractionRenderer decodes HTML entities to UTF-8 (marky-yfh7)
            let backend = Md4cScanBackend;
            let results = backend.scan_headings("# Hello &amp; World\n").unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].text, "Hello & World");
        }
    }
}
