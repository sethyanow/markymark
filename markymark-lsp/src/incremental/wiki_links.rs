//! Incremental update helpers for wiki-link entries.

use markymark_index::WikiLinkOwned;
use markymark_parser::InputEdit;

use super::{
    adjust_bytes_after_edit, adjust_range_after_edit, any_edit_in_entry_gap, range_intersects_edit,
    range_is_after_edit_end, range_within_neighbor_window, range_within_new_end_window,
};

/// Returns true if this wiki-link is affected by any of the pending edits.
///
/// Only entries that directly intersect the edit or are within the byte-level
/// neighbor window need re-extraction. Entries merely *after* the edit are
/// retained with adjusted positions instead of being re-extracted.
pub fn wiki_link_affected_by_edits(wl: &WikiLinkOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(wl.range, edit)
            || range_within_neighbor_window(wl.start_byte, wl.end_byte, edit, 100)
    })
}

/// Returns true if any wiki-link in the old index needs re-extraction.
pub fn wiki_links_need_update(
    old_wiki_links: &[WikiLinkOwned],
    pending_edits: &[InputEdit],
) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    let byte_ranges: Vec<(usize, usize)> = old_wiki_links
        .iter()
        .map(|link| (link.start_byte, link.end_byte))
        .collect();
    old_wiki_links
        .iter()
        .any(|link| wiki_link_affected_by_edits(link, pending_edits))
        || any_edit_starts_at_or_after_last_wiki_link(old_wiki_links, pending_edits)
        || any_edit_in_entry_gap(&byte_ranges, pending_edits, 100)
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
/// Keeps old entries not affected by edits (with position adjustment for entries
/// after the edit); takes new entries from affected regions.
pub fn merge_incremental_wiki_links(
    old_wiki_links: &[WikiLinkOwned],
    new_wiki_links: &[WikiLinkOwned],
    pending_edits: &[InputEdit],
) -> Vec<WikiLinkOwned> {
    let mut merged = Vec::new();
    for old in old_wiki_links {
        if !wiki_link_affected_by_edits(old, pending_edits) {
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
    for new_link in new_wiki_links {
        if wiki_link_affected_by_edits(new_link, pending_edits)
            || pending_edits.iter().any(|edit| {
                range_within_new_end_window(new_link.start_byte, new_link.end_byte, edit, 100)
            })
        {
            merged.push(new_link.clone());
        }
    }
    merged.sort_by_key(|wl| (wl.range.start.line, wl.range.start.character));
    merged
}
