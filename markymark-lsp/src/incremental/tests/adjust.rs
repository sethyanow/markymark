use super::super::*;
use markymark_core::{Position, Range};
use markymark_parser::Point;

// ─── Position adjustment tests ────────────────────────────────────────────

#[test]
fn test_range_is_after_edit_end_true_for_entry_on_later_line() {
    let range = Range::new(Position::new(5, 0), Position::new(5, 20));
    let edit = InputEdit {
        start_byte: 10,
        old_end_byte: 15,
        new_end_byte: 16,
        start_position: Point { row: 2, column: 5 },
        old_end_position: Point { row: 2, column: 10 },
        new_end_position: Point { row: 2, column: 11 },
    };
    assert!(range_is_after_edit_end(range, &edit));
}

#[test]
fn test_range_is_after_edit_end_false_for_entry_before_edit() {
    let range = Range::new(Position::new(1, 0), Position::new(1, 20));
    let edit = InputEdit {
        start_byte: 100,
        old_end_byte: 105,
        new_end_byte: 106,
        start_position: Point { row: 5, column: 0 },
        old_end_position: Point { row: 5, column: 5 },
        new_end_position: Point { row: 5, column: 6 },
    };
    assert!(!range_is_after_edit_end(range, &edit));
}

#[test]
fn test_range_is_after_edit_end_same_line_after_column() {
    // Entry on same line as edit end but after the edit's old_end column
    let range = Range::new(Position::new(5, 20), Position::new(5, 30));
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 55,
        new_end_byte: 56,
        start_position: Point { row: 5, column: 10 },
        old_end_position: Point { row: 5, column: 15 },
        new_end_position: Point { row: 5, column: 16 },
    };
    assert!(
        range_is_after_edit_end(range, &edit),
        "entry at (5,20) is after edit end at (5,15)"
    );
}

#[test]
fn test_adjust_range_after_edit_single_char_insert_different_line() {
    // Edit inserts 1 char on line 3; entry on line 10 should only shift bytes
    let mut range = Range::new(Position::new(10, 5), Position::new(10, 15));
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 51,
        start_position: Point { row: 3, column: 10 },
        old_end_position: Point { row: 3, column: 10 },
        new_end_position: Point { row: 3, column: 11 },
    };
    adjust_range_after_edit(&mut range, &edit);
    // Line delta = 0, so lines unchanged
    assert_eq!(range.start.line, 10);
    assert_eq!(range.end.line, 10);
    // Characters unchanged (different line from edit)
    assert_eq!(range.start.character, 5);
    assert_eq!(range.end.character, 15);
}

#[test]
fn test_adjust_range_after_edit_single_char_insert_same_line() {
    // Edit inserts 1 char on line 5 col 10; entry starts at line 5 col 20
    let mut range = Range::new(Position::new(5, 20), Position::new(5, 30));
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 51,
        start_position: Point { row: 5, column: 10 },
        old_end_position: Point { row: 5, column: 10 },
        new_end_position: Point { row: 5, column: 11 },
    };
    adjust_range_after_edit(&mut range, &edit);
    // Same line as edit end → column shifts by +1
    assert_eq!(range.start.line, 5);
    assert_eq!(range.start.character, 21);
    assert_eq!(range.end.line, 5);
    assert_eq!(range.end.character, 31);
}

#[test]
fn test_adjust_range_after_edit_newline_insert() {
    // Edit inserts a newline at line 3 col 10: old_end=(3,10), new_end=(4,0)
    let mut range = Range::new(Position::new(5, 5), Position::new(5, 15));
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 51,
        start_position: Point { row: 3, column: 10 },
        old_end_position: Point { row: 3, column: 10 },
        new_end_position: Point { row: 4, column: 0 },
    };
    adjust_range_after_edit(&mut range, &edit);
    // Line delta = +1, entry was on different line (5 > 3)
    assert_eq!(range.start.line, 6);
    assert_eq!(range.end.line, 6);
    // Characters unchanged (different line from edit)
    assert_eq!(range.start.character, 5);
    assert_eq!(range.end.character, 15);
}

