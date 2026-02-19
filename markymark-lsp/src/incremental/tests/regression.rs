use super::super::*;
use super::{make_block_owned, make_ml_bytes, make_xt_bytes};
use markymark_core::{Position, Range};
use markymark_parser::Point;

// ─── marky-wjf: Regression tests for gap detection ────────────────────────

/// marky-wjf: Edit in a large gap between two wiki links must trigger re-extraction.
#[test]
fn test_wiki_links_need_update_detects_edit_in_large_gap() {
    // Link A at bytes 100-150, Link B at bytes 500-550.
    // Edit at byte 300 — outside 100-byte neighbor window of both entries,
    // before last entry, no range intersection. Current code misses this.
    let wl_a = WikiLinkOwned {
        target: "PageA".to_string(),
        alias: None,
        heading: None,
        range: Range::new(Position::new(2, 0), Position::new(2, 20)),
        start_byte: 100,
        end_byte: 150,
    };
    let wl_b = WikiLinkOwned {
        target: "PageB".to_string(),
        alias: None,
        heading: None,
        range: Range::new(Position::new(10, 0), Position::new(10, 20)),
        start_byte: 500,
        end_byte: 550,
    };
    let edit = InputEdit {
        start_byte: 300,
        old_end_byte: 300,
        new_end_byte: 315,
        start_position: Point { row: 6, column: 0 },
        old_end_position: Point { row: 6, column: 0 },
        new_end_position: Point { row: 6, column: 15 },
    };
    assert!(
        wiki_links_need_update(&[wl_a, wl_b], &[edit]),
        "edit in large gap between entries must trigger re-extraction"
    );
}

/// marky-wjf: Edit in a large gap between two blocks must trigger re-extraction.
#[test]
fn test_blocks_need_update_detects_edit_in_large_gap() {
    let block_a = make_block_owned("block-a", 2, 10, 18, 100, 150);
    let block_b = make_block_owned("block-b", 10, 10, 18, 500, 550);
    let edit = InputEdit {
        start_byte: 300,
        old_end_byte: 300,
        new_end_byte: 315,
        start_position: Point { row: 6, column: 0 },
        old_end_position: Point { row: 6, column: 0 },
        new_end_position: Point { row: 6, column: 15 },
    };
    assert!(
        blocks_need_update(&[block_a, block_b], &[edit]),
        "edit in large gap between entries must trigger re-extraction"
    );
}

/// marky-wjf: Edit in a large gap between two markdown links must trigger re-extraction.
#[test]
fn test_markdown_links_need_update_detects_edit_in_large_gap() {
    let ml_a = make_ml_bytes(2, 0, 2, 20, 100, 150);
    let ml_b = make_ml_bytes(10, 0, 10, 20, 500, 550);
    let edit = InputEdit {
        start_byte: 300,
        old_end_byte: 300,
        new_end_byte: 315,
        start_position: Point { row: 6, column: 0 },
        old_end_position: Point { row: 6, column: 0 },
        new_end_position: Point { row: 6, column: 15 },
    };
    assert!(
        markdown_links_need_update(&[ml_a, ml_b], &[edit]),
        "edit in large gap between entries must trigger re-extraction"
    );
}

/// marky-wjf: Edit in a large gap between two XML tags must trigger re-extraction.
#[test]
fn test_xml_tags_need_update_detects_edit_in_large_gap() {
    let xt_a = make_xt_bytes(2, 0, 2, 20, "div", 100, 150);
    let xt_b = make_xt_bytes(10, 0, 10, 20, "span", 500, 550);
    let edit = InputEdit {
        start_byte: 300,
        old_end_byte: 300,
        new_end_byte: 315,
        start_position: Point { row: 6, column: 0 },
        old_end_position: Point { row: 6, column: 0 },
        new_end_position: Point { row: 6, column: 15 },
    };
    assert!(
        xml_tags_need_update(&[xt_a, xt_b], &[edit]),
        "edit in large gap between entries must trigger re-extraction"
    );
}

