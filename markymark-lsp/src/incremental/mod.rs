//! Incremental index update helpers for the LSP state.
//!
//! These functions implement range-based selective re-extraction for the five
//! independent markdown extractors (wiki_links, blocks, tags, markdown_links,
//! xml_tags). When a `did_change` event arrives, the LSP accumulates
//! [`InputEdit`] ranges and passes them here to decide which extractors need
//! re-running and how to merge old and new data.
//!
//! ## Design invariants
//!
//! - **Tags always full-rebuild**: [`Tag`][markymark_parser] carries no source
//!   range, so incremental optimisation is impossible. Always pass `None` for
//!   tags in [`IncrementalOverrides`].
//! - **All four non-heading extractors** (wiki_links, blocks, markdown_links,
//!   xml_tags) carry both [`Range`] and byte offsets (`start_byte`/`end_byte`).
//!   All four use the same three-check incremental pattern: `range_intersects_edit`
//!   || `range_within_neighbor_window` || `any_edit_starts_at_or_after_last_*`.

use markymark_core::Range;
use markymark_index::{
    BlockOwned, DocumentIndex, IncrementalOverrides, MarkdownLinkOwned, WikiLinkOwned, XmlTagOwned,
};
use markymark_parser::{InputEdit, MarkdownTree};

mod wiki_links;
pub use wiki_links::*;

// ─── Byte-offset helpers (used by state.rs for apply_document_changes) ───────

/// Byte bounds computed from a LSP incremental-change range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncrementalByteBounds {
    /// Byte offset of the start of the change.
    pub start_byte: usize,
    /// Byte offset of the old end of the change.
    pub old_end_byte: usize,
    /// True if the start position was clamped to the text length.
    pub start_clamped: bool,
    /// True if the end position was clamped to the text length.
    pub end_clamped: bool,
    /// True when the raw end position was before the raw start.
    pub end_before_start: bool,
}

