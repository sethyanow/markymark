//! Integration tests for JSON5 structured document parsing.

use markymark_core::structured::{DocumentKind, ValueKind};
use markymark_core::Position;
use markymark_parser::structured::parse_structured;

#[test]
fn test_json5_flat_object() {
    let source = r#"{"a": 1, "b": "hello"}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();
    assert_eq!(ast.kind, DocumentKind::Json5);
    assert_eq!(ast.keys.len(), 2);

    assert_eq!(ast.keys[0].key, "a");
    assert_eq!(ast.keys[0].path, "a");
    assert_eq!(ast.keys[0].depth, 0);
    assert_eq!(ast.keys[0].value_kind, ValueKind::Number);

    assert_eq!(ast.keys[1].key, "b");
    assert_eq!(ast.keys[1].path, "b");
    assert_eq!(ast.keys[1].depth, 0);
    assert_eq!(ast.keys[1].value_kind, ValueKind::String);
}

#[test]
fn test_json5_nested_object() {
    let source = r#"{"db": {"host": "localhost", "port": 5432}}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 3);

    assert_eq!(ast.keys[0].key, "db");
    assert_eq!(ast.keys[0].path, "db");
    assert_eq!(ast.keys[0].depth, 0);
    assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

    assert_eq!(ast.keys[1].key, "host");
    assert_eq!(ast.keys[1].path, "db.host");
    assert_eq!(ast.keys[1].depth, 1);
    assert_eq!(ast.keys[1].value_kind, ValueKind::String);

    assert_eq!(ast.keys[2].key, "port");
    assert_eq!(ast.keys[2].path, "db.port");
    assert_eq!(ast.keys[2].depth, 1);
    assert_eq!(ast.keys[2].value_kind, ValueKind::Number);
}

#[test]
fn test_json5_arrays() {
    let source = r#"{"items": [1, 2, 3]}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys[0].key, "items");
    assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

    assert_eq!(ast.keys[1].path, "items[0]");
    assert_eq!(ast.keys[1].depth, 1);
    assert_eq!(ast.keys[1].value_kind, ValueKind::Number);

    assert_eq!(ast.keys[2].path, "items[1]");
    assert_eq!(ast.keys[3].path, "items[2]");
}

#[test]
fn test_json5_unquoted_keys() {
    let source = "{name: 'Alice', age: 30}";
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 2);
    assert_eq!(ast.keys[0].key, "name");
    assert_eq!(ast.keys[0].path, "name");
    assert_eq!(ast.keys[0].value_kind, ValueKind::String);

    assert_eq!(ast.keys[1].key, "age");
    assert_eq!(ast.keys[1].value_kind, ValueKind::Number);
}

#[test]
fn test_json5_trailing_commas() {
    let source = r#"{
  "a": 1,
  "b": 2,
}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();
    assert_eq!(ast.keys.len(), 2);
    assert_eq!(ast.keys[0].key, "a");
    assert_eq!(ast.keys[1].key, "b");
}

#[test]
fn test_json5_single_line_comments() {
    let source = r#"{
  // This is a comment
  "key": "value"
}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();
    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "key");
    assert_eq!(ast.keys[0].value_kind, ValueKind::String);
}

#[test]
fn test_json5_block_comments() {
    let source = r#"{
  /* block comment */
  "key": "value"
}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();
    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "key");
}

#[test]
fn test_json5_position_accuracy() {
    let source = "{\n  \"name\": \"Alice\"\n}";
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 1);
    let entry = &ast.keys[0];
    assert_eq!(entry.key, "name");

    // "name" starts at byte 4 (line 1, char 2), key node includes quotes: "name"
    assert_eq!(entry.key_range.start, Position::new(1, 2));
    assert_eq!(entry.key_range.end, Position::new(1, 8));

    // "Alice" starts at byte 11
    assert_eq!(entry.value_range.start, Position::new(1, 10));
    assert_eq!(entry.value_range.end, Position::new(1, 17));
}

#[test]
fn test_json5_unquoted_key_position_accuracy() {
    // Unquoted keys: position should cover just the identifier, not quotes.
    let source = "{\n  name: \"Alice\"\n}";
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 1);
    let entry = &ast.keys[0];
    assert_eq!(entry.key, "name");

    // "name" starts at byte 4 (line 1, char 2), length 4
    assert_eq!(entry.key_range.start, Position::new(1, 2));
    assert_eq!(entry.key_range.end, Position::new(1, 6));

    // "Alice" starts at byte 10
    assert_eq!(entry.value_range.start, Position::new(1, 8));
    assert_eq!(entry.value_range.end, Position::new(1, 15));
}

#[test]
fn test_json5_empty_object() {
    let ast = parse_structured("{}", DocumentKind::Json5).unwrap();
    assert_eq!(ast.kind, DocumentKind::Json5);
    assert!(ast.keys.is_empty());
}

