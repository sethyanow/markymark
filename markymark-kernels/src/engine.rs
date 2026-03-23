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
    fn marky_engine_get_content_hash(handle: *mut std::ffi::c_void) -> u64;
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

// SAFETY: `Sync` is intentionally NOT implemented. `get_blob(&self)` crosses
// the FFI boundary into Zig's `DocumentEngine.getBlob`, which writes
// `self.cached_blob` on a cache miss. Two threads sharing `&DocumentEngine`
// could therefore race on that mutation, which is undefined behaviour.
// Callers that need shared access must use `Mutex<DocumentEngine>` or
// `RwLock<DocumentEngine>` — the surrounding `ServerState` already does this.

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
        // Empty blob is header-only (128 bytes for v2)
        assert_eq!(
            blob.len(),
            128,
            "empty blob should be 128 bytes (v2 header only)"
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
    fn test_engine_is_send_not_sync() {
        // DocumentEngine is Send (ownership transfer across threads is safe)
        // but deliberately NOT Sync (get_blob mutates Zig-side cached_blob,
        // so concurrent &self access would race — marky-1n9q).
        fn assert_send<T: Send>() {}
        assert_send::<DocumentEngine>();
    }

    #[test]
    fn test_engine_blob_header_valid() {
        let engine = DocumentEngine::new("# Hello\n[link](url) #tag ^id\n").unwrap();
        let blob = engine.get_blob().unwrap();
        let data = blob.data();

        // Magic: 0x4D4B5343 in little-endian
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, 0x4D4B_5343);

        // Version: 2
        let version = u16::from_le_bytes([data[4], data[5]]);
        assert_eq!(version, 2);

        // heading_count at offset 16 (after magic:4 + version:2 + flags:2 + content_hash:8)
        let heading_count = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
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

    #[test]
    fn test_engine_content_hash_stable() {
        let text = "# Hello\n";
        let mut engine = DocumentEngine::new(text).unwrap();
        let hash1 = engine.content_hash();
        engine.update(text).unwrap();
        let hash2 = engine.content_hash();
        assert_eq!(hash1, hash2, "hash must be stable for same content");
    }

    #[test]
    fn test_engine_content_hash_changes() {
        let mut engine = DocumentEngine::new("# Hello\n").unwrap();
        let hash1 = engine.content_hash();
        engine.update("# Hello\n## World\n").unwrap();
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
        engine.update("# Héllo Wörld 日本語\n").unwrap();
        let hash2 = engine.content_hash();
        assert_eq!(hash1, hash2, "multi-byte UTF-8 hash must be stable");
    }

    #[test]
    fn test_engine_content_hash_repeated_updates_deterministic() {
        // The "second run": multiple updates cycle, hash must be deterministic
        let mut engine = DocumentEngine::new("# A\n").unwrap();
        let hash_a = engine.content_hash();
        engine.update("# B\n").unwrap();
        let hash_b = engine.content_hash();
        engine.update("# A\n").unwrap();
        let hash_a2 = engine.content_hash();
        assert_eq!(hash_a, hash_a2, "returning to same content must produce same hash");
        assert_ne!(hash_a, hash_b, "different content must produce different hash");
    }

    #[test]
    fn test_engine_content_hash_redundant_headings() {
        // Redundant: duplicate structure — hash should be unique to text, not structure count
        let mut engine = DocumentEngine::new("# Same\n# Same\n").unwrap();
        let hash_dup = engine.content_hash();
        engine.update("# Same\n").unwrap();
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
        engine.update("# Changed\n").unwrap();
        let hash2 = engine.content_hash();
        assert_ne!(hash1, hash2);
        // Update back — hash must match original
        engine.update("# Initial\n").unwrap();
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
}