/// Compute byte offsets from LSP line/character positions.
pub fn incremental_byte_bounds(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> IncrementalByteBounds {
    let raw_start_byte =
        crate::convert::lsp_position_to_byte_offset(text, start_line, start_character);
    let raw_end_byte = crate::convert::lsp_position_to_byte_offset(text, end_line, end_character);

    let end_before_start = raw_end_byte < raw_start_byte;
    let start_byte = raw_start_byte.min(text.len());
    let old_end_byte = raw_end_byte.min(text.len()).max(start_byte);

    IncrementalByteBounds {
        start_byte,
        old_end_byte,
        start_clamped: position_was_clamped(text, start_line, start_character),
        end_clamped: position_was_clamped(text, end_line, end_character),
        end_before_start,
    }
}

/// Returns true if an LSP position was clamped (beyond the actual text).
pub fn position_was_clamped(text: &str, line: u32, character: u32) -> bool {
    let target_line = line as usize;
    let target_character = character as usize;
    let Some(line_text) = text.split('\n').nth(target_line) else {
        return true;
    };
    let content = line_text.strip_suffix('\r').unwrap_or(line_text);
    target_character > content.encode_utf16().count()
}

// ─── Range helpers ─────────────────────────────────────────────────────────────

/// Returns true if the given range overlaps with the edit region.
pub fn range_intersects_edit(range: Range, edit: &InputEdit) -> bool {
    let range_start = (range.start.line, range.start.character);
    let range_end = (range.end.line, range.end.character);
    let edit_start = (
        edit.start_position.row as u32,
        edit.start_position.column as u32,
    );
    let edit_end = (
        edit.old_end_position.row as u32,
        edit.old_end_position.column as u32,
    );
    range_start < edit_end && range_end > edit_start
}

/// Returns true if the range starts at or after the edit start (conservative:
/// anything at or after the edit must be re-validated).
pub fn range_is_after_edit_start(range: Range, edit: &InputEdit) -> bool {
    let range_start = (range.start.line, range.start.character);
    let edit_start = (
        edit.start_position.row as u32,
        edit.start_position.column as u32,
    );
    range_start >= edit_start
}

/// Returns true if any edit falls in a region not covered by any entry's byte
/// range extended by `window_bytes`.
///
/// This catches insertions in large gaps between consecutive entries (or before
/// the first entry) where the fixed-size neighbor window doesn't reach.
///
/// Complexity: O(edits × entries), both typically small.
pub fn any_edit_in_entry_gap(
    entry_byte_ranges: &[(usize, usize)],
    pending_edits: &[InputEdit],
    window_bytes: usize,
) -> bool {
    if entry_byte_ranges.is_empty() {
        // Caller handles the empty-entries case with a full rebuild
        // (build_markdown_index_incremental checks old.is_empty() first).
        return false;
    }
    pending_edits.iter().any(|edit| {
        !entry_byte_ranges.iter().any(|&(start, end)| {
            start <= edit.old_end_byte.saturating_add(window_bytes)
                && end.saturating_add(window_bytes) >= edit.start_byte
        })
    })
}

/// Returns true if the byte range is within `window_bytes` of the edit region.
/// Used for extractors that have byte offsets (e.g., wiki_links, markdown links, XML tags, blocks).
pub fn range_within_neighbor_window(
    start_byte: usize,
    end_byte: usize,
    edit: &InputEdit,
    window_bytes: usize,
) -> bool {
    start_byte <= edit.old_end_byte.saturating_add(window_bytes)
        && end_byte.saturating_add(window_bytes) >= edit.start_byte
}

/// Returns true if the range starts at or after the edit's old end position.
/// Used to identify entries that need position adjustment (but not re-extraction).
///
/// Uses `>=` (not strict `>`) so that entries starting exactly at the insertion
/// point of a zero-width edit (where `start == old_end`) still receive position
/// adjustment. With strict `>`, such entries would get neither intersection nor
/// adjustment, leaving them with stale coordinates.
pub fn range_is_after_edit_end(range: Range, edit: &InputEdit) -> bool {
    let range_start = (range.start.line, range.start.character);
    let edit_old_end = (
        edit.old_end_position.row as u32,
        edit.old_end_position.column as u32,
    );
    range_start >= edit_old_end
}

fn saturating_add_u32_delta(value: u32, delta: i128) -> u32 {
    if delta >= 0 {
        let add = u32::try_from(delta).unwrap_or(u32::MAX);
        value.saturating_add(add)
    } else {
        let sub = u32::try_from(delta.unsigned_abs()).unwrap_or(u32::MAX);
        value.saturating_sub(sub)
    }
}

fn saturating_add_usize_delta(value: usize, delta: i128) -> usize {
    if delta >= 0 {
        let add = usize::try_from(delta).unwrap_or(usize::MAX);
        value.saturating_add(add)
    } else {
        let sub = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        value.saturating_sub(sub)
    }
}

/// Adjust a Range's line/character positions for an entry that starts after the edit's old end.
///
/// When an edit changes the document length, entries after the edit shift. This function
/// applies the line/column delta from the InputEdit to keep positions accurate.
pub fn adjust_range_after_edit(range: &mut Range, edit: &InputEdit) {
    let old_end_row = u32::try_from(edit.old_end_position.row).unwrap_or(u32::MAX);
    let line_delta = i128::try_from(edit.new_end_position.row).unwrap_or(i128::MAX)
        - i128::try_from(edit.old_end_position.row).unwrap_or(i128::MAX);

    // Adjust start position
    if range.start.line == old_end_row {
        let col_delta = i128::try_from(edit.new_end_position.column).unwrap_or(i128::MAX)
            - i128::try_from(edit.old_end_position.column).unwrap_or(i128::MAX);
        range.start.line = saturating_add_u32_delta(range.start.line, line_delta);
        range.start.character = saturating_add_u32_delta(range.start.character, col_delta);
    } else {
        range.start.line = saturating_add_u32_delta(range.start.line, line_delta);
    }

    // Adjust end position
    if range.end.line == old_end_row {
        let col_delta = i128::try_from(edit.new_end_position.column).unwrap_or(i128::MAX)
            - i128::try_from(edit.old_end_position.column).unwrap_or(i128::MAX);
        range.end.line = saturating_add_u32_delta(range.end.line, line_delta);
        range.end.character = saturating_add_u32_delta(range.end.character, col_delta);
    } else {
        range.end.line = saturating_add_u32_delta(range.end.line, line_delta);
    }
}

/// Adjust byte offsets for an entry that starts after the edit's old end.
pub fn adjust_bytes_after_edit(start_byte: &mut usize, end_byte: &mut usize, edit: &InputEdit) {
    let byte_delta = i128::try_from(edit.new_end_byte).unwrap_or(i128::MAX)
        - i128::try_from(edit.old_end_byte).unwrap_or(i128::MAX);
    *start_byte = saturating_add_usize_delta(*start_byte, byte_delta);
    *end_byte = saturating_add_usize_delta(*end_byte, byte_delta);
}

// ─── Block incremental helpers ─────────────────────────────────────────────────

/// Returns true if this block ID is affected by any of the pending edits.
///
/// Only entries that directly intersect the edit or are within the byte-level
/// neighbor window need re-extraction.
pub fn block_affected_by_edits(block: &BlockOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(block.range, edit)
            || range_within_neighbor_window(block.start_byte, block.end_byte, edit, 100)
    })
}

