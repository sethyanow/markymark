//! Helper functions for document indexing.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use super::types::*;

use markymark_core::arena::arena_alloc_str;

/// Convert an owned frontmatter value to an arena-allocated entry.
pub(super) fn owned_value_to_arena<'arena>(
    value: FrontmatterValueOwned,
    arena: &'arena Bump,
) -> FrontmatterValueEntry<'arena> {
    match value {
        FrontmatterValueOwned::String(s) => {
            FrontmatterValueEntry::String(arena_alloc_str(arena, &s))
        }
        FrontmatterValueOwned::Integer(n) => FrontmatterValueEntry::Integer(n),
        FrontmatterValueOwned::Float(f) => FrontmatterValueEntry::Float(f),
        FrontmatterValueOwned::Boolean(b) => FrontmatterValueEntry::Boolean(b),
        FrontmatterValueOwned::List(items) => {
            let mut list = BumpVec::new_in(arena);
            for item in items {
                list.push(owned_value_to_arena(item, arena));
            }
            FrontmatterValueEntry::List(list.into_bump_slice())
        }
        FrontmatterValueOwned::Map(entries) => {
            let mut map = BumpVec::new_in(arena);
            for (k, v) in entries {
                map.push((arena_alloc_str(arena, &k), owned_value_to_arena(v, arena)));
            }
            FrontmatterValueEntry::Map(map.into_bump_slice())
        }
        FrontmatterValueOwned::Null => FrontmatterValueEntry::Null,
    }
}

/// Convert heading text to a URL-safe slug.
pub fn slugify(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut slug = String::with_capacity(lower.len());

    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '-' {
            slug.push(ch);
        } else if ch == ' ' {
            slug.push('-');
        }
        // Other non-alphanumeric chars are stripped entirely
    }

    // Collapse consecutive dashes
    let mut result = String::with_capacity(slug.len());
    let mut prev_dash = false;
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(ch);
            prev_dash = false;
        }
    }

    // Trim dashes from start/end
    result.trim_matches('-').to_string()
}

/// Build flat TOC entries with depth calculation.
pub(crate) fn build_toc<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> &'arena [TocEntry<'arena>] {
    let mut toc = BumpVec::new_in(arena);
    let mut level_stack: Vec<u8> = Vec::new();

    for h in headings {
        while let Some(&top) = level_stack.last() {
            if top >= h.level {
                level_stack.pop();
            } else {
                break;
            }
        }

        let depth = level_stack.len();
        level_stack.push(h.level);

        toc.push(TocEntry {
            text: h.text,
            slug: h.slug,
            level: h.level,
            depth,
        });
    }

    toc.into_bump_slice()
}

#[derive(Debug, Clone)]
struct TempOutline<'arena> {
    heading: Option<HeadingEntry<'arena>>,
    children: Vec<TempOutline<'arena>>,
}

fn get_temp_node_mut<'tree, 'arena>(
    root: &'tree mut TempOutline<'arena>,
    path: &[usize],
) -> &'tree mut TempOutline<'arena> {
    let mut current = root;
    for &idx in path {
        current = &mut current.children[idx];
    }
    current
}

fn freeze_outline<'arena>(arena: &'arena Bump, node: TempOutline<'arena>) -> OutlineNode<'arena> {
    let mut children = BumpVec::new_in(arena);
    for child in node.children {
        children.push(freeze_outline(arena, child));
    }

    OutlineNode {
        heading: node.heading,
        children: children.into_bump_slice(),
    }
}

