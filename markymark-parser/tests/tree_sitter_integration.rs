use markymark_parser::Parser;

#[test]
fn parser_new_creates_instance() {
    let parser = Parser::new();
    assert!(parser.is_ok());
}

#[test]
fn parse_empty_document_returns_empty_ast() {
    let mut parser = Parser::new().unwrap();
    let result = parser.parse("");

    assert!(result.is_ok());
    let ast = result.unwrap();
    assert_eq!(ast.root_elements().len(), 0);
}

#[test]
fn parse_simple_heading_extracts_text_and_level() {
    let mut parser = Parser::new().unwrap();
    let markdown = "# Hello World\n";
    let result = parser.parse(markdown);

    assert!(result.is_ok());
    let ast = result.unwrap();
    let elements = ast.root_elements();

    assert_eq!(elements.len(), 1);

    let heading = elements[0].as_heading().expect("should be heading");
    assert_eq!(heading.level(), 1);
    assert_eq!(heading.text(), "Hello World");
}

#[test]
fn parse_multiple_heading_levels() {
    let mut parser = Parser::new().unwrap();
    let markdown = r#"# Level 1
## Level 2
### Level 3
"#;

    let result = parser.parse(markdown);
    assert!(result.is_ok());

    let ast = result.unwrap();
    let elements = ast.root_elements();
    assert_eq!(elements.len(), 3);

    assert_eq!(elements[0].as_heading().unwrap().level(), 1);
    assert_eq!(elements[1].as_heading().unwrap().level(), 2);
    assert_eq!(elements[2].as_heading().unwrap().level(), 3);
}

#[test]
fn parse_preserves_heading_range() {
    let mut parser = Parser::new().unwrap();
    let markdown = "# Heading\n";

    let ast = parser.parse(markdown).unwrap();
    let heading = ast.root_elements()[0].as_heading().unwrap();

    let range = heading.range();
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 0);
    // tree-sitter-md includes the trailing newline in heading nodes,
    // so end position is at the start of the next line (line 1, col 0)
    assert!(range.end.line <= 1);
}

#[test]
fn parse_paragraph_with_inline_text() {
    let mut parser = Parser::new().unwrap();
    let markdown = "This is a paragraph.\n";

    let ast = parser.parse(markdown).unwrap();
    let elements = ast.root_elements();

    assert_eq!(elements.len(), 1);
    let para = elements[0].as_paragraph().expect("should be paragraph");
    assert_eq!(para.text(), "This is a paragraph.");
}

#[test]
fn incremental_edit_updates_ast() {
    let mut parser = Parser::new().unwrap();
    let original = "# Original\n";

    let mut ast = parser.parse(original).unwrap();
    assert_eq!(
        ast.root_elements()[0].as_heading().unwrap().text(),
        "Original"
    );

    // Simulate edit: replace "Original" with "Updated"
    let new_text = "# Updated\n";
    ast = parser
        .parse_incremental(&ast, new_text, 2, 10, 2, 9)
        .unwrap();

    assert_eq!(
        ast.root_elements()[0].as_heading().unwrap().text(),
        "Updated"
    );
}
