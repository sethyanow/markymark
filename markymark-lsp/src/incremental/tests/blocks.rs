use super::super::*;
use super::make_block_owned;
use markymark_parser::Point;

// ─── Block incremental merge tests (migrated from state/mod.rs) ──────────

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
fn test_blocks_need_update_returns_true_for_pre_block_edit_in_gap() {
    // Edit at byte 0-1, block at bytes 500-508 (far beyond 100-byte neighbor window).
    // marky-wjf: The edit is in an uncovered gap before the first entry.
    // The edit could create new block IDs that position adjustment alone can't detect.
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
    // range_within_neighbor_window: false (500 >> 100 + 1)
    // any_edit_in_entry_gap: TRUE (edit at byte 0 is outside all entries' windows)
    assert!(
        blocks_need_update(&old_blocks, &[edit]),
        "edit in uncovered gap should trigger re-extraction (marky-wjf)"
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
fn test_merge_incremental_blocks_adjusts_positions_for_entry_after_edit() {
    // Old has two blocks; edit is between them (no intersection with either).
    // Block-A at row 0 (before edit) → unaffected, no position change
    // Block-B at row 5 (after edit, far from neighbor window) → unaffected, position-adjusted
    let old_blocks = vec![
        make_block_owned("block-a", 0, 10, 18, 10, 18),
        make_block_owned("block-b", 5, 10, 18, 500, 508), // 400 bytes from edit → outside 100-byte window
    ];
    let new_blocks = vec![
        make_block_owned("block-a", 0, 10, 18, 10, 18),
        make_block_owned("block-b", 5, 10, 18, 501, 509),
    ];
    // Edit at row 3 (between the two blocks), 1 byte insert
    let edit = InputEdit {
        start_byte: 100,
        old_end_byte: 100,
        new_end_byte: 101,
        start_position: Point { row: 3, column: 0 },
        old_end_position: Point { row: 3, column: 0 },
        new_end_position: Point { row: 3, column: 1 },
    };
    let merged = merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
    assert_eq!(merged.len(), 2, "merged should contain exactly two blocks");
    // Block-A: before edit, no change
    let block_a = merged.iter().find(|b| b.id == "block-a").unwrap();
    assert_eq!(block_a.start_byte, 10, "block-a bytes unchanged");
    assert_eq!(block_a.end_byte, 18);
    // Block-B: after edit, position adjusted by +1 byte
    let block_b = merged.iter().find(|b| b.id == "block-b").unwrap();
    assert_eq!(
        block_b.start_byte, 501,
        "block-b start_byte should shift +1 after edit"
    );
    assert_eq!(block_b.end_byte, 509, "block-b end_byte should shift +1");
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