#[test]
fn test_adjust_bytes_after_edit_insertion() {
    let mut start_byte: usize = 200;
    let mut end_byte: usize = 210;
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 55, // 5 bytes inserted
        start_position: Point { row: 3, column: 0 },
        old_end_position: Point { row: 3, column: 0 },
        new_end_position: Point { row: 3, column: 5 },
    };
    adjust_bytes_after_edit(&mut start_byte, &mut end_byte, &edit);
    assert_eq!(start_byte, 205);
    assert_eq!(end_byte, 215);
}

#[test]
fn test_adjust_bytes_after_edit_deletion() {
    let mut start_byte: usize = 200;
    let mut end_byte: usize = 210;
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 55, // 5 bytes deleted
        new_end_byte: 50,
        start_position: Point { row: 3, column: 0 },
        old_end_position: Point { row: 3, column: 5 },
        new_end_position: Point { row: 3, column: 0 },
    };
    adjust_bytes_after_edit(&mut start_byte, &mut end_byte, &edit);
    assert_eq!(start_byte, 195);
    assert_eq!(end_byte, 205);
}

// ─── Regression: insertion at exact old_end must still adjust entries ──────

#[test]
fn test_range_is_after_edit_end_at_insertion_point() {
    // Regression: with strict `>`, an entry starting exactly at the insertion
    // point (where start == old_end) got no position adjustment, leaving stale
    // coordinates. With `>=` it is correctly identified as needing adjustment.
    let range = Range::new(Position::new(3, 10), Position::new(3, 20));
    // Pure insertion at (3,10): start == old_end
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 55,
        start_position: Point { row: 3, column: 10 },
        old_end_position: Point { row: 3, column: 10 },
        new_end_position: Point { row: 3, column: 15 },
    };
    assert!(
        range_is_after_edit_end(range, &edit),
        "entry at exact insertion point (old_end) must be classified as after-edit for adjustment"
    );
}

#[test]
fn test_merge_incremental_wiki_links_adjusts_entry_at_insertion_point() {
    // End-to-end regression: a wiki link starting exactly at the insertion point
    // must have its position adjusted, not left stale.
    // Pure insertion at (3,10) adding 5 bytes / 5 columns
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 55,
        start_position: Point { row: 3, column: 10 },
        old_end_position: Point { row: 3, column: 10 },
        new_end_position: Point { row: 3, column: 15 },
    };
    // The link at byte 50 IS within the neighbor window of the edit at byte 50,
    // so place the link far enough away to avoid being flagged as affected.
    let old_wl_far = WikiLinkOwned {
        target: "Page".to_string(),
        alias: None,
        heading: None,
        range: Range::new(Position::new(3, 10), Position::new(3, 20)),
        start_byte: 500,
        end_byte: 510,
    };
    // New extraction has the link at the adjusted position
    let new_wl = WikiLinkOwned {
        target: "Page".to_string(),
        alias: None,
        heading: None,
        range: Range::new(Position::new(3, 15), Position::new(3, 25)),
        start_byte: 505,
        end_byte: 515,
    };
    let merged = merge_incremental_wiki_links(&[old_wl_far], &[new_wl], &[edit]);
    assert_eq!(merged.len(), 1);
    // The old entry should be adjusted: column +5 (same line as old_end), bytes +5
    assert_eq!(
        merged[0].range.start.character, 15,
        "column should shift by +5 from insertion"
    );
    assert_eq!(merged[0].start_byte, 505, "start_byte should shift by +5");
    assert_eq!(merged[0].end_byte, 515, "end_byte should shift by +5");
}

// ─── Saturating arithmetic tests ──────────────────────────────────────────

