//! Regression tests for panic safety and data-loss bugs in markdown parsing.
//!
//! Background (see bones issue for context):
//!
//! `markymark_parser::Parser::parse_block_tree_only(source)` normalizes source
//! by appending `\n` when missing, parses against the normalized buffer, and
//! returns **only** the resulting `tree_sitter::Tree` — not the normalized
//! source. Callers that keep their original un-normalized `source` end up with
//! tree positions that reference a buffer one byte longer than what they hold.
//!
//! `markymark_index::document::from_engine::extract_content_blocks(source)` is
//! the observed victim: it passes un-normalized `source` to
//! `parse_block_tree_only` then to `is_logseq_heading(node, source)`, whose
//! `node.utf8_text(source.as_bytes())` slices `source[start..end]` and panics
//! with `range end index N+1 out of range for slice of length N`.
//!
//! The panic is currently swallowed by `std::panic::catch_unwind` in
//! `extract_content_blocks`, which returns `Vec::new()`. User-visible effect:
//! every block in the file is silently dropped from the index, so
//! `block_text()` returns `""` and `content_blocks()` is empty.
//!
//! These tests fail under the current (buggy) behavior and pass once the
//! normalization leak is fixed.

use markymark_index::DocumentIndex;

/// Build an index from raw text without any post-processing.
fn index(source: &str) -> DocumentIndex {
    DocumentIndex::from_text(source)
}

// ---------------------------------------------------------------------------
// Bug #1: `parse_block_tree_only` normalization leak → silent empty index
// ---------------------------------------------------------------------------

#[test]
fn logseq_heading_no_trailing_newline_does_not_drop_other_blocks() {
    // Reproducer: an 11-byte file with no trailing newline, one Logseq-style
    // heading as the sole block. Under the bug, the tree-sitter node for the
    // list_item has end_byte == 12 while source.len() == 11 — panic in
    // `is_logseq_heading::utf8_text`, entire block extraction discarded.
    //
    // After the fix, the file parses, the Logseq heading is recognised as a
    // heading (not a content block), and `content_blocks()` is consistent
    // with the post-filter result.
    let source = "- # Heading";
    let idx = index(source);
    // Should index the Logseq-style heading under `.headings()`.
    let headings = idx.headings();
    assert!(
        !headings.is_empty(),
        "Logseq heading without trailing newline should still be indexed; \
         got empty headings — suggests the parser panicked and bailed"
    );
}

#[test]
fn paragraph_followed_by_logseq_heading_no_newline_preserves_paragraph() {
    // A file where a valid paragraph precedes the panic-triggering list item.
    // Under the bug: the whole block extraction dies, paragraph is dropped.
    // After the fix: paragraph is preserved, Logseq heading is recognised.
    let source = "This is a paragraph.\n\n- # Logseq heading";
    let idx = index(source);

    let blocks = idx.content_blocks();
    assert!(
        !blocks.is_empty(),
        "a preceding paragraph must not be dropped when a later list item \
         triggers a tree-sitter off-by-one; got zero content blocks"
    );

    // And the paragraph's text must be recoverable via block_text.
    let texts: Vec<&str> = blocks.iter().map(|b| idx.block_text(b)).collect();
    assert!(
        texts.iter().any(|t| t.contains("paragraph")),
        "paragraph text missing from content blocks: {texts:?}"
    );
}

#[test]
fn nested_list_no_trailing_newline_does_not_panic() {
    // Reproducer from the torture harness: four-level nested list, no trailing
    // newline. Triggers the same off-by-one in block-tree walk.
    let source = "- a\n  - b\n    - c\n      - d";
    let idx = index(source);

    // We don't assert a specific block count — tree-sitter's treatment of
    // nested lists varies. The assertion is that indexing completes without
    // losing blocks to a catch_unwind swallow.
    //
    // Under the bug, content_blocks() is empty (catch_unwind swallowed).
    // After the fix, there is at least one list_item block.
    let blocks = idx.content_blocks();
    assert!(
        !blocks.is_empty(),
        "deeply nested list without trailing newline should produce at least \
         one content block; empty result indicates the block extraction \
         panicked and was swallowed"
    );
}

// ---------------------------------------------------------------------------
// Bug #2: `block_text()` returning "" for blocks whose end_byte was computed
// against normalized source but stored against original source
// ---------------------------------------------------------------------------

#[test]
fn block_text_is_nonempty_for_trailing_paragraph_no_newline() {
    // A trailing paragraph with no newline. If the tree-sitter node's end_byte
    // lands on `source.len() + 1` (the virtual normalization byte),
    // `source.get(start..end)` returns None and `block_text` yields "".
    let source = "# Title\n\nthe trailing paragraph";
    let idx = index(source);

    let blocks = idx.content_blocks();
    assert!(!blocks.is_empty(), "expected at least one content block");

    let texts: Vec<&str> = blocks.iter().map(|b| idx.block_text(b)).collect();
    assert!(
        texts.iter().any(|t| t.contains("trailing paragraph")),
        "trailing paragraph text is empty — block byte range was computed \
         against the normalized source but stored against the un-normalized \
         one, so source.get(start..end) returned None. texts={texts:?}"
    );
}

// ---------------------------------------------------------------------------
// Defensive coverage: variations that might hit the same pathological end_byte
// ---------------------------------------------------------------------------

#[test]
fn single_byte_list_marker_does_not_panic() {
    // Smallest possible list: one byte. Exercises the minimum boundary of the
    // bug and any other size-1 edge cases in byte range arithmetic.
    let idx = index("-");
    let _ = idx.content_blocks();
}

#[test]
fn crlf_line_endings_with_trailing_list_item_no_final_eol() {
    // CRLF line endings with no trailing EOL at all. Different normalization
    // shape from LF — exercises whether the fix handles both.
    let source = "# Title\r\n\r\n- item";
    let idx = index(source);
    let blocks = idx.content_blocks();
    assert!(
        !blocks.is_empty(),
        "CRLF file with trailing list item and no final EOL must still index"
    );
}

#[test]
fn bom_prefix_and_trailing_list_item() {
    // UTF-8 BOM + list item, no trailing newline. BOM adds three leading bytes
    // that some parsers silently skip — the question is whether tree-sitter's
    // byte positions agree with the file the caller has on disk.
    let source = "\u{feff}- item";
    let idx = index(source);
    let _ = idx.content_blocks();
}

#[test]
fn four_space_indented_list_item_no_newline() {
    // The pathological nested-list case minimized: one item with four leading
    // spaces (would be an indented code block in some grammars) and no EOL.
    let source = "    - item";
    let idx = index(source);
    let _ = idx.content_blocks();
}
