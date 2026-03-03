//! Tests for FrontmatterValue -> FrontmatterValueRef and Frontmatter -> FrontmatterMap conversions.

use markymark_core::frontmatter::{FrontmatterError, FrontmatterMap, TypedFrontmatter};
use markymark_parser::Parser;

#[test]
fn parser_frontmatter_to_map_string_and_integer() {
    let mut parser = Parser::new().unwrap();
    let md = "---\ntitle: Hello World\npriority: 3\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    assert_eq!(map.get_string("title"), Some("Hello World"));
    assert_eq!(map.get_integer("priority"), Some(3));
    assert_eq!(map.len(), 2);
}

#[test]
fn parser_frontmatter_to_map_boolean_and_null() {
    let mut parser = Parser::new().unwrap();
    let md = "---\ndraft: true\nempty:\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    assert_eq!(map.get_boolean("draft"), Some(true));
    assert!(map.is_null("empty"));
}

#[test]
fn parser_frontmatter_to_map_list() {
    let mut parser = Parser::new().unwrap();
    let md = "---\ntags: [rust, markdown]\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    let list = map.get_list("tags").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_string(), Some("rust"));
    assert_eq!(list[1].as_string(), Some("markdown"));
}

#[test]
fn parser_frontmatter_to_map_float() {
    let mut parser = Parser::new().unwrap();
    let md = "---\nratio: 2.5\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    assert_eq!(map.get_float("ratio"), Some(2.5));
}

#[test]
fn parser_frontmatter_to_map_empty() {
    let mut parser = Parser::new().unwrap();
    let md = "---\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    assert!(map.is_empty());
}

// -- Integration: TypedFrontmatter from parser output --

#[derive(Debug)]
struct TestTask {
    title: String,
    priority: i64,
    draft: bool,
}

impl TypedFrontmatter for TestTask {
    fn from_frontmatter(map: &FrontmatterMap<'_>) -> Result<Self, FrontmatterError> {
        let title = map
            .get_string("title")
            .ok_or_else(|| FrontmatterError::MissingField {
                field: "title".into(),
            })?
            .to_string();

        let priority = map
            .get_integer("priority")
            .ok_or_else(|| match map.get("priority") {
                Some(v) => FrontmatterError::TypeMismatch {
                    field: "priority".into(),
                    expected: "Integer",
                    actual: v.variant_name().into(),
                },
                None => FrontmatterError::MissingField {
                    field: "priority".into(),
                },
            })?;

        let draft = map.get_boolean("draft").unwrap_or(false);

        Ok(Self {
            title,
            priority,
            draft,
        })
    }
}

#[test]
fn typed_frontmatter_from_parser_output() {
    let mut parser = Parser::new().unwrap();
    let md = "---\ntitle: My Task\npriority: 2\ndraft: true\n---\n\n# Content\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    let task = TestTask::from_frontmatter(&map).unwrap();
    assert_eq!(task.title, "My Task");
    assert_eq!(task.priority, 2);
    assert!(task.draft);
}

#[test]
fn typed_frontmatter_missing_field_from_parser() {
    let mut parser = Parser::new().unwrap();
    let md = "---\npriority: 1\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    let err = TestTask::from_frontmatter(&map).unwrap_err();
    assert!(matches!(err, FrontmatterError::MissingField { ref field } if field == "title"));
}

#[test]
fn typed_frontmatter_type_mismatch_from_parser() {
    let mut parser = Parser::new().unwrap();
    let md = "---\ntitle: Ok\npriority: not-a-number\n---\n\n# Body\n";
    let ast = parser.parse(md).unwrap();
    let fm = ast.frontmatter().unwrap();
    let map: FrontmatterMap<'_> = (&fm).into();

    let err = TestTask::from_frontmatter(&map).unwrap_err();
    assert!(matches!(
        err,
        FrontmatterError::TypeMismatch {
            ref field,
            expected: "Integer",
            ..
        } if field == "priority"
    ));
}
