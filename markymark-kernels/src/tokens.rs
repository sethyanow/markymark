//! Token estimation and content hashing via SIMD kernels.
//!
//! Wraps the Zig `marky_estimate_tokens` and `marky_content_hash` FFI functions.

extern "C" {
    fn marky_estimate_tokens(text: *const u8, len: u32) -> u32;
    fn marky_content_hash(text: *const u8, len: u32) -> u64;
}

/// Estimate the approximate BPE token count for the given text.
///
/// Uses SIMD word-boundary detection with a 1.3x multiplier to approximate
/// tokenizer output. Returns 0 for empty input.
///
/// This is a fast heuristic, not an exact tokenizer. Useful for budget
/// estimation and size-gating before calling a real tokenizer.
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    // SAFETY: text.as_ptr() is valid for text.len() bytes.
    // marky_estimate_tokens is a pure function with no side effects.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    unsafe { marky_estimate_tokens(text.as_ptr(), len) }
}

/// Compute a deterministic FNV-1a 64-bit hash of the given text.
///
/// Returns the FNV-1a offset basis (`0xcbf29ce484222325`) for empty input.
/// Useful for content-addressable deduplication and change detection.
pub fn content_hash(text: &str) -> u64 {
    let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    // SAFETY: text.as_ptr() is valid for text.len() bytes.
    // marky_content_hash is a pure function with no side effects.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    unsafe { marky_content_hash(text.as_ptr(), len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_basic() {
        let count = estimate_tokens("hello world foo bar");
        // 4 words * 1.3 = 5.2 -> (4*13+5)/10 = 57/10 = 5
        assert_eq!(count, 5);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        let count = estimate_tokens("hello");
        // 1 word * 1.3 = 1.3 -> (1*13+5)/10 = 18/10 = 1
        assert_eq!(count, 1);
    }

    #[test]
    fn test_estimate_tokens_multiline() {
        let text = "line one\nline two\nline three\n";
        let count = estimate_tokens(text);
        // 6 words * 1.3 = 7.8 -> (6*13+5)/10 = 83/10 = 8
        assert!(count > 0);
    }

    // -- content_hash tests --

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2, "same input should produce same hash");
    }

    #[test]
    fn test_content_hash_distinct() {
        let h1 = content_hash("abc");
        let h2 = content_hash("def");
        assert_ne!(h1, h2, "different inputs should produce different hashes");
    }

    #[test]
    fn test_content_hash_empty() {
        let h = content_hash("");
        // FNV-1a offset basis
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn test_content_hash_known_vectors() {
        // FNV-1a 64-bit known test vectors
        assert_eq!(content_hash("a"), 0xaf63dc4c8601ec8c);
        assert_eq!(content_hash("foobar"), 0x85944171f73967e8);
    }
}