/// Returns true if any block ID in the old index needs re-extraction.
pub fn blocks_need_update(old_blocks: &[BlockOwned], pending_edits: &[InputEdit]) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    let byte_ranges: Vec<(usize, usize)> = old_blocks
        .iter()
        .map(|block| (block.start_byte, block.end_byte))
        .collect();
    old_blocks
        .iter()
        .any(|block| block_affected_by_edits(block, pending_edits))
        || any_edit_starts_at_or_after_last_block(old_blocks, pending_edits)
        || any_edit_in_entry_gap(&byte_ranges, pending_edits, 100)
}

/// Returns true if any edit starts at or after the last block ID end.
pub fn any_edit_starts_at_or_after_last_block(
    old_blocks: &[BlockOwned],
    pending_edits: &[InputEdit],
) -> bool {
    let Some(last_old_end) = old_blocks
        .iter()
        .map(|block| (block.range.end.line, block.range.end.character))
        .max()
    else {
        return false;
    };

    pending_edits.iter().any(|edit| {
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        edit_start >= last_old_end
    })
}

/// Extract all block IDs from the AST as owned data.
pub fn extract_blocks_owned(ast: &markymark_parser::Ast) -> Vec<BlockOwned> {
    ast.extract_block_ids()
        .into_iter()
        .map(|b| BlockOwned {
            id: b.id().to_string(),
            range: b.range(),
            start_byte: b.start_byte(),
            end_byte: b.end_byte(),
        })
        .collect()
}

/// Merge old and new block IDs using selective purge-and-replace.
pub fn merge_incremental_blocks(
    old_blocks: &[BlockOwned],
    new_blocks: &[BlockOwned],
    pending_edits: &[InputEdit],
) -> Vec<BlockOwned> {
    let mut merged = Vec::new();
    for old in old_blocks {
        if !block_affected_by_edits(old, pending_edits) {
            let mut adjusted = old.clone();
            for edit in pending_edits {
                if range_is_after_edit_end(adjusted.range, edit) {
                    adjust_range_after_edit(&mut adjusted.range, edit);
                    adjust_bytes_after_edit(&mut adjusted.start_byte, &mut adjusted.end_byte, edit);
                }
            }
            merged.push(adjusted);
        }
    }
    for new_block in new_blocks {
        if block_affected_by_edits(new_block, pending_edits) {
            merged.push(new_block.clone());
        }
    }
    merged.sort_by_key(|b| (b.range.start.line, b.range.start.character));
    merged
}

// ─── MarkdownLink incremental helpers ─────────────────────────────────────────