/// marky-wjf: Edit far before the first entry (outside neighbor window) must trigger.
#[test]
fn test_wiki_links_need_update_detects_edit_before_first_entry() {
    let wl = WikiLinkOwned {
        target: "Page".to_string(),
        alias: None,
        heading: None,
        range: Range::new(Position::new(10, 0), Position::new(10, 20)),
        start_byte: 500,
        end_byte: 550,
    };
    let edit = InputEdit {
        start_byte: 10,
        old_end_byte: 10,
        new_end_byte: 25,
        start_position: Point { row: 0, column: 10 },
        old_end_position: Point { row: 0, column: 10 },
        new_end_position: Point { row: 0, column: 25 },
    };
    assert!(
        wiki_links_need_update(&[wl], &[edit]),
        "edit far before first entry must trigger re-extraction"
    );
}

/// marky-wjf: any_edit_in_entry_gap returns false when edit IS covered by an entry's window.
#[test]
fn test_any_edit_in_entry_gap_returns_false_when_covered() {
    use super::super::any_edit_in_entry_gap;
    // Entry at bytes 100-150, edit at byte 200 — within 100-byte window of entry end (150).
    let ranges = [(100usize, 150usize)];
    let edit = InputEdit {
        start_byte: 200,
        old_end_byte: 200,
        new_end_byte: 210,
        start_position: Point { row: 5, column: 0 },
        old_end_position: Point { row: 5, column: 0 },
        new_end_position: Point { row: 5, column: 10 },
    };
    assert!(
        !any_edit_in_entry_gap(&ranges, &[edit], 100),
        "edit within entry's window should NOT be detected as a gap"
    );
}

/// marky-wjf: any_edit_in_entry_gap detects gap when edit is 1 byte past the window boundary.
#[test]
fn test_any_edit_in_entry_gap_boundary() {
    use super::super::any_edit_in_entry_gap;
    // Entry at bytes 100-150, window=100. Coverage extends to byte 250.
    // Edit at byte 251 is 1 byte past the window boundary.
    let ranges = [(100usize, 150usize)];
    let edit = InputEdit {
        start_byte: 251,
        old_end_byte: 251,
        new_end_byte: 260,
        start_position: Point { row: 8, column: 0 },
        old_end_position: Point { row: 8, column: 0 },
        new_end_position: Point { row: 8, column: 9 },
    };
    assert!(
        any_edit_in_entry_gap(&ranges, &[edit], 100),
        "edit 1 byte past window boundary should be detected as a gap"
    );
    // Edit at byte 250 is exactly at the window boundary — still covered.
    let edit_at_boundary = InputEdit {
        start_byte: 250,
        old_end_byte: 250,
        new_end_byte: 260,
        start_position: Point { row: 8, column: 0 },
        old_end_position: Point { row: 8, column: 0 },
        new_end_position: Point { row: 8, column: 10 },
    };
    assert!(
        !any_edit_in_entry_gap(&ranges, &[edit_at_boundary], 100),
        "edit exactly at window boundary should be covered (inclusive)"
    );
}

/// marky-wjf: Multiple edits where only one falls in a gap.
#[test]
fn test_blocks_need_update_multiple_edits_one_in_gap() {
    let block_a = make_block_owned("block-a", 2, 10, 18, 100, 150);
    let block_b = make_block_owned("block-b", 10, 10, 18, 500, 550);
    // Edit 1: within block_a's neighbor window (near byte 150)
    let edit_covered = InputEdit {
        start_byte: 200,
        old_end_byte: 200,
        new_end_byte: 205,
        start_position: Point { row: 4, column: 0 },
        old_end_position: Point { row: 4, column: 0 },
        new_end_position: Point { row: 4, column: 5 },
    };
    // Edit 2: in the gap at byte 300 (outside both windows)
    let edit_in_gap = InputEdit {
        start_byte: 300,
        old_end_byte: 300,
        new_end_byte: 310,
        start_position: Point { row: 6, column: 0 },
        old_end_position: Point { row: 6, column: 0 },
        new_end_position: Point { row: 6, column: 10 },
    };
    assert!(
        blocks_need_update(&[block_a, block_b], &[edit_covered, edit_in_gap]),
        "one of two edits is in a gap — must trigger re-extraction"
    );
}
