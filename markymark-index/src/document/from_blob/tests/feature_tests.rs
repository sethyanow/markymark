// Feature tests — code spans, tasks, embeds, callouts, block refs, and properties.

use super::super::super::DocumentIndex;
use super::super::header::{HEADING_SIZE, V1_HEADER_SIZE};
use super::{blob_for, make_v1_empty_blob, make_v2_empty_blob};

// ── Code span tests (marky-vsh2) ────────────────────────────────────

#[test]
fn test_from_blob_code_spans_basic() {
    let blob = blob_for("Hello `world` end");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "world");
    assert_eq!(cs[0].start_byte, 6); // offset of opening backtick
    assert_eq!(cs[0].end_byte, 13); // past closing backtick
}

#[test]
fn test_from_blob_code_spans_multiple() {
    let blob = blob_for("`a` and `b`");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].text, "a");
    assert_eq!(cs[1].text, "b");
    assert!(cs[1].start_byte > cs[0].start_byte);
}

#[test]
fn test_from_blob_code_spans_empty() {
    let blob = blob_for("No code here");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.code_spans().is_empty());
}

#[test]
fn test_from_blob_code_spans_in_heading() {
    let blob = blob_for("# Title `code` end");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.headings().len(), 1);
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "code");
}

#[test]
fn test_from_blob_code_spans_backward_compat() {
    // A true v1 blob with 1 heading but no code spans.
    // v1 has code_span_count=0 by default (offset 48 is zero-initialized).
    let pool = b"a heading\0a-heading\0\0\0";
    let pool_size = pool.len() as u32;
    let total_size = V1_HEADER_SIZE as u32 + HEADING_SIZE as u32 + pool_size;

    let mut v1_blob = vec![0u8; total_size as usize];
    // Start from the empty v1 header template.
    v1_blob[..V1_HEADER_SIZE].copy_from_slice(&make_v1_empty_blob());
    // Patch counts and sizes.
    v1_blob[16..20].copy_from_slice(&1u32.to_le_bytes()); // heading_count
    v1_blob[36..40].copy_from_slice(&pool_size.to_le_bytes()); // text_pool_size
    v1_blob[44..48].copy_from_slice(&total_size.to_le_bytes()); // total_blob_size

    // BlobHeading: text_off=0, text_len=9, slug_off=10, slug_len=9, level=1
    let h = V1_HEADER_SIZE;
    v1_blob[h..h + 4].copy_from_slice(&0u32.to_le_bytes());
    v1_blob[h + 4..h + 8].copy_from_slice(&9u32.to_le_bytes());
    v1_blob[h + 8..h + 12].copy_from_slice(&10u32.to_le_bytes());
    v1_blob[h + 12..h + 16].copy_from_slice(&9u32.to_le_bytes());
    v1_blob[h + 36] = 1; // level

    // Text pool follows the heading section.
    let pool_start = V1_HEADER_SIZE + HEADING_SIZE;
    v1_blob[pool_start..pool_start + pool.len()].copy_from_slice(pool);

    let index = DocumentIndex::from_blob(&v1_blob).expect("v1 blob should parse");
    assert!(index.code_spans().is_empty());
    assert_eq!(index.headings().len(), 1);
}

#[test]
fn test_from_blob_code_span_positions() {
    let blob = blob_for("line1\n`code`\nline3");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "code");
    // Code span is on line 1 (0-indexed), col 0
    assert_eq!(cs[0].range.start.line, 1);
    assert_eq!(cs[0].range.start.character, 0);
}

// ── Task/Embed from_blob tests (marky-bmu9) ──────────────────────────

#[test]
fn test_from_blob_v2_with_tasks() {
    let blob = blob_for("- [ ] Todo\n- [x] Done\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let tasks = index.tasks();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].state, "unchecked");
    assert_eq!(tasks[0].text, "Todo");
    assert_eq!(tasks[1].state, "checked");
    assert_eq!(tasks[1].text, "Done");
}

#[test]
fn test_from_blob_v2_with_embeds() {
    let blob = blob_for("![[target]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let embeds = index.embeds();
    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].target, "target");
}

#[test]
fn test_from_blob_v2_tasks_and_embeds() {
    let blob = blob_for("- [x] Task\n\n![[embed]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.tasks().len(), 1);
    assert_eq!(index.embeds().len(), 1);
}

#[test]
fn test_from_blob_v1_no_tasks_or_embeds() {
    let blob = make_v1_empty_blob();
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.tasks().is_empty());
    assert!(index.embeds().is_empty());
}