#[test]
fn test_json5_value_kinds() {
    let source = r#"{"s": "text", "n": 42, "b": true, "f": false, "x": null, "a": [], "o": {}}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys[0].value_kind, ValueKind::String);
    assert_eq!(ast.keys[1].value_kind, ValueKind::Number);
    assert_eq!(ast.keys[2].value_kind, ValueKind::Boolean);
    assert_eq!(ast.keys[3].value_kind, ValueKind::Boolean);
    assert_eq!(ast.keys[4].value_kind, ValueKind::Null);
    assert_eq!(ast.keys[5].value_kind, ValueKind::Array);
    assert_eq!(ast.keys[6].value_kind, ValueKind::Object);
}

#[test]
fn test_json5_root_keys() {
    let source = r#"{"top": {"nested": 1}, "other": 2}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    let roots = ast.root_keys();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].key, "top");
    assert_eq!(roots[1].key, "other");
}

#[test]
fn test_json5_single_quoted_strings() {
    let source = "{'key': 'value'}";
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();
    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "key");
    assert_eq!(ast.keys[0].value_kind, ValueKind::String);
}

#[test]
fn test_json5_nested_array_of_objects() {
    let source = r#"{"servers": [{"host": "a"}, {"host": "b"}]}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys[0].path, "servers");
    assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

    assert_eq!(ast.keys[1].path, "servers[0]");
    assert_eq!(ast.keys[1].value_kind, ValueKind::Object);

    assert_eq!(ast.keys[2].path, "servers[0].host");
    assert_eq!(ast.keys[2].value_kind, ValueKind::String);
    assert_eq!(ast.keys[2].depth, 2);

    assert_eq!(ast.keys[3].path, "servers[1]");
    assert_eq!(ast.keys[4].path, "servers[1].host");
}

#[test]
fn test_json5_dispatch_replaces_not_implemented() {
    // This test replaces test_parse_structured_dispatch_json5_unimplemented
    // from mod.rs -- Json5 should now succeed.
    let result = parse_structured("{key: 'val'}", DocumentKind::Json5);
    assert!(
        result.is_ok(),
        "Json5 should be implemented, got: {:?}",
        result.err()
    );
}

#[test]
fn test_json5_mixed_features() {
    // Combines multiple JSON5 features in one document.
    let source = r#"{
  // Database config
  db: {
    host: 'localhost',
    port: 5432,
  },
  /* Feature flags */
  features: [true, false],
}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    // db (Object), db.host (String), db.port (Number), features (Array), features[0], features[1]
    assert_eq!(ast.keys[0].key, "db");
    assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

    assert_eq!(ast.keys[1].path, "db.host");
    assert_eq!(ast.keys[1].value_kind, ValueKind::String);

    assert_eq!(ast.keys[2].path, "db.port");
    assert_eq!(ast.keys[2].value_kind, ValueKind::Number);

    assert_eq!(ast.keys[3].key, "features");
    assert_eq!(ast.keys[3].value_kind, ValueKind::Array);

    assert_eq!(ast.keys[4].path, "features[0]");
    assert_eq!(ast.keys[4].value_kind, ValueKind::Boolean);

    assert_eq!(ast.keys[5].path, "features[1]");
    assert_eq!(ast.keys[5].value_kind, ValueKind::Boolean);
}

#[test]
fn test_json5_escaped_key_newline() {
    // Key with escape sequence: serde decodes \n to actual newline,
    // scanner must do the same to match the serde map lookup.
    let source = r#"{"foo\nbar": 42}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "foo\nbar"); // decoded newline, not literal \n
    assert_eq!(ast.keys[0].value_kind, ValueKind::Number);
}

#[test]
fn test_json5_escaped_key_tab_and_backslash() {
    let source = r#"{"a\tb": 1, "c\\d": 2}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 2);
    assert_eq!(ast.keys[0].key, "a\tb"); // decoded tab
    assert_eq!(ast.keys[1].key, "c\\d"); // decoded backslash
}

#[test]
fn test_json5_escaped_key_unicode() {
    // \u0041 is 'A'
    let source = r#"{"k\u0041y": "val"}"#;
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "kAy"); // \u0041 decoded to 'A'
    assert_eq!(ast.keys[0].value_kind, ValueKind::String);
}

#[test]
fn test_json5_escaped_single_quoted_key() {
    // Single-quoted string with escape sequences
    let source = "{'foo\\'bar': 1}";
    let ast = parse_structured(source, DocumentKind::Json5).unwrap();

    assert_eq!(ast.keys.len(), 1);
    assert_eq!(ast.keys[0].key, "foo'bar"); // decoded escaped single quote
}
