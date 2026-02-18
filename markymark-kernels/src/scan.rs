//! SIMD-accelerated markdown element scanning.
//!
//! This module wraps the Zig `marky_scan_*` FFI functions for
//! heading, link, tag, and block-ID extraction. Each function allocates
//! an output buffer, calls the Zig kernel, and retries with a larger
//! buffer if the initial capacity is insufficient.

use std::fmt;

/// Maximum number of retry attempts when the output buffer is too small.
const MAX_RETRIES: u32 = 3;

/// Initial buffer capacity for scan results.
const INITIAL_CAP: usize = 64;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by kernel scan functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// FFI returned -1: null pointer or invalid argument.
    InvalidInput,
    /// FFI returned -2 after all retry attempts: output buffer still too small.
    BufferTooSmall,
    /// FFI returned an unexpected negative code.
    InternalError(i32),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "kernel: invalid input (null pointer)"),
            Self::BufferTooSmall => write!(f, "kernel: buffer too small after retries"),
            Self::InternalError(code) => write!(f, "kernel: internal error (code {code})"),
        }
    }
}

impl std::error::Error for KernelError {}

// ---------------------------------------------------------------------------
// C ABI mirror types (repr(C) to match Zig extern structs)
// ---------------------------------------------------------------------------

/// C ABI mirror of Zig `HeadingScan`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CHeadingScan {
    offset: u32,
    length: u16,
    level: u8,
    _padding: u8,
}

/// C ABI mirror of Zig `LinkScan`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CLinkScan {
    offset: u32,
    text_offset: u32,
    text_length: u16,
    target_offset: u32,
    target_length: u16,
    link_type: u8,
    _padding: u8,
}

/// C ABI mirror of Zig `TagScan`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CTagScan {
    offset: u32,
    length: u16,
    _padding: [u8; 2],
}

/// C ABI mirror of Zig `BlockIdScan`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CBlockIdScan {
    offset: u32,
    length: u16,
    _padding: [u8; 2],
}

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn marky_scan_headings(
        text: *const u8,
        len: u32,
        out: *mut CHeadingScan,
        cap: u32,
        written: *mut u32,
    ) -> i32;

    fn marky_scan_links(
        text: *const u8,
        len: u32,
        out: *mut CLinkScan,
        cap: u32,
        written: *mut u32,
    ) -> i32;

    fn marky_scan_tags(
        text: *const u8,
        len: u32,
        out: *mut CTagScan,
        cap: u32,
        written: *mut u32,
    ) -> i32;

    fn marky_scan_block_ids(
        text: *const u8,
        len: u32,
        out: *mut CBlockIdScan,
        cap: u32,
        written: *mut u32,
    ) -> i32;

    fn marky_fuzzy_match(
        query: *const u8,
        query_len: u32,
        candidate: *const u8,
        candidate_len: u32,
    ) -> i32;

    fn marky_fuzzy_match_batch(
        query: *const u8,
        query_len: u32,
        candidate_ptrs: *const *const u8,
        candidate_lens: *const u32,
        candidate_count: u32,
        scores_out: *mut i32,
        indices_out: *mut u32,
        output_cap: u32,
        top_k: u32,
        written: *mut u32,
    ) -> i32;
}

// ---------------------------------------------------------------------------
// Rust result types
// ---------------------------------------------------------------------------

/// A heading found by the SIMD scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingScan {
    /// Heading text extracted from the source.
    pub text: String,
    /// Byte offset of the heading text start in the source.
    pub offset: u32,
    /// Heading level (1–6).
    pub level: u8,
}

/// Link type discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// Standard markdown link `[text](url)`.
    Markdown = 0,
    /// Wiki-style link `[[target]]` or `[[target|display]]`.
    Wiki = 1,
}

/// A link found by the SIMD scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkScan {
    /// Byte offset of the link start in the source.
    pub offset: u32,
    /// Display text of the link.
    pub text: String,
    /// Link target / URL.
    pub target: String,
    /// Whether this is a markdown or wiki link.
    pub link_type: LinkType,
}

/// A tag found by the SIMD scanner (e.g. `#topic`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagScan {
    /// Tag name without the leading `#`.
    pub name: String,
    /// Byte offset of the `#` in the source.
    pub offset: u32,
}

/// A block ID found by the SIMD scanner (e.g. `^block-id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIdScan {
    /// Block ID without the leading `^`.
    pub id: String,
    /// Byte offset of the `^` in the source.
    pub offset: u32,
}

