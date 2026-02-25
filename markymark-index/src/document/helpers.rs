//! Helper functions for document indexing.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use std::collections::HashMap as StdHashMap;

use super::types::*;

use markymark_core::prelude::Position;

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

/// Deduplicate a slug given a set of already-used slugs.
pub(super) fn dedup_slug(base: &str, used: &mut StdHashMap<String, usize>) -> String {
    let count = used.entry(base.to_string()).or_insert(0);
    let slug = if *count == 0 {
        base.to_string()
    } else {
        format!("{}-{}", base, count)
    };
    *count += 1;
    slug
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

// ---------------------------------------------------------------------------
// Byte-offset to Position helpers (for scan-based construction)
// ---------------------------------------------------------------------------

/// Build a sorted list of byte offsets where each line starts.
/// Line 0 starts at offset 0. Line N starts after the N-th newline.
pub(super) fn byte_offset_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

/// Convert a byte offset to a Position (0-based line, 0-based character).
pub(super) fn byte_offset_to_position(line_starts: &[u32], offset: u32) -> Position {
    let line = match line_starts.binary_search(&offset) {
        Ok(exact) => exact,
        Err(insert) => insert - 1,
    };
    let col = offset - line_starts[line];
    Position::new(line as u32, col)
}

/// Extract frontmatter from a parsed AST as owned data.
///
/// Returns `(frontmatter_entries, aliases)` where aliases are extracted from the
/// `aliases` frontmatter key. Used by both `from_ast` and `from_scan_with_frontmatter`.
pub(super) fn extract_frontmatter_from_ast(
    ast: &markymark_parser::Ast,
) -> (Vec<FrontmatterOwnedEntry>, Vec<String>) {
    let mut frontmatter_owned = Vec::new();
    let mut aliases_owned = Vec::new();

    if let Some(fm) = ast.frontmatter() {
        use markymark_parser::FrontmatterValue;
        for (key, value) in fm.iter() {
            let key_str = (*key).to_string();
            let value_owned = match value {
                FrontmatterValue::String(s) => FrontmatterValueOwned::String((*s).to_string()),
                FrontmatterValue::List(items) => {
                    FrontmatterValueOwned::List(items.iter().map(|s| s.to_string()).collect())
                }
            };
            if key_str == "aliases" {
                match &value_owned {
                    FrontmatterValueOwned::String(s) => {
                        if !s.is_empty() {
                            aliases_owned.push(s.clone());
                        }
                    }
                    FrontmatterValueOwned::List(items) => {
                        aliases_owned.extend(items.iter().cloned());
                    }
                }
            }
            frontmatter_owned.push(FrontmatterOwnedEntry {
                key: key_str,
                value: value_owned,
            });
        }
    }

    (frontmatter_owned, aliases_owned)
}

/// Find the earliest frontmatter close delimiter (`\n---\n` or `\n---\r\n`).
///
/// Returns `(byte_position, delimiter_len)` or `None`. Using `min()` instead
/// of `or_else()` prevents picking a later CRLF close over an earlier LF close
/// in mixed-ending files.
fn find_frontmatter_close(rest: &str) -> Option<(usize, usize)> {
    let lf = rest.find("\n---\n").map(|p| (p, 5));
    let crlf = rest.find("\n---\r\n").map(|p| (p, 6));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a @ Some(_), None) => a,
        (None, b @ Some(_)) => b,
        (None, None) => None,
    }
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
                let items: Vec<String> = inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                FrontmatterValueOwned::List(items)
            } else {
                FrontmatterValueOwned::String(value_str.to_string())
            };

            if key == "aliases" {
                match &value {
                    FrontmatterValueOwned::String(s) => {
                        if !s.is_empty() {
                            aliases.push(s.clone());
                        }
                    }
                    FrontmatterValueOwned::List(items) => {
                        aliases.extend(items.iter().cloned());
                    }
                }
            }

            frontmatter.push(FrontmatterOwnedEntry { key, value });
        }
    }

    (frontmatter, aliases)
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
