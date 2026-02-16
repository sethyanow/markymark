//! Token estimation via SIMD word-boundary detection.
//!
//! Wraps the Zig `marky_estimate_tokens` FFI function which uses SIMD
//! to count word boundaries and applies a 1.3x BPE multiplier.

extern "C" {
    fn marky_estimate_tokens(text: *const u8, len: u32) -> u32;
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
    // SAFETY: text.as_ptr() is valid for text.len() bytes.
    // marky_estimate_tokens is a pure function with no side effects.
    unsafe { marky_estimate_tokens(text.as_ptr(), text.len() as u32) }
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
}