/// Result of fuzzy matching a query against a candidate symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Integer score where 0 means no match and higher is better.
    pub score: i32,
    /// True when the match begins at candidate position 0.
    pub starts_with: bool,
}

/// Ranked result from batched fuzzy matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyBatchMatch {
    /// Original candidate index in the provided input slice.
    pub index: u32,
    /// Integer score where 0 means no match and higher is better.
    pub score: i32,
    /// True when the match begins at candidate position 0.
    pub starts_with: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Safely extract a `&str` slice from `source` at the given byte range.
/// Falls back to the nearest valid UTF-8 char boundary if the offset
/// lands inside a multi-byte character.
fn safe_slice(source: &str, offset: u32, length: u32) -> &str {
    let start = offset as usize;
    let end = start + length as usize;
    let bytes = source.as_bytes();

    if start >= bytes.len() {
        return "";
    }
    let end = end.min(bytes.len());

    // Round start forward and end backward to char boundaries.
    let start = if source.is_char_boundary(start) {
        start
    } else {
        // Walk forward to next boundary (max 3 bytes for UTF-8).
        (start..end)
            .find(|&i| source.is_char_boundary(i))
            .unwrap_or(end)
    };
    let end = if source.is_char_boundary(end) {
        end
    } else {
        (start..end)
            .rev()
            .find(|&i| source.is_char_boundary(i))
            .unwrap_or(start)
    };

    &source[start..end]
}

fn starts_with_ascii_case_insensitive(query: &str, candidate: &str) -> bool {
    if candidate.chars().count() < query.chars().count() {
        return false;
    }

    query
        .chars()
        .zip(candidate.chars())
        .all(|(q, c)| q.eq_ignore_ascii_case(&c))
}

/// Call an FFI scan function with exponential buffer retry.
///
/// `ffi_fn` is called with (text_ptr, text_len, out_ptr, cap, written_ptr) -> i32.
/// On success (0), returns the written count. On -2, doubles the buffer and retries.
///
/// # Safety
/// The caller must pass a valid FFI function pointer that matches the C ABI.
unsafe fn call_scan_ffi<T: Copy>(
    text: &[u8],
    buf: &mut Vec<T>,
    ffi_fn: unsafe extern "C" fn(*const u8, u32, *mut T, u32, *mut u32) -> i32,
) -> Result<u32, KernelError> {
    let text_ptr = text.as_ptr();
    let text_len = text.len() as u32;

    for _ in 0..=MAX_RETRIES {
        let cap = buf.len() as u32;
        let mut written: u32 = 0;

        // SAFETY: buf has capacity `cap`, text_ptr valid for text_len bytes,
        // written is a valid mutable reference. FFI function writes at most
        // `cap` elements to buf.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let rc = unsafe { ffi_fn(text_ptr, text_len, buf.as_mut_ptr(), cap, &mut written) };

        match rc {
            0 => return Ok(written),
            -1 => return Err(KernelError::InvalidInput),
            -2 => {
                // Double capacity and retry
                let new_cap = (buf.len() * 2).max(INITIAL_CAP);
                // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
                buf.resize(new_cap, unsafe { std::mem::zeroed() });
            }
            other => return Err(KernelError::InternalError(other)),
        }
    }

    Err(KernelError::BufferTooSmall)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan text for ATX headings using SIMD-accelerated detection.
///
/// Returns a vector of [`HeadingScan`] results with heading text, offset, and level.
/// Empty input returns an empty vector (not an error).
pub fn scan_headings(text: &str) -> Result<Vec<HeadingScan>, KernelError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let mut buf: Vec<CHeadingScan> = vec![unsafe { std::mem::zeroed() }; INITIAL_CAP];
    // SAFETY: marky_scan_headings matches the expected C ABI signature.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let written = unsafe { call_scan_ffi(text.as_bytes(), &mut buf, marky_scan_headings) }?;

    let results = buf[..written as usize]
        .iter()
        .map(|c| HeadingScan {
            text: safe_slice(text, c.offset, c.length as u32).to_owned(),
            offset: c.offset,
            level: c.level,
        })
        .collect();

    Ok(results)
}

