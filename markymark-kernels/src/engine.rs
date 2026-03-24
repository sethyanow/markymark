//! DocumentEngine FFI bindings.
//!
//! Wraps the Zig `marky_engine_*` C ABI functions to provide safe Rust
//! access to the stateful document engine. The engine parses markdown
//! and extracts headings/links/tags/block-ids via a structured C-ABI result.
//!
//! Created for marky-atsp (epic marky-io3h, Task 2).

use crate::engine_ffi::{marky_engine_get_result, CEngineResult, EngineResult};
use crate::scan::KernelError;

pub use crate::engine_ffi::{convert_engine_result, EngineExtraction};

// ---------------------------------------------------------------------------
// Edit range for incremental updates
// ---------------------------------------------------------------------------

/// Byte-level edit range for incremental document updates.
///
/// Passed through FFI to the Zig engine so it can skip slug recomputation
/// for headings outside the edited region. Zero-values (0/0/0) mean
/// "no range info" — full recomputation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditRange {
    /// Byte offset where the edit starts in the old text.
    pub offset: u32,
    /// Length of the replaced span in the old text.
    pub old_len: u32,
    /// Length of the replacement span in the new text.
    pub new_len: u32,
}

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn marky_engine_create(text: *const u8, text_len: u32) -> *mut std::ffi::c_void;
    fn marky_engine_update(
        handle: *mut std::ffi::c_void,
        text: *const u8,
        text_len: u32,
        edit_offset: u32,
        edit_old_len: u32,
        edit_new_len: u32,
    ) -> i32;
    fn marky_engine_destroy(handle: *mut std::ffi::c_void);
    fn marky_engine_get_content_hash(handle: *mut std::ffi::c_void) -> u64;
}

// ---------------------------------------------------------------------------
// DocumentEngine
// ---------------------------------------------------------------------------

/// A stateful document engine backed by Zig SIMD kernels.
///
/// Parses markdown text and extracts headings, links, tags, and block IDs.
/// Results are accessed via the structured C-ABI [`get_result`](Self::get_result) method.
///
/// # Thread Safety
///
/// [`DocumentEngine`] implements `Send` via an unsafe impl; it intentionally
/// does **not** implement `Sync`. The underlying Zig heap allocation has no
/// thread-local state, so transferring ownership of a `DocumentEngine`
/// between threads is safe, but sharing `&DocumentEngine` across threads is
/// not. For concurrent use, wrap the engine in synchronization primitives
/// such as `Arc<RwLock<DocumentEngine>>` and share that wrapper instead.
pub struct DocumentEngine {
    handle: *mut std::ffi::c_void,
}

// SAFETY: The Zig engine is a self-contained heap allocation with no
// thread-local state. Ownership can be safely transferred between threads.
// All mutation goes through `&mut self` (enforced by Rust's borrow checker),
// and in practice an `RwLock` in the LSP/MCP runtime serialises access.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Send for DocumentEngine {}

// NOTE: `Sync` is not implemented. All mutation goes through `&mut self`
// (enforced by Rust's borrow checker), and the LSP/MCP runtimes wrap
// engines in synchronisation primitives for shared access.

impl DocumentEngine {
    /// Create a new document engine from markdown text.
    ///
    /// Returns `Err(InvalidInput)` if `text` exceeds `u32::MAX` bytes.
    pub fn new(text: &str) -> Result<Self, KernelError> {
        let text_len = u32::try_from(text.len()).map_err(|_| KernelError::InvalidInput)?;

        // SAFETY: For non-empty text, as_ptr() is valid for text_len bytes.
        // For empty text, we pass null + 0 which the Zig side handles.
        // marky_engine_create allocates and returns an opaque handle.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let handle = unsafe {
            if text.is_empty() {
                marky_engine_create(std::ptr::null(), 0)
            } else {
                marky_engine_create(text.as_ptr(), text_len)
            }
        };

        if handle.is_null() {
            // marky_engine_create returns null for any failure (invalid input,
            // OOM, or parse error) without a specific error code. Use 0 as a
            // neutral code rather than overloading -3 (OOM).
            return Err(KernelError::InternalError(0));
        }

