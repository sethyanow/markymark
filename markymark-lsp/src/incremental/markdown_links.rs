//! Incremental update helpers for markdown-link entries.

use markymark_index::MarkdownLinkOwned;
use markymark_parser::InputEdit;

use super::{
    adjust_bytes_after_edit, adjust_range_after_edit, any_edit_in_entry_gap, range_intersects_edit,
    range_is_after_edit_end, range_within_neighbor_window,
};

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