/// Build outline tree from heading entries.
pub(crate) fn build_outline<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> OutlineNode<'arena> {
    let mut root = TempOutline {
        heading: None,
        children: Vec::new(),
    };

    // Stack entries are (heading level, path of child indices from root).
    let mut stack: Vec<(u8, Vec<usize>)> = Vec::new();

    for h in headings {
        let node = TempOutline {
            heading: Some(h.clone()),
            children: Vec::new(),
        };

        while let Some((lvl, _)) = stack.last() {
            if *lvl >= h.level {
                stack.pop();
            } else {
                break;
            }
        }

        if stack.is_empty() {
            root.children.push(node);
            let idx = root.children.len() - 1;
            stack.push((h.level, vec![idx]));
        } else {
            let parent_path = stack.last().expect("stack not empty").1.clone();
            let parent = get_temp_node_mut(&mut root, &parent_path);
            parent.children.push(node);
            let child_idx = parent.children.len() - 1;

            let mut child_path = parent_path;
            child_path.push(child_idx);
            stack.push((h.level, child_path));
        }
    }

    freeze_outline(arena, root)
}

/// Extract alias strings from a frontmatter value.
///
/// Accepts String (single alias) or List (multiple aliases, string items only).
/// Non-string types (Integer, Float, Boolean, Null, Map) and non-string list
/// items are silently ignored — aliases are always strings in practice.
fn collect_alias_strings(value: &FrontmatterValueOwned, aliases: &mut Vec<String>) {
    match value {
        FrontmatterValueOwned::String(s) => {
            if !s.is_empty() {
                aliases.push(s.clone());
            }
        }
        FrontmatterValueOwned::List(items) => {
            for item in items {
                if let FrontmatterValueOwned::String(s) = item {
                    if !s.is_empty() {
                        aliases.push(s.clone());
                    }
                }
            }
        }
        _ => {} // Non-string/list types ignored for aliases
    }
}

/// Find the earliest frontmatter close delimiter (`\n---\n` or `\n---\r\n`).
///
/// Returns `(byte_position, delimiter_len)` or `None`. Using `min()` instead
/// of `or_else()` prevents picking a later CRLF close over an earlier LF close
/// in mixed-ending files.
fn find_frontmatter_close(rest: &str) -> Option<(usize, usize)> {
    // Empty frontmatter at EOF (rest is just the closing delimiter)
    if rest == "---" || rest == "---\r" {
        return Some((0, rest.len()));
    }

    // Empty frontmatter with trailing newline (rest starts with closing delimiter).
    // CRLF check MUST come before LF check ("---\r\n".starts_with("---\n") is false,
    // but ordering is safer for consistency with the rest of this function).
    if rest.starts_with("---\r\n") {
        return Some((0, 5));
    }
    if rest.starts_with("---\n") {
        return Some((0, 4));
    }

    let lf = rest.find("\n---\n").map(|p| (p, 5));
    let crlf = rest.find("\n---\r\n").map(|p| (p, 6));
    // Handle closing --- at EOF without trailing newline
    let eof = if rest.ends_with("\r\n---") {
        Some((rest.len() - 5, 5)) // content_end, delimiter_len (includes \r\n)
    } else if rest.ends_with("\n---") {
        Some((rest.len() - 4, 4)) // content_end, delimiter_len (includes \n)
    } else {
        None
    };

    [lf, crlf, eof]
        .into_iter()
        .flatten()
        .min_by_key(|(pos, _)| *pos)
}

/// Parse frontmatter from raw markdown source text as owned data.
///
/// Standalone parser that doesn't require a tree-sitter AST. Replicates
/// the simple YAML parsing from `markymark_parser::extract_frontmatter`.
/// Returns `(frontmatter_entries, aliases)`.
pub fn parse_frontmatter_owned(source: &str) -> (Vec<FrontmatterOwnedEntry>, Vec<String>) {
    // Check for YAML frontmatter delimiters (LF or CRLF)
    let rest = if let Some(r) = source.strip_prefix("---\r\n") {
        r
    } else if let Some(r) = source.strip_prefix("---\n") {
        r
    } else {
        return (Vec::new(), Vec::new());
    };

    let yaml_content = match find_frontmatter_close(rest) {
        Some((end_pos, _)) => &rest[..end_pos],
        None => return (Vec::new(), Vec::new()),
    };

    let mut frontmatter = Vec::new();
    let mut aliases = Vec::new();

    for line in yaml_content.lines() {
        let mut parts = line.splitn(2, ':');
        if let (Some(raw_key), Some(raw_value)) = (parts.next(), parts.next()) {
            let key = raw_key.trim().to_string();
            if key.is_empty() {
                continue;
            }
            let value_str = raw_value.trim();

            let value = if value_str.starts_with('[') && value_str.ends_with(']') {
                let inner = &value_str[1..value_str.len() - 1];
                let items: Vec<FrontmatterValueOwned> = inner
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(scalar_to_owned)
                    .collect();
                FrontmatterValueOwned::List(items)
            } else {
                scalar_to_owned(value_str)
            };

            if key == "aliases" {
                collect_alias_strings(&value, &mut aliases);
            }

            frontmatter.push(FrontmatterOwnedEntry { key, value });
        }
    }

    (frontmatter, aliases)
}

