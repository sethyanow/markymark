use markymark_parser::Parser;

#[test]
fn extract_simple_wiki_link() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [[Target Page]] for details.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page(), Some("Target Page"));
    assert_eq!(links[0].alias(), None);
}

#[test]
fn extract_wiki_link_with_alias() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [[Target Page|Display Text]] for details.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page(), Some("Target Page"));
    assert_eq!(links[0].alias(), Some("Display Text"));
}

#[test]
fn extract_wiki_link_to_heading() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [[Page#Section]] for details.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page(), Some("Page"));
    assert_eq!(links[0].target_heading(), Some("Section"));
}

#[test]
fn extract_wiki_link_to_current_page_heading() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [[#Introduction]] above.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page(), None);
    assert_eq!(links[0].target_heading(), Some("Introduction"));
}

#[test]
fn extract_wiki_link_to_block_id() {
    let mut parser = Parser::new().unwrap();
    let markdown = "See [[Page#^block123]] for context.\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page(), Some("Page"));
    assert_eq!(links[0].target_block_id(), Some("block123"));
}

#[test]
fn extract_multiple_wiki_links() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Links: [[Page1]], [[Page2]], and [[Page3]].\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 3);
    assert_eq!(links[0].target_page(), Some("Page1"));
    assert_eq!(links[1].target_page(), Some("Page2"));
    assert_eq!(links[2].target_page(), Some("Page3"));
}

#[test]
fn wiki_link_preserves_range() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Text [[Link]] more text\n";

    let ast = parser.parse(markdown).unwrap();
    let links = ast.extract_wiki_links();

    assert_eq!(links.len(), 1);
    let range = links[0].range();

    // [[Link]] starts at character 5, ends at character 13
    assert_eq!(range.start.character, 5);
    assert_eq!(range.end.character, 13);
}

#[test]
fn extract_obsidian_embed() {
    let mut parser = Parser::new().unwrap();
    let markdown = "![[image.png]]\n";

    let ast = parser.parse(markdown).unwrap();
    let embeds = ast.extract_embeds();

    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].target(), "image.png");
    assert_eq!(embeds[0].is_embed(), true);
}
