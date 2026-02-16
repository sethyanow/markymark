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
// Zig SIMD scan backend (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "zig-kernels")]
mod zig_scan_backend {
    use super::*;
    use markymark_kernels::scan;
    use markymark_kernels::tokens;

    /// SIMD-accelerated scan backend backed by Zig kernels.
    ///
    /// Zero-size struct — all state lives in the kernel FFI calls.
    pub struct ZigScanBackend;

    impl ZigScanBackend {
        fn map_kernel_error(e: scan::KernelError) -> ScanError {
            match e {
                scan::KernelError::InvalidInput => {
                    ScanError::InvalidInput("kernel: invalid input".into())
                }
                scan::KernelError::BufferTooSmall => {
                    ScanError::InternalError("kernel: buffer too small after retries".into())
                }
                scan::KernelError::InternalError(code) => {
                    ScanError::InternalError(format!("kernel: internal error (code {code})"))
                }
            }
        }
    }

    impl ScanBackend for ZigScanBackend {
        fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
            scan::scan_headings(text)
                .map(|v| {
                    v.into_iter()
                        .map(|h| HeadingResult {
                            text: h.text,
                            offset: h.offset,
                            level: h.level,
                        })
                        .collect()
                })
                .map_err(Self::map_kernel_error)
        }

        fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
            scan::scan_links(text)
                .map(|v| {
                    v.into_iter()
                        .map(|l| LinkResult {
                            offset: l.offset,
                            text: l.text,
                            target: l.target,
                            link_type: match l.link_type {
                                scan::LinkType::Wiki => ScanLinkType::Wiki,
                                scan::LinkType::Markdown => ScanLinkType::Markdown,
                            },
                        })
                        .collect()
                })
                .map_err(Self::map_kernel_error)
        }

        fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError> {
            scan::scan_tags(text)
                .map(|v| {
                    v.into_iter()
                        .map(|t| TagResult {
                            name: t.name,
                            offset: t.offset,
                        })
                        .collect()
                })
                .map_err(Self::map_kernel_error)
        }

        fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
            scan::scan_block_ids(text)
                .map(|v| {
                    v.into_iter()
                        .map(|b| BlockIdResult {
                            id: b.id,
                            offset: b.offset,
                        })
                        .collect()
                })
                .map_err(Self::map_kernel_error)
        }

        fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError> {
            Ok(tokens::estimate_tokens(text))
        }
    }
}

#[cfg(feature = "zig-kernels")]
pub use zig_scan_backend::ZigScanBackend;

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
}

// ---------------------------------------------------------------------------
// Zig-kernels backed tests (only run with `zig-kernels` feature)
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "zig-kernels"))]
mod zig_tests {
    use super::*;

    #[test]
    fn test_zig_scan_backend_headings() {
        let backend = ZigScanBackend;
        let results = backend.scan_headings("# Hello\n## World\n").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "Hello");
        assert_eq!(results[0].level, 1);
        assert_eq!(results[1].text, "World");
        assert_eq!(results[1].level, 2);
    }

    #[test]
    fn test_zig_scan_backend_links() {
        let backend = ZigScanBackend;
        let results = backend
            .scan_links("Click [here](https://example.com) and [[wiki]]")
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "here");
        assert_eq!(results[0].target, "https://example.com");
        assert_eq!(results[0].link_type, ScanLinkType::Markdown);
        assert_eq!(results[1].link_type, ScanLinkType::Wiki);
    }

    #[test]
    fn test_zig_scan_backend_tags() {
        let backend = ZigScanBackend;
        let results = backend.scan_tags("text #tag1 #tag2").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "tag1");
        assert_eq!(results[1].name, "tag2");
    }

    #[test]
    fn test_zig_scan_backend_block_ids() {
        let backend = ZigScanBackend;
        let results = backend.scan_block_ids("text ^my-block\n").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "my-block");
    }

    #[test]
    fn test_zig_scan_backend_estimate_tokens() {
        let backend = ZigScanBackend;
        let count = backend.estimate_tokens("hello world foo bar").unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_zig_scan_backend_empty_input() {
        let backend = ZigScanBackend;
        assert!(backend.scan_headings("").unwrap().is_empty());
        assert!(backend.scan_links("").unwrap().is_empty());
        assert!(backend.scan_tags("").unwrap().is_empty());
        assert!(backend.scan_block_ids("").unwrap().is_empty());
        assert_eq!(backend.estimate_tokens("").unwrap(), 0);
    }

    #[test]
    fn test_zig_scan_backend_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ZigScanBackend>();
    }

    #[test]
    fn test_zig_scan_backend_as_trait_object() {
        let backend: Box<dyn ScanBackend> = Box::new(ZigScanBackend);
        let results = backend.scan_headings("# Test").unwrap();
        assert_eq!(results.len(), 1);
    }
}