/// Returns true if this markdown link is affected by any of the pending edits.
///
/// Entries that directly intersect the edit or are within the byte-level
/// neighbor window need re-extraction. Entries merely *after* the edit are
/// retained with adjusted positions instead of being re-extracted.
pub fn markdown_link_affected_by_edits(
    ml: &MarkdownLinkOwned,
    pending_edits: &[InputEdit],
) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(ml.range, edit)
            || range_within_neighbor_window(ml.start_byte, ml.end_byte, edit, 100)
    })
}

/// Returns true if any markdown link in the old index needs re-extraction.
pub fn markdown_links_need_update(
    old_mls: &[MarkdownLinkOwned],
    pending_edits: &[InputEdit],
) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    let byte_ranges: Vec<(usize, usize)> = old_mls
        .iter()
        .map(|ml| (ml.start_byte, ml.end_byte))
        .collect();
    old_mls
        .iter()
        .any(|ml| markdown_link_affected_by_edits(ml, pending_edits))
        || any_edit_starts_at_or_after_last_markdown_link(old_mls, pending_edits)
        || any_edit_in_entry_gap(&byte_ranges, pending_edits, 100)
}

/// Returns true if any edit starts at or after the last markdown link end.
/// Catches insertions after the last link that might create new links.
pub fn any_edit_starts_at_or_after_last_markdown_link(
    old_mls: &[MarkdownLinkOwned],
    pending_edits: &[InputEdit],
) -> bool {
    let Some(last_old_end) = old_mls
        .iter()
        .map(|ml| (ml.range.end.line, ml.range.end.character))
        .max()
    else {
        return false;
    };

    pending_edits.iter().any(|edit| {
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        edit_start >= last_old_end
    })
}

/// Extract all markdown links from the AST as owned data.
pub fn extract_markdown_links_owned(ast: &markymark_parser::Ast) -> Vec<MarkdownLinkOwned> {
    ast.extract_markdown_links()
        .into_iter()
        .map(|ml| {
            let (start_byte, end_byte) = ml.byte_range();
            MarkdownLinkOwned {
                text: ml.text().to_string(),
                url: ml.url().to_string(),
                anchor: ml.anchor().map(str::to_string),
                range: ml.range(),
                start_byte,
                end_byte,
            }
        })
        .collect()
}

/// Merge old and new markdown links using selective purge-and-replace.
pub fn merge_incremental_markdown_links(
    old_mls: &[MarkdownLinkOwned],
    new_mls: &[MarkdownLinkOwned],
    pending_edits: &[InputEdit],
) -> Vec<MarkdownLinkOwned> {
    let mut merged = Vec::new();
    for old in old_mls {
        if !markdown_link_affected_by_edits(old, pending_edits) {
            let mut adjusted = old.clone();
            for edit in pending_edits {
                if range_is_after_edit_end(adjusted.range, edit) {
                    adjust_range_after_edit(&mut adjusted.range, edit);
                    adjust_bytes_after_edit(&mut adjusted.start_byte, &mut adjusted.end_byte, edit);
                }
            }
            merged.push(adjusted);
        }
    }
    for new_ml in new_mls {
        if markdown_link_affected_by_edits(new_ml, pending_edits) {
            merged.push(new_ml.clone());
        }
    }
    merged.sort_by_key(|ml| (ml.range.start.line, ml.range.start.character));
    merged
}

// ─── XmlTag incremental helpers ────────────────────────────────────────────────

/// Returns true if this XML tag is affected by any of the pending edits.
///
/// Entries that directly intersect the edit or are within the byte-level
/// neighbor window need re-extraction.
pub fn xml_tag_affected_by_edits(xt: &XmlTagOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(xt.range, edit)
            || range_within_neighbor_window(xt.start_byte, xt.end_byte, edit, 100)
    })
}

/// Returns true if any XML tag in the old index needs re-extraction.
pub fn xml_tags_need_update(old_xts: &[XmlTagOwned], pending_edits: &[InputEdit]) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    let byte_ranges: Vec<(usize, usize)> = old_xts
        .iter()
        .map(|xt| (xt.start_byte, xt.end_byte))
        .collect();
    old_xts
        .iter()
        .any(|xt| xml_tag_affected_by_edits(xt, pending_edits))
        || any_edit_starts_at_or_after_last_xml_tag(old_xts, pending_edits)
        || any_edit_in_entry_gap(&byte_ranges, pending_edits, 100)
}

