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
    // Heading::from_node trims to last content child end; "Heading" ends at col 9.
    assert_eq!(range.end.line, 0);
    assert!(
        range.end.character > 0,
        "end character should be past the heading text"
    );
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
    use markymark_parser::{byte_to_point, InputEdit};

    let mut parser = Parser::new().unwrap();
    let mut source = String::from("# Original\n");

    let mut ast = parser.parse(&source).unwrap();
    assert_eq!(
        ast.root_elements()[0].as_heading().unwrap().text(),
        "Original"
    );

    // Take tree for incremental reuse
    let mut md_tree = ast.take_md_tree().unwrap();

    // Edit: replace "Original" (bytes 2..10) with "Updated" (7 bytes)
    let start_byte = 2;
    let old_end_byte = 10;
    let new_text = "Updated";
    let new_end_byte = start_byte + new_text.len();

    let start_position = byte_to_point(&source, start_byte);
    let old_end_position = byte_to_point(&source, old_end_byte);

    source.replace_range(start_byte..old_end_byte, new_text);

    let new_end_position = byte_to_point(&source, new_end_byte);

    md_tree.edit(&InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    });

    ast = parser.parse_with_old_tree(&source, Some(&md_tree)).unwrap();

    assert_eq!(
        ast.root_elements()[0].as_heading().unwrap().text(),
        "Updated"
    );
}

#[test]
fn incremental_parse_matches_full_reparse() {
    use markymark_parser::{byte_to_point, InputEdit};

    let mut parser = Parser::new().unwrap();
    let mut source = String::from(
        "# Hello World\n\nSome paragraph text here.\n\n## Section Two\n\nMore content.\n",
    );

    // Initial parse
    let mut ast = parser.parse(&source).unwrap();
    let mut md_tree = ast.take_md_tree().unwrap();

    // Edit: insert " Beautiful" after "Hello" (byte 7)
    let start_byte = 7;
    let old_end_byte = 7; // insertion, no deletion
    let insert_text = " Beautiful";
    let new_end_byte = start_byte + insert_text.len();

    let start_position = byte_to_point(&source, start_byte);
    let old_end_position = byte_to_point(&source, old_end_byte);

    source.replace_range(start_byte..old_end_byte, insert_text);

    let new_end_position = byte_to_point(&source, new_end_byte);

    md_tree.edit(&InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    });

    // Incremental parse
    let incremental_ast = parser.parse_with_old_tree(&source, Some(&md_tree)).unwrap();

    // Full parse of same final text
    let full_ast = parser.parse(&source).unwrap();

    // Compare: both should produce identical heading text
    let inc_headings: Vec<_> = incremental_ast
        .root_elements()
        .iter()
        .filter_map(|e| e.as_heading())
        .map(|h| (h.level(), h.text().to_string()))
        .collect();
    let full_headings: Vec<_> = full_ast
        .root_elements()
        .iter()
        .filter_map(|e| e.as_heading())
        .map(|h| (h.level(), h.text().to_string()))
        .collect();

    assert_eq!(inc_headings, full_headings);
    assert_eq!(inc_headings[0].1, "Hello Beautiful World");
    assert_eq!(inc_headings[1].1, "Section Two");
}

#[test]
fn incremental_100_sequential_edits() {
    use markymark_parser::{byte_to_point, InputEdit};

    let mut parser = Parser::new().unwrap();
    let mut source = String::from("# Title\n\nContent goes here.\n");

    let mut ast = parser.parse(&source).unwrap();
    let mut md_tree = ast.take_md_tree().unwrap();

    // Apply 100 single-char insertions at the end of "Content goes here"
    // Insert before the period at position of '.'
    for i in 0..100 {
        let insert_pos = source.find('.').unwrap();
        let ch = ((b'a' + (i % 26)) as char).to_string();
        let new_end_byte = insert_pos + ch.len();

        let start_position = byte_to_point(&source, insert_pos);
        let old_end_position = start_position;

        source.replace_range(insert_pos..insert_pos, &ch);

        let new_end_position = byte_to_point(&source, new_end_byte);

        md_tree.edit(&InputEdit {
            start_byte: insert_pos,
            old_end_byte: insert_pos,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        });

        // Re-parse incrementally after each edit
        ast = parser.parse_with_old_tree(&source, Some(&md_tree)).unwrap();
        md_tree = ast.take_md_tree().unwrap();
    }

    // Full parse of the final text
    let full_ast = parser.parse(&source).unwrap();

    // Verify heading is unchanged
    assert_eq!(ast.root_elements()[0].as_heading().unwrap().text(), "Title");
    assert_eq!(
        full_ast.root_elements()[0].as_heading().unwrap().text(),
        "Title"
    );

    // Verify both produce the same number of root elements
    assert_eq!(ast.root_elements().len(), full_ast.root_elements().len());
}
