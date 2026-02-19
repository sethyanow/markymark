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
struct CMd4cResult {
    headings: *mut CMd4cHeading,
    links: *mut CMd4cLink,
    text_blob: *const u8,
    headings_count: u32,
    links_count: u32,
    text_blob_len: u32,
    _padding: u32,
}

// Compile-time size assertions — must match Zig side
const _: () = assert!(std::mem::size_of::<CMd4cHeading>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cLink>() == 24);
const _: () = assert!(std::mem::size_of::<CMd4cResult>() == 40);

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

/// Results from md4c single-pass extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cExtraction {
    pub headings: Vec<Md4cHeading>,
    pub links: Vec<Md4cLink>,
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
            let text_end = text_start + h.text_length as usize;
            let text = std::str::from_utf8(&blob[text_start..text_end])
                .unwrap_or("")
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
            let text_end = text_start + l.text_length as usize;
            let target_start = l.target_offset as usize;
            let target_end = target_start + l.target_length as usize;
            let text = std::str::from_utf8(&blob[text_start..text_end])
                .unwrap_or("")
                .to_owned();
            let target = std::str::from_utf8(&blob[target_start..target_end])
                .unwrap_or("")
                .to_owned();
            links.push(Md4cLink {
                text,
                target,
                source_offset: l.source_offset,
                is_wiki: l.is_wiki != 0,
            });
        }
    }

    Ok(Md4cExtraction { headings, links })
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
        assert_eq!(std::mem::size_of::<CMd4cResult>(), 40);
    }
}
