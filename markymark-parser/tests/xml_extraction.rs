use markymark_parser::Parser;

// ===========================================================================
// Basic HTML/XML Tags
// ===========================================================================

#[test]
fn extract_simple_opening_closing_tag() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<div>content</div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "div");
    assert!(!tags[0].is_self_closing());
}

#[test]
fn extract_self_closing_tag() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<br/>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "br");
    assert!(tags[0].is_self_closing());
}

#[test]
fn extract_tag_with_attributes() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<div class=\"note\" id=\"main\">text</div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "div");

    let attrs = tags[0].attributes();
    assert_eq!(attrs.get("class"), Some(&"note"));
    assert_eq!(attrs.get("id"), Some(&"main"));
}

#[test]
fn extract_void_html_element() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<img src=\"photo.jpg\">\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "img");
    assert!(tags[0].is_self_closing());
    assert_eq!(tags[0].attributes().get("src"), Some(&"photo.jpg"));
}

// ===========================================================================
// AGENTS.md / AI-Oriented XML Tags (PRIMARY USE CASE)
// ===========================================================================

#[test]
fn extract_agent_tag() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<agent>content</agent>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "agent");
    assert_eq!(tags[0].content(), Some("content"));
}

#[test]
fn extract_nested_custom_tags() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<agent>\n<goal>Build X</goal>\n</agent>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    // Should find both the outer <agent> and inner <goal>
    assert!(tags.len() >= 2);

    let tag_names: Vec<&str> = tags.iter().map(|t| t.tag_name()).collect();
    assert!(tag_names.contains(&"agent"));
    assert!(tag_names.contains(&"goal"));
}

#[test]
fn extract_tag_with_markdown_content() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<prompt>Complex prompt text with **markdown** inside</prompt>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "prompt");
    // Content should preserve the inner text including markdown syntax
    let content = tags[0].content().unwrap();
    assert!(content.contains("**markdown**"));
}

#[test]
fn extract_document_level_tags() {
    let mut parser = Parser::new().unwrap();
    let markdown = "# Heading\n\n<agent>\nSome agent instructions\n</agent>\n\nMore text\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "agent");
}

// ===========================================================================
// HTML Blocks in Markdown
// ===========================================================================

#[test]
fn extract_html_block() {
    let mut parser = Parser::new().unwrap();
    let markdown =
        "Paragraph before.\n\n<div>\n  <p>HTML block content</p>\n</div>\n\nParagraph after.\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    let tag_names: Vec<&str> = tags.iter().map(|t| t.tag_name()).collect();
    assert!(tag_names.contains(&"div"));
    assert!(tag_names.contains(&"p"));
}

#[test]
fn extract_inline_html() {
    let mut parser = Parser::new().unwrap();
    let markdown = "Some text <strong>bold</strong> more text\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "strong");
    assert_eq!(tags[0].content(), Some("bold"));
}

// ===========================================================================
// Edge Cases
// ===========================================================================

#[test]
fn extract_empty_tag() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<div></div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "div");
    // Empty tag should have empty or None content
    let content = tags[0].content();
    assert!(content.is_none() || content == Some(""));
}

#[test]
fn extract_tag_with_whitespace_content() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<div>   </div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "div");
}

#[test]
fn extract_multiple_tags_same_line() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<a>first</a> <b>second</b>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 2);

    let tag_names: Vec<&str> = tags.iter().map(|t| t.tag_name()).collect();
    assert!(tag_names.contains(&"a"));
    assert!(tag_names.contains(&"b"));
}

#[test]
fn extract_deeply_nested_tags() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<a><b><c>deep</c></b></a>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert!(tags.len() >= 3);

    let tag_names: Vec<&str> = tags.iter().map(|t| t.tag_name()).collect();
    assert!(tag_names.contains(&"a"));
    assert!(tag_names.contains(&"b"));
    assert!(tag_names.contains(&"c"));
}

// ===========================================================================
// Range tracking
// ===========================================================================

#[test]
fn xml_tag_preserves_range() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<agent>content</agent>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    let range = tags[0].range();

    // Tag starts at the beginning of the line
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 0);
}

// ===========================================================================
// Attributes edge cases
// ===========================================================================

#[test]
fn extract_tag_no_attributes_has_empty_map() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<agent>content</agent>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert!(tags[0].attributes().is_empty());
}

#[test]
fn extract_tag_with_single_attribute() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<div class=\"important\">text</div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    let attrs = tags[0].attributes();
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs.get("class"), Some(&"important"));
}

// ===========================================================================
// Self-closing same-name nesting (regression test for PR #8 review)
// ===========================================================================

#[test]
fn extract_self_closing_same_name_inside_opening_tag() {
    let mut parser = Parser::new().unwrap();
    let markdown = "<agent><agent/></agent>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    // Should find both: the outer <agent>...</agent> and the inner <agent/>
    assert_eq!(tags.len(), 2);

    let self_closing: Vec<_> = tags.iter().filter(|t| t.is_self_closing()).collect();
    let non_self_closing: Vec<_> = tags.iter().filter(|t| !t.is_self_closing()).collect();

    assert_eq!(self_closing.len(), 1);
    assert_eq!(self_closing[0].tag_name(), "agent");

    assert_eq!(non_self_closing.len(), 1);
    assert_eq!(non_self_closing[0].tag_name(), "agent");
    assert!(non_self_closing[0].content().is_some());
}

#[test]
fn extract_attribute_with_gt_in_value() {
    let mut parser = Parser::new().unwrap();
    // Attribute value contains > which should not break tag detection
    let markdown = "<div data-expr=\"a > b\">content</div>\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "div");
    assert_eq!(tags[0].attributes().get("data-expr"), Some(&"a > b"));
    assert_eq!(tags[0].content(), Some("content"));
}

// ===========================================================================
// Fenced code blocks
// ===========================================================================

#[test]
fn ignore_xml_like_generics_inside_fenced_code_blocks() {
    let mut parser = Parser::new().unwrap();
    let markdown = concat!(
        "<agent>outside</agent>\n\n",
        "```rust\n",
        "fn wrap<T>(value: T) -> std::sync::Arc<std::sync::Mutex<T>> {\n",
        "    std::sync::Arc::new(std::sync::Mutex::new(value))\n",
        "}\n",
        "```\n\n",
        "~~~rust\n",
        "fn dyn_box(value: Box<dyn std::fmt::Display>) {}\n",
        "~~~\n",
    );

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name(), "agent");
    assert_eq!(tags[0].content(), Some("outside"));
}

// ===========================================================================
// No XML tags
// ===========================================================================

#[test]
fn no_xml_tags_in_plain_markdown() {
    let mut parser = Parser::new().unwrap();
    let markdown = "# Heading\n\nJust a paragraph with **bold** and *italic*.\n";

    let ast = parser.parse(markdown).unwrap();
    let tags = ast.extract_xml_tags();

    assert!(tags.is_empty());
}
