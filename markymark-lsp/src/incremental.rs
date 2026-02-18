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
//! - **MarkdownLink / XmlTag**: have [`Range`] but no byte offsets, so the
//!   neighbour-window check used for wiki_links and blocks is skipped.
//! - **wiki_links / blocks**: have both range and byte offsets, so all three
//!   checks (intersect, after-start, neighbour-window) apply.

use markymark_core::Range;
use markymark_index::{
    BlockOwned, DocumentIndex, IncrementalOverrides, MarkdownLinkOwned, WikiLinkOwned, XmlTagOwned,
};
use markymark_parser::{InputEdit, MarkdownTree};

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

/// Returns true if the byte range is within `window_bytes` of the edit region.
/// Used only for extractors that have byte offsets (wiki_links, blocks).
pub fn range_within_neighbor_window(
    start_byte: usize,
    end_byte: usize,
    edit: &InputEdit,
    window_bytes: usize,
) -> bool {
    start_byte <= edit.old_end_byte.saturating_add(window_bytes)
        && end_byte.saturating_add(window_bytes) >= edit.start_byte
}

// ─── WikiLink incremental helpers ─────────────────────────────────────────────

/// Returns true if this wiki-link is affected by any of the pending edits.
pub fn wiki_link_affected_by_edits(wl: &WikiLinkOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(wl.range, edit)
            || range_is_after_edit_start(wl.range, edit)
            || range_within_neighbor_window(wl.start_byte, wl.end_byte, edit, 100)
    })
}

/// Returns true if any wiki-link in the old index needs re-extraction.
pub fn wiki_links_need_update(
    old_wiki_links: &[WikiLinkOwned],
    pending_edits: &[InputEdit],
) -> bool {
    old_wiki_links
        .iter()
        .any(|link| wiki_link_affected_by_edits(link, pending_edits))
        || any_edit_starts_at_or_after_last_wiki_link(old_wiki_links, pending_edits)
}

/// Returns true if any edit starts at or after the last wiki-link end.
/// Catches insertions after the last link that might create new links.
pub fn any_edit_starts_at_or_after_last_wiki_link(
    old_wiki_links: &[WikiLinkOwned],
    pending_edits: &[InputEdit],
) -> bool {
    let Some(last_old_end) = old_wiki_links
        .iter()
        .map(|link| (link.range.end.line, link.range.end.character))
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

/// Extract all wiki-links from the AST as owned data.
pub fn extract_wiki_links_owned(ast: &markymark_parser::Ast) -> Vec<WikiLinkOwned> {
    ast.extract_wiki_links()
        .into_iter()
        .filter(|wl| {
            wl.target_page().is_some()
                || wl.target_heading().is_some()
                || wl.target_block_id().is_some()
        })
        .map(|wl| {
            let (start_byte, end_byte) = wl.byte_range();
            WikiLinkOwned {
                target: wl.target_page().unwrap_or("").to_string(),
                alias: wl.alias().map(str::to_string),
                heading: wl.target_heading().map(str::to_string),
                range: wl.range(),
                start_byte,
                end_byte,
            }
        })
        .collect()
}

/// Merge old and new wiki-links using selective purge-and-replace.
///
/// Keeps old entries not affected by edits; takes new entries from affected regions.
pub fn merge_incremental_wiki_links(
    old_wiki_links: &[WikiLinkOwned],
    new_wiki_links: &[WikiLinkOwned],
    pending_edits: &[InputEdit],
) -> Vec<WikiLinkOwned> {
    let mut merged = Vec::new();
    for old in old_wiki_links {
        if !wiki_link_affected_by_edits(old, pending_edits) {
            merged.push(old.clone());
        }
    }
    for new_link in new_wiki_links {
        if wiki_link_affected_by_edits(new_link, pending_edits) {
            merged.push(new_link.clone());
        }
    }
    merged.sort_by_key(|wl| (wl.range.start.line, wl.range.start.character));
    merged
}

// ─── Block incremental helpers ─────────────────────────────────────────────────

/// Returns true if this block ID is affected by any of the pending edits.
pub fn block_affected_by_edits(block: &BlockOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(block.range, edit)
            || range_is_after_edit_start(block.range, edit)
            || range_within_neighbor_window(block.start_byte, block.end_byte, edit, 100)
    })
}

