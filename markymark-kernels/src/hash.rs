//! Entity hash extraction (FNV-1a word hashing).
//!
//! Wraps the Zig `zig_extract_entity_hashes` FFI function which tokenizes
//! text on whitespace/punctuation and produces an FNV-1a u32 hash per word.

use crate::scan::KernelError;

/// Maximum number of buffer-grow attempts (initial + retries).
const MAX_ATTEMPTS: u32 = 4;

/// Initial buffer capacity for entity hash results.
const INITIAL_CAP: usize = 64;

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn zig_extract_entity_hashes(
        text_ptr: *const u8,
        text_len: u32,
        output_ids: *mut u32,
        capacity: u32,
        written: *mut u32,
    ) -> i32;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract entity hashes from text using FNV-1a word hashing.
///
/// Tokenizes text on whitespace and punctuation boundaries, then hashes each
/// word with FNV-1a to produce a truncated u32 hash per token. Useful for
/// entity-based similarity comparisons (e.g. Jaccard on hash sets).
///
/// Returns an empty vector for empty input (not an error).
pub fn extract_entity_hashes(text: &str) -> Result<Vec<u32>, KernelError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let mut buf: Vec<u32> = vec![0; INITIAL_CAP];
    let text_bytes = text.as_bytes();
    let text_ptr = text_bytes.as_ptr();
    let text_len = u32::try_from(text_bytes.len()).map_err(|_| KernelError::InvalidInput)?;

    for _ in 0..MAX_ATTEMPTS {
        let cap = u32::try_from(buf.len()).map_err(|_| KernelError::InvalidInput)?;
        let mut written: u32 = 0;

        // SAFETY: text_ptr valid for text_len bytes, buf has capacity cap,
        // written is a valid mutable reference. FFI writes at most cap u32s.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe {
            zig_extract_entity_hashes(text_ptr, text_len, buf.as_mut_ptr(), cap, &mut written)
        };

        match rc {
            0 => {
                // Defensive: clamp written to capacity in case of FFI contract violation
                let n = (written).min(cap) as usize;
                buf.truncate(n);
                return Ok(buf);
            }
            -1 => return Err(KernelError::InvalidInput),
            -2 => {
                // Double capacity and retry
                let new_cap = (buf.len() * 2).max(INITIAL_CAP);
                buf.resize(new_cap, 0);
            }
            other => return Err(KernelError::InternalError(other)),
        }
    }

    Err(KernelError::BufferTooSmall)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_hashes_known_text() {
        let hashes = extract_entity_hashes("hello world").unwrap();
        assert_eq!(hashes.len(), 2, "two words should produce two hashes");
        // Hashes should be non-zero
        assert_ne!(hashes[0], 0);
        assert_ne!(hashes[1], 0);
        // Different words should produce different hashes
        assert_ne!(hashes[0], hashes[1]);
    }

    #[test]
    fn test_entity_hashes_empty_text() {
        let hashes = extract_entity_hashes("").unwrap();
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_entity_hashes_single_word() {
        let hashes = extract_entity_hashes("rust").unwrap();
        assert_eq!(hashes.len(), 1);
        assert_ne!(hashes[0], 0);
    }

    #[test]
    fn test_entity_hashes_deterministic() {
        let h1 = extract_entity_hashes("hello world").unwrap();
        let h2 = extract_entity_hashes("hello world").unwrap();
        assert_eq!(h1, h2, "same input should produce same hashes");
    }

    #[test]
    fn test_entity_hashes_punctuation_splits() {
        // Punctuation should act as word separator
        let hashes = extract_entity_hashes("hello,world").unwrap();
        assert_eq!(hashes.len(), 2, "comma should split into two words");
    }

    #[test]
    fn test_entity_hashes_multiline() {
        let hashes = extract_entity_hashes("line one\nline two\nline three").unwrap();
        // Should have 6 word hashes
        assert_eq!(hashes.len(), 6);
    }

    #[test]
    fn test_entity_hashes_whitespace_only() {
        let hashes = extract_entity_hashes("   \t\n  ").unwrap();
        assert!(
            hashes.is_empty(),
            "whitespace-only should produce no hashes"
        );
    }

    #[test]
    fn test_entity_hashes_large_text() {
        // Generate text with more words than INITIAL_CAP to test buffer retry
        let words: Vec<&str> = (0..200).map(|_| "word").collect();
        let text = words.join(" ");
        let hashes = extract_entity_hashes(&text).unwrap();
        assert_eq!(hashes.len(), 200);
    }
}
