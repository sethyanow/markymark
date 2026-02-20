//! DocumentEngine FFI bindings.
//!
//! Wraps the Zig `marky_engine_*` C ABI functions to provide safe Rust
//! access to the stateful document engine. The engine parses markdown,
//! extracts headings/links/tags/block-ids, and serializes state to a
//! flat binary blob for zero-copy transfer.
//!
//! Created for marky-atsp (epic marky-io3h, Task 2).

use crate::scan::KernelError;

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn marky_engine_create(text: *const u8, text_len: u32) -> *mut std::ffi::c_void;
    fn marky_engine_update(handle: *mut std::ffi::c_void, text: *const u8, text_len: u32) -> i32;
    fn marky_engine_get_blob(
        handle: *mut std::ffi::c_void,
        blob_ptr: *mut *const u8,
        blob_len: *mut u32,
    ) -> i32;
    fn marky_engine_destroy(handle: *mut std::ffi::c_void);
}

// ---------------------------------------------------------------------------
// ScanBlob — thin view over serialized engine state
// ---------------------------------------------------------------------------

/// A borrowed view of the serialized blob from a [`DocumentEngine`].
///
/// The blob data is owned by the engine and valid until the next
/// [`DocumentEngine::update`] or drop. The lifetime `'a` ties this
/// to the engine borrow, so the borrow checker prevents use-after-update.
#[derive(Debug)]
pub struct ScanBlob<'a> {
    data: &'a [u8],
}

impl<'a> ScanBlob<'a> {
    /// Raw blob bytes.
    pub fn data(&self) -> &[u8] {
        self.data
    }

    /// Blob size in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the blob is empty (should never be — minimum is 64-byte header).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ---------------------------------------------------------------------------
// DocumentEngine
// ---------------------------------------------------------------------------

/// A stateful document engine backed by Zig SIMD kernels.
///
/// Parses markdown text, extracts headings, links, tags, and block IDs,
/// and serializes the result to a flat binary blob. The engine caches
/// the blob until the next update.
///
/// # Thread Safety
///
/// [`DocumentEngine`] implements `Send` and `Sync` via unsafe impls. The
/// underlying Zig heap allocation has no thread-local state, so ownership
/// transfer is safe. Concurrent reads (`get_blob`) are safe; mutation
/// (`update`) requires `&mut self`. Wrap in `RwLock` for shared concurrent access.
pub struct DocumentEngine {
    handle: *mut std::ffi::c_void,
}

// SAFETY: The Zig engine is a self-contained heap allocation with no
// thread-local state. Ownership can be safely transferred between threads.
// All mutation goes through `&mut self` (enforced by Rust's borrow checker),
// and in practice an `RwLock` in the LSP/MCP runtime serialises access.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Send for DocumentEngine {}

// SAFETY: Shared access (`&self`) only calls `get_blob`, which reads cached
// state (no interior mutation on cache hit). The `RwLock` in the runtime
// ensures no reader overlaps with a writer.
//
// WARNING: This `Sync` impl is only safe under external read-write locking.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Sync for DocumentEngine {}

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
            return Err(KernelError::InternalError(-3));
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

    /// Get the serialized blob for the current engine state.
    ///
    /// The blob is lazily computed on first call and cached until the next
    /// [`update`](Self::update). The returned [`ScanBlob`] borrows `&self`,
    /// so the borrow checker prevents calling `update` while a blob reference
    /// is held.
    pub fn get_blob(&self) -> Result<ScanBlob<'_>, KernelError> {
        let mut blob_ptr: *const u8 = std::ptr::null();
        let mut blob_len: u32 = 0;

        // SAFETY: handle is valid, blob_ptr and blob_len are stack-local.
        // On success, blob_ptr points into engine-owned memory valid until
        // the next update() or destroy(). The ScanBlob lifetime is tied to
        // &self, preventing use-after-update.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe { marky_engine_get_blob(self.handle, &mut blob_ptr, &mut blob_len) };

        match rc {
            0 => {
                // SAFETY: blob_ptr is valid for blob_len bytes, owned by engine.
                // We create a slice with lifetime tied to &self.
                // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
                let data = unsafe { std::slice::from_raw_parts(blob_ptr, blob_len as usize) };
                Ok(ScanBlob { data })
            }
            -1 => Err(KernelError::InvalidInput),
            -3 => Err(KernelError::InternalError(-3)),
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
    fn test_engine_lifecycle() {
        let mut engine = DocumentEngine::new("# Hello\n").unwrap();
        engine.update("# World\n").unwrap();

        let blob = engine.get_blob().unwrap();
        let data = blob.data();
        assert!(data.len() >= 64, "blob must include header");

        // Validate magic: 0x4D4B5343 ("MKSC") in little-endian
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, 0x4D4B_5343, "blob magic mismatch");
        // Drop cleans up
    }

    #[test]
    fn test_engine_empty_input() {
        let engine = DocumentEngine::new("").unwrap();
        let blob = engine.get_blob().unwrap();
        // Empty blob is header-only (64 bytes)
        assert_eq!(
            blob.len(),
            64,
            "empty blob should be 64 bytes (header only)"
        );
    }

    #[test]
    fn test_engine_update_changes_blob() {
        let mut engine = DocumentEngine::new("# A\n").unwrap();
        let blob1_len = engine.get_blob().unwrap().len();

        engine.update("# B\n## C\n## D\n").unwrap();
        let blob2_len = engine.get_blob().unwrap().len();

        assert_ne!(
            blob1_len, blob2_len,
            "blob should change after update with different content"
        );
    }

    #[test]
    fn test_engine_multiple_updates() {
        let mut engine = DocumentEngine::new("# Init\n").unwrap();
        for i in 0..100 {
            engine
                .update(&format!("# Heading {i}\n[link](url) #tag\n"))
                .unwrap();
        }
        // No crash, no leak — Drop cleans up
    }

    #[test]
    fn test_engine_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DocumentEngine>();
        assert_sync::<DocumentEngine>();
    }

    #[test]
    fn test_engine_blob_header_valid() {
        let engine = DocumentEngine::new("# Hello\n[link](url) #tag ^id\n").unwrap();
        let blob = engine.get_blob().unwrap();
        let data = blob.data();

        // Magic: 0x4D4B5343 in little-endian
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, 0x4D4B_5343);

        // Version: 1
        let version = u16::from_le_bytes([data[4], data[5]]);
        assert_eq!(version, 1);

        // heading_count at offset 12 (after magic:4 + version:2 + flags:2 + content_hash:8)
        let heading_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        assert!(
            heading_count >= 1,
            "should have at least 1 heading, got {heading_count}"
        );
    }

    #[test]
    fn test_engine_blob_caching() {
        let engine = DocumentEngine::new("# Test\n").unwrap();
        let blob1 = engine.get_blob().unwrap();
        let blob2 = engine.get_blob().unwrap();

        // Same cached blob — data should be identical
        assert_eq!(blob1.data(), blob2.data());
        assert_eq!(blob1.len(), blob2.len());
    }

    #[test]
    fn test_engine_debug_format() {
        let engine = DocumentEngine::new("# Debug\n").unwrap();
        let debug = format!("{engine:?}");
        assert!(debug.contains("DocumentEngine"));
        assert!(debug.contains("handle_null: false"));
    }
}
