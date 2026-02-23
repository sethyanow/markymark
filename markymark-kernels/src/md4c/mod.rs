//! md4c ExtractionRenderer FFI bindings.
//!
//! Wraps the Zig `marky_md4c_extract` / `marky_md4c_free` C ABI functions
//! to provide safe Rust access to the single-pass md4c extraction pipeline.
//! Created for marky-6zl8.

#[cfg(test)]
mod tests;

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
#[derive(Clone, Copy)]
struct CMd4cTask {
    source_offset: u32,
    end_offset: u32,
    text_offset: u32,
    text_length: u32,
    state: u8,
    _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cEmbed {
    source_offset: u32,
    end_offset: u32,
    target_offset: u32,
    target_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cCallout {
    source_offset: u32,
    end_offset: u32,
    type_offset: u32,
    type_length: u32,
    title_offset: u32,
    title_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cBlockRef {
    source_offset: u32,
    uuid_offset: u32,
    uuid_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cQueryBlock {
    source_offset: u32,
    end_offset: u32,
    query_offset: u32,
    query_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cLinkDefinition {
    source_offset: u32,
    end_offset: u32,
    label_offset: u32,
    label_length: u32,
    url_offset: u32,
    url_length: u32,
    title_offset: u32,
    title_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cProperty {
    key_offset: u32,
    key_length: u32,
    value_offset: u32,
    value_length: u32,
    value_type: u8,
    _pad: [u8; 3],
}

#[repr(C)]
struct CMd4cResult {
    headings: *mut CMd4cHeading,
    links: *mut CMd4cLink,
    code_spans: *mut CMd4cCodeSpan,
    tasks: *mut CMd4cTask,
    embeds: *mut CMd4cEmbed,
    callouts: *mut CMd4cCallout,
    block_refs: *mut CMd4cBlockRef,
    query_blocks: *mut CMd4cQueryBlock,
    link_definitions: *mut CMd4cLinkDefinition,
    properties: *mut CMd4cProperty,
    text_blob: *const u8,
    headings_count: u32,
    links_count: u32,
    code_spans_count: u32,
    tasks_count: u32,
    embeds_count: u32,
    callouts_count: u32,
    block_refs_count: u32,
    query_blocks_count: u32,
    link_definitions_count: u32,
    properties_count: u32,
    text_blob_len: u32,
}

// Compile-time size assertions — must match Zig side
const _: () = assert!(std::mem::size_of::<CMd4cHeading>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cLink>() == 24);
const _: () = assert!(std::mem::size_of::<CMd4cCodeSpan>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cTask>() == 20);
const _: () = assert!(std::mem::size_of::<CMd4cEmbed>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cCallout>() == 24);
const _: () = assert!(std::mem::size_of::<CMd4cBlockRef>() == 12);
const _: () = assert!(std::mem::size_of::<CMd4cQueryBlock>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cLinkDefinition>() == 32);
const _: () = assert!(std::mem::size_of::<CMd4cProperty>() == 20);
const _: () = assert!(std::mem::size_of::<CMd4cResult>() == 136);

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

/// A task list item extracted by the md4c single-pass parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cTask {
    /// Checkbox state: "checked" or "unchecked".
    pub state: String,
    /// Task description text.
    pub text: String,
    /// Byte offset of the `[` in `[x]` in source.
    pub source_offset: u32,
    /// Byte offset past the task text.
    pub end_offset: u32,
}

/// An embed reference extracted by the md4c single-pass parser (e.g. `![[target]]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cEmbed {
    /// The embedded resource path.
    pub target: String,
    /// Byte offset of `!` in `![[target]]` in source.
    pub source_offset: u32,
    /// Byte offset past `]]`.
    pub end_offset: u32,
}

/// A callout extracted by the md4c single-pass parser (e.g. `> [!note] Title`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cCallout {
    /// Callout type (e.g. "note", "warning", "tip").
    pub callout_type: String,
    /// Optional callout title.
    pub title: Option<String>,
    /// Byte offset of `>` in source.
    pub source_offset: u32,
    /// Byte offset past the callout block.
    pub end_offset: u32,
}

/// A block reference extracted by the md4c single-pass parser (e.g. `((uuid))`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cBlockRef {
    /// The UUID string.
    pub uuid: String,
    /// Byte offset of `(` in `((uuid))` in source.
    pub source_offset: u32,
}

/// A query block extracted by the md4c parser (e.g. `{{query ...}}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cQueryBlock {
    /// The query text.
    pub query: String,
    /// Byte offset of first `{` of `{{query ...}}` in source.
    pub source_offset: u32,
    /// Byte offset past closing `}}` in source.
    pub end_offset: u32,
}

/// A link definition extracted by the md4c parser (e.g. `[label]: url "title"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cLinkDefinition {
    /// The link label.
    pub label: String,
    /// The link URL.
    pub url: String,
    /// Optional title.
    pub title: Option<String>,
    /// Byte offset of `[` in source.
    pub source_offset: u32,
    /// Byte offset past end of definition line.
    pub end_offset: u32,
}