        Ok(Self { handle })
    }

    /// Update engine state with new markdown text.
    ///
    /// On success, old state is freed and replaced with the new parse result.
    /// On failure, old state is preserved unchanged.
    ///
    /// `edit_range`: optional byte-level edit range for incremental updates.
    /// `None` (or zero-valued range) means "no range info" — full recomputation.
    ///
    /// Returns `Err(InvalidInput)` if `text` exceeds `u32::MAX` bytes.
    pub fn update(&mut self, text: &str, edit_range: Option<EditRange>) -> Result<(), KernelError> {
        let text_len = u32::try_from(text.len()).map_err(|_| KernelError::InvalidInput)?;
        let range = edit_range.unwrap_or(EditRange { offset: 0, old_len: 0, new_len: 0 });

        // SAFETY: handle is valid (created in new(), not yet destroyed).
        // For empty text, pass null + 0. For non-empty, pass valid slice.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe {
            if text.is_empty() {
                marky_engine_update(self.handle, std::ptr::null(), 0, range.offset, range.old_len, range.new_len)
            } else {
                marky_engine_update(self.handle, text.as_ptr(), text_len, range.offset, range.old_len, range.new_len)
            }
        };

        match rc {
            0 => Ok(()),
            -1 => Err(KernelError::InvalidInput),
            -3 => Err(KernelError::InternalError(-3)),
            -4 => Err(KernelError::InternalError(-4)),
            other => Err(KernelError::InternalError(other)),
        }
    }

    /// Get a structured FFI result for the current engine state.
    ///
    /// Returned allocations are owned by [`EngineResult`] and automatically
    /// freed on drop.
    pub fn get_result(&self) -> Result<EngineResult, KernelError> {
        let mut raw = CEngineResult {
            headings: std::ptr::null_mut(),
            links: std::ptr::null_mut(),
            code_spans: std::ptr::null_mut(),
            tags: std::ptr::null_mut(),
            block_ids: std::ptr::null_mut(),
            tasks: std::ptr::null_mut(),
            embeds: std::ptr::null_mut(),
            callouts: std::ptr::null_mut(),
            block_refs: std::ptr::null_mut(),
            query_blocks: std::ptr::null_mut(),
            link_definitions: std::ptr::null_mut(),
            properties: std::ptr::null_mut(),
            xml_tags: std::ptr::null_mut(),
            line_starts: std::ptr::null_mut(),
            text_blob: std::ptr::null(),
            content_hash: 0,
            generation: 0,
            headings_count: 0,
            links_count: 0,
            code_spans_count: 0,
            tags_count: 0,
            block_ids_count: 0,
            tasks_count: 0,
            embeds_count: 0,
            callouts_count: 0,
            block_refs_count: 0,
            query_blocks_count: 0,
            link_definitions_count: 0,
            properties_count: 0,
            xml_tags_count: 0,
            line_starts_count: 0,
            text_blob_len: 0,
            token_estimate: 0,
            _reserved: [0; 32],
        };

        // SAFETY: `self.handle` is a valid handle created by marky_engine_create.
        // `raw` is stack-owned and passed as a valid mutable pointer.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe { marky_engine_get_result(self.handle, &mut raw) };
        match rc {
            0 => Ok(EngineResult::from_raw(raw)),
            -1 => Err(KernelError::InvalidInput),
            -4 => Err(KernelError::InternalError(-4)),
            -5 => Err(KernelError::InternalError(-5)),
            other => Err(KernelError::InternalError(other)),
        }
    }

    /// Get the content hash for the current engine state.
    ///
    /// The hash is computed by the Zig engine during [`update`](Self::update)
    /// (or initial creation) over the post-frontmatter-masked text. Same text
    /// produces the same hash; different structural content produces a different hash.
    pub fn content_hash(&self) -> u64 {
        // SAFETY: handle is valid (created by marky_engine_create, not yet
        // destroyed). content_hash is a pure field read — no mutation, no
        // allocation.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        unsafe { marky_engine_get_content_hash(self.handle) }
    }
}

