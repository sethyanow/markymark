//! Zig SIMD kernel FFI bindings for markymark.
//!
//! This crate provides Rust-safe wrappers around the C ABI functions exported by
//! `libmarky_kernels.a` (built from the `zig/` directory). The build script
//! (`build.rs`) compiles the Zig library and links it statically.
//!
//! # Module Structure
//!
//! - [`scan`] — Heading, link, tag, and block-ID extraction via SIMD
//! - [`embed`] — Embedding index operations (add, search, cosine similarity)
//! - [`similarity`] — Jaccard similarity and related set operations
//! - [`tokens`] — Token estimation and content hashing
//! - [`hash`] — Entity hashing (FNV-1a based)
//! - [`index_serde`] — Binary index serialization (mmap-friendly format)

pub mod embed;
pub mod hash;
pub mod index_serde;
pub mod scan;
pub mod similarity;
pub mod tokens;

pub use scan::{
    fuzzy_match, fuzzy_match_batch, scan_block_ids, scan_headings, scan_links, scan_tags,
    BlockIdScan, FuzzyBatchMatch, FuzzyMatch, HeadingScan, KernelError, LinkScan, LinkType,
    TagScan,
};

// Re-export the raw FFI version check for linkage verification
extern "C" {
    fn marky_version() -> u32;
}

/// Returns the version of the linked Zig kernel library.
///
/// Format: `0xMMmmpp` where MM=major, mm=minor, pp=patch.
/// Returns `0x000100` for version 0.1.0.
///
/// This function verifies that the Zig static library is correctly linked.
pub fn kernel_version() -> u32 {
    // SAFETY: marky_version is a pure function with no side effects,
    // exported from libmarky_kernels.a via c_adapter.zig.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    unsafe { marky_version() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_version() {
        let version = kernel_version();
        assert_eq!(version, 0x000100, "Expected kernel version 0.1.0");
    }

    #[test]
    fn test_kernel_version_components() {
        let v = kernel_version();
        let major = (v >> 16) & 0xFF;
        let minor = (v >> 8) & 0xFF;
        let patch = v & 0xFF;
        assert_eq!(major, 0);
        assert_eq!(minor, 1);
        assert_eq!(patch, 0);
    }
}
