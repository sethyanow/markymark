//! md4c ExtractionRenderer FFI bindings.
//!
//! Wraps the Zig `marky_md4c_extract` / `marky_md4c_free` C ABI functions
//! to provide safe Rust access to the single-pass md4c extraction pipeline.
//! Created for marky-6zl8.

use crate::scan::KernelError;

// ---------------------------------------------------------------------------
// C ABI mirror types (repr(C) to match Zig extern structs in exports.zig)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cHeading {
    source_offset: u32,
    text_offset: u32,
    text_length: u32,
    level: u8,
    _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cLink {
    source_offset: u32,
    text_offset: u32,
    target_offset: u32,
    text_length: u32,
    target_length: u32,
    is_wiki: u8,
    _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cCodeSpan {
    source_offset: u32,
    end_offset: u32,
    text_offset: u32,
    text_length: u32,
}

#[repr(C)]
struct CMd4cResult {
    headings: *mut CMd4cHeading,
    links: *mut CMd4cLink,
    code_spans: *mut CMd4cCodeSpan,
    text_blob: *const u8,
    headings_count: u32,
    links_count: u32,
    code_spans_count: u32,
    text_blob_len: u32,
}

// Compile-time size assertions — must match Zig side
const _: () = assert!(std::mem::size_of::<CMd4cHeading>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cLink>() == 24);
const _: () = assert!(std::mem::size_of::<CMd4cCodeSpan>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cResult>() == 48);

extern "C" {
    fn marky_md4c_extract(text: *const u8, len: u32, out: *mut CMd4cResult) -> i32;
    fn marky_md4c_free(result: *mut CMd4cResult);
}

// ---------------------------------------------------------------------------
// Public Rust types
// ---------------------------------------------------------------------------

/// A heading extracted by the md4c single-pass parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cHeading {
    /// Decoded heading text (entity references NOT decoded).
    pub text: String,
    /// Byte offset in source where the heading starts (e.g. position of `#`).
    pub source_offset: u32,
    /// Heading level (1-6).
    pub level: u8,
}

/// A link extracted by the md4c single-pass parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cLink {
    /// Display text of the link.
    pub text: String,
    /// Link target (URL or wiki page name).
    pub target: String,
    /// Byte offset in source where the link starts (e.g. position of `[`).
    pub source_offset: u32,
    /// Whether this is a `[[wiki]]` link.
    pub is_wiki: bool,
}

/// An inline code span extracted by the md4c single-pass parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cCodeSpan {
    /// The backtick-delimited text content.
    pub text: String,
    /// Byte offset in source of the opening backtick.
    pub source_offset: u32,
    /// Byte offset in source past the closing backtick.
    pub end_offset: u32,
}

/// Results from md4c single-pass extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cExtraction {
    pub headings: Vec<Md4cHeading>,
    pub links: Vec<Md4cLink>,
    pub code_spans: Vec<Md4cCodeSpan>,
}

// ---------------------------------------------------------------------------
// Safe wrapper
// ---------------------------------------------------------------------------

/// Extract headings and links from markdown text using the md4c single-pass parser.
///
/// This calls the Zig ExtractionRenderer via FFI, which performs a single parse
/// pass extracting all headings and links with byte offsets.
pub fn extract_md4c(text: &str) -> Result<Md4cExtraction, KernelError> {
    if text.is_empty() {
        return Ok(Md4cExtraction {
            headings: Vec::new(),
            links: Vec::new(),
            code_spans: Vec::new(),
        });
    }

    // SAFETY: text.as_ptr() is valid for text.len() bytes (borrowed from &str).
    // out is stack-local and valid for the duration of the call.
    // marky_md4c_extract reads text and writes to out, retaining no pointers
    // after return.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let mut out: CMd4cResult = unsafe { std::mem::zeroed() };
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    let rc = unsafe { marky_md4c_extract(text.as_ptr(), text.len() as u32, &mut out) };

    match rc {
        0 => {
            let result = convert_result(&out);
            // SAFETY: out was populated by marky_md4c_extract.
            // marky_md4c_free frees the 3 Zig-allocated arrays and zeroes the struct.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
            unsafe { marky_md4c_free(&mut out) };
            result
        }
        -1 => Err(KernelError::InvalidInput),
        -3 => Err(KernelError::InternalError(-3)),
        -4 => Err(KernelError::InternalError(-4)),
        other => Err(KernelError::InternalError(other)),
    }
}

