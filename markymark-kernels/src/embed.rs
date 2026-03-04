//! Embedding index operations via Zig SIMD kernels.
//!
//! Wraps the Zig `zig_embedding_index_*` FFI functions for creating,
//! populating, and searching embedding indices. The [`EmbeddingIndex`] struct
//! provides a safe Rust API with automatic cleanup via [`Drop`].
//!
//! # Thread Safety
//!
//! [`EmbeddingIndex`] implements `Send` and `Sync` via unsafe impls. The
//! underlying Zig heap allocation has no thread-local state, so ownership
//! transfer is safe. Concurrent reads (`search`, `count`) are safe; mutation
//! (`add`) requires `&mut self`. Wrap in `RwLock` for shared concurrent access.

use crate::scan::KernelError;

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn zig_embedding_index_create(dims: u32) -> *mut std::ffi::c_void;
    fn zig_embedding_index_destroy(handle: *mut std::ffi::c_void);
    fn zig_embedding_index_add(
        handle: *mut std::ffi::c_void,
        id: *const u8,
        id_len: u32,
        embedding: *const f32,
        dims: u32,
    ) -> i32;
    fn zig_embedding_index_search(
        handle: *mut std::ffi::c_void,
        query: *const f32,
        dims: u32,
        result_ids: *mut *const u8,
        result_id_lens: *mut u32,
        result_scores: *mut f32,
        k: u32,
        written: *mut u32,
    ) -> i32;
    fn zig_embedding_index_remove(
        handle: *mut std::ffi::c_void,
        id: *const u8,
        id_len: u32,
    ) -> i32;
    fn zig_embedding_index_count(handle: *mut std::ffi::c_void) -> i32;
}

// ---------------------------------------------------------------------------
// Search result type
// ---------------------------------------------------------------------------

/// A single search result from an embedding index query.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// The ID string of the matching entry.
    pub id: String,
    /// The cosine similarity score (higher = more similar).
    pub score: f32,
}

// ---------------------------------------------------------------------------
// EmbeddingIndex
// ---------------------------------------------------------------------------

/// An in-memory embedding index backed by Zig SIMD kernels.
///
/// Supports adding named embeddings and performing top-K cosine similarity
/// search. The index manages its own memory via an opaque Zig handle.
///
/// # Examples
///
/// ```no_run
/// use markymark_kernels::embed::EmbeddingIndex;
///
/// let mut idx = EmbeddingIndex::new(4).unwrap();
/// idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
/// idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
///
/// let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
/// assert_eq!(results[0].id, "doc1");
/// ```
pub struct EmbeddingIndex {
    handle: *mut std::ffi::c_void,
    dims: u32,
}

// SAFETY: The Zig embedding index is a self-contained heap allocation with no
// thread-local state. Ownership can be safely transferred between threads.
// All mutation goes through `&mut self` (enforced by Rust's borrow checker),
// and in practice an `RwLock` in the MCP runtime engine serialises access.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Send for EmbeddingIndex {}

// SAFETY: Shared access (`&self`) only calls `search` and `count`, which are
// read-only operations on the Zig side (no interior mutation). Concurrent
// readers are safe. The `RwLock` in RuntimeEngine ensures no reader overlaps
// with a writer.
//
// WARNING: This `Sync` impl is only safe under external read-write locking.
// If `EmbeddingIndex` is accessed without the `RwLock` guard held by the
// caller, data races are possible. Do not share this type across threads
// without external synchronization.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Sync for EmbeddingIndex {}

impl EmbeddingIndex {
    /// Create a new embedding index for vectors of the given dimensionality.
    ///
    /// Returns `Err(InvalidInput)` if `dims` is 0.
    pub fn new(dims: u32) -> Result<Self, KernelError> {
        if dims == 0 {
            return Err(KernelError::InvalidInput);
        }

        // SAFETY: zig_embedding_index_create is a pure allocation function.
        // Returns null on failure (dims==0 or allocation error).
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let handle = unsafe { zig_embedding_index_create(dims) };
        if handle.is_null() {
            return Err(KernelError::InternalError(-3));
        }

        Ok(Self { handle, dims })
    }