/// marky-oiv: adjust_range_after_edit must not underflow when delta is large negative.
#[test]
fn test_adjust_range_after_edit_saturates_on_large_negative_delta() {
    // Entry at (2, 3)–(2, 10); edit deletes many lines: old_end=(10,50), new_end=(0,0)
    // Line delta = 0 - 10 = -10. For entry on same line as old_end: col_delta = 0 - 50 = -50.
    // Without saturation: (2 as i64 + -10) as u32 = -8 as u32 = underflow!
    let mut range = Range::new(Position::new(2, 3), Position::new(2, 10));
    let edit = InputEdit {
        start_byte: 0,
        old_end_byte: 200,
        new_end_byte: 0,
        start_position: Point { row: 0, column: 0 },
        old_end_position: Point {
            row: 10,
            column: 50,
        },
        new_end_position: Point { row: 0, column: 0 },
    };
    adjust_range_after_edit(&mut range, &edit);
    // Must clamp to 0, not wrap around to u32::MAX
    assert_eq!(range.start.line, 0, "line must saturate at 0");
    assert_eq!(range.end.line, 0, "end line must saturate at 0");
}

/// marky-oiv: adjust_range_after_edit column must saturate when on the same line as edit.
#[test]
fn test_adjust_range_after_edit_column_saturates_on_same_line() {
    // Entry at line 5, col 3; edit on same line deletes columns: old_end=(5,50), new_end=(5,0)
    // col_delta = 0 - 50 = -50. Without saturation: (3 + -50) as u32 = underflow!
    let mut range = Range::new(Position::new(5, 3), Position::new(5, 10));
    let edit = InputEdit {
        start_byte: 50,
        old_end_byte: 100,
        new_end_byte: 50,
        start_position: Point { row: 5, column: 0 },
        old_end_position: Point { row: 5, column: 50 },
        new_end_position: Point { row: 5, column: 0 },
    };
    adjust_range_after_edit(&mut range, &edit);
    assert_eq!(
        range.start.character, 0,
        "character must saturate at 0, not underflow"
    );
    assert_eq!(
        range.end.character, 0,
        "end character must saturate at 0, not underflow"
    );
}

/// marky-oiv: adjust_bytes_after_edit must not underflow on large deletion.
#[test]
fn test_adjust_bytes_after_edit_saturates_on_large_deletion() {
    // Entry at bytes 10–20; edit removes 100 bytes before it: old_end=200, new_end=100
    // byte_delta = 100 - 200 = -100. Without saturation: (10 + -100) as usize = underflow!
    let mut start_byte: usize = 10;
    let mut end_byte: usize = 20;
    let edit = InputEdit {
        start_byte: 0,
        old_end_byte: 200,
        new_end_byte: 100,
        start_position: Point { row: 0, column: 0 },
        old_end_position: Point { row: 10, column: 0 },
        new_end_position: Point { row: 5, column: 0 },
    };
    adjust_bytes_after_edit(&mut start_byte, &mut end_byte, &edit);
    assert_eq!(
        start_byte, 0,
        "start_byte must saturate at 0, not underflow"
    );
    assert_eq!(end_byte, 0, "end_byte must saturate at 0, not underflow");
}

/// marky-v8y: positive line deltas must saturate at u32::MAX, not wrap.
#[test]
fn test_adjust_range_after_edit_saturates_on_large_positive_delta() {
    let mut range = Range::new(
        Position::new(u32::MAX - 2, 10),
        Position::new(u32::MAX - 1, 20),
    );
    let edit = InputEdit {
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 1,
        start_position: Point { row: 0, column: 0 },
        old_end_position: Point { row: 0, column: 0 },
        new_end_position: Point {
            row: u32::MAX as usize,
            column: 0,
        },
    };

    adjust_range_after_edit(&mut range, &edit);

    assert_eq!(range.start.line, u32::MAX);
    assert_eq!(range.end.line, u32::MAX);
}

/// marky-v8y: positive byte deltas must saturate at usize::MAX, not wrap.
#[test]
fn test_adjust_bytes_after_edit_saturates_on_large_positive_delta() {
    let mut start_byte = usize::MAX - 5;
    let mut end_byte = usize::MAX - 1;
    let edit = InputEdit {
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 10,
        start_position: Point { row: 0, column: 0 },
        old_end_position: Point { row: 0, column: 0 },
        new_end_position: Point { row: 0, column: 10 },
    };

    adjust_bytes_after_edit(&mut start_byte, &mut end_byte, &edit);

    assert_eq!(start_byte, usize::MAX);
    assert_eq!(end_byte, usize::MAX);
}
