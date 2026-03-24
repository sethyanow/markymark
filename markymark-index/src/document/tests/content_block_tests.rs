//! Content block extraction tests (marky-3cy / marky-7pak).
//!
//! Tests tree-sitter AST extraction of paragraphs, list items, code blocks,
//! blockquotes, thematic breaks, and tables into ContentBlock entries.

use super::*;

fn build_index(source: &str) -> DocumentIndex {
    DocumentIndex::from_text(source)
}

// ---------------------------------------------------------------------------
// Basic extraction: one block kind per test
// ---------------------------------------------------------------------------

#[test]
fn extract_paragraph() {
    let index = build_index("# Heading\n\nHello world.\n");
    let blocks = index.content_blocks();
    assert!(!blocks.is_empty(), "should extract paragraph block");
    assert!(
        blocks.iter().any(|b| b.kind == BlockKind::Paragraph),
        "should have a Paragraph block"
    );
}

#[test]
fn extract_list_items() {
    let source = "# Heading\n\n- item one\n- item two\n- item three\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    let list_items: Vec<_> = blocks
        .iter()
        .filter(|b| b.kind == BlockKind::ListItem)
        .collect();
    assert_eq!(
        list_items.len(),
        3,
        "should extract 3 flat list items, got {}",
        list_items.len()
    );
}

#[test]
fn extract_fenced_code_block() {
    let source = "# Code\n\n```rust\nfn main() {}\n```\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(
        blocks.iter().any(|b| b.kind == BlockKind::CodeBlock),
        "should extract fenced code block"
    );
}

#[test]
fn extract_blockquote() {
    let source = "# Quote\n\n> This is a quote.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(
        blocks.iter().any(|b| b.kind == BlockKind::BlockQuote),
        "should extract blockquote"
    );
}

#[test]
fn extract_thematic_break() {
    let source = "# Break\n\nAbove.\n\n---\n\nBelow.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(
        blocks.iter().any(|b| b.kind == BlockKind::ThematicBreak),
        "should extract thematic break"
    );
}

#[test]
fn extract_table() {
    let source = "# Table\n\n| A | B |\n| - | - |\n| 1 | 2 |\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(
        blocks.iter().any(|b| b.kind == BlockKind::Table),
        "should extract table"
    );
}

// ---------------------------------------------------------------------------
// Parent heading assignment
// ---------------------------------------------------------------------------

#[test]
fn parent_heading_assigned() {
    let source = "# First\n\nParagraph under first.\n\n## Second\n\nParagraph under second.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(blocks.len() >= 2, "should have at least 2 blocks");

    // First paragraph should reference heading index 0
    let first_para = &blocks[0];
    assert_eq!(
        first_para.parent_heading,
        Some(0),
        "first para under heading 0"
    );

    // Second paragraph should reference heading index 1
    let second_para = &blocks[1];
    assert_eq!(
        second_para.parent_heading,
        Some(1),
        "second para under heading 1"
    );
}

#[test]
fn blocks_before_heading_have_no_parent() {
    let source = "Paragraph before any heading.\n\n# Heading\n\nParagraph after heading.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    assert!(blocks.len() >= 2, "should have at least 2 blocks");

    let first_para = &blocks[0];
    assert_eq!(
        first_para.parent_heading, None,
        "block before any heading should have parent_heading=None"
    );

    let second_para = &blocks[1];
    assert_eq!(
        second_para.parent_heading,
        Some(0),
        "block after heading should have parent_heading=Some(0)"
    );
}

// ---------------------------------------------------------------------------
// block_text() on extracted blocks
// ---------------------------------------------------------------------------

#[test]
fn block_text_for_extracted_paragraph() {
    let source = "# Heading\n\nHello world paragraph.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    let para = blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .expect("should have paragraph");
    let text = index.block_text(para);
    assert!(
        text.contains("Hello world paragraph"),
        "block_text should contain paragraph content, got: {text:?}"
    );
}

#[test]
fn block_text_for_code_block() {
    let source = "# Code\n\n```rust\nfn main() {}\n```\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    let code = blocks
        .iter()
        .find(|b| b.kind == BlockKind::CodeBlock)
        .expect("should have code block");
    let text = index.block_text(code);
    assert!(
        text.contains("fn main()"),
        "block_text should contain code content, got: {text:?}"
    );
}

