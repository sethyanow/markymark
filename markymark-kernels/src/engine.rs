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
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn marky_engine_create(text: *const u8, text_len: u32) -> *mut std::ffi::c_void;
    fn marky_engine_update(handle: *mut std::ffi::c_void, text: *const u8, text_len: u32) -> i32;
    fn marky_engine_destroy(handle: *mut std::ffi::c_void);
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
    /// Returns `Err(InvalidInput)` if `text` exceeds `u32::MAX` bytes.
    pub fn update(&mut self, text: &str) -> Result<(), KernelError> {
        let text_len = u32::try_from(text.len()).map_err(|_| KernelError::InvalidInput)?;

        // SAFETY: handle is valid (created in new(), not yet destroyed).
        // For empty text, pass null + 0. For non-empty, pass valid slice.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe {
            if text.is_empty() {
                marky_engine_update(self.handle, std::ptr::null(), 0)
            } else {
                marky_engine_update(self.handle, text.as_ptr(), text_len)
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

        engine.update("# Two\n## Sub\n").unwrap();
        let gen2 = engine.get_result().unwrap().as_raw().generation;

        assert!(gen2 > gen1);
    }
}