/// Returns true if any edit starts at or after the last XML tag end.
pub fn any_edit_starts_at_or_after_last_xml_tag(
    old_xts: &[XmlTagOwned],
    pending_edits: &[InputEdit],
) -> bool {
    let Some(last_old_end) = old_xts
        .iter()
        .map(|xt| (xt.range.end.line, xt.range.end.character))
        .max()
    else {
        return false;
    };

    pending_edits.iter().any(|edit| {
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        edit_start >= last_old_end
    })
}

/// Extract all XML tags from the AST as owned data with sorted attributes.
pub fn extract_xml_tags_owned(ast: &markymark_parser::Ast) -> Vec<XmlTagOwned> {
    ast.extract_xml_tags()
        .into_iter()
        .map(|xt| {
            let mut attributes: Vec<(String, String)> = xt
                .attributes()
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect();
            attributes.sort_by(|a, b| a.0.cmp(&b.0));
            let (start_byte, end_byte) = xt.byte_range();
            XmlTagOwned {
                tag_name: xt.tag_name().to_string(),
                attributes,
                is_self_closing: xt.is_self_closing(),
                is_unclosed: xt.is_unclosed(),
                range: xt.range(),
                start_byte,
                end_byte,
            }
        })
        .collect()
}

/// Merge old and new XML tags using selective purge-and-replace.
pub fn merge_incremental_xml_tags(
    old_xts: &[XmlTagOwned],
    new_xts: &[XmlTagOwned],
    pending_edits: &[InputEdit],
) -> Vec<XmlTagOwned> {
    let mut merged = Vec::new();
    for old in old_xts {
        if !xml_tag_affected_by_edits(old, pending_edits) {
            let mut adjusted = old.clone();
            for edit in pending_edits {
                if range_is_after_edit_end(adjusted.range, edit) {
                    adjust_range_after_edit(&mut adjusted.range, edit);
                    adjust_bytes_after_edit(&mut adjusted.start_byte, &mut adjusted.end_byte, edit);
                }
            }
            merged.push(adjusted);
        }
    }
    for new_xt in new_xts {
        if xml_tag_affected_by_edits(new_xt, pending_edits) {
            merged.push(new_xt.clone());
        }
    }
    merged.sort_by_key(|xt| (xt.range.start.line, xt.range.start.character));
    merged
}

// ─── Main incremental build entry point ───────────────────────────────────────

