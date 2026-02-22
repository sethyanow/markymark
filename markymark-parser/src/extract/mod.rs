//! Extraction functions for arena-allocated markdown types.

mod blocks;
pub use blocks::{extract_block_ids, extract_block_refs, extract_callouts, extract_query_blocks};
mod frontmatter;
mod links;
pub use links::{
    extract_embeds, extract_link_definitions, extract_markdown_links, extract_wiki_links,
};
pub use frontmatter::{extract_frontmatter, extract_page_properties};
mod tasks;
pub use tasks::extract_tasks;

use crate::types::arena_alloc_str;
use crate::types::*;
use markymark_core::arena::new_arena_hashmap;
use markymark_core::prelude::*;
use regex::Regex;
use std::sync::LazyLock;

// ============================================================================
// Compiled regex patterns (LazyLock — compiled once, reused across calls)
// ============================================================================

static SIMPLE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([a-zA-Z0-9_/-]+)").unwrap());
static MULTI_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[\[([^\]]+)\]\]").unwrap());
static XML_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-zA-Z_:][a-zA-Z0-9_.:-]*)\s*=\s*"([^"]*)""#).unwrap());

/// Extract tags
pub fn extract_tags<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<Tag<'a>> {
    let mut tags = Vec::new();

    // Extract multi-word tags first (they're more specific)
    for captures in MULTI_TAG_RE.captures_iter(source) {
        if let Some(name_match) = captures.get(1) {
            tags.push(Tag::new(arena_alloc_str(arena, name_match.as_str())));
        }
    }

    // Extract simple tags
    for captures in SIMPLE_TAG_RE.captures_iter(source) {
        if let Some(name_match) = captures.get(1) {
            // Skip if it's the start of a wiki link (already captured as multi-word)
            if !source[name_match.start()..].starts_with("[[") {
                tags.push(Tag::new(arena_alloc_str(arena, name_match.as_str())));
            }
        }
    }

    tags
}




fn parse_fence_marker(line: &str) -> Option<(u8, usize)> {
    let line_bytes = line.as_bytes();
    if line_bytes.is_empty() {
        return None;
    }

    let marker = line_bytes[0];
    if marker != b'`' && marker != b'~' {
        return None;
    }

    let mut marker_len = 0;
    while marker_len < line_bytes.len() && line_bytes[marker_len] == marker {
        marker_len += 1;
    }

    if marker_len >= 3 {
        Some((marker, marker_len))
    } else {
        None
    }
}

fn is_fence_closing_line(line: &str, marker: u8, min_len: usize) -> bool {
    let line_bytes = line.as_bytes();
    if line_bytes.is_empty() || line_bytes[0] != marker {
        return false;
    }

    let mut marker_len = 0;
    while marker_len < line_bytes.len() && line_bytes[marker_len] == marker {
        marker_len += 1;
    }

    marker_len >= min_len
        && line_bytes[marker_len..]
            .iter()
            .all(|b| *b == b' ' || *b == b'\t' || *b == b'\r' || *b == b'\n')
}

fn collect_fenced_code_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut line_start = 0;
    let source_len = source.len();

    // (marker byte, marker length, fence start byte offset)
    let mut active_fence: Option<(u8, usize, usize)> = None;

    while line_start < source_len {
        let line_end = source[line_start..]
            .find('\n')
            .map(|offset| line_start + offset + 1)
            .unwrap_or(source_len);

        let line = &source[line_start..line_end];

        // CommonMark allows up to 3 spaces of indentation for fenced code blocks.
        // Tabs expand to the next 4-column tab stop per CommonMark spec.
        let mut indent_cols = 0usize;
        let mut indent_bytes = 0usize;
        for b in line.bytes() {
            match b {
                b' ' => {
                    indent_cols += 1;
                    indent_bytes += 1;
                }
                b'\t' => {
                    indent_cols = (indent_cols / 4 + 1) * 4;
                    indent_bytes += 1;
                }
                _ => break,
            }
            if indent_cols >= 4 {
                break;
            }
        }
        let fence_candidate = if indent_cols <= 3 {
            &line[indent_bytes..]
        } else {
            ""
        };

        if let Some((marker, marker_len, fence_start)) = active_fence {
            if is_fence_closing_line(fence_candidate, marker, marker_len) {
                ranges.push((fence_start, line_end));
                active_fence = None;
            }
        } else if let Some((marker, marker_len)) = parse_fence_marker(fence_candidate) {
            active_fence = Some((marker, marker_len, line_start));
        }

        line_start = line_end;
    }

    if let Some((_, _, fence_start)) = active_fence {
        ranges.push((fence_start, source_len));
    }

    ranges
}

