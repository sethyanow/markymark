use markymark_parser::Parser;

#[test]
fn extract_standard_markdown_link() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [link text](https://example.com) for more.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_markdown_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].text(), "link text");
    assert_eq!(links[0].url(), "https://example.com");
}

#[test]
fn extract_markdown_link_with_anchor() {
    let mut parser = Parser::new().unwrap();
    let markdown = "[Section](doc.md#heading)\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_markdown_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].url(), "doc.md");
    assert_eq!(links[0].anchor(), Some("heading"));
}

#[test]
fn extract_reference_style_link() {
    let mut parser = Parser::new().unwrap();
    let markdown = r#"[link text][ref]

[ref]: https://example.com "Title"
"#;

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_markdown_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].text(), "link text");
    assert_eq!(links[0].reference(), Some("ref"));
}

#[test]
fn extract_link_definitions() {
    let mut parser = Parser::new().unwrap();
    let markdown = "[ref]: https://example.com \"Optional Title\"\n";

    let ast = parser.parse(markdown).unwrap();
    let defs = ast.extract_link_definitions();

    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].label(), "ref");
    assert_eq!(defs[0].url(), "https://example.com");
    assert_eq!(defs[0].title(), Some("Optional Title"));
}

#[test]
fn extract_obsidian_block_id() {
    let mut parser = Parser::new().unwrap();
    let markdown = "This paragraph has a block ID. ^block123\n";

    let ast = parser.parse(markdown).unwrap();
    let blocks = ast.extract_block_ids();

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id(), "block123");
    // Range covers ^block123 on line 0 (0-indexed)
    let r = blocks[0].range();
    assert_eq!(r.start.line, 0);
    assert_eq!(r.start.character, 31); // position of ^ in "This paragraph has a block ID. ^"
    assert_eq!(r.end.character, 40); // exclusive end of ^block123 (no trailing newline)
}

#[test]
fn extract_logseq_block_ref() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Reference: ((507f1f77-bcf8-4f25-a06b-93c0e8e83e01))\n";

    let ast = parser.parse(markdown).unwrap();
    let refs = ast.extract_block_refs();

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].uuid(), "507f1f77-bcf8-4f25-a06b-93c0e8e83e01");
}

#[test]
fn extract_tags_simple() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Some text #tag1 and #tag2\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_tags();

    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name(), "tag1");
    assert_eq!(tags[1].name(), "tag2");
}

#[test]
fn extract_tags_nested() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Tags: #project/feature/bug\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name(), "project/feature/bug");
    assert_eq!(tags[0].segments(), vec!["project", "feature", "bug"]);
}

#[test]
fn extract_tags_multi_word() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Multi-word: #[[multi word tag]]\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name(), "multi word tag");
}