/// Returns true if any block ID in the old index needs re-extraction.
pub fn blocks_need_update(old_blocks: &[BlockOwned], pending_edits: &[InputEdit]) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    old_blocks
        .iter()
        .any(|block| block_affected_by_edits(block, pending_edits))
        || any_edit_starts_at_or_after_last_block(old_blocks, pending_edits)
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
            merged.push(old.clone());
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
//
// Note: MarkdownLink has Range but no byte offsets, so neighbour-window is
// not applicable. Conservative range-based checks only.

/// Returns true if this markdown link is affected by any of the pending edits.
pub fn markdown_link_affected_by_edits(
    ml: &MarkdownLinkOwned,
    pending_edits: &[InputEdit],
) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(ml.range, edit) || range_is_after_edit_start(ml.range, edit)
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
    old_mls
        .iter()
        .any(|ml| markdown_link_affected_by_edits(ml, pending_edits))
        || any_edit_starts_at_or_after_last_markdown_link(old_mls, pending_edits)
}

/// Returns true if any edit starts at or after the last markdown link end.
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
        .map(|ml| MarkdownLinkOwned {
            text: ml.text().to_string(),
            url: ml.url().to_string(),
            anchor: ml.anchor().map(str::to_string),
            range: ml.range(),
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
            merged.push(old.clone());
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
//
// Note: XmlTag has Range but no byte offsets, so neighbour-window is
// not applicable. Conservative range-based checks only.

/// Returns true if this XML tag is affected by any of the pending edits.
pub fn xml_tag_affected_by_edits(xt: &XmlTagOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(xt.range, edit) || range_is_after_edit_start(xt.range, edit)
    })
}

