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
    make_ml_bytes(start_line, start_col, end_line, end_col, 0, 0)
}

fn make_ml_bytes(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    start_byte: usize,
    end_byte: usize,
) -> MarkdownLinkOwned {
    MarkdownLinkOwned {
        text: "link".to_string(),
        url: "https://example.com".to_string(),
        anchor: None,
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
        start_byte,
        end_byte,
    }
}

fn make_xt(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    tag_name: &str,
) -> XmlTagOwned {
    make_xt_bytes(start_line, start_col, end_line, end_col, tag_name, 0, 0)
}

fn make_xt_bytes(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    tag_name: &str,
    start_byte: usize,
    end_byte: usize,
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
        start_byte,
        end_byte,
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
fn test_markdown_link_after_edit_not_affected_when_no_intersection_and_far() {
    // Link at line 5, bytes 500-520; edit at line 3, bytes 0-1.
    // No range intersection AND outside the 100-byte neighbor window →
    // link is NOT affected (only needs position adjustment).
    let ml = make_ml_bytes(5, 0, 5, 20, 500, 520);
    let edit = make_edit(3, 0, 3, 5);
    assert!(
        !markdown_link_affected_by_edits(&ml, &[edit]),
        "link far from edit (no intersection, outside neighbor window) should NOT be affected"
    );
    // needs_update still returns false: no entry affected and edit is before last link
    assert!(
        !markdown_links_need_update(&[ml], &[edit]),
        "link far from edit with no intersection should not trigger re-extraction"
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
    // range_within_neighbor_window: false (500 >> 100 + 1)
    // Block is NOT affected — will get position adjustment instead of re-extraction
    assert!(
        !blocks_need_update(&old_blocks, &[edit]),
        "block far from edit is not affected; position adjustment handles shifting"
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

// ─── Full parity test: incremental must match full rebuild INCLUDING positions ─

#[test]
fn test_incremental_wiki_links_parity_with_positions() {
    // Build a document with wiki links, apply a single-char edit in prose
    // between links, verify incremental produces identical wiki link positions
    // to a full rebuild.
    use markymark_parser::Parser;

    let original = "# Title\n\nSee [[PageA]] for details.\n\nSome prose text here.\n\nAlso check [[PageB]] and [[PageC]].\n";
    let mut parser = Parser::new().unwrap();

    // Initial parse
    let ast0 = parser.parse(original).unwrap();
    let index0 = DocumentIndex::from_ast(ast0);

    // Extract old wiki links
    let old_wiki_links: Vec<WikiLinkOwned> = index0
        .wiki_links()
        .iter()
        .map(|wl| WikiLinkOwned {
            target: wl.target.to_string(),
            alias: wl.alias.map(str::to_string),
            heading: wl.heading.map(str::to_string),
            range: wl.range,
            start_byte: wl.start_byte,
            end_byte: wl.end_byte,
        })
        .collect();

    // Single-char insertion in prose (line 4, col 5: "Some Xprose text here.")
    let insert_byte = original.find("prose").unwrap();
    let modified = format!("{}X{}", &original[..insert_byte], &original[insert_byte..]);

    let edit = InputEdit {
        start_byte: insert_byte,
        old_end_byte: insert_byte,
        new_end_byte: insert_byte + 1,
        start_position: Point { row: 4, column: 5 },
        old_end_position: Point { row: 4, column: 5 },
        new_end_position: Point { row: 4, column: 6 },
    };

    // Full rebuild from modified text
    let ast_full = parser.parse(&modified).unwrap();
    let full_index = DocumentIndex::from_ast(ast_full);
    let full_wiki_links: Vec<_> = full_index
        .wiki_links()
        .iter()
        .map(|wl| (wl.target.to_string(), wl.range, wl.start_byte, wl.end_byte))
        .collect();

    // Incremental rebuild
    let ast_inc = parser.parse(&modified).unwrap();
    let inc_index =
        build_markdown_index_incremental(ast_inc, &[edit], Some(&old_wiki_links), None, None, None);
    let inc_wiki_links: Vec<_> = inc_index
        .wiki_links()
        .iter()
        .map(|wl| (wl.target.to_string(), wl.range, wl.start_byte, wl.end_byte))
        .collect();

    assert_eq!(
        full_wiki_links.len(),
        inc_wiki_links.len(),
        "same number of wiki links: full={} inc={}",
        full_wiki_links.len(),
        inc_wiki_links.len()
    );

    for (i, (full, inc)) in full_wiki_links
        .iter()
        .zip(inc_wiki_links.iter())
        .enumerate()
    {
        assert_eq!(
            full, inc,
            "wiki link {i} mismatch:\n  full: {full:?}\n  inc:  {inc:?}"
        );
    }
}

#[test]
fn test_incremental_blocks_parity_with_positions() {
    // Build a document with block IDs, apply an edit, verify positions match
    use markymark_parser::Parser;

    let original = "# Title\n\nBlock A ^block-a\n\nSome text here.\n\nBlock B ^block-b\n";
    let mut parser = Parser::new().unwrap();

    let ast0 = parser.parse(original).unwrap();
    let index0 = DocumentIndex::from_ast(ast0);

    let old_blocks: Vec<BlockOwned> = index0
        .block_ids()
        .filter_map(|id| index0.block_by_id(id))
        .map(|entry| BlockOwned {
            id: entry.id.to_string(),
            range: entry.range,
            start_byte: entry.start_byte,
            end_byte: entry.end_byte,
        })
        .collect();

    // Insert "X" in "Some text here." (line 4, col 5)
    let insert_byte = original.find("text here").unwrap();
    let modified = format!("{}X{}", &original[..insert_byte], &original[insert_byte..]);

    let edit = InputEdit {
        start_byte: insert_byte,
        old_end_byte: insert_byte,
        new_end_byte: insert_byte + 1,
        start_position: Point { row: 4, column: 5 },
        old_end_position: Point { row: 4, column: 5 },
        new_end_position: Point { row: 4, column: 6 },
    };

    // Full rebuild
    let ast_full = parser.parse(&modified).unwrap();
    let full_index = DocumentIndex::from_ast(ast_full);

    // Incremental
    let ast_inc = parser.parse(&modified).unwrap();
    let inc_index =
        build_markdown_index_incremental(ast_inc, &[edit], None, Some(&old_blocks), None, None);

    // Compare block IDs and positions (sort by ID for stable ordering)
    let mut full_blocks: Vec<_> = full_index
        .block_ids()
        .filter_map(|id| full_index.block_by_id(id))
        .map(|e| (e.id.to_string(), e.range, e.start_byte, e.end_byte))
        .collect();
    let mut inc_blocks: Vec<_> = inc_index
        .block_ids()
        .filter_map(|id| inc_index.block_by_id(id))
        .map(|e| (e.id.to_string(), e.range, e.start_byte, e.end_byte))
        .collect();
    full_blocks.sort_by(|a, b| a.0.cmp(&b.0));
    inc_blocks.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(full_blocks.len(), inc_blocks.len());
    for (full, inc) in full_blocks.iter().zip(inc_blocks.iter()) {
        assert_eq!(
            full, inc,
            "block mismatch:\n  full: {full:?}\n  inc:  {inc:?}"
        );
    }
}