/// Validate and slice blob[start..start+len], returning InternalError(-101) on
/// overflow or out-of-bounds rather than panicking.
///
/// Error codes:
///   -100  invalid UTF-8 in blob (existing convention)
///   -101  blob slice out-of-bounds or start+len overflow (marky-ta07)
fn safe_blob_slice(blob: &[u8], start: usize, len: usize) -> Result<&[u8], KernelError> {
    let end = start
        .checked_add(len)
        .ok_or(KernelError::InternalError(-101))?;
    blob.get(start..end).ok_or(KernelError::InternalError(-101))
}

/// Convert C ABI result to owned Rust types.
fn convert_result(out: &CMd4cResult) -> Result<Md4cExtraction, KernelError> {
    let blob = if out.text_blob_len > 0 && !out.text_blob.is_null() {
        // SAFETY: text_blob was allocated by Zig with text_blob_len bytes.
        // Valid until marky_md4c_free is called (which happens after this function returns).
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        unsafe { std::slice::from_raw_parts(out.text_blob, out.text_blob_len as usize) }
    } else {
        &[]
    };

    let mut headings = Vec::with_capacity(out.headings_count as usize);
    if out.headings_count > 0 && !out.headings.is_null() {
        // SAFETY: headings pointer is valid for headings_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_headings =
            unsafe { std::slice::from_raw_parts(out.headings, out.headings_count as usize) };
        for h in c_headings {
            let text_start = h.text_offset as usize;
            // T2-11: Propagate invalid UTF-8 as an error rather than silently
            // falling back to "". Zig packing always produces valid UTF-8 (the
            // source is a &str), so this fires only if the blob is corrupted.
            // marky-ta07: bounds-check before slicing to avoid OOB panic on bad FFI offsets.
            let text =
                std::str::from_utf8(safe_blob_slice(blob, text_start, h.text_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            headings.push(Md4cHeading {
                text,
                source_offset: h.source_offset,
                level: h.level,
            });
        }
    }

    let mut links = Vec::with_capacity(out.links_count as usize);
    if out.links_count > 0 && !out.links.is_null() {
        // SAFETY: links pointer is valid for links_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_links = unsafe { std::slice::from_raw_parts(out.links, out.links_count as usize) };
        for l in c_links {
            let text_start = l.text_offset as usize;
            let target_start = l.target_offset as usize;
            // T2-11: Propagate invalid UTF-8 as an error rather than silently falling back.
            // marky-ta07: bounds-check before slicing to avoid OOB panic on bad FFI offsets.
            let text =
                std::str::from_utf8(safe_blob_slice(blob, text_start, l.text_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            let target = std::str::from_utf8(safe_blob_slice(
                blob,
                target_start,
                l.target_length as usize,
            )?)
            .map_err(|_| KernelError::InternalError(-100))?
            .to_owned();
            links.push(Md4cLink {
                text,
                target,
                source_offset: l.source_offset,
                is_wiki: l.is_wiki != 0,
            });
        }
    }

    let mut code_spans = Vec::with_capacity(out.code_spans_count as usize);
    if out.code_spans_count > 0 && !out.code_spans.is_null() {
        // SAFETY: code_spans pointer is valid for code_spans_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_code_spans =
            unsafe { std::slice::from_raw_parts(out.code_spans, out.code_spans_count as usize) };
        for cs in c_code_spans {
            let text_start = cs.text_offset as usize;
            let text =
                std::str::from_utf8(safe_blob_slice(blob, text_start, cs.text_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            code_spans.push(Md4cCodeSpan {
                text,
                source_offset: cs.source_offset,
                end_offset: cs.end_offset,
            });
        }
    }

    Ok(Md4cExtraction {
        headings,
        links,
        code_spans,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_heading() {
        let result = extract_md4c("# Hello\n").unwrap();
        assert_eq!(result.headings.len(), 1);
        assert_eq!(result.headings[0].text, "Hello");
        assert_eq!(result.headings[0].level, 1);
        assert_eq!(result.headings[0].source_offset, 0);
    }

    #[test]
    fn test_extract_link() {
        let result = extract_md4c("[click](https://example.com)\n").unwrap();
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].text, "click");
        assert_eq!(result.links[0].target, "https://example.com");
        assert!(!result.links[0].is_wiki);
    }

    #[test]
    fn test_extract_wiki_link() {
        let result = extract_md4c("[[Target]]\n").unwrap();
        assert_eq!(result.links.len(), 1);
        assert!(result.links[0].is_wiki);
        assert_eq!(result.links[0].target, "Target");
    }

    #[test]
    fn test_extract_mixed_document() {
        let input =
            "# Title\n\nSome [link](url) text.\n\n## Section\n\nSee [[Wiki]] for details.\n";
        let result = extract_md4c(input).unwrap();
        assert_eq!(result.headings.len(), 2);
        assert_eq!(result.headings[0].text, "Title");
        assert_eq!(result.headings[1].text, "Section");
        assert_eq!(result.links.len(), 2);
        assert!(!result.links[0].is_wiki);
        assert!(result.links[1].is_wiki);
    }

    #[test]
    fn test_extract_empty_input() {
        let result = extract_md4c("").unwrap();
        assert!(result.headings.is_empty());
        assert!(result.links.is_empty());
    }

    #[test]
    fn test_extract_entity_decoded() {
        // ExtractionRenderer decodes HTML entities to UTF-8 (marky-yfh7)
        let result = extract_md4c("# Hello &amp; World\n").unwrap();
        assert_eq!(result.headings[0].text, "Hello & World");
    }

    #[test]
    fn test_text_is_valid_utf8() {
        let result = extract_md4c("# Héllo Wörld\n").unwrap();
        assert_eq!(result.headings[0].text, "Héllo Wörld");
    }

    #[test]
    fn test_struct_sizes_match_zig() {
        assert_eq!(std::mem::size_of::<CMd4cHeading>(), 16);
        assert_eq!(std::mem::size_of::<CMd4cLink>(), 24);
        assert_eq!(std::mem::size_of::<CMd4cCodeSpan>(), 16);
        assert_eq!(std::mem::size_of::<CMd4cResult>(), 48);
    }

    /// Regression test for T2-11: silent `.unwrap_or("")` masked data corruption.
    ///
    /// If the blob contains invalid UTF-8 (e.g. due to Zig packing bugs), the
    /// old code silently returned an empty string. The fixed code returns
    /// `KernelError::InternalError(-100)` so callers can detect corruption.
    #[test]
    fn test_invalid_utf8_in_heading_blob_returns_error() {
        // 0xFF 0xFE is invalid UTF-8.
        let blob = [0xFF_u8, 0xFE, b'!'];
        let heading = CMd4cHeading {
            source_offset: 0,
            text_offset: 0,
            text_length: 3,
            level: 1,
            _padding: [0, 0, 0],
        };
        let out = CMd4cResult {
            headings: &heading as *const _ as *mut _,
            links: std::ptr::null_mut(),
            text_blob: blob.as_ptr(),
            headings_count: 1,
            links_count: 0,
            code_spans: std::ptr::null_mut(),
            text_blob_len: 3,
            code_spans_count: 0,
        };
        // Before fix: returns Ok(headings[0].text == "") — silent data loss.
        // After fix: returns Err(KernelError::InternalError(-100)).
        let result = convert_result(&out);
        assert!(
            result.is_err(),
            "invalid UTF-8 in blob must return Err, not silently produce empty string"
        );
    }

    /// Regression test for T2-11: invalid UTF-8 in link target blob.
    #[test]
    fn test_invalid_utf8_in_link_blob_returns_error() {
        let blob = [b'o', b'k', 0xFF_u8]; // "ok" text, invalid target
        let link = CMd4cLink {
            source_offset: 0,
            text_offset: 0,
            target_offset: 2,
            text_length: 2,
            target_length: 1,
            is_wiki: 0,
            _padding: [0, 0, 0],
        };
        let out = CMd4cResult {
            headings: std::ptr::null_mut(),
            links: &link as *const _ as *mut _,
            text_blob: blob.as_ptr(),
            headings_count: 0,
            links_count: 1,
            code_spans: std::ptr::null_mut(),
            text_blob_len: 3,
            code_spans_count: 0,
        };
        let result = convert_result(&out);
        assert!(
            result.is_err(),
            "invalid UTF-8 in link target blob must return Err"
        );
    }

    /// Regression test for marky-ta07: heading text_offset beyond blob end must
    /// return KernelError, not panic with OOB slice.
    #[test]
    fn test_oob_heading_offset_returns_error() {
        let blob = [b'h', b'i']; // 2-byte blob
        let heading = CMd4cHeading {
            source_offset: 0,
            text_offset: 5, // past end of 2-byte blob
            text_length: 3,
            level: 1,
            _padding: [0, 0, 0],
        };
        let out = CMd4cResult {
            headings: &heading as *const _ as *mut _,
            links: std::ptr::null_mut(),
            code_spans: std::ptr::null_mut(),
            text_blob: blob.as_ptr(),
            headings_count: 1,
            links_count: 0,
            code_spans_count: 0,
            text_blob_len: blob.len() as u32,
        };
        let result = convert_result(&out);
        assert!(
            matches!(result, Err(KernelError::InternalError(-101))),
            "OOB heading offset must return InternalError(-101), got: {result:?}"
        );
    }

    /// Regression test for marky-ta07: link target_offset beyond blob end must
    /// return KernelError, not panic with OOB slice.
    #[test]
    fn test_oob_link_offset_returns_error() {
        let blob = [b'o', b'k']; // 2-byte blob
        let link = CMd4cLink {
            source_offset: 0,
            text_offset: 0,
            target_offset: 10, // past end of 2-byte blob
            text_length: 2,
            target_length: 3,
            is_wiki: 0,
            _padding: [0, 0, 0],
        };
        let out = CMd4cResult {
            headings: std::ptr::null_mut(),
            links: &link as *const _ as *mut _,
            code_spans: std::ptr::null_mut(),
            text_blob: blob.as_ptr(),
            headings_count: 0,
            links_count: 1,
            code_spans_count: 0,
            text_blob_len: blob.len() as u32,
        };
        let result = convert_result(&out);
        assert!(
            matches!(result, Err(KernelError::InternalError(-101))),
            "OOB link target offset must return InternalError(-101), got: {result:?}"
        );
    }

    /// Regression test for marky-ta07: offset + length that overflows usize must
    /// return KernelError, not panic or wrap.
    #[test]
    fn test_overflow_offset_returns_error() {
        let blob = [b'x'; 4];
        let heading = CMd4cHeading {
            source_offset: 0,
            text_offset: u32::MAX, // usize::MAX addition would overflow
            text_length: 1,
            level: 1,
            _padding: [0, 0, 0],
        };
        let out = CMd4cResult {
            headings: &heading as *const _ as *mut _,
            links: std::ptr::null_mut(),
            code_spans: std::ptr::null_mut(),
            text_blob: blob.as_ptr(),
            headings_count: 1,
            links_count: 0,
            code_spans_count: 0,
            text_blob_len: blob.len() as u32,
        };
        let result = convert_result(&out);
        assert!(
            matches!(result, Err(KernelError::InternalError(-101))),
            "overflow offset must return InternalError(-101), got: {result:?}"
        );
    }

    // --- Code span tests (marky-pdyo) ---

    #[test]
    fn test_extract_code_span() {
        let result = extract_md4c("here is `hello` world\n").unwrap();
        assert_eq!(result.code_spans.len(), 1);
        assert_eq!(result.code_spans[0].text, "hello");
        assert_eq!(result.code_spans[0].source_offset, 8);
        assert_eq!(result.code_spans[0].end_offset, 15);
    }

    #[test]
    fn test_extract_code_span_mixed_document() {
        let result = extract_md4c("# Title `code` [link](url)\n").unwrap();
        assert_eq!(result.headings.len(), 1);
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.code_spans.len(), 1);
        assert_eq!(result.code_spans[0].text, "code");
    }

    #[test]
    fn test_extract_no_code_spans() {
        let result = extract_md4c("Just plain text.\n").unwrap();
        assert!(result.code_spans.is_empty());
    }

    #[test]
    fn test_extract_multiple_code_spans() {
        let result = extract_md4c("`a` then `b`\n").unwrap();
        assert_eq!(result.code_spans.len(), 2);
        assert_eq!(result.code_spans[0].text, "a");
        assert_eq!(result.code_spans[1].text, "b");
        assert!(result.code_spans[1].source_offset > result.code_spans[0].source_offset);
    }
}
