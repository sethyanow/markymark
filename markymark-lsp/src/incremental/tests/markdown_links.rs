use super::super::*;
use super::{make_edit, make_ml, make_ml_bytes};
use markymark_parser::Point;

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
fn test_markdown_link_after_edit_not_affected_when_no_intersection_and_far() {
    // Link at line 5, bytes 500-520; edit at line 3, bytes 0-1.
    // No range intersection AND outside the 100-byte neighbor window →
    // the individual link is NOT affected (only needs position adjustment).
    let ml = make_ml_bytes(5, 0, 5, 20, 500, 520);
    let edit = make_edit(3, 0, 3, 5);
    assert!(
        !markdown_link_affected_by_edits(&ml, &[edit]),
        "link far from edit (no intersection, outside neighbor window) should NOT be affected"
    );
    // marky-wjf: needs_update now returns TRUE because the edit is in an
    // uncovered gap (before the first entry, outside the neighbor window).
    // The edit could create new links that position adjustment alone can't detect.
    assert!(
        markdown_links_need_update(&[ml], &[edit]),
        "edit in uncovered gap should trigger re-extraction (marky-wjf)"
    );
}

#[test]
fn test_merge_incremental_markdown_links_keeps_unaffected_with_position_adjustment() {
    // Two links: ml1 at line 0 (bytes 0-20), ml2 at line 10 (bytes 500-520)
    // Edit only at line 0, cols 5-8 (intersects ml1, NOT ml2)
    let ml1 = make_ml_bytes(0, 0, 0, 20, 0, 20);
    let ml2 = make_ml_bytes(10, 0, 10, 20, 500, 520);
    // New extraction provides updated ml1
    let new_ml1 = make_ml_bytes(0, 0, 0, 20, 0, 20);
    let edit = make_edit(0, 5, 0, 8); // overlaps ml1, not ml2
    let merged = merge_incremental_markdown_links(&[ml1.clone(), ml2.clone()], &[new_ml1], &[edit]);
    // ml1 intersects the edit → dropped from old, added from new
    // ml2 does NOT intersect → retained from old with position adjustment
    // (edit is same line start/end, no line delta → ml2 position unchanged)
    assert_eq!(merged.len(), 2, "both links should be in merged result");
    assert_eq!(merged[0].range.start.line, 0, "ml1 from new extraction");
    assert_eq!(merged[1].range.start.line, 10, "ml2 retained from old");
}

#[test]
fn test_markdown_links_need_update_true_for_edit_near_link_via_neighbor_window() {
    // Edit at bytes 0-1 is within the 100-byte neighbor window of a link at bytes 50-70
    let ml = make_ml_bytes(5, 0, 5, 20, 50, 70);
    let edit = make_edit(0, 0, 0, 5); // bytes 0-1, within 100 bytes of link
    assert!(
        markdown_links_need_update(&[ml], &[edit]),
        "edit within neighbor window should trigger re-extraction"
    );
}

#[test]
fn test_markdown_links_need_update_true_for_edit_after_last_link() {
    // Edit after last link catches potential new link creation
    let ml = make_ml_bytes(0, 0, 0, 20, 0, 20);
    let edit = InputEdit {
        start_byte: 200,
        old_end_byte: 200,
        new_end_byte: 210,
        start_position: Point { row: 10, column: 0 },
        old_end_position: Point { row: 10, column: 0 },
        new_end_position: Point {
            row: 10,
            column: 10,
        },
    };
    assert!(
        markdown_links_need_update(&[ml], &[edit]),
        "edit after last link should trigger re-extraction"
    );
}