    /// Add an embedding to the index with the given ID.
    ///
    /// Both the ID and vector are copied into the Zig index. If an entry with
    /// the same ID already exists, its vector is replaced in-place.
    ///
    /// Returns `Err(InvalidInput)` if:
    /// - `id` is empty
    /// - `embedding` length does not match the index dimensionality
    pub fn add(&mut self, id: &str, embedding: &[f32]) -> Result<(), KernelError> {
        if id.is_empty() {
            return Err(KernelError::InvalidInput);
        }
        if embedding.len() != self.dims as usize {
            return Err(KernelError::InvalidInput);
        }
        let id_len = u32::try_from(id.len()).map_err(|_| KernelError::InvalidInput)?;

        // SAFETY: handle is valid (created in new(), not yet destroyed).
        // id and embedding are valid slices with correct lengths.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe {
            zig_embedding_index_add(
                self.handle,
                id.as_ptr(),
                id_len,
                embedding.as_ptr(),
                self.dims,
            )
        };

        match rc {
            0 => Ok(()),
            -1 => Err(KernelError::InvalidInput),
            -3 => Err(KernelError::InternalError(-3)),
            other => Err(KernelError::InternalError(other)),
        }
    }

    /// Remove an entry by ID, freeing its Zig-side allocations.
    ///
    /// Returns `true` if the entry was found and removed, `false` otherwise.
    pub fn remove(&mut self, id: &str) -> bool {
        if id.is_empty() {
            return false;
        }
        let Ok(id_len) = u32::try_from(id.len()) else {
            return false;
        };

        // SAFETY: handle is valid (created in new(), not yet destroyed).
        // id is a valid slice with correct length.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe { zig_embedding_index_remove(self.handle, id.as_ptr(), id_len) };
        rc == 0
    }

    /// Search the index for the top-K most similar embeddings to `query`.
    ///
    /// Results are sorted by descending cosine similarity score.
    ///
    /// Returns an empty vector if `k` is 0 or the index is empty.
    /// Returns `Err(InvalidInput)` if `query` length does not match dimensions.
    pub fn search(&self, query: &[f32], k: u32) -> Result<Vec<SearchResult>, KernelError> {
        if query.len() != self.dims as usize {
            return Err(KernelError::InvalidInput);
        }
        if k == 0 {
            return Ok(Vec::new());
        }

        let mut result_ids: Vec<*const u8> = vec![std::ptr::null(); k as usize];
        let mut result_id_lens: Vec<u32> = vec![0; k as usize];
        let mut result_scores: Vec<f32> = vec![0.0; k as usize];
        let mut written: u32 = 0;

        // SAFETY: handle is valid, query is a valid slice of correct length.
        // Output arrays are correctly sized to hold up to k results.
        // The returned ID pointers borrow from the index (valid until mutation).
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe {
            zig_embedding_index_search(
                self.handle,
                query.as_ptr(),
                self.dims,
                result_ids.as_mut_ptr(),
                result_id_lens.as_mut_ptr(),
                result_scores.as_mut_ptr(),
                k,
                &mut written,
            )
        };

        match rc {
            0 => {
                // Defensive: clamp written to k in case of FFI contract violation
                let written = (written).min(k) as usize;
                let results = (0..written)
                    .map(|i| {
                        // SAFETY: The Zig index returned valid pointers into its
                        // own storage. We copy them into owned Strings immediately.
                        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
                        let id_slice = unsafe {
                            std::slice::from_raw_parts(result_ids[i], result_id_lens[i] as usize)
                        };
                        let id = String::from_utf8_lossy(id_slice).into_owned();
                        SearchResult {
                            id,
                            score: result_scores[i],
                        }
                    })
                    .collect();
                Ok(results)
            }
            -1 => Err(KernelError::InvalidInput),
            -2 => {
                // k==0 case — we handle this above but the Zig layer also
                // returns -2 for k==0. Treat as empty result.
                Ok(Vec::new())
            }
            other => Err(KernelError::InternalError(other)),
        }
    }

    /// Return the number of entries in the index.
    pub fn count(&self) -> u32 {
        // SAFETY: handle is valid.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe { zig_embedding_index_count(self.handle) };
        if rc < 0 {
            0
        } else {
            rc as u32
        }
    }

    /// Return the dimensionality of vectors in this index.
    pub fn dimensions(&self) -> u32 {
        self.dims
    }
}

