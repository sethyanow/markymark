use markymark_parser::Parser;

#[test]
fn extract_yaml_frontmatter() {
    let mut parser = Parser::new().unwrap();
    let markdown = r#"---
title: My Document
tags: [rust, markdown]
date: 2024-01-15
---

# Content starts here
"#;

    let ast = parser.parse(markdown).unwrap();
    let frontmatter = ast.frontmatter();

    assert!(frontmatter.is_some());
    let fm = frontmatter.unwrap();

    assert_eq!(fm.get_string("title"), Some("My Document"));
    assert_eq!(fm.get_string_list("tags"), Some(vec!["rust", "markdown"]));
    assert_eq!(fm.get_string("date"), Some("2024-01-15"));
}

#[test]
fn extract_logseq_page_properties() {
    let mut parser = Parser::new().unwrap();
    let markdown = r#"title:: My Page
tags:: [[project]], [[important]]
created:: [[2024-01-15]]

# Content
"#;

    let ast = parser.parse(markdown).unwrap();
    let props = ast.page_properties();

    assert!(props.is_some());
    let properties = props.unwrap();

    assert_eq!(properties.get("title").unwrap().as_str(), Some("My Page"));
    assert!(properties.get("tags").unwrap().is_list());
    assert!(properties.get("created").unwrap().is_page_ref());
}

#[test]
fn extract_inline_properties() {
    let mut parser = Parser::new().unwrap();
    let markdown = "- Task item\n  status:: done\n  priority:: high\n";

    let ast = parser.parse(markdown).unwrap();
    let list_items = ast.extract_list_items();

    assert_eq!(list_items.len(), 1);
    let item = list_items[0];

    let props = item.properties();
    assert_eq!(*props.get("status").unwrap(), "done");
    assert_eq!(*props.get("priority").unwrap(), "high");
}

#[test]
fn parse_deep_nested_lists() {
    let mut parser = Parser::new().unwrap();
    let markdown = r#"- Level 1
  - Level 2
    - Level 3
      - Level 4
        - Level 5
          - Level 6
            - Level 7
              - Level 8
                - Level 9
                  - Level 10
"#;

    let ast = parser.parse(markdown).unwrap();
    let list_items = ast.extract_list_items();

    // Should handle 10 levels of nesting (Logseq requirement)
    assert_eq!(list_items.len(), 1);
    let root = list_items[0];

    // Navigate down the tree
    let mut current = root;
    let mut depth = 1;

    while let Some(children) = current.children() {
        if !children.is_empty() {
            current = &children[0];
            depth += 1;
        } else {
            break;
        }
    }

    assert_eq!(depth, 10, "Should support 10 levels of nesting");
}

#[test]
fn list_item_text_includes_nested_content() {
    // ListItem.text() returns the full span of the list_item node,
    // including any nested child list content. Property parsing skips
    // lines without "::", so child markers don't corrupt the map.
    let mut parser = Parser::new().unwrap();
    let markdown = "- Parent item\n  status:: done\n  - Child item\n";

    let ast = parser.parse(markdown).unwrap();
    let list_items = ast.extract_list_items();

    assert_eq!(list_items.len(), 1);
    let parent = list_items[0];

    // text() includes the full span (parent + children)
    let text = parent.text();
    assert!(
        text.contains("Parent item"),
        "text should contain parent content"
    );
    assert!(
        text.contains("Child item"),
        "text should include nested child content"
    );

    // Properties should still parse correctly despite nested content
    let props = parent.properties();
    assert_eq!(*props.get("status").unwrap(), "done");

    // Child list should be present
    let children = parent.children().expect("should have children");
    assert_eq!(children.len(), 1);
    assert!(children[0].text().contains("Child item"));
}
