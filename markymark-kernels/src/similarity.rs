//! Similarity computations via Zig SIMD kernels.
//!
//! Wraps the Zig `zig_cosine_similarity` and `zig_jaccard_similarity` FFI
//! functions exported from `c_adapter.zig`.

use crate::scan::KernelError;

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn zig_cosine_similarity(a: *const f32, b: *const f32, dims: u32) -> f32;
    fn zig_jaccard_similarity(
        set1: *const u32,
        set1_len: u32,
        set2: *const u32,
        set2_len: u32,
    ) -> f32;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute the cosine similarity between two f32 vectors.
///
/// Both vectors must have the same length (`dims`). Returns a value in
/// `[-1.0, 1.0]` where 1.0 means identical direction.
///
/// Returns `Err(InvalidInput)` if either slice is empty or lengths differ.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, KernelError> {
    if a.is_empty() || b.is_empty() {
        return Err(KernelError::InvalidInput);
    }
    if a.len() != b.len() {
        return Err(KernelError::InvalidInput);
    }

    let dims = u32::try_from(a.len()).map_err(|_| KernelError::InvalidInput)?;

    // SAFETY: a and b are valid slices with matching lengths.
    // zig_cosine_similarity returns -2.0 on null pointer (cannot happen here)
    // or on dims==0 (checked above).
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let result = unsafe { zig_cosine_similarity(a.as_ptr(), b.as_ptr(), dims) };

    // The Zig function returns -2.0 as a sentinel for invalid input.
    if result == -2.0 {
        return Err(KernelError::InvalidInput);
    }

    Ok(result)
}

/// Compute the Jaccard similarity between two sorted u32 sets.
///
/// Both sets must be sorted in ascending order. Returns a value in `[0.0, 1.0]`
/// where 1.0 means identical sets.
///
/// Returns `Ok(0.0)` if both sets are empty (vacuously similar).
/// Returns `Ok(0.0)` if one set is empty and the other is not.
pub fn jaccard_similarity(set1: &[u32], set2: &[u32]) -> Result<f32, KernelError> {
    // Both empty → 0.0 (Zig returns 0.0 for empty sets)
    if set1.is_empty() && set2.is_empty() {
        return Ok(0.0);
    }

    let set1_len = u32::try_from(set1.len()).map_err(|_| KernelError::InvalidInput)?;
    let set2_len = u32::try_from(set2.len()).map_err(|_| KernelError::InvalidInput)?;

    // SAFETY: set1 and set2 are valid slices. zig_jaccard_similarity returns
    // -1.0 on null pointer (cannot happen with valid slice pointers).
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let result =
        unsafe { zig_jaccard_similarity(set1.as_ptr(), set1_len, set2.as_ptr(), set2_len) };

    // The Zig function returns -1.0 as a sentinel for null pointers.
    if result < 0.0 {
        return Err(KernelError::InvalidInput);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- cosine_similarity tests --

    #[test]
    fn test_cosine_identical_vectors() {
        let v = vec![1.0, 0.0, 0.0];
        let result = cosine_similarity(&v, &v).unwrap();
        assert!(
            (result - 1.0).abs() < 1e-5,
            "identical vectors should be 1.0"
        );
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let result = cosine_similarity(&a, &b).unwrap();
        assert!(result.abs() < 1e-5, "orthogonal vectors should be ~0.0");
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let result = cosine_similarity(&a, &b).unwrap();
        assert!(
            (result + 1.0).abs() < 1e-5,
            "opposite vectors should be -1.0"
        );
    }

    #[test]
    fn test_cosine_known_vectors() {
        // [3, 4] · [4, 3] / (5 * 5) = 24/25 = 0.96
        let a = vec![3.0, 4.0];
        let b = vec![4.0, 3.0];
        let result = cosine_similarity(&a, &b).unwrap();
        assert!((result - 0.96).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_empty_vectors() {
        let empty: Vec<f32> = vec![];
        assert_eq!(
            cosine_similarity(&empty, &empty),
            Err(KernelError::InvalidInput)
        );
    }

    #[test]
    fn test_cosine_length_mismatch() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), Err(KernelError::InvalidInput));
    }

    // -- jaccard_similarity tests --

    #[test]
    fn test_jaccard_identical_sets() {
        let set = vec![1, 2, 3, 4, 5];
        let result = jaccard_similarity(&set, &set).unwrap();
        assert!((result - 1.0).abs() < 1e-5, "identical sets should be 1.0");
    }

    #[test]
    fn test_jaccard_disjoint_sets() {
        let a = vec![1, 2, 3];
        let b = vec![4, 5, 6];
        let result = jaccard_similarity(&a, &b).unwrap();
        assert!(result.abs() < 1e-5, "disjoint sets should be 0.0");
    }

    #[test]
    fn test_jaccard_partial_overlap() {
        let a = vec![1, 2, 3, 4];
        let b = vec![3, 4, 5, 6];
        let result = jaccard_similarity(&a, &b).unwrap();
        // intersection = {3,4} = 2, union = {1,2,3,4,5,6} = 6, J = 2/6 ≈ 0.333
        assert!((result - 1.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_jaccard_both_empty() {
        let empty: Vec<u32> = vec![];
        let result = jaccard_similarity(&empty, &empty).unwrap();
        assert!((result - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_jaccard_one_empty() {
        let a = vec![1, 2, 3];
        let empty: Vec<u32> = vec![];
        let result = jaccard_similarity(&a, &empty).unwrap();
        assert!((result - 0.0).abs() < 1e-5);
    }
}