/// Extract XML/HTML tags from the document source.
///
/// Uses a single-pass stack-based tokenizer for O(n) performance.
/// Handles self-closing tags, void HTML elements, nested same-name tags,
/// and attribute values containing `>`.
pub fn extract_xml_tags<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<XmlTag<'a>> {
    let mut tags = Vec::new();

    // HTML void elements that are self-closing even without />
    const VOID_ELEMENTS: &[&str] = &[
        "br", "hr", "img", "input", "meta", "link", "source", "track", "wbr", "area", "base",
        "col", "embed", "param",
    ];

    let parse_attrs = |attr_str: &str,
                       arena: &'a bumpalo::Bump|
     -> markymark_core::arena::ArenaHashMap<'a, &'a str, &'a str> {
        let mut attrs = new_arena_hashmap(arena);
        for cap in XML_ATTR_RE.captures_iter(attr_str) {
            if let (Some(key), Some(val)) = (cap.get(1), cap.get(2)) {
                attrs.insert(
                    arena_alloc_str(arena, key.as_str()),
                    arena_alloc_str(arena, val.as_str()),
                );
            }
        }
        attrs
    };

    let compute_range = |start: usize, end: usize| -> Range {
        let start_line = source[..start].matches('\n').count() as u32;
        let start_line_offset = source[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let start_char = (start - start_line_offset) as u32;

        let end_line = source[..end].matches('\n').count() as u32;
        let end_line_offset = source[..end].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let end_char = (end - end_line_offset) as u32;

        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    };

    /// Frame on the tag-matching stack for open tags awaiting their close.
    struct StackFrame<'a> {
        tag_name: &'a str,
        attrs: markymark_core::arena::ArenaHashMap<'a, &'a str, &'a str>,
        tag_start: usize,
        content_start: usize,
    }

    // Find the end of a tag starting at `<`, respecting quoted attribute values
    // that may contain `>`. Returns the byte index *after* the closing `>`.
    let find_tag_end = |from: usize| -> Option<usize> {
        let bytes = source.as_bytes();
        let mut i = from;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    // Skip to closing quote
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += 1;
                    }
                }
                b'\'' => {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'\'' {
                        i += 1;
                    }
                }
                b'>' => return Some(i + 1),
                _ => {}
            }
            i += 1;
        }
        None
    };

    let fenced_ranges = collect_fenced_code_ranges(source);
    let mut current_fence_idx = 0usize;

    let mut stack: Vec<StackFrame<'a>> = Vec::new();
    let bytes = source.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        while current_fence_idx < fenced_ranges.len() && pos >= fenced_ranges[current_fence_idx].1 {
            current_fence_idx += 1;
        }

        if current_fence_idx < fenced_ranges.len() {
            let (fence_start, fence_end) = fenced_ranges[current_fence_idx];
            if pos >= fence_start {
                pos = fence_end;
                continue;
            }
        }

        if bytes[pos] != b'<' {
            pos += 1;
            continue;
        }

        // We found a '<'. Determine what kind of tag it is.
        let tag_start = pos;

        // Find the end of this tag (quote-aware)
        let tag_end = match find_tag_end(tag_start) {
            Some(end) => end,
            None => break, // Malformed, no closing >
        };

        let tag_str = &source[tag_start..tag_end];

        // Is this a closing tag?  </name>
        if tag_str.starts_with("</") {
            let name_start = 2;
            let name_end = tag_str[name_start..]
                .find(|c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != '_')
                .map(|p| name_start + p)
                .unwrap_or(tag_str.len() - 1);
            let tag_name = &tag_str[name_start..name_end];

            if !tag_name.is_empty() {
                // Walk the stack backwards to find the matching open tag
                let mut idx = stack.len();
                while idx > 0 {
                    idx -= 1;
                    if stack[idx].tag_name == tag_name {
                        let frame = stack.remove(idx);
                        let content_str = &source[frame.content_start..tag_start];
                        let content: Option<&'a str> = if content_str.is_empty() {
                            None
                        } else {
                            Some(arena_alloc_str(arena, content_str))
                        };
                        let range = compute_range(frame.tag_start, tag_end);
                        tags.push(XmlTag::new(
                            frame.tag_name,
                            frame.attrs,
                            false,
                            content,
                            range,
                            frame.tag_start,
                            tag_end,
                        ));
                        break;
                    }
                }
            }

            pos = tag_end;
            continue;
        }

        // Not a closing tag — extract the tag name
        let name_start = 1; // skip '<'
        let name_end = tag_str[name_start..]
            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != '_')
            .map(|p| name_start + p)
            .unwrap_or(tag_str.len() - 1);
        let tag_name = &tag_str[name_start..name_end];

        if tag_name.is_empty() || !tag_name.as_bytes()[0].is_ascii_alphabetic() {
            pos = tag_end;
            continue;
        }

        // Extract attribute region (between tag name and closing > or />)
        let attr_region = &tag_str[name_end..tag_str.len() - 1]; // strip trailing >
        let is_self_closing = tag_str.ends_with("/>") || attr_region.trim_end().ends_with('/');
        let is_void = VOID_ELEMENTS
            .iter()
            .any(|v| v.eq_ignore_ascii_case(tag_name));

        let attrs = parse_attrs(attr_region, arena);
        let tag_name = arena_alloc_str(arena, tag_name);

        if is_self_closing || is_void {
            let range = compute_range(tag_start, tag_end);
            tags.push(XmlTag::new(
                tag_name, attrs, true, None, range, tag_start, tag_end,
            ));
        } else {
            // Regular opening tag — push onto stack
            stack.push(StackFrame {
                tag_name,
                attrs,
                tag_start,
                content_start: tag_end,
            });
        }

        pos = tag_end;
    }

    // Emit remaining unclosed tags from the stack
    for frame in stack {
        let range = compute_range(frame.tag_start, frame.content_start);
        tags.push(XmlTag::unclosed(
            frame.tag_name,
            frame.attrs,
            range,
            frame.tag_start,
            frame.content_start,
        ));
    }

    // Sort by position in source for consistent ordering
    tags.sort_by_key(|t| {
        let r = t.range();
        (r.start.line, r.start.character)
    });

    tags
}