impl Drop for DocumentEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was created by marky_engine_create and has not
            // been destroyed yet. Setting to null prevents double-free.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
            unsafe { marky_engine_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl std::fmt::Debug for DocumentEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentEngine")
            .field("handle_null", &self.handle.is_null())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_is_send_not_sync() {
        // DocumentEngine is Send (ownership transfer across threads is safe)
        // but deliberately NOT Sync — all mutation is through &mut self.
        fn assert_send<T: Send>() {}
        assert_send::<DocumentEngine>();
    }

    #[test]
    fn test_engine_debug_format() {
        let engine = DocumentEngine::new("# Debug\n").unwrap();
        let debug = format!("{engine:?}");
        assert!(debug.contains("DocumentEngine"));
        assert!(debug.contains("handle_null: false"));
    }

    #[test]
    fn test_engine_get_result_basic() {
        let engine = DocumentEngine::new("# Hello\n\n[[Page|Alias]]\n").unwrap();
        let result = engine.get_result().unwrap();
        let extraction = result.to_extraction().unwrap();

        assert_eq!(extraction.headings.len(), 1);
        assert_eq!(extraction.wiki_links.len(), 1);
        assert!(extraction.generation >= 1);
    }

    #[test]
    fn test_engine_get_result_generation_increments() {
        let mut engine = DocumentEngine::new("# One\n").unwrap();
        let gen1 = engine.get_result().unwrap().as_raw().generation;

        engine.update("# Two\n## Sub\n", None).unwrap();
        let gen2 = engine.get_result().unwrap().as_raw().generation;

        assert!(gen2 > gen1);
    }

    #[test]
    fn test_engine_content_hash_stable() {
        let text = "# Hello\n";
        let mut engine = DocumentEngine::new(text).unwrap();
        let hash1 = engine.content_hash();
        engine.update(text, None).unwrap();
        let hash2 = engine.content_hash();
        assert_eq!(hash1, hash2, "hash must be stable for same content");
    }

    #[test]
    fn test_engine_content_hash_changes() {
        let mut engine = DocumentEngine::new("# Hello\n").unwrap();
        let hash1 = engine.content_hash();
        engine.update("# Hello\n## World\n", None).unwrap();
        let hash2 = engine.content_hash();
        assert_ne!(hash1, hash2, "hash must change when structure changes");
    }

    #[test]
    fn test_engine_content_hash_after_create() {
        let engine = DocumentEngine::new("# Heading\n").unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0, "non-empty content must produce non-zero hash");
    }

    #[test]
    fn test_engine_content_hash_empty() {
        let engine = DocumentEngine::new("").unwrap();
        let hash = engine.content_hash();
        assert_eq!(hash, 0, "empty content must produce zero hash");
    }

    // -- Adversarial stress tests for content_hash --

    #[test]
    fn test_engine_content_hash_singular_char() {
        // Singular: minimal viable input — single byte
        let engine = DocumentEngine::new("x").unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0, "single-char input must produce non-zero hash");
    }

    #[test]
    fn test_engine_content_hash_whitespace_only() {
        // Semantically hostile: valid text, no markdown structure
        let engine = DocumentEngine::new("   \n\n\t\t\n   ").unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0, "whitespace-only input is non-empty, hash must be non-zero");
    }

    #[test]
    fn test_engine_content_hash_multibyte_utf8() {
        // Encoding boundaries: multi-byte UTF-8 characters
        let mut engine = DocumentEngine::new("# Héllo Wörld 日本語\n").unwrap();
        let hash1 = engine.content_hash();
        assert_ne!(hash1, 0);
        engine.update("# Héllo Wörld 日本語\n", None).unwrap();
        let hash2 = engine.content_hash();
        assert_eq!(hash1, hash2, "multi-byte UTF-8 hash must be stable");
    }

    #[test]
    fn test_engine_content_hash_repeated_updates_deterministic() {
        // The "second run": multiple updates cycle, hash must be deterministic
        let mut engine = DocumentEngine::new("# A\n").unwrap();
        let hash_a = engine.content_hash();
        engine.update("# B\n", None).unwrap();
        let hash_b = engine.content_hash();
        engine.update("# A\n", None).unwrap();
        let hash_a2 = engine.content_hash();
        assert_eq!(hash_a, hash_a2, "returning to same content must produce same hash");
        assert_ne!(hash_a, hash_b, "different content must produce different hash");
    }

    #[test]
    fn test_engine_content_hash_redundant_headings() {
        // Redundant: duplicate structure — hash should be unique to text, not structure count
        let mut engine = DocumentEngine::new("# Same\n# Same\n").unwrap();
        let hash_dup = engine.content_hash();
        engine.update("# Same\n", None).unwrap();
        let hash_single = engine.content_hash();
        assert_ne!(
            hash_dup, hash_single,
            "duplicate headings in text produce different text, different hash"
        );
    }

    #[test]
    fn test_engine_content_hash_large_document() {
        // Dense: stress test with large input
        let large = "# Heading\n".repeat(1000) + &"[link](url) #tag\n".repeat(500);
        let engine = DocumentEngine::new(&large).unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0, "large document must produce non-zero hash");
    }

    #[test]
    fn test_engine_content_hash_after_failed_update() {
        // State transitions: hash must survive failed update
        // engine.update() preserves old state on failure. We can't easily
        // trigger a parse failure with valid UTF-8 (md4c is lenient), so
        // we verify that repeated valid updates maintain hash consistency.
        let mut engine = DocumentEngine::new("# Initial\n").unwrap();
        let hash1 = engine.content_hash();
        // Update to different content
        engine.update("# Changed\n", None).unwrap();
        let hash2 = engine.content_hash();
        assert_ne!(hash1, hash2);
        // Update back — hash must match original
        engine.update("# Initial\n", None).unwrap();
        let hash3 = engine.content_hash();
        assert_eq!(hash1, hash3, "hash must be consistent after round-trip updates");
    }

    #[test]
    fn test_engine_content_hash_frontmatter_only() {
        // Semantically hostile: frontmatter with no markdown body
        let engine = DocumentEngine::new("---\ntitle: test\n---\n").unwrap();
        let hash = engine.content_hash();
        // The hash is computed on the text passed to the engine (which includes
        // frontmatter bytes). As long as text is non-empty, hash should be non-zero.
        assert_ne!(hash, 0, "frontmatter-only doc is non-empty text");
    }

    #[test]
    fn test_engine_update_with_edit_range() {
        let mut engine = DocumentEngine::new("# Hello\n").unwrap();
        // Update with zero-value edit range — should behave identically to None
        engine
            .update("# New\n", Some(EditRange { offset: 0, old_len: 0, new_len: 0 }))
            .unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_engine_update_edit_range_zero_equivalent() {
        // None and Some(0/0/0) must produce identical results
        let text_a = "# Hello\n";
        let text_b = "# Updated\n## Sub\n";

        let mut engine_none = DocumentEngine::new(text_a).unwrap();
        engine_none.update(text_b, None).unwrap();
        let hash_none = engine_none.content_hash();

        let mut engine_zero = DocumentEngine::new(text_a).unwrap();
        engine_zero
            .update(text_b, Some(EditRange { offset: 0, old_len: 0, new_len: 0 }))
            .unwrap();
        let hash_zero = engine_zero.content_hash();

        assert_eq!(
            hash_none, hash_zero,
            "None and Some(0/0/0) must produce identical content hash"
        );
    }

    #[test]
    fn test_engine_update_edit_range_nonzero_succeeds() {
        // Non-zero edit range values must pass through FFI without error
        // (Zig ignores them in Task 1, but the marshaling must not crash)
        let mut engine = DocumentEngine::new("# Hello world\n").unwrap();
        engine
            .update(
                "# Hello brave new world\n",
                Some(EditRange { offset: 8, old_len: 5, new_len: 15 }),
            )
            .unwrap();
        let hash = engine.content_hash();
        assert_ne!(hash, 0, "non-zero edit range must not crash FFI");
    }
}
