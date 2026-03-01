use crate::types::arena_alloc_str;
use crate::types::*;
use markymark_core::prelude::*;
use regex::Regex;
use std::sync::LazyLock;

static WIKI_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static INLINE_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());
static REF_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([^\]]+)\]").unwrap());
static LINK_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)^\[([^\]]+)\]:\s+(\S+)(?:\s+"([^"]+)")?"#).unwrap());
static EMBED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[\[([^\]]+)\]\]").unwrap());

/// Extract wiki links from elements
pub fn extract_wiki_links<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<WikiLink<'a>> {
    let mut links = Vec::new();

    for captures in WIKI_LINK_RE.captures_iter(source) {
        if let Some(content_match) = captures.get(1) {
            let content = content_match.as_str();
            let start = content_match.start() - 2; // Account for [[
            let end = content_match.end() + 2; // Account for ]]

            // Parse the content to extract components
            let (target, alias, heading, block_id) = parse_wiki_link_content(content, arena);

            // Calculate position in source
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;

            links.push(WikiLink::new(
                target.unwrap_or_default(),
                alias,
                heading,
                block_id,
                Range::new(
                    Position::new(line, start_char),
                    Position::new(line, end_char),
                ),
                start,
                end,
            ));
        }
    }

    links
}

/// Parse wiki link content into components
fn parse_wiki_link_content<'a>(
    content: &str,
    arena: &'a bumpalo::Bump,
) -> (
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
) {
    // Check for alias first: [[target|alias]]
    if let Some(pipe_pos) = content.find('|') {
        let target_part = &content[..pipe_pos];
        let alias: Option<&'a str> = Some(arena_alloc_str(arena, content[pipe_pos + 1..].trim()));
        let (target, heading, block_id) = parse_target_part(target_part, arena);
        return (target, alias, heading, block_id);
    }

    // No alias, parse target
    let (target, heading, block_id) = parse_target_part(content, arena);
    (target, None, heading, block_id)
}

/// Parse target part: [[Page#heading]] or [[Page#^blockid]] or [[#heading]]
fn parse_target_part<'a>(
    target: &str,
    arena: &'a bumpalo::Bump,
) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
    if let Some(hash_pos) = target.find('#') {
        let page_part = &target[..hash_pos];
        let after_hash = &target[hash_pos + 1..];

        // Check if it's a block ID (starts with ^)
        if let Some(stripped) = after_hash.strip_prefix('^') {
            let block_id: Option<&'a str> = Some(arena_alloc_str(arena, stripped));
            let page: Option<&'a str> = if page_part.is_empty() {
                None
            } else {
                Some(arena_alloc_str(arena, page_part))
            };
            return (page, None, block_id);
        }

        // It's a heading
        let heading: Option<&'a str> = Some(arena_alloc_str(arena, after_hash));
        let page: Option<&'a str> = if page_part.is_empty() {
            None
        } else {
            Some(arena_alloc_str(arena, page_part))
        };
        return (page, heading, None);
    }

    // Just a plain page link
    (Some(arena_alloc_str(arena, target)), None, None)
}

/// Extract markdown links
pub fn extract_markdown_links<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<MarkdownLink<'a>> {
    let mut links = Vec::new();

    // Extract inline links
    for captures in INLINE_LINK_RE.captures_iter(source) {
        if let (Some(text_match), Some(url_match)) = (captures.get(1), captures.get(2)) {
            let text = arena_alloc_str(arena, text_match.as_str());
            let url_str = url_match.as_str();

            // Check for anchor
            let (url, anchor) = if let Some(hash_pos) = url_str.find('#') {
                (
                    arena_alloc_str(arena, &url_str[..hash_pos]),
                    Some(arena_alloc_str(arena, &url_str[hash_pos + 1..]) as &'a str),
                )
            } else {
                (arena_alloc_str(arena, url_str), None)
            };

            // Calculate range: from '[' (1 before text_match) to ')' (1 after url_match)
            let full_match = captures.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );

            links.push(MarkdownLink::new(
                text, url, anchor, None, range, start, end,
            ));
        }
    }

    // Extract reference links
    for captures in REF_LINK_RE.captures_iter(source) {
        if let (Some(text_match), Some(ref_match)) = (captures.get(1), captures.get(2)) {
            let text = arena_alloc_str(arena, text_match.as_str());
            let reference = arena_alloc_str(arena, ref_match.as_str());

            let full_match = captures.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );

            links.push(MarkdownLink::new(
                text,
                arena_alloc_str(arena, ""),
                None,
                Some(reference),
                range,
                start,
                end,
            ));
        }
    }

    links
}

/// Extract link definitions
pub fn extract_link_definitions<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<LinkDefinition<'a>> {
    let mut defs = Vec::new();

    for captures in LINK_DEF_RE.captures_iter(source) {
        if let (Some(label_match), Some(url_match)) = (captures.get(1), captures.get(2)) {
            let label = arena_alloc_str(arena, label_match.as_str());
            let url = arena_alloc_str(arena, url_match.as_str());
            let title: Option<&'a str> =
                captures.get(3).map(|m| arena_alloc_str(arena, m.as_str()));
            defs.push(LinkDefinition::new(label, url, title));
        }
    }

    defs
}

/// Extract embeds
pub fn extract_embeds<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<Embed<'a>> {
    let mut embeds = Vec::new();

    for captures in EMBED_RE.captures_iter(source) {
        if let Some(content_match) = captures.get(1) {
            embeds.push(Embed::new(arena_alloc_str(arena, content_match.as_str())));
        }
    }

    embeds
}
