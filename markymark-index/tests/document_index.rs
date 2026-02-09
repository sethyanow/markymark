use markymark_index::DocumentIndex;
use markymark_parser::Parser;

// Types used indirectly via DocumentIndex methods:
// HeadingEntry, BlockEntry, TocEntry, OutlineNode,
// WikiLinkEntry, TagEntry, MarkdownLinkEntry

/// Helper: parse markdown source and build a DocumentIndex.
fn index_from(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse");
    DocumentIndex::from_ast(&ast)
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
    assert_eq!(headings[1].slug, "special-chrs");
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
    assert_eq!(block.unwrap().id, "my-block-id");
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
    assert_eq!(links[1].alias.as_deref(), Some("alias"));
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

#[test]
fn test_tags_indexed() {
    let idx = index_from("Some text #tag1 and #project/feature");
    let tags = idx.tags();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "tag1");
    assert_eq!(tags[1].name, "project/feature");
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
    assert_eq!(links[1].url, "./docs.md#section");
    assert_eq!(links[1].anchor.as_deref(), Some("section"));
}