// ---------------------------------------------------------------------------
// ^block-id merge
// ---------------------------------------------------------------------------

#[test]
fn block_id_merged_into_content_block() {
    let source = "# Heading\n\nThis is a paragraph. ^my-block\n";
    let index = build_index(source);

    // block_by_id should still work (backward compat)
    let block = index.block_by_id("my-block");
    assert!(block.is_some(), "block_by_id should find ^my-block");

    // content_blocks should have the paragraph
    let blocks = index.content_blocks();
    let para = blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .expect("should have paragraph");

    // The paragraph's block_id should be set if byte ranges overlap
    // (The ^block-id marker is within the paragraph's byte range)
    assert_eq!(
        para.block_id,
        Some("my-block"),
        "paragraph should have block_id merged from ^my-block marker"
    );
}

// ---------------------------------------------------------------------------
// Frontmatter exclusion
// ---------------------------------------------------------------------------

#[test]
fn frontmatter_blocks_excluded() {
    let source = "---\ntitle: Hello\n---\n\n# Heading\n\nParagraph.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    // Should only have the paragraph, not any block from the frontmatter region
    for b in blocks {
        let text = index.block_text(b);
        assert!(
            !text.contains("title"),
            "frontmatter content should not appear in content blocks"
        );
    }
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_document_no_blocks() {
    let index = build_index("");
    assert!(
        index.content_blocks().is_empty(),
        "empty document should have no content blocks"
    );
}

#[test]
fn headings_only_no_blocks() {
    let index = build_index("# First\n\n## Second\n\n### Third\n");
    assert!(
        index.content_blocks().is_empty(),
        "document with only headings should have no content blocks"
    );
}

#[test]
fn content_blocks_ordered_by_position() {
    let source = "# Heading\n\nFirst paragraph.\n\nSecond paragraph.\n\n- list item\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    for window in blocks.windows(2) {
        assert!(
            window[0].start_byte <= window[1].start_byte,
            "content blocks should be ordered by start_byte: {} > {}",
            window[0].start_byte,
            window[1].start_byte
        );
    }
}

#[test]
fn logseq_heading_not_extracted_as_list_item() {
    // Logseq-style heading: `- # Heading` should be treated as HeadingEntry,
    // NOT extracted as a ListItem content block.
    let source = "- # Logseq Heading\n\nParagraph.\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    // Should NOT have a ListItem for the logseq heading
    let list_items: Vec<_> = blocks
        .iter()
        .filter(|b| b.kind == BlockKind::ListItem)
        .collect();
    assert!(
        list_items.is_empty(),
        "logseq-style heading should not produce ListItem content block"
    );
}

#[test]
fn ordered_list_items_extracted() {
    let source = "# List\n\n1. first\n2. second\n3. third\n";
    let index = build_index(source);
    let blocks = index.content_blocks();
    let list_items: Vec<_> = blocks
        .iter()
        .filter(|b| b.kind == BlockKind::ListItem)
        .collect();
    assert_eq!(
        list_items.len(),
        3,
        "should extract 3 ordered list items, got {}",
        list_items.len()
    );
}

// from_scan_path_has_empty_content_blocks removed — scan path deleted (marky-0xtn)

#[test]
fn multiple_block_kinds_mixed() {
    let source = "\
# Mixed Document

A paragraph.

- list item one
- list item two

> a blockquote

```
code block
```

---

Another paragraph.
";
    let index = build_index(source);
    let blocks = index.content_blocks();

    let kinds: Vec<_> = blocks.iter().map(|b| b.kind).collect();
    assert!(
        kinds.contains(&BlockKind::Paragraph),
        "should have paragraphs"
    );
    assert!(
        kinds.contains(&BlockKind::ListItem),
        "should have list items"
    );
    assert!(
        kinds.contains(&BlockKind::BlockQuote),
        "should have blockquote"
    );
    assert!(
        kinds.contains(&BlockKind::CodeBlock),
        "should have code block"
    );
    assert!(
        kinds.contains(&BlockKind::ThematicBreak),
        "should have thematic break"
    );
}

#[test]
fn document_with_only_frontmatter_no_blocks() {
    let source = "---\ntitle: Hello\n---\n";
    let index = build_index(source);
    assert!(
        index.content_blocks().is_empty(),
        "document with only frontmatter should have no content blocks"
    );
}