/// Convert a raw scalar string to an owned typed value using the shared detection functions.
fn scalar_to_owned(raw: &str) -> FrontmatterValueOwned {
    use markymark_parser::{detect_yaml_scalar, strip_yaml_quotes, YamlScalarHint};
    let (stripped, was_quoted) = strip_yaml_quotes(raw);
    if was_quoted {
        return FrontmatterValueOwned::String(stripped.to_string());
    }
    match detect_yaml_scalar(stripped) {
        YamlScalarHint::Null => FrontmatterValueOwned::Null,
        YamlScalarHint::Boolean(b) => FrontmatterValueOwned::Boolean(b),
        YamlScalarHint::Integer => match stripped.parse::<i64>() {
            Ok(n) => FrontmatterValueOwned::Integer(n),
            Err(_) => FrontmatterValueOwned::String(stripped.to_string()),
        },
        YamlScalarHint::Float => match stripped.parse::<f64>() {
            Ok(f) if f.is_finite() => FrontmatterValueOwned::Float(f),
            _ => FrontmatterValueOwned::String(stripped.to_string()),
        },
        YamlScalarHint::Str => FrontmatterValueOwned::String(stripped.to_string()),
    }
}

/// Mask YAML frontmatter so md4c doesn't misparse `---` as a setext heading.
///
/// Replaces all non-newline bytes in the `---\n...\n---\n` block with spaces,
/// preserving line counting and byte offsets for the scan backend. Returns the
/// original string unchanged if no frontmatter is present.
pub fn mask_frontmatter(source: &str) -> String {
    // Handle both LF and CRLF line endings
    let (prefix_len, rest) = if let Some(r) = source.strip_prefix("---\r\n") {
        (5, r)
    } else if let Some(r) = source.strip_prefix("---\n") {
        (4, r)
    } else {
        return source.to_string();
    };
    let (close_pos, close_len) = match find_frontmatter_close(rest) {
        Some(v) => v,
        None => return source.to_string(),
    };
    let fm_end = prefix_len + close_pos + close_len;
    let mut bytes: Vec<u8> = source.bytes().collect();
    for b in &mut bytes[..fm_end] {
        if *b != b'\n' && *b != b'\r' {
            *b = b' ';
        }
    }
    // Replacing every non-newline byte with 0x20 (space) always produces valid
    // UTF-8: multi-byte sequences have all bytes replaced, yielding ASCII spaces.
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_string())
}

/// Return the byte offset where the frontmatter region ends (exclusive).
///
/// Returns 0 if the source has no frontmatter. Used by content block extraction
/// to exclude blocks whose start_byte falls within the frontmatter region.
pub fn frontmatter_byte_end(source: &str) -> usize {
    let (prefix_len, rest) = if let Some(r) = source.strip_prefix("---\r\n") {
        (5, r)
    } else if let Some(r) = source.strip_prefix("---\n") {
        (4, r)
    } else {
        return 0;
    };
    match find_frontmatter_close(rest) {
        Some((close_pos, close_len)) => prefix_len + close_pos + close_len,
        None => 0,
    }
}