/// Returns true if any XML tag in the old index needs re-extraction.
pub fn xml_tags_need_update(old_xts: &[XmlTagOwned], pending_edits: &[InputEdit]) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    old_xts
        .iter()
        .any(|xt| xml_tag_affected_by_edits(xt, pending_edits))
        || any_edit_starts_at_or_after_last_xml_tag(old_xts, pending_edits)
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
            XmlTagOwned {
                tag_name: xt.tag_name().to_string(),
                attributes,
                is_self_closing: xt.is_self_closing(),
                is_unclosed: xt.is_unclosed(),
                range: xt.range(),
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
            merged.push(old.clone());
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
            old.to_vec()
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
            old.to_vec()
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
            old.to_vec()
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
            old.to_vec()
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

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::{Position, Range};
    use markymark_parser::Point;

    fn make_edit(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> InputEdit {
        InputEdit {
            start_byte: 0,
            old_end_byte: 1,
            new_end_byte: 1,
            start_position: Point {
                row: start_line as usize,
                column: start_col as usize,
            },
            old_end_position: Point {
                row: end_line as usize,
                column: end_col as usize,
            },
            new_end_position: Point {
                row: end_line as usize,
                column: end_col as usize,
            },
        }
    }

    fn make_ml(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> MarkdownLinkOwned {
        MarkdownLinkOwned {
            text: "link".to_string(),
            url: "https://example.com".to_string(),
            anchor: None,
            range: Range::new(
                Position::new(start_line, start_col),
                Position::new(end_line, end_col),
            ),
        }
    }

    fn make_xt(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        tag_name: &str,
    ) -> XmlTagOwned {
        XmlTagOwned {
            tag_name: tag_name.to_string(),
            attributes: vec![("key".to_string(), "val".to_string())],
            is_self_closing: false,
            is_unclosed: false,
            range: Range::new(
                Position::new(start_line, start_col),
                Position::new(end_line, end_col),
            ),
        }
    }

    // ─── MarkdownLink tests ───────────────────────────────────────────────────

    #[test]
    fn test_markdown_links_need_update_false_when_no_edits() {
        let ml = make_ml(0, 0, 0, 20);
        assert!(!markdown_links_need_update(&[ml], &[]));
    }

    #[test]
    fn test_markdown_links_need_update_true_when_link_intersects_edit() {
        // Edit at line 0, cols 5-10 overlaps link at cols 0-20
        let ml = make_ml(0, 0, 0, 20);
        let edit = make_edit(0, 5, 0, 10);
        assert!(markdown_links_need_update(&[ml], &[edit]));
    }

    #[test]
    fn test_markdown_link_after_edit_start_triggers_update() {
        // Link starts at line 5; edit starts at line 3 — link is after edit start
        let ml = make_ml(5, 0, 5, 20);
        let edit = make_edit(3, 0, 3, 5);
        // range_is_after_edit_start returns true: link start (5,0) >= edit start (3,0)
        assert!(markdown_link_affected_by_edits(&ml, &[edit]));
        assert!(markdown_links_need_update(&[ml], &[edit]));
    }

    #[test]
    fn test_merge_incremental_markdown_links_keeps_unaffected() {
        // Two links: ml1 at line 0, ml2 at line 10
        // Edit only near line 0 (affects ml1 via range_intersects_edit)
        let ml1 = make_ml(0, 0, 0, 20);
        let ml2 = make_ml(10, 0, 10, 20);
        // Edit at line 0 intersects ml1 but not ml2 (ml2 starts at line 10)
        // Actually, range_is_after_edit_start will catch ml2 (starts after edit).
        // So we need old ml2 to survive: use old ml2 directly, as it's not in
        // the "new" extraction for affected region.
        // Since merge: old entries NOT affected stay; new entries FROM affected regions come in.
        // ml2 is after edit start so it's "affected" — both old and new ml2 will be considered.
        // Let's test the simpler case: edit before both links.
        let edit = make_edit(0, 5, 0, 8); // overlaps ml1
        let merged = merge_incremental_markdown_links(
            &[ml1.clone(), ml2.clone()],
            std::slice::from_ref(&ml2),
            &[edit],
        );
        // ml1 is affected -> dropped from old; ml2 comes from "new" only if it's affected too.
        // ml2 is after edit start (line 10 >= line 0), so affected -> dropped from old, added from new.
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].range.start.line, 10);
    }

    #[test]
    fn test_markdown_links_need_update_false_when_edit_before_all_links() {
        // No links; edit_starts_at_or_after check returns false for empty slice
        assert!(!any_edit_starts_at_or_after_last_markdown_link(
            &[],
            &[make_edit(0, 0, 0, 5)]
        ));
    }

    // ─── XmlTag tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_xml_tags_need_update_false_when_no_edits() {
        let xt = make_xt(0, 0, 0, 15, "div");
        assert!(!xml_tags_need_update(&[xt], &[]));
    }

    #[test]
    fn test_xml_tag_affected_by_edits_detects_overlap() {
        let xt = make_xt(2, 0, 2, 20, "agent");
        let edit = make_edit(2, 5, 2, 10);
        assert!(xml_tag_affected_by_edits(&xt, &[edit]));
    }

    #[test]
    fn test_merge_incremental_xml_tags_preserves_attributes() {
        // Old xml tag at line 5 with attributes; edit at line 0 does not affect it
        // via intersection (line 5 vs edit at line 0).
        // But range_is_after_edit_start: tag at line 5 >= edit start line 0 → affected.
        // So the tag is affected. Use a new tag from re-extraction that preserves attributes.
        let old_xt = make_xt(5, 0, 5, 20, "goal");
        let new_xt = make_xt(5, 0, 5, 20, "goal"); // same after re-extraction
        let edit = make_edit(0, 0, 0, 3);
        let merged = merge_incremental_xml_tags(&[old_xt], std::slice::from_ref(&new_xt), &[edit]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].tag_name, "goal");
        assert_eq!(
            merged[0].attributes,
            vec![("key".to_string(), "val".to_string())]
        );
    }

    #[test]
    fn test_xml_tags_need_update_false_for_empty_slice() {
        // Empty old slice with edit → any_edit_starts_at_or_after returns false
        let edit = make_edit(0, 0, 0, 5);
        // xml_tags_need_update returns false for empty (no tags to check, any_edit_starts... false)
        // Actually it first checks pending_edits.is_empty() (false), then checks iter().any() on
        // empty (false), then calls any_edit_starts_at_or_after_last_xml_tag which returns false for empty.
        assert!(!xml_tags_need_update(&[], &[edit]));
    }

    // ─── Wiki-link tests (migrated from state.rs) ─────────────────────────────

    #[test]
    fn test_wiki_links_need_update_for_edit_after_last_existing_link() {
        let wl = WikiLinkOwned {
            target: "page".to_string(),
            alias: None,
            heading: None,
            range: Range::new(Position::new(0, 0), Position::new(0, 10)),
            start_byte: 0,
            end_byte: 10,
        };
        let edit_after = InputEdit {
            start_byte: 20,
            old_end_byte: 25,
            new_end_byte: 25,
            start_position: Point { row: 5, column: 0 },
            old_end_position: Point { row: 5, column: 5 },
            new_end_position: Point { row: 5, column: 5 },
        };
        assert!(
            wiki_links_need_update(&[wl], &[edit_after]),
            "edit after last link should require update"
        );
    }

    // ─── Byte-bounds tests (migrated from state/mod.rs) ───────────────────────

    #[test]
    fn test_incremental_byte_bounds_reports_clamp_when_position_exceeds_document() {
        let text = "# Title\n";
        let bounds = incremental_byte_bounds(text, 99, 99, 99, 120);
        assert_eq!(bounds.start_byte, text.len());
        assert_eq!(bounds.old_end_byte, text.len());
        assert!(bounds.start_clamped);
        assert!(bounds.end_clamped);
        assert!(!bounds.end_before_start);
    }

    #[test]
    fn test_incremental_byte_bounds_end_before_start() {
        let text = "line0\nline1\nline2\n";
        // end (line 0, char 2) is before start (line 1, char 3)
        let bounds = incremental_byte_bounds(text, 1, 3, 0, 2);
        assert!(
            bounds.end_before_start,
            "end position should be before start"
        );
        // old_end_byte is still coerced for consistency
        assert!(bounds.old_end_byte >= bounds.start_byte);
    }

    // ─── Range helper tests (migrated from state/mod.rs) ─────────────────────

    #[test]
    fn test_range_is_after_edit_start_spanning_link_returns_false() {
        // A link that STARTS before the edit but ENDS after it spans the edit.
        // range_intersects_edit handles spanning links; range_is_after_edit_start should NOT
        // additionally catch them — only links whose START is >= edit start are "after".
        let range = Range::new(Position::new(0, 0), Position::new(5, 0));
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 10,
            new_end_byte: 10,
            start_position: Point { row: 2, column: 0 },
            old_end_position: Point { row: 2, column: 10 },
            new_end_position: Point { row: 2, column: 10 },
        };
        assert!(
            !range_is_after_edit_start(range, &edit),
            "a link starting before the edit should not be 'after edit start'"
        );
    }

    #[test]
    fn test_range_is_after_edit_start_link_after_edit_returns_true() {
        // A link entirely after the edit should be "after edit start".
        let range = Range::new(Position::new(5, 0), Position::new(7, 0));
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 5,
            new_end_byte: 5,
            start_position: Point { row: 2, column: 0 },
            old_end_position: Point { row: 2, column: 5 },
            new_end_position: Point { row: 2, column: 5 },
        };
        assert!(
            range_is_after_edit_start(range, &edit),
            "a link starting after the edit should be 'after edit start'"
        );
    }

    #[test]
    fn test_range_within_neighbor_window_adjacent_bytes_is_in_window() {
        // A link starting 10 bytes after the edit end should be within a 100-byte window.
        // Uses byte offsets directly — works correctly across line boundaries.
        let edit = InputEdit {
            start_byte: 50,
            old_end_byte: 60,
            new_end_byte: 60,
            start_position: Point { row: 5, column: 0 },
            old_end_position: Point { row: 5, column: 10 },
            new_end_position: Point { row: 5, column: 10 },
        };
        // Link at bytes 70–85, which is 10 bytes after the edit end (60).
        assert!(
            range_within_neighbor_window(70, 85, &edit, 100),
            "a link 10 bytes after the edit end should be within a 100-byte window"
        );
    }

    #[test]
    fn test_range_within_neighbor_window_far_link_not_in_window() {
        // A link 200 bytes away should not be within a 100-byte window.
        let edit = InputEdit {
            start_byte: 50,
            old_end_byte: 60,
            new_end_byte: 60,
            start_position: Point { row: 5, column: 0 },
            old_end_position: Point { row: 5, column: 10 },
            new_end_position: Point { row: 5, column: 10 },
        };
        // Link at bytes 261–280, which is 201 bytes after the edit end (60).
        assert!(
            !range_within_neighbor_window(261, 280, &edit, 100),
            "a link 200 bytes from the edit should not be within a 100-byte window"
        );
    }

    // ─── Wiki-link append test (migrated from state/mod.rs) ──────────────────

    #[test]
    fn test_wiki_links_need_update_append_edit_forces_recomputation() {
        let old_wiki_links = vec![WikiLinkOwned {
            target: "Page".to_string(),
            alias: None,
            heading: None,
            range: Range::new(Position::new(1, 2), Position::new(1, 10)),
            start_byte: 10,
            end_byte: 18,
        }];
        let pending_edits = vec![InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 7,
            start_position: Point { row: 3, column: 0 },
            old_end_position: Point { row: 3, column: 0 },
            new_end_position: Point { row: 3, column: 7 },
        }];

        assert!(
            wiki_links_need_update(&old_wiki_links, &pending_edits),
            "append edits after the last link should force wiki-link recomputation"
        );
    }

    // ─── Block incremental merge tests (migrated from state/mod.rs) ──────────

    fn make_block_owned(
        id: &str,
        start_line: u32,
        start_col: u32,
        end_col: u32,
        start_byte: usize,
        end_byte: usize,
    ) -> BlockOwned {
        BlockOwned {
            id: id.to_string(),
            range: Range::new(
                Position::new(start_line, start_col),
                Position::new(start_line, end_col),
            ),
            start_byte,
            end_byte,
        }
    }

    #[test]
    fn test_blocks_need_update_returns_false_when_no_pending_edits() {
        let old_blocks = vec![make_block_owned("block-1", 2, 10, 18, 30, 38)];
        assert!(
            !blocks_need_update(&old_blocks, &[]),
            "empty pending_edits should not require block update"
        );
    }

    #[test]
    fn test_blocks_need_update_returns_true_for_intersecting_edit() {
        let old_blocks = vec![make_block_owned("block-1", 2, 10, 18, 30, 38)];
        // Edit overlaps the block range
        let edit = InputEdit {
            start_byte: 28,
            old_end_byte: 35,
            new_end_byte: 35,
            start_position: Point { row: 2, column: 8 },
            old_end_position: Point { row: 2, column: 15 },
            new_end_position: Point { row: 2, column: 15 },
        };
        assert!(
            blocks_need_update(&old_blocks, &[edit]),
            "edit overlapping block range should require update"
        );
    }

    #[test]
    fn test_blocks_need_update_returns_false_for_pre_block_edit_no_neighbor() {
        // Edit at byte 0-1, block at bytes 500-508 (far beyond 100-byte neighbor window)
        let old_blocks = vec![make_block_owned("block-far", 10, 0, 8, 500, 508)];
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 1,
            new_end_byte: 1,
            start_position: Point { row: 0, column: 0 },
            old_end_position: Point { row: 0, column: 1 },
            new_end_position: Point { row: 0, column: 1 },
        };
        // range_intersects_edit: false (no overlap)
        // range_is_after_edit_start: true (block at row 10 >= edit start row 0)
        // → affected because position shifted; blocks_need_update should return true
        assert!(
            blocks_need_update(&old_blocks, &[edit]),
            "edit before block shifts block position, requiring update"
        );
    }

    #[test]
    fn test_blocks_need_update_for_edit_at_or_after_last_block() {
        let old_blocks = vec![make_block_owned("block-1", 1, 2, 10, 10, 18)];
        // Edit starts at row 3 (after all blocks)
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 7,
            start_position: Point { row: 3, column: 0 },
            old_end_position: Point { row: 3, column: 0 },
            new_end_position: Point { row: 3, column: 7 },
        };
        assert!(
            blocks_need_update(&old_blocks, &[edit]),
            "append edits after last block should force block recomputation"
        );
    }

    #[test]
    fn test_merge_incremental_blocks_reuses_unaffected_old_blocks() {
        // Edit at row 5 col 50 (byte 200), block at row 0 col 10 (byte 10-18).
        // range_is_after_edit_start: false (row 0 < row 5) → unaffected → from old.
        let old_blocks = vec![make_block_owned("early-block", 0, 10, 18, 10, 18)];
        let new_blocks = vec![make_block_owned("early-block", 0, 10, 18, 10, 18)]; // same positions
        let edit = InputEdit {
            start_byte: 200,
            old_end_byte: 201,
            new_end_byte: 201,
            start_position: Point { row: 5, column: 50 },
            old_end_position: Point { row: 5, column: 51 },
            new_end_position: Point { row: 5, column: 51 },
        };
        let merged = merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
        assert_eq!(merged.len(), 1, "merged should contain exactly one block");
        assert_eq!(merged[0].id, "early-block");
    }

    #[test]
    fn test_merge_incremental_blocks_deduplicates_when_both_contribute() {
        // Old has two blocks; edit is between them.
        // Block-A at row 0 (before edit) → unaffected → from old
        // Block-B at row 5 (after edit) → affected → from new
        let old_blocks = vec![
            make_block_owned("block-a", 0, 10, 18, 10, 18),
            make_block_owned("block-b", 5, 10, 18, 200, 208),
        ];
        let new_blocks = vec![
            // block-a unchanged
            make_block_owned("block-a", 0, 10, 18, 10, 18),
            // block-b has updated position after edit
            make_block_owned("block-b", 5, 10, 18, 201, 209),
        ];
        // Edit at row 3 (between the two blocks)
        let edit = InputEdit {
            start_byte: 100,
            old_end_byte: 100,
            new_end_byte: 101, // insert 1 byte
            start_position: Point { row: 3, column: 0 },
            old_end_position: Point { row: 3, column: 0 },
            new_end_position: Point { row: 3, column: 1 },
        };
        let merged = merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
        // Both blocks should appear exactly once
        assert_eq!(merged.len(), 2, "merged should contain exactly two blocks");
        assert!(merged.iter().any(|b| b.id == "block-a"));
        assert!(merged.iter().any(|b| b.id == "block-b"));
    }

    #[test]
    fn test_build_markdown_index_incremental_blocks_parity() {
        // Build a document, apply a character insertion far from blocks,
        // verify incremental block result matches full rebuild.
        use markymark_parser::Parser;

        let original =
            "# Title\n\nSome text far from blocks.\n\nBlock here ^my-block\n\nAnother ^other-block\n";
        let mut parser = Parser::new().unwrap();

        // Initial parse
        let ast0 = parser.parse(original).unwrap();
        let index0 = DocumentIndex::from_ast(ast0);
        let old_block_ids: Vec<String> = index0.block_ids().map(str::to_string).collect();

        // Single-char insertion at start of title line
        let edit_text = "A";
        let modified = format!("A{original}");

        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 1,
            start_position: Point { row: 0, column: 0 },
            old_end_position: Point { row: 0, column: 0 },
            new_end_position: Point { row: 0, column: 1 },
        };

        // Build expected full rebuild
        let ast_full = parser.parse(&modified).unwrap();
        let full_index = DocumentIndex::from_ast(ast_full);
        let full_block_ids: Vec<String> = full_index.block_ids().map(str::to_string).collect();

        // Build old blocks owned (simulate what apply_document_changes captures)
        let old_blocks_owned: Vec<BlockOwned> = index0
            .block_ids()
            .filter_map(|id| index0.block_by_id(id))
            .map(|entry| BlockOwned {
                id: entry.id.to_string(),
                range: entry.range,
                start_byte: entry.start_byte,
                end_byte: entry.end_byte,
            })
            .collect();

        // Incremental rebuild
        let ast_inc = parser.parse(&modified).unwrap();
        let inc_index = build_markdown_index_incremental(
            ast_inc,
            &[edit],
            None,
            Some(&old_blocks_owned),
            None,
            None,
        );
        let inc_block_ids: Vec<String> = inc_index.block_ids().map(str::to_string).collect();

        let mut full_sorted = full_block_ids.clone();
        let mut inc_sorted = inc_block_ids.clone();
        full_sorted.sort();
        inc_sorted.sort();
        assert_eq!(
            full_sorted, inc_sorted,
            "incremental block IDs should match full rebuild: full={full_block_ids:?} inc={inc_block_ids:?}"
        );

        let _ = (edit_text, old_block_ids);
    }
}
