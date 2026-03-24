//! Tests for FrontmatterValueEntry -> FrontmatterValueRef and frontmatter_map_from_entries.

use markymark_core::frontmatter::{FrontmatterError, FrontmatterMap, TypedFrontmatter};
use markymark_index::document::{frontmatter_map_from_entries, DocumentIndex};
use markymark_parser::Parser;

/// Helper: parse markdown and build a DocumentIndex, then extract frontmatter.
fn parse_and_index(md: &str) -> DocumentIndex {
    DocumentIndex::from_text(md)
}

#[test]
fn index_frontmatter_to_map_string_and_integer() {
    let idx = parse_and_index("---\ntitle: Hello\npriority: 3\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    assert_eq!(map.get_string("title"), Some("Hello"));
    assert_eq!(map.get_integer("priority"), Some(3));
    assert_eq!(map.len(), 2);
}

#[test]
fn index_frontmatter_to_map_boolean_and_null() {
    let idx = parse_and_index("---\ndraft: true\nempty:\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    assert_eq!(map.get_boolean("draft"), Some(true));
    assert!(map.is_null("empty"));
}

#[test]
fn index_frontmatter_to_map_list() {
    let idx = parse_and_index("---\ntags: [rust, markdown]\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    let list = map.get_list("tags").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_string(), Some("rust"));
    assert_eq!(list[1].as_string(), Some("markdown"));
}

#[test]
fn index_frontmatter_to_map_float() {
    let idx = parse_and_index("---\nratio: 2.5\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    assert_eq!(map.get_float("ratio"), Some(2.5));
}

#[test]
fn index_frontmatter_to_map_empty() {
    let idx = parse_and_index("---\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    assert!(map.is_empty());
}

// -- Integration: TypedFrontmatter from index output --

#[derive(Debug)]
struct BlogPost {
    title: String,
    draft: bool,
    tags: Vec<String>,
}

impl TypedFrontmatter for BlogPost {
    fn from_frontmatter(map: &FrontmatterMap<'_>) -> Result<Self, FrontmatterError> {
        let title = map
            .get_string("title")
            .ok_or_else(|| FrontmatterError::MissingField {
                field: "title".into(),
            })?
            .to_string();

        let draft = map.get_boolean("draft").unwrap_or(false);

        let tags = map
            .get_list("tags")
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_string().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self { title, draft, tags })
    }
}

#[test]
fn typed_frontmatter_from_index_output() {
    let idx = parse_and_index(
        "---\ntitle: My Post\ndraft: true\ntags: [rust, markdown]\n---\n\n# Content\n",
    );
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    let post = BlogPost::from_frontmatter(&map).unwrap();
    assert_eq!(post.title, "My Post");
    assert!(post.draft);
    assert_eq!(post.tags, vec!["rust", "markdown"]);
}

#[test]
fn typed_frontmatter_missing_field_from_index() {
    let idx = parse_and_index("---\ndraft: false\n---\n\n# Body\n");
    let entries = idx.frontmatter();
    let map = frontmatter_map_from_entries(entries);

    let err = BlogPost::from_frontmatter(&map).unwrap_err();
    assert!(matches!(
        err,
        FrontmatterError::MissingField { ref field } if field == "title"
    ));
}

#[test]
fn round_trip_parser_and_index_produce_same_values() {
    let md = "---\ntitle: Consistency\npriority: 5\ndraft: true\n---\n\n# Body\n";

    // Parser path
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let parser_map: FrontmatterMap<'_> = (&fm).into();

    // Index path
    let idx = parse_and_index(md);
    let entries = idx.frontmatter();
    let index_map = frontmatter_map_from_entries(entries);

    // Both paths should produce identical typed results
    assert_eq!(
        parser_map.get_string("title"),
        index_map.get_string("title")
    );
    assert_eq!(
        parser_map.get_integer("priority"),
        index_map.get_integer("priority")
    );
    assert_eq!(
        parser_map.get_boolean("draft"),
        index_map.get_boolean("draft")
    );
}
