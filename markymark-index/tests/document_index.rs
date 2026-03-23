use markymark_index::DocumentIndex;

/// Helper: build a DocumentIndex from raw text via the engine path.
fn index_from(source: &str) -> DocumentIndex {
    DocumentIndex::from_text(source)
}

// ---------------------------------------------------------------------------
// Heading indexing
// ---------------------------------------------------------------------------

#[test]
fn test_empty_document_index() {
    let idx = index_from("");
    assert!(
        idx.headings().is_empty(),
        "empty doc should have no headings"
    );
    assert!(idx.toc().is_empty(), "empty doc should have no TOC entries");
    assert!(
        idx.wiki_links().is_empty(),
        "empty doc should have no wiki links"
    );
    assert!(idx.tags().is_empty(), "empty doc should have no tags");
}

#[test]
fn test_single_heading_index() {
    let idx = index_from("# Hello");
    let headings = idx.headings();
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].text, "Hello");
    assert_eq!(headings[0].slug, "hello");
    assert_eq!(headings[0].level, 1);
}

#[test]
fn test_multiple_headings_index() {
    let idx = index_from("# Title\n\n## Section\n\n### Subsection");
    let headings = idx.headings();
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[1].level, 2);
    assert_eq!(headings[2].level, 3);
}

#[test]
fn test_heading_slug_generation() {
    let idx = index_from("# Hello World\n\n## Special Ch@rs!\n\n### Already-slugged");
    let headings = idx.headings();

    assert_eq!(headings[0].slug, "hello-world");
    // Engine slugifies @ as '-' (then dash-collapse produces "special-ch-rs")
    // vs the Rust slugify which strips @ entirely ("special-chrs").
    // Engine behavior is canonical since LSP/MCP use it.
    assert_eq!(headings[1].slug, "special-ch-rs");
    assert_eq!(headings[2].slug, "already-slugged");
}

#[test]
fn test_heading_lookup_by_slug() {
    let idx = index_from("# Introduction\n\n## Details");
    let found = idx.heading_by_slug("introduction");
    assert!(found.is_some());
    let heading = found.unwrap();
    assert_eq!(heading.text, "Introduction");
    assert_eq!(heading.level, 1);
}

#[test]
fn test_heading_lookup_miss() {
    let idx = index_from("# Present");
    assert!(
        idx.heading_by_slug("absent").is_none(),
        "missing slug should return None"
    );
}

#[test]
fn test_duplicate_heading_slugs() {
    let idx = index_from("# Foo\n\n## Foo\n\n### Foo");
    let headings = idx.headings();
    assert_eq!(headings.len(), 3);

    // First occurrence keeps the bare slug
    assert_eq!(headings[0].slug, "foo");
    // Subsequent duplicates get a numeric suffix
    assert_eq!(headings[1].slug, "foo-1");
    assert_eq!(headings[2].slug, "foo-2");
}

// ---------------------------------------------------------------------------
// Block ID indexing
// ---------------------------------------------------------------------------

#[test]
fn test_block_id_index() {
    let source = "Some paragraph ^my-block-id";
    let idx = index_from(source);
    let block = idx.block_by_id("my-block-id");
    assert!(block.is_some(), "block ID should be indexed");
    let b = block.unwrap();
    assert_eq!(b.id, "my-block-id");
    // Range propagates from source for go-to-definition (not 0,0,0,0)
    assert_eq!(b.range.start.line, 0);
    assert_eq!(b.range.start.character, 15); // position of ^ in "Some paragraph ^"
    assert_eq!(b.range.end.character, 27); // exclusive end of "my-block-id"
}

// ---------------------------------------------------------------------------
// Table of contents
// ---------------------------------------------------------------------------

#[test]
fn test_toc_generation() {
    let idx = index_from("# Title\n\n## Section A\n\n## Section B");
    let toc = idx.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].text, "Title");
    assert_eq!(toc[0].level, 1);
    assert_eq!(toc[1].text, "Section A");
    assert_eq!(toc[1].level, 2);
    assert_eq!(toc[2].text, "Section B");
    assert_eq!(toc[2].level, 2);
}

#[test]
fn test_toc_nested_headings() {
    let idx = index_from("# Root\n\n## Child\n\n### Grandchild\n\n## Child 2");
    let toc = idx.toc();
    assert_eq!(toc.len(), 4);

    // depth reflects nesting under the first h1
    assert_eq!(toc[0].depth, 0); // # Root
    assert_eq!(toc[1].depth, 1); // ## Child
    assert_eq!(toc[2].depth, 2); // ### Grandchild
    assert_eq!(toc[3].depth, 1); // ## Child 2
}

// ---------------------------------------------------------------------------
// Outline tree
// ---------------------------------------------------------------------------

