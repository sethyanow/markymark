use crate::types::arena_alloc_str;
use crate::types::*;
use markymark_core::prelude::*;
use regex::Regex;
use std::sync::LazyLock;

static BLOCK_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)\^([a-zA-Z0-9_-]+)\s*$").unwrap());
static BLOCK_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(\(([0-9a-f-]{36})\)\)").unwrap());
static CALLOUT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\u003e\s+\[!([a-z]+)\]\s+(.*)$").unwrap());
static QUERY_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{query\s+([^}]+)\}\}").unwrap());

/// Extract block IDs with source ranges for go-to-definition.
pub fn extract_block_ids<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<BlockId<'a>> {
    let mut blocks = Vec::new();

    for captures in BLOCK_ID_RE.captures_iter(source) {
        if let (Some(id_match), Some(full_match)) = (captures.get(1), captures.get(0)) {
            let start = full_match.start();
            // Use id_match.end() to exclude trailing whitespace captured by \s*$
            let end = id_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = (end - line_start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );
            blocks.push(BlockId::new(
                arena_alloc_str(arena, id_match.as_str()),
                range,
                start,
                end,
            ));
        }
    }

    blocks
}

/// Extract block refs
pub fn extract_block_refs<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<BlockRef<'a>> {
    let mut refs = Vec::new();

    for captures in BLOCK_REF_RE.captures_iter(source) {
        if let (Some(full_match), Some(uuid_match)) = (captures.get(0), captures.get(1)) {
            let start = full_match.start();
            let end = full_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );
            refs.push(BlockRef::new(
                arena_alloc_str(arena, uuid_match.as_str()),
                range,
            ));
        }
    }

    refs
}

/// Extract callouts
pub fn extract_callouts<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<Callout<'a>> {
    let mut callouts = Vec::new();

    for captures in CALLOUT_RE.captures_iter(source) {
        if let (Some(type_match), title_match) = (captures.get(1), captures.get(2)) {
            let callout_type = arena_alloc_str(arena, type_match.as_str());
            let title: Option<&'a str> = title_match
                .map(|m| m.as_str().trim())
                .filter(|s| !s.is_empty())
                .map(|s| arena_alloc_str(arena, s));
            callouts.push(Callout::new(callout_type, title));
        }
    }

    callouts
}

/// Extract query blocks
pub fn extract_query_blocks<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<QueryBlock<'a>> {
    let mut queries = Vec::new();

    for captures in QUERY_BLOCK_RE.captures_iter(source) {
        if let Some(query_match) = captures.get(1) {
            queries.push(QueryBlock::new(arena_alloc_str(
                arena,
                query_match.as_str().trim(),
            )));
        }
    }

    queries
}
