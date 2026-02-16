//! Content hashing (FNV-1a 64-bit).
//!
//! Wraps the Zig `marky_content_hash` FFI function for computing
//! deterministic content fingerprints.

extern "C" {
    fn marky_content_hash(text: *const u8, len: u32) -> u64;
}

/// FNV offset basis (hash of empty input).
pub const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;

/// Compute a deterministic FNV-1a 64-bit hash of the given text.
///
/// Returns [`FNV_OFFSET_BASIS`] for empty input (the standard FNV-1a
/// offset basis). Useful for content deduplication and change detection.
pub fn content_hash(text: &str) -> u64 {
    if text.is_empty() {
        return FNV_OFFSET_BASIS;
    }
    // SAFETY: text.as_ptr() is valid for text.len() bytes.
    // marky_content_hash is a pure function with no side effects.
    unsafe { marky_content_hash(text.as_ptr(), text.len() as u32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_empty() {
        assert_eq!(content_hash(""), FNV_OFFSET_BASIS);
    }

    #[test]
    fn test_content_hash_distinct() {
        let h1 = content_hash("abc");
        let h2 = content_hash("def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_known_vectors() {
        // FNV-1a known test vectors
        assert_eq!(content_hash("a"), 0xaf63dc4c8601ec8c);
        assert_eq!(content_hash("foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn test_content_hash_nonzero() {
        let h = content_hash("test content");
        assert_ne!(h, 0);
        assert_ne!(h, FNV_OFFSET_BASIS);
    }
}
