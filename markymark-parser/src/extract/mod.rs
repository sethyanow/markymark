//! Extraction functions for arena-allocated markdown types.
//!
//! Only frontmatter/property extraction remains in Rust. All markdown-content
//! extraction (headings, links, tags, blocks, etc.) is handled by the Zig
//! ExtractionRenderer via the scan backend.

mod frontmatter;
pub use frontmatter::{extract_frontmatter, extract_page_properties};
