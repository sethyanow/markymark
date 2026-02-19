//! Incremental update helpers for block-ID entries.

use markymark_index::BlockOwned;
use markymark_parser::InputEdit;

use super::{
    adjust_bytes_after_edit, adjust_range_after_edit, any_edit_in_entry_gap, range_intersects_edit,
    range_is_after_edit_end, range_within_neighbor_window,
};

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