/// A YAML frontmatter property extracted by md4c.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cProperty {
    /// The property key.
    pub key: String,
    /// The raw property value text.
    pub value: String,
    /// Property value type: 0=string, 1=list, 2=page_ref.
    pub value_type: u8,
}

/// Results from md4c single-pass extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md4cExtraction {
    pub headings: Vec<Md4cHeading>,
    pub links: Vec<Md4cLink>,
    pub code_spans: Vec<Md4cCodeSpan>,
    pub tasks: Vec<Md4cTask>,
    pub embeds: Vec<Md4cEmbed>,
    pub callouts: Vec<Md4cCallout>,
    pub block_refs: Vec<Md4cBlockRef>,
    pub query_blocks: Vec<Md4cQueryBlock>,
    pub link_definitions: Vec<Md4cLinkDefinition>,
    pub properties: Vec<Md4cProperty>,
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
            tasks: Vec::new(),
            embeds: Vec::new(),
            callouts: Vec::new(),
            block_refs: Vec::new(),
            query_blocks: Vec::new(),
            link_definitions: Vec::new(),
            properties: Vec::new(),
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

/// Map md4c task checkbox mark byte to state string.
fn task_state_str(mark: u8) -> &'static str {
    match mark {
        b'x' | b'X' => "checked",
        _ => "unchecked",
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

    let mut tasks = Vec::with_capacity(out.tasks_count as usize);
    if out.tasks_count > 0 && !out.tasks.is_null() {
        // SAFETY: tasks pointer is valid for tasks_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_tasks = unsafe { std::slice::from_raw_parts(out.tasks, out.tasks_count as usize) };
        for t in c_tasks {
            let text_start = t.text_offset as usize;
            let text =
                std::str::from_utf8(safe_blob_slice(blob, text_start, t.text_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            tasks.push(Md4cTask {
                state: task_state_str(t.state).to_owned(),
                text,
                source_offset: t.source_offset,
                end_offset: t.end_offset,
            });
        }
    }

    let mut embeds = Vec::with_capacity(out.embeds_count as usize);
    if out.embeds_count > 0 && !out.embeds.is_null() {
        // SAFETY: embeds pointer is valid for embeds_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_embeds =
            unsafe { std::slice::from_raw_parts(out.embeds, out.embeds_count as usize) };
        for e in c_embeds {
            let target_start = e.target_offset as usize;
            let target = std::str::from_utf8(safe_blob_slice(
                blob,
                target_start,
                e.target_length as usize,
            )?)
            .map_err(|_| KernelError::InternalError(-100))?
            .to_owned();
            embeds.push(Md4cEmbed {
                target,
                source_offset: e.source_offset,
                end_offset: e.end_offset,
            });
        }
    }

    let mut callouts = Vec::with_capacity(out.callouts_count as usize);
    if out.callouts_count > 0 && !out.callouts.is_null() {
        // SAFETY: callouts pointer is valid for callouts_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_callouts =
            unsafe { std::slice::from_raw_parts(out.callouts, out.callouts_count as usize) };
        for c in c_callouts {
            let type_start = c.type_offset as usize;
            let callout_type =
                std::str::from_utf8(safe_blob_slice(blob, type_start, c.type_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            let title = if c.title_length == 0 {
                None
            } else {
                let title_start = c.title_offset as usize;
                Some(
                    std::str::from_utf8(safe_blob_slice(
                        blob,
                        title_start,
                        c.title_length as usize,
                    )?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned(),
                )
            };
            callouts.push(Md4cCallout {
                callout_type,
                title,
                source_offset: c.source_offset,
                end_offset: c.end_offset,
            });
        }
    }

    let mut block_refs = Vec::with_capacity(out.block_refs_count as usize);
    if out.block_refs_count > 0 && !out.block_refs.is_null() {
        // SAFETY: block_refs pointer is valid for block_refs_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_block_refs =
            unsafe { std::slice::from_raw_parts(out.block_refs, out.block_refs_count as usize) };
        for br in c_block_refs {
            let uuid_start = br.uuid_offset as usize;
            let uuid =
                std::str::from_utf8(safe_blob_slice(blob, uuid_start, br.uuid_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            block_refs.push(Md4cBlockRef {
                uuid,
                source_offset: br.source_offset,
            });
        }
    }

    let mut query_blocks = Vec::with_capacity(out.query_blocks_count as usize);
    if out.query_blocks_count > 0 && !out.query_blocks.is_null() {
        // SAFETY: query_blocks pointer is valid for query_blocks_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_query_blocks = unsafe {
            std::slice::from_raw_parts(out.query_blocks, out.query_blocks_count as usize)
        };
        for qb in c_query_blocks {
            let query_start = qb.query_offset as usize;
            let query =
                std::str::from_utf8(safe_blob_slice(blob, query_start, qb.query_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            query_blocks.push(Md4cQueryBlock {
                query,
                source_offset: qb.source_offset,
                end_offset: qb.end_offset,
            });
        }
    }

    let mut link_definitions = Vec::with_capacity(out.link_definitions_count as usize);
    if out.link_definitions_count > 0 && !out.link_definitions.is_null() {
        // SAFETY: link_definitions pointer is valid for link_definitions_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_link_defs = unsafe {
            std::slice::from_raw_parts(
                out.link_definitions,
                out.link_definitions_count as usize,
            )
        };
        for ld in c_link_defs {
            let label_start = ld.label_offset as usize;
            let label = std::str::from_utf8(safe_blob_slice(
                blob,
                label_start,
                ld.label_length as usize,
            )?)
            .map_err(|_| KernelError::InternalError(-100))?
            .to_owned();
            let url_start = ld.url_offset as usize;
            let url =
                std::str::from_utf8(safe_blob_slice(blob, url_start, ld.url_length as usize)?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned();
            let title = if ld.title_length == 0 {
                None
            } else {
                let title_start = ld.title_offset as usize;
                Some(
                    std::str::from_utf8(safe_blob_slice(
                        blob,
                        title_start,
                        ld.title_length as usize,
                    )?)
                    .map_err(|_| KernelError::InternalError(-100))?
                    .to_owned(),
                )
            };
            link_definitions.push(Md4cLinkDefinition {
                label,
                url,
                title,
                source_offset: ld.source_offset,
                end_offset: ld.end_offset,
            });
        }
    }

    let mut properties = Vec::with_capacity(out.properties_count as usize);
    if out.properties_count > 0 && !out.properties.is_null() {
        // SAFETY: properties pointer is valid for properties_count elements,
        // allocated by Zig page_allocator.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        let c_properties =
            unsafe { std::slice::from_raw_parts(out.properties, out.properties_count as usize) };
        for p in c_properties {
            let key_start = p.key_offset as usize;
            let key = std::str::from_utf8(safe_blob_slice(
                blob,
                key_start,
                p.key_length as usize,
            )?)
            .map_err(|_| KernelError::InternalError(-100))?
            .to_owned();
            let value_start = p.value_offset as usize;
            let value = std::str::from_utf8(safe_blob_slice(
                blob,
                value_start,
                p.value_length as usize,
            )?)
            .map_err(|_| KernelError::InternalError(-100))?
            .to_owned();
            properties.push(Md4cProperty {
                key,
                value,
                value_type: p.value_type,
            });
        }
    }

    Ok(Md4cExtraction {
        headings,
        links,
        code_spans,
        tasks,
        embeds,
        callouts,
        block_refs,
        query_blocks,
        link_definitions,
        properties,
    })
}
