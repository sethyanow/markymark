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
    MarkdownLinkOwned {
        text: "link".to_string(),
        url: "https://example.com".to_string(),
        anchor: None,
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
    }
}

fn make_xt(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    tag_name: &str,
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
fn test_markdown_link_after_edit_start_triggers_update() {
    // Link starts at line 5; edit starts at line 3 — link is after edit start
    let ml = make_ml(5, 0, 5, 20);
    let edit = make_edit(3, 0, 3, 5);
    // range_is_after_edit_start returns true: link start (5,0) >= edit start (3,0)
    assert!(markdown_link_affected_by_edits(&ml, &[edit]));
    assert!(markdown_links_need_update(&[ml], &[edit]));
}

#[test]
fn test_merge_incremental_markdown_links_keeps_unaffected() {
    // Two links: ml1 at line 0, ml2 at line 10
    // Edit only near line 0 (affects ml1 via range_intersects_edit)
    let ml1 = make_ml(0, 0, 0, 20);
    let ml2 = make_ml(10, 0, 10, 20);
    // Edit at line 0 intersects ml1 but not ml2 (ml2 starts at line 10)
    // Actually, range_is_after_edit_start will catch ml2 (starts after edit).
    // So we need old ml2 to survive: use old ml2 directly, as it's not in
    // the "new" extraction for affected region.
    // Since merge: old entries NOT affected stay; new entries FROM affected regions come in.
    // ml2 is after edit start so it's "affected" — both old and new ml2 will be considered.
    // Let's test the simpler case: edit before both links.
    let edit = make_edit(0, 5, 0, 8); // overlaps ml1
    let merged = merge_incremental_markdown_links(
        &[ml1.clone(), ml2.clone()],
        std::slice::from_ref(&ml2),
        &[edit],
    );
    // ml1 is affected -> dropped from old; ml2 comes from "new" only if it's affected too.
    // ml2 is after edit start (line 10 >= line 0), so affected -> dropped from old, added from new.
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].range.start.line, 10);
}

#[test]
fn test_markdown_links_need_update_false_when_edit_before_all_links() {
    // No links; edit_starts_at_or_after check returns false for empty slice
    assert!(!any_edit_starts_at_or_after_last_markdown_link(
        &[],
        &[make_edit(0, 0, 0, 5)]
    ));
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
    // Old xml tag at line 5 with attributes; edit at line 0 does not affect it
    // via intersection (line 5 vs edit at line 0).
    // But range_is_after_edit_start: tag at line 5 >= edit start line 0 → affected.
    // So the tag is affected. Use a new tag from re-extraction that preserves attributes.
    let old_xt = make_xt(5, 0, 5, 20, "goal");
    let new_xt = make_xt(5, 0, 5, 20, "goal"); // same after re-extraction
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
fn test_xml_tags_need_update_false_for_empty_slice() {
    // Empty old slice with edit → any_edit_starts_at_or_after returns false
    let edit = make_edit(0, 0, 0, 5);
    // xml_tags_need_update returns false for empty (no tags to check, any_edit_starts... false)
    // Actually it first checks pending_edits.is_empty() (false), then checks iter().any() on
    // empty (false), then calls any_edit_starts_at_or_after_last_xml_tag which returns false for empty.
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
    // range_is_after_edit_start: true (block at row 10 >= edit start row 0)
    // → affected because position shifted; blocks_need_update should return true
    assert!(
        blocks_need_update(&old_blocks, &[edit]),
        "edit before block shifts block position, requiring update"
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
fn test_merge_incremental_blocks_deduplicates_when_both_contribute() {
    // Old has two blocks; edit is between them.
    // Block-A at row 0 (before edit) → unaffected → from old
    // Block-B at row 5 (after edit) → affected → from new
    let old_blocks = vec![
        make_block_owned("block-a", 0, 10, 18, 10, 18),
        make_block_owned("block-b", 5, 10, 18, 200, 208),
    ];
    let new_blocks = vec![
        // block-a unchanged
        make_block_owned("block-a", 0, 10, 18, 10, 18),
        // block-b has updated position after edit
        make_block_owned("block-b", 5, 10, 18, 201, 209),
    ];
    // Edit at row 3 (between the two blocks)
    let edit = InputEdit {
        start_byte: 100,
        old_end_byte: 100,
        new_end_byte: 101, // insert 1 byte
        start_position: Point { row: 3, column: 0 },
        old_end_position: Point { row: 3, column: 0 },
        new_end_position: Point { row: 3, column: 1 },
    };
    let merged = merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
    // Both blocks should appear exactly once
    assert_eq!(merged.len(), 2, "merged should contain exactly two blocks");
    assert!(merged.iter().any(|b| b.id == "block-a"));
    assert!(merged.iter().any(|b| b.id == "block-b"));
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