impl Drop for EmbeddingIndex {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was created by zig_embedding_index_create and
            // has not been destroyed yet. Setting to null prevents double-free.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
            unsafe { zig_embedding_index_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl std::fmt::Debug for EmbeddingIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingIndex")
            .field("dims", &self.dims)
            .field("count", &self.count())
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
    fn test_embedding_index_lifecycle() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(idx.count(), 2);
        assert_eq!(idx.dimensions(), 4);
        // Drop cleans up — no leak or crash
    }

    #[test]
    fn test_embedding_index_search_empty() {
        let idx = EmbeddingIndex::new(4).unwrap();
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_embedding_index_add_and_search() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        idx.add("doc3", &[0.707, 0.707, 0.0, 0.0]).unwrap();

        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 3).unwrap();
        assert_eq!(results.len(), 3);

        // doc1 should be the top match (identical vector)
        assert_eq!(results[0].id, "doc1");
        assert!((results[0].score - 1.0).abs() < 1e-5);

        // Results should be in descending score order
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_embedding_index_new_zero_dims() {
        assert!(matches!(
            EmbeddingIndex::new(0),
            Err(KernelError::InvalidInput)
        ));
    }

    #[test]
    fn test_embedding_index_add_empty_id() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        assert_eq!(
            idx.add("", &[1.0, 0.0, 0.0, 0.0]),
            Err(KernelError::InvalidInput)
        );
    }

    #[test]
    fn test_embedding_index_add_wrong_dims() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        assert_eq!(idx.add("doc1", &[1.0, 0.0]), Err(KernelError::InvalidInput));
    }

    #[test]
    fn test_embedding_index_search_wrong_dims() {
        let idx = EmbeddingIndex::new(4).unwrap();
        assert_eq!(idx.search(&[1.0, 0.0], 5), Err(KernelError::InvalidInput));
    }

    #[test]
    fn test_embedding_index_search_k_zero() {
        let idx = EmbeddingIndex::new(4).unwrap();
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_embedding_index_search_k_zero_wrong_dims() {
        // k==0 with wrong dims should still return InvalidInput (dims checked first)
        let idx = EmbeddingIndex::new(4).unwrap();
        assert_eq!(idx.search(&[1.0, 0.0], 0), Err(KernelError::InvalidInput));
    }

    #[test]
    fn test_embedding_index_search_k_greater_than_count() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();

        // Search for top-5 but only 1 entry exists
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_embedding_index_duplicate_id_replaces() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc1", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        assert_eq!(idx.count(), 1, "duplicate ID should replace, not add");

        // Search should return the updated vector
        let results = idx.search(&[0.0, 1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].id, "doc1");
        assert!((results[0].score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_embedding_index_100_entries() {
        let mut idx = EmbeddingIndex::new(8).unwrap();

        for i in 0..100u32 {
            let mut vec = [0.0f32; 8];
            vec[(i % 8) as usize] = 1.0;
            idx.add(&format!("e-{i:03}"), &vec).unwrap();
        }

        assert_eq!(idx.count(), 100);

        let mut query = [0.0f32; 8];
        query[0] = 1.0;
        let results = idx.search(&query, 5).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 5);

        // Verify descending score order
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_embedding_index_remove_existing() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(idx.count(), 2);

        assert!(idx.remove("doc1"));
        assert_eq!(idx.count(), 1);

        // Only doc2 should remain in search results
        let results = idx.search(&[0.0, 1.0, 0.0, 0.0], 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc2");
    }

    #[test]
    fn test_embedding_index_remove_nonexistent() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        assert!(!idx.remove("nope"));
        assert_eq!(idx.count(), 1);
    }

    #[test]
    fn test_embedding_index_remove_empty_id() {
        let mut idx = EmbeddingIndex::new(4).unwrap();
        assert!(!idx.remove(""));
    }

    #[test]
    fn test_embedding_index_debug() {
        let idx = EmbeddingIndex::new(4).unwrap();
        let debug = format!("{:?}", idx);
        assert!(debug.contains("EmbeddingIndex"));
        assert!(debug.contains("dims: 4"));
    }

    /// Regression test for marky-36a: EmbeddingIndex must be Send + Sync so it
    /// can live inside RwLock<HashMap<..., RealmData>> in the MCP runtime engine.
    #[test]
    fn test_embedding_index_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<EmbeddingIndex>();
        assert_sync::<EmbeddingIndex>();
    }
}