/// Scan text for markdown and wiki links using SIMD-accelerated detection.
///
/// Returns a vector of [`LinkScan`] results with display text, target, offset,
/// and link type. Empty input returns an empty vector (not an error).
pub fn scan_links(text: &str) -> Result<Vec<LinkScan>, KernelError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let mut buf: Vec<CLinkScan> = vec![unsafe { std::mem::zeroed() }; INITIAL_CAP];
    // SAFETY: marky_scan_links matches the expected C ABI signature.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let written = unsafe { call_scan_ffi(text.as_bytes(), &mut buf, marky_scan_links) }?;

    let results = buf[..written as usize]
        .iter()
        .map(|c| LinkScan {
            offset: c.offset,
            text: safe_slice(text, c.text_offset, c.text_length as u32).to_owned(),
            target: safe_slice(text, c.target_offset, c.target_length as u32).to_owned(),
            link_type: if c.link_type == 1 {
                LinkType::Wiki
            } else {
                LinkType::Markdown
            },
        })
        .collect();

    Ok(results)
}

/// Scan text for `#tag` patterns using SIMD-accelerated detection.
///
/// Returns a vector of [`TagScan`] results with tag name (without `#`) and offset.
/// Empty input returns an empty vector (not an error).
pub fn scan_tags(text: &str) -> Result<Vec<TagScan>, KernelError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let mut buf: Vec<CTagScan> = vec![unsafe { std::mem::zeroed() }; INITIAL_CAP];
    // SAFETY: marky_scan_tags matches the expected C ABI signature.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let written = unsafe { call_scan_ffi(text.as_bytes(), &mut buf, marky_scan_tags) }?;

    let results = buf[..written as usize]
        .iter()
        .map(|c| {
            // Tag name starts after the '#', so offset+1, length as-is
            TagScan {
                name: safe_slice(text, c.offset + 1, c.length as u32).to_owned(),
                offset: c.offset,
            }
        })
        .collect();

    Ok(results)
}

/// Scan text for `^block-id` patterns using SIMD-accelerated detection.
///
/// Returns a vector of [`BlockIdScan`] results with block ID (without `^`) and offset.
/// Empty input returns an empty vector (not an error).
pub fn scan_block_ids(text: &str) -> Result<Vec<BlockIdScan>, KernelError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let mut buf: Vec<CBlockIdScan> = vec![unsafe { std::mem::zeroed() }; INITIAL_CAP];
    // SAFETY: marky_scan_block_ids matches the expected C ABI signature.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let written = unsafe { call_scan_ffi(text.as_bytes(), &mut buf, marky_scan_block_ids) }?;

    let results = buf[..written as usize]
        .iter()
        .map(|c| {
            // Block ID starts after the '^', so offset+1, length as-is
            BlockIdScan {
                id: safe_slice(text, c.offset + 1, c.length as u32).to_owned(),
                offset: c.offset,
            }
        })
        .collect();

    Ok(results)
}

/// Fuzzy-match a query against a candidate symbol string.
///
/// Returns a score where 0 means no match and higher means a stronger match.
/// Prefix matches are marked via `starts_with` for caller-side tie-breaking.
pub fn fuzzy_match(query: &str, candidate: &str) -> Result<FuzzyMatch, KernelError> {
    if query.is_empty() || candidate.is_empty() {
        return Ok(FuzzyMatch {
            score: 0,
            starts_with: false,
        });
    }

    // SAFETY: Pointers are valid for their respective byte lengths for this call.
    // FFI function does not mutate input buffers.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let score = unsafe {
        marky_fuzzy_match(
            query.as_ptr(),
            query.len() as u32,
            candidate.as_ptr(),
            candidate.len() as u32,
        )
    };

    if score < 0 {
        return Err(KernelError::InvalidInput);
    }

    let starts_with = score > 0 && starts_with_ascii_case_insensitive(query, candidate);

    Ok(FuzzyMatch { score, starts_with })
}