/// Build a [`DocumentIndex`] from an already-parsed AST, reusing old extractor
/// data where edits don't affect those regions.
///
/// When `pending_edits` is empty, falls back to a full rebuild via
/// [`DocumentIndex::from_ast`].
///
/// Tags are always fully rebuilt (no source range available from parser).
pub fn build_markdown_index_incremental(
    ast: markymark_parser::Ast,
    pending_edits: &[InputEdit],
    old_wiki_links: Option<&[WikiLinkOwned]>,
    old_blocks: Option<&[BlockOwned]>,
    old_markdown_links: Option<&[MarkdownLinkOwned]>,
    old_xml_tags: Option<&[XmlTagOwned]>,
) -> DocumentIndex {
    if pending_edits.is_empty() {
        return DocumentIndex::from_ast(ast);
    }

    // Compute merged wiki-links
    let merged_wiki_links = old_wiki_links.map(|old| {
        if old.is_empty() {
            extract_wiki_links_owned(&ast)
        } else if !wiki_links_need_update(old, pending_edits) {
            // No entries affected — reuse old with position adjustment
            old.iter()
                .map(|link| {
                    let mut adj = link.clone();
                    for edit in pending_edits {
                        if range_is_after_edit_end(adj.range, edit) {
                            adjust_range_after_edit(&mut adj.range, edit);
                            adjust_bytes_after_edit(&mut adj.start_byte, &mut adj.end_byte, edit);
                        }
                    }
                    adj
                })
                .collect()
        } else {
            let new_wiki_links = extract_wiki_links_owned(&ast);
            merge_incremental_wiki_links(old, &new_wiki_links, pending_edits)
        }
    });

    // Compute merged blocks
    let merged_blocks = old_blocks.map(|old| {
        if old.is_empty() {
            extract_blocks_owned(&ast)
        } else if !blocks_need_update(old, pending_edits) {
            old.iter()
                .map(|block| {
                    let mut adj = block.clone();
                    for edit in pending_edits {
                        if range_is_after_edit_end(adj.range, edit) {
                            adjust_range_after_edit(&mut adj.range, edit);
                            adjust_bytes_after_edit(&mut adj.start_byte, &mut adj.end_byte, edit);
                        }
                    }
                    adj
                })
                .collect()
        } else {
            let new_blocks = extract_blocks_owned(&ast);
            merge_incremental_blocks(old, &new_blocks, pending_edits)
        }
    });

    // Compute merged markdown links
    let merged_markdown_links = old_markdown_links.map(|old| {
        if old.is_empty() {
            extract_markdown_links_owned(&ast)
        } else if !markdown_links_need_update(old, pending_edits) {
            old.iter()
                .map(|ml| {
                    let mut adj = ml.clone();
                    for edit in pending_edits {
                        if range_is_after_edit_end(adj.range, edit) {
                            adjust_range_after_edit(&mut adj.range, edit);
                            adjust_bytes_after_edit(&mut adj.start_byte, &mut adj.end_byte, edit);
                        }
                    }
                    adj
                })
                .collect()
        } else {
            let new_mls = extract_markdown_links_owned(&ast);
            merge_incremental_markdown_links(old, &new_mls, pending_edits)
        }
    });

    // Compute merged XML tags
    let merged_xml_tags = old_xml_tags.map(|old| {
        if old.is_empty() {
            extract_xml_tags_owned(&ast)
        } else if !xml_tags_need_update(old, pending_edits) {
            old.iter()
                .map(|xt| {
                    let mut adj = xt.clone();
                    for edit in pending_edits {
                        if range_is_after_edit_end(adj.range, edit) {
                            adjust_range_after_edit(&mut adj.range, edit);
                            adjust_bytes_after_edit(&mut adj.start_byte, &mut adj.end_byte, edit);
                        }
                    }
                    adj
                })
                .collect()
        } else {
            let new_xts = extract_xml_tags_owned(&ast);
            merge_incremental_xml_tags(old, &new_xts, pending_edits)
        }
    });

    // Tags: always None — no source range, cannot be incrementally merged
    let overrides = IncrementalOverrides {
        wiki_links: merged_wiki_links,
        blocks: merged_blocks,
        tags: None,
        markdown_links: merged_markdown_links,
        xml_tags: merged_xml_tags,
    };
    DocumentIndex::from_ast_with_overrides_opt(ast, overrides)
}

/// Parse `text` with optional tree reuse and build a markdown document index.
///
/// This is the incremental-aware entry point used by `ServerState`. When
/// `old_tree` is `Some`, tree-sitter reuses unchanged subtrees.
#[allow(clippy::too_many_arguments)]
pub fn build_markdown_index_with_old_tree(
    parser: &mut markymark_parser::Parser,
    text: &str,
    old_tree: Option<&MarkdownTree>,
    pending_edits: &[InputEdit],
    old_wiki_links: Option<&[WikiLinkOwned]>,
    old_blocks: Option<&[BlockOwned]>,
    old_markdown_links: Option<&[MarkdownLinkOwned]>,
    old_xml_tags: Option<&[XmlTagOwned]>,
) -> (DocumentIndex, Option<MarkdownTree>) {
    let mut ast = parser
        .parse_with_old_tree(text, old_tree)
        .expect("failed to parse document");
    let md_tree = ast.take_md_tree();
    let index = build_markdown_index_incremental(
        ast,
        pending_edits,
        old_wiki_links,
        old_blocks,
        old_markdown_links,
        old_xml_tags,
    );
    (index, md_tree)
}

#[cfg(test)]
mod tests;
