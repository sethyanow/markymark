use super::super::*;
use super::{make_edit, make_xt, make_xt_bytes};
use markymark_parser::Point;

#[test]
fn test_xml_tags_need_update_true_for_edit_near_tag_via_neighbor_window() {
    // Edit within 100-byte neighbor window of an XML tag
    let xt = make_xt_bytes(10, 0, 10, 20, "div", 50, 70);
    let edit = make_edit(5, 0, 5, 10); // bytes 0-1, within 100 bytes of tag
    assert!(
        xml_tags_need_update(&[xt], &[edit]),
        "edit within neighbor window should trigger re-extraction"
    );
}

#[test]
fn test_xml_tags_need_update_true_for_edit_after_last_tag() {
    let xt = make_xt_bytes(0, 0, 0, 20, "div", 0, 20);
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
        xml_tags_need_update(&[xt], &[edit]),
        "edit after last tag should trigger re-extraction"
    );
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
    // Old xml tag at line 5 (bytes 500-520) with attributes; edit at line 0 does not
    // affect it (no range intersection, outside 100-byte neighbor window).
    // The tag is retained from old with position adjustment.
    // New extraction provides the same tag, verifying attributes are preserved either way.
    let old_xt = make_xt_bytes(5, 0, 5, 20, "goal", 500, 520);
    let new_xt = make_xt_bytes(5, 0, 5, 20, "goal", 500, 520); // same after re-extraction
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
fn test_xml_tags_need_update_false_for_empty_slice_with_edits() {
    // Empty old slice: no entries to affect, no "last entry" for after-check.
    // This is correct because build_markdown_index_incremental takes the
    // old.is_empty() branch (full extraction) before calling _need_update.
    let edit = make_edit(0, 0, 0, 5);
    assert!(!xml_tags_need_update(&[], &[edit]));
}