/// Batched fuzzy-match ranking with deterministic top-k ordering.
///
/// Returns up to `top_k` matches sorted by:
/// 1. score descending
/// 2. candidate index ascending
pub fn fuzzy_match_batch(
    query: &str,
    candidates: &[&str],
    top_k: usize,
) -> Result<Vec<FuzzyBatchMatch>, KernelError> {
    if query.is_empty() || candidates.is_empty() || top_k == 0 {
        return Ok(Vec::new());
    }

    let candidate_count = u32::try_from(candidates.len()).map_err(|_| KernelError::InvalidInput)?;
    let output_cap = top_k.min(candidates.len());
    let output_cap_u32 = u32::try_from(output_cap).map_err(|_| KernelError::InvalidInput)?;
    let top_k_u32 = output_cap_u32;

    let mut candidate_ptrs: Vec<*const u8> = Vec::with_capacity(candidates.len());
    let mut candidate_lens: Vec<u32> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        candidate_ptrs.push(candidate.as_ptr());
        candidate_lens.push(u32::try_from(candidate.len()).map_err(|_| KernelError::InvalidInput)?);
    }

    let mut scores = vec![0_i32; output_cap];
    let mut indices = vec![0_u32; output_cap];
    let mut written: u32 = 0;

    // SAFETY: all pointers are derived from live Rust slices/vectors for the duration
    // of this call; output buffers are sized to `output_cap`.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let rc = unsafe {
        marky_fuzzy_match_batch(
            query.as_ptr(),
            u32::try_from(query.len()).map_err(|_| KernelError::InvalidInput)?,
            candidate_ptrs.as_ptr(),
            candidate_lens.as_ptr(),
            candidate_count,
            scores.as_mut_ptr(),
            indices.as_mut_ptr(),
            output_cap_u32,
            top_k_u32,
            &mut written,
        )
    };

    match rc {
        0 => {
            let mut out = Vec::with_capacity(written as usize);
            for i in 0..(written as usize) {
                let index = indices[i];
                let candidate = candidates
                    .get(index as usize)
                    .ok_or(KernelError::InternalError(-99))?;
                out.push(FuzzyBatchMatch {
                    index,
                    score: scores[i],
                    starts_with: starts_with_ascii_case_insensitive(query, candidate),
                });
            }
            Ok(out)
        }
        -1 => Err(KernelError::InvalidInput),
        -2 => Err(KernelError::BufferTooSmall),
        other => Err(KernelError::InternalError(other)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- scan_headings tests --

    #[test]
    fn test_scan_headings_basic() {
        let text = "# Hello\n## World\n";
        let results = scan_headings(text).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "Hello");
        assert_eq!(results[0].level, 1);
        assert_eq!(results[1].text, "World");
        assert_eq!(results[1].level, 2);
    }

    #[test]
    fn test_scan_headings_empty() {
        let results = scan_headings("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_headings_no_headings() {
        let results = scan_headings("Just some plain text\nwithout headings\n").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_headings_many() {
        // Generate more headings than INITIAL_CAP to test buffer retry
        let mut text = String::new();
        for i in 0..100 {
            text.push_str(&format!("# Heading {i}\n"));
        }
        let results = scan_headings(&text).unwrap();
        assert_eq!(results.len(), 100);
        assert_eq!(results[0].text, "Heading 0");
        assert_eq!(results[99].text, "Heading 99");
    }

    #[test]
    fn test_scan_headings_levels() {
        let text = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
        let results = scan_headings(text).unwrap();
        assert_eq!(results.len(), 6);
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.level, (i + 1) as u8);
        }
    }

    // -- scan_links tests --

    #[test]
    fn test_scan_links_markdown() {
        let text = "Click [here](https://example.com) for more.";
        let results = scan_links(text).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "here");
        assert_eq!(results[0].target, "https://example.com");
        assert_eq!(results[0].link_type, LinkType::Markdown);
    }

    #[test]
    fn test_scan_links_wiki() {
        let text = "See [[My Page]] for details.";
        let results = scan_links(text).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].link_type, LinkType::Wiki);
    }

    #[test]
    fn test_scan_links_empty() {
        let results = scan_links("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_links_mixed() {
        let text = "[md](url) and [[wiki]]";
        let results = scan_links(text).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].link_type, LinkType::Markdown);
        assert_eq!(results[1].link_type, LinkType::Wiki);
    }

    // -- scan_tags tests --

    #[test]
    fn test_scan_tags_basic() {
        let text = "text #tag1 #tag2";
        let results = scan_tags(text).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "tag1");
        assert_eq!(results[1].name, "tag2");
    }

    #[test]
    fn test_scan_tags_empty() {
        let results = scan_tags("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_tags_no_tags() {
        let results = scan_tags("plain text without tags").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_tags_offset() {
        let text = "text #topic";
        let results = scan_tags(text).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, 5); // '#' is at byte 5
    }

    // -- scan_block_ids tests --

    #[test]
    fn test_scan_block_ids_basic() {
        let text = "Some text ^my-block\n";
        let results = scan_block_ids(text).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "my-block");
    }

    #[test]
    fn test_scan_block_ids_empty() {
        let results = scan_block_ids("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_block_ids_not_at_eol() {
        // Block IDs must be at end of line
        let text = "^id more text\n";
        let results = scan_block_ids(text).unwrap();
        assert!(results.is_empty());
    }

    // -- fuzzy_match tests --

    #[test]
    fn test_fuzzy_match_prefix_scores_higher_than_substring() {
        let prefix = fuzzy_match("st", "stage").unwrap();
        let substring = fuzzy_match("st", "setup").unwrap();

        assert!(prefix.score > 0);
        assert!(substring.score > 0);
        assert!(prefix.score > substring.score);
        assert!(prefix.starts_with);
        assert!(!substring.starts_with);
    }

    #[test]
    fn test_fuzzy_match_is_case_insensitive() {
        let mixed = fuzzy_match("ST", "Setup").unwrap();
        assert!(mixed.score > 0);
        assert!(!mixed.starts_with);
    }

    #[test]
    fn test_fuzzy_match_supports_subsequence() {
        let subseq = fuzzy_match("stp", "setup").unwrap();
        assert!(subseq.score > 0);
    }

    #[test]
    fn test_fuzzy_match_returns_zero_for_non_match() {
        let no_match = fuzzy_match("zzz", "setup").unwrap();
        assert_eq!(no_match.score, 0);
        assert!(!no_match.starts_with);
    }

    #[test]
    fn test_fuzzy_match_query_longer_than_candidate_is_not_prefix() {
        let no_match = fuzzy_match("setup", "set").unwrap();
        assert_eq!(no_match.score, 0);
        assert!(!no_match.starts_with);
    }

    #[test]
    fn test_fuzzy_match_batch_top_k_stable_ties() {
        let candidates = vec!["acb", "adb", "aeb"];
        let results = fuzzy_match_batch("ab", &candidates, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 0);
        assert_eq!(results[1].index, 1);
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn test_fuzzy_match_batch_empty_query_contract() {
        let candidates = vec!["stage", "setup"];
        let results = fuzzy_match_batch("", &candidates, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_match_batch_no_match_returns_zero_written() {
        let candidates = vec!["stage", "setup"];
        let results = fuzzy_match_batch("zzz", &candidates, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_match_batch_subsequence_ranking_order() {
        let candidates = vec!["setup", "stop", "list"];
        let results = fuzzy_match_batch("stp", &candidates, 3).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 1);
        assert_eq!(results[1].index, 0);
    }

    #[test]
    fn test_fuzzy_match_batch_case_insensitive_match() {
        let candidates = vec!["Setup", "stage"];
        let results = fuzzy_match_batch("ST", &candidates, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 1);
    }

    #[test]
    fn test_fuzzy_match_batch_large_fixture_correct_top_k() {
        let mut candidates: Vec<String> =
            (0..10_000).map(|i| format!("candidate-{i:05}")).collect();
        candidates[123] = "start-of-line".to_string();
        candidates[4567] = "stateful".to_string();
        candidates[9876] = "stack".to_string();

        let refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        let results = fuzzy_match_batch("sta", &refs, 3).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|m| m.score > 0));
    }

    #[test]
    fn benchmark_fuzzy_match_batch_100k_candidates() {
        if std::env::var("MARKYMARK_RUN_100K_BENCH").ok().as_deref() != Some("1") {
            return;
        }

        let mut candidates: Vec<String> =
            (0..100_000).map(|i| format!("candidate-{i:05}")).collect();
        candidates[123] = "start-of-line".to_string();
        candidates[4567] = "stateful".to_string();
        candidates[98_765] = "stack".to_string();

        let refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        let started = std::time::Instant::now();
        let results = fuzzy_match_batch("sta", &refs, 25).unwrap();
        let elapsed = started.elapsed();

        eprintln!("fuzzy_match_batch benchmark (100k candidates): {elapsed:?}");
        assert!(!results.is_empty());
        assert!(results.len() <= 25);
        assert!(results.iter().all(|m| m.score > 0));
    }

    // -- safe_slice tests --

    #[test]
    fn test_safe_slice_basic() {
        assert_eq!(safe_slice("hello world", 6, 5), "world");
    }

    #[test]
    fn test_safe_slice_out_of_bounds() {
        assert_eq!(safe_slice("hi", 10, 5), "");
    }

    #[test]
    fn test_safe_slice_clamps_end() {
        assert_eq!(safe_slice("hello", 3, 100), "lo");
    }
}
