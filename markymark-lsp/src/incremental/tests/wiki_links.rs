use super::super::*;
use markymark_core::{Position, Range};
use markymark_parser::Point;

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
