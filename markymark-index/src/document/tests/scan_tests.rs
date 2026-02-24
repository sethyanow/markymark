//! Scan-based (ZigScanBackend) construction tests.

use super::*;
use markymark_core::scanner::ZigScanBackend;

fn build_index_from_scan(source: &str) -> DocumentIndex {
    let backend = ZigScanBackend;
    DocumentIndex::from_scan(source, &backend)
}

fn build_index_from_ast(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    DocumentIndex::from_ast(ast)
}

#[test]
fn test_from_scan_empty_document() {
    let index = build_index_from_scan("");
    assert!(index.headings().is_empty());
    assert!(index.wiki_links().is_empty());
    assert!(index.tags().is_empty());
    assert!(index.markdown_links().is_empty());
    assert!(index.toc().is_empty());
}

#[test]
fn test_from_scan_single_heading() {
    let index = build_index_from_scan("# Hello\n");
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Hello");
    assert_eq!(index.headings()[0].level, 1);
    assert_eq!(index.headings()[0].slug, "hello");
}

#[test]
fn test_from_scan_multiple_headings() {
    let index = build_index_from_scan("# First\n\n## Second\n\n### Third\n");
    assert_eq!(index.headings().len(), 3);
    assert_eq!(index.headings()[0].level, 1);
    assert_eq!(index.headings()[1].level, 2);
    assert_eq!(index.headings()[2].level, 3);
    assert!(index.heading_by_slug("first").is_some());
    assert!(index.heading_by_slug("second").is_some());
}

#[test]
fn test_from_scan_toc_builds() {
    let index = build_index_from_scan("# Root\n\n## Child\n\n### Grandchild\n");
    let toc = index.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].depth, 0);
    assert_eq!(toc[1].depth, 1);
    assert_eq!(toc[2].depth, 2);
}

#[test]
fn test_from_scan_outline_builds() {
    let index = build_index_from_scan("# Root\n\n## Child\n");
    let outline = index.outline();
    assert_eq!(outline.children.len(), 1);
    assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
}

#[test]
fn test_from_scan_markdown_links() {
    let index = build_index_from_scan("See [example](https://example.com) here\n");
    assert_eq!(index.markdown_links().len(), 1);
    assert_eq!(index.markdown_links()[0].text, "example");
    assert_eq!(index.markdown_links()[0].url, "https://example.com");
}

#[test]
fn test_from_scan_wiki_links() {
    let index = build_index_from_scan("See [[My Page]] here\n");
    assert_eq!(index.wiki_links().len(), 1);
    assert_eq!(index.wiki_links()[0].target, "My Page");
}

#[test]
fn test_from_scan_tags() {
    let index = build_index_from_scan("text #topic #project\n");
    assert_eq!(index.tags().len(), 2);
    assert!(index.tags().iter().any(|t| t.name == "topic"));
    assert!(index.tags().iter().any(|t| t.name == "project"));
}

#[test]
fn test_from_scan_block_ids() {
    let index = build_index_from_scan("some content ^my-block\n");
    assert!(index.block_by_id("my-block").is_some());
}

#[test]
fn test_from_scan_xml_tags_empty() {
    let index = build_index_from_scan("<goal>Ship</goal>\n");
    assert!(index.xml_tags().is_empty());
}

#[test]
fn test_from_ast_unchanged() {
    let index = build_index_from_ast("# Heading\n\n[[Page]]\n#tag\n");
    assert_eq!(index.headings()[0].text, "Heading");
    assert!(!index.wiki_links().is_empty());
    assert!(index.tags().iter().any(|t| t.name == "tag"));
}

#[test]
fn test_parity_headings() {
    let text = "# First\n\n## Second\n\n### Third\n";
    let ast_idx = build_index_from_ast(text);
    let scan_idx = build_index_from_scan(text);

    assert_eq!(ast_idx.headings().len(), scan_idx.headings().len());
    for (a, s) in ast_idx.headings().iter().zip(scan_idx.headings().iter()) {
        assert_eq!(a.text, s.text);
        assert_eq!(a.level, s.level);
        assert_eq!(a.slug, s.slug);
    }
}

// --- Bug fix tests: wiki link range calculation (marky-x3x #1) ---

#[test]
fn test_from_scan_wiki_link_range_no_alias() {
    let index = build_index_from_scan("See [[My Page]] here\n");
    let wl = &index.wiki_links()[0];
    assert_eq!(wl.target, "My Page");
    assert_eq!(wl.range.start, Position::new(0, 4));
    assert_eq!(wl.range.end, Position::new(0, 15));
}

#[test]
fn test_from_scan_wiki_link_range_with_alias() {
    let index = build_index_from_scan("See [[target|display]] here\n");
    let wl = &index.wiki_links()[0];
    assert_eq!(wl.target, "target");
    assert!(wl.alias.is_some());
    assert_eq!(wl.alias.unwrap(), "display");
    assert_eq!(wl.range.start, Position::new(0, 4));
    assert_eq!(wl.range.end, Position::new(0, 22));
}

#[test]
fn test_from_scan_markdown_link_range() {
    let index = build_index_from_scan("See [example](https://example.com) here\n");
    let ml = &index.markdown_links()[0];
    assert_eq!(ml.text, "example");
    assert_eq!(ml.range.start, Position::new(0, 4));
    assert_eq!(ml.range.end, Position::new(0, 34));
}

// --- Bug fix test: block ID range (marky-x3x #2) ---

#[test]
fn test_from_scan_block_id_range_nonzero_width() {
    let index = build_index_from_scan("some content ^my-block\n");
    let block = index.block_by_id("my-block").unwrap();
    assert_eq!(block.range.start, Position::new(0, 13));
    assert_eq!(block.range.end, Position::new(0, 22));
    assert_ne!(block.range.start, block.range.end);
}