#[test]
fn test_from_blob_v2_empty_task_text() {
    // A task list item with no text content: `- [ ] \n`
    let blob = blob_for("- [ ] \n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    // May or may not have a task depending on engine behavior; just verify no panic
    let _ = index.tasks();
}

// ── Callout + Block ref tests (marky-8ac8) ──────────────────────────

#[test]
fn test_from_blob_callout_note() {
    let blob = blob_for("> [!note]\n> Some content here\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.callouts().len(), 1, "expected 1 callout");
    assert_eq!(index.callouts()[0].callout_type, "note");
    assert!(index.callouts()[0].title.is_none());
}

#[test]
fn test_from_blob_callout_with_title() {
    let blob = blob_for("> [!tip] My Custom Title\n> Content\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.callouts().len(), 1, "expected 1 callout");
    assert_eq!(index.callouts()[0].callout_type, "tip");
    assert_eq!(index.callouts()[0].title, Some("My Custom Title"));
}

#[test]
fn test_from_blob_callout_range_and_bytes() {
    let blob = blob_for("> [!warning]\n> Watch out\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.callouts().len(), 1);
    let c = &index.callouts()[0];
    assert_eq!(c.callout_type, "warning");
    // start_byte should be at the '>' character
    assert!(c.start_byte < c.end_byte, "start_byte < end_byte");
}

#[test]
fn test_from_blob_no_callout_for_plain_blockquote() {
    let blob = blob_for("> This is just a regular quote\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(
        index.callouts().is_empty(),
        "plain blockquotes should not produce callouts"
    );
}

#[test]
fn test_from_blob_block_ref_basic() {
    let blob = blob_for("Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.block_refs().len(), 1, "expected 1 block ref");
    assert_eq!(
        index.block_refs()[0].uuid,
        "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    );
}

#[test]
fn test_from_blob_block_ref_no_match_for_invalid() {
    let blob = blob_for("Text ((not-a-valid-uuid)) more\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(
        index.block_refs().is_empty(),
        "invalid UUID should not produce block ref"
    );
}

#[test]
fn test_from_blob_v1_no_callouts_or_block_refs() {
    let blob = make_v1_empty_blob();
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.callouts().is_empty());
    assert!(index.block_refs().is_empty());
}

#[test]
fn test_from_blob_v2_empty_no_callouts_or_block_refs() {
    let blob = make_v2_empty_blob();
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.callouts().is_empty());
    assert!(index.block_refs().is_empty());
}

// ── Property tests (B-6) ──────────────────────────────────────────────

#[test]
fn test_from_blob_properties_string() {
    use crate::document::PropertyValueEntry;
    let text = "tags:: project\nstatus:: active\n\n# Content\n";
    let blob = blob_for(text);
    let idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let props = idx.properties();
    assert_eq!(props.len(), 2, "expected 2 properties");
    assert_eq!(props[0].key, "tags");
    assert_eq!(props[1].key, "status");
    // Both are simple strings
    match &props[0].value {
        PropertyValueEntry::String(v) => assert_eq!(*v, "project"),
        other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn test_from_blob_properties_list() {
    use crate::document::PropertyValueEntry;
    let text = "tags:: foo, bar, baz\n\n# Content\n";
    let blob = blob_for(text);
    let idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let props = idx.properties();
    assert_eq!(props.len(), 1);
    match &props[0].value {
        PropertyValueEntry::List(items) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], "foo");
            assert_eq!(items[1], "bar");
            assert_eq!(items[2], "baz");
        }
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn test_from_blob_properties_page_ref() {
    use crate::document::PropertyValueEntry;
    let text = "author:: [[Jane]]\n\n# Content\n";
    let blob = blob_for(text);
    let idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let props = idx.properties();
    assert_eq!(props.len(), 1);
    match &props[0].value {
        PropertyValueEntry::PageRef(v) => assert_eq!(*v, "Jane"),
        other => panic!("expected PageRef, got {:?}", other),
    }
}

#[test]
fn test_from_blob_no_properties() {
    let text = "# Just heading\n\nNo properties here.\n";
    let blob = blob_for(text);
    let idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(idx.properties().is_empty());
}