#[test]
fn test_outline_tree() {
    let idx = index_from("# Root\n\n## Child A\n\n### Grandchild\n\n## Child B");
    let outline = idx.outline();

    // Root outline node has no heading itself (it's the document root)
    assert!(outline.heading.is_none());

    // Top-level child: the h1
    assert_eq!(outline.children.len(), 1);
    let root_h1 = &outline.children[0];
    assert_eq!(root_h1.heading.as_ref().unwrap().text, "Root");

    // h1 has two h2 children
    assert_eq!(root_h1.children.len(), 2);
    assert_eq!(
        root_h1.children[0].heading.as_ref().unwrap().text,
        "Child A"
    );
    assert_eq!(
        root_h1.children[1].heading.as_ref().unwrap().text,
        "Child B"
    );

    // First h2 has one h3 grandchild
    assert_eq!(root_h1.children[0].children.len(), 1);
    assert_eq!(
        root_h1.children[0].children[0]
            .heading
            .as_ref()
            .unwrap()
            .text,
        "Grandchild"
    );
}

// ---------------------------------------------------------------------------
// Wiki links
// ---------------------------------------------------------------------------

#[test]
fn test_wiki_links_indexed() {
    let idx = index_from("Check [[PageA]] and [[PageB|alias]].");
    let links = idx.wiki_links();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].target, "PageA");
    assert_eq!(links[1].target, "PageB");
    assert_eq!(links[1].alias, Some("alias"));
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

#[test]
fn test_tags_indexed() {
    // md4c/Zig scanner treats '/' as a tag boundary, so #project/feature
    // yields "project" (not "project/feature" as the old regex extractor did).
    let idx = index_from("Some text #tag1 and #project/feature");
    let tags = idx.tags();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "tag1");
    assert_eq!(tags[1].name, "project");
}

// ---------------------------------------------------------------------------
// Markdown links
// ---------------------------------------------------------------------------

#[test]
fn test_markdown_links_indexed() {
    let idx = index_from("See [Google](https://google.com) and [Docs](./docs.md#section).");
    let links = idx.markdown_links();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].text, "Google");
    assert_eq!(links[0].url, "https://google.com");
    assert_eq!(links[1].text, "Docs");
    assert_eq!(links[1].url, "./docs.md");
    assert_eq!(links[1].anchor, Some("section"));
}

// ---------------------------------------------------------------------------
// XML tag indexing
// ---------------------------------------------------------------------------

#[test]
fn test_empty_document_has_no_xml_tags() {
    let idx = index_from("");
    assert!(
        idx.xml_tags().is_empty(),
        "empty doc should have no XML tags"
    );
}

#[test]
fn test_single_xml_tag_indexed() {
    // md4c requires block-level HTML (tag on its own line) for XML extraction.
    let idx = index_from("<agent>\nsome content\n</agent>\n");
    let xml = idx.xml_tags();
    assert_eq!(xml.len(), 1);
    assert_eq!(xml[0].tag_name, "agent");
    assert!(!xml[0].is_self_closing);
}

#[test]
fn test_self_closing_xml_tag() {
    let idx = index_from("<br/>");
    let xml = idx.xml_tags();
    assert_eq!(xml.len(), 1);
    assert_eq!(xml[0].tag_name, "br");
    assert!(xml[0].is_self_closing);
}

#[test]
fn test_xml_tag_with_attributes() {
    // md4c requires block-level HTML. Blob path does not preserve per-tag
    // attributes (BlobXmlTag stores name/range/flags only).
    let idx = index_from("<goal>\nwin\n</goal>\n");
    let xml = idx.xml_tags();
    assert_eq!(xml.len(), 1);
    assert_eq!(xml[0].tag_name, "goal");
}

#[test]
fn test_multiple_xml_tags() {
    // Each tag on its own block-level HTML block (separated by blank lines).
    let idx = index_from("<agent>\nA\n</agent>\n\n<goal>\nB\n</goal>\n\n<task>\nC\n</task>\n");
    let xml = idx.xml_tags();
    assert_eq!(xml.len(), 3);
    assert_eq!(xml[0].tag_name, "agent");
    assert_eq!(xml[1].tag_name, "goal");
    assert_eq!(xml[2].tag_name, "task");
}

#[test]
fn test_xml_tags_mixed_with_markdown() {
    // Block-level HTML between markdown blocks (separated by blank lines).
    let idx = index_from("# Heading\n\n<agent>\ncontent\n</agent>\n\nSome paragraph");
    assert_eq!(idx.headings().len(), 1);
    assert_eq!(idx.xml_tags().len(), 1);
    assert_eq!(idx.xml_tags()[0].tag_name, "agent");
}

#[test]
fn test_xml_tag_range_tracked() {
    // Block-level HTML for md4c extraction.
    let idx = index_from("<agent>\ncontent\n</agent>\n");
    let xml = idx.xml_tags();
    assert_eq!(xml.len(), 1);
    // Range should cover the tag — at minimum start at line 0
    assert_eq!(xml[0].range.start.line, 0);
}
