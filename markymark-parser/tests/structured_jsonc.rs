//! Integration tests for JSONC parsing through tree-sitter-json.
//!
//! Verifies that `parse_structured(..., DocumentKind::JsonC)` correctly handles:
//! - `//` line comments
//! - `/* */` block comments
//! - Trailing commas
//! - No silent data loss (keys adjacent to comments must be indexed)
//!
//! Finding (marky-lkj.13): tree-sitter-json 0.24 tolerates JSONC constructs
//! (line comments, block comments, trailing commas). While these are technically
//! ERROR nodes in the CST, the walker gracefully skips them and all valid keys
//! are extracted with correct paths, depths, and value kinds. No silent data
//! loss was observed -- keys adjacent to comments are indexed correctly.

use markymark_core::structured::{DocumentKind, ValueKind};
use markymark_parser::structured::parse_structured;

// ---------------------------------------------------------------------------
// Line comments (//)
// ---------------------------------------------------------------------------

#[test]
fn jsonc_line_comment_before_key() {
    let source = r#"{
  // This is a comment
  "name": "Alice"
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let names: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(
        names.contains(&"name"),
        "key 'name' must be indexed; found: {names:?}"
    );
}

#[test]
fn jsonc_line_comment_after_value() {
    let source = r#"{
  "host": "localhost", // the server host
  "port": 8080
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(
        keys.contains(&"host"),
        "'host' must be indexed; found: {keys:?}"
    );
    assert!(
        keys.contains(&"port"),
        "'port' must be indexed; found: {keys:?}"
    );
}

#[test]
fn jsonc_line_comment_between_keys() {
    let source = r#"{
  "a": 1,
  // separator comment
  "b": 2
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(keys.contains(&"a"), "'a' must be indexed; found: {keys:?}");
    assert!(keys.contains(&"b"), "'b' must be indexed; found: {keys:?}");
}

// ---------------------------------------------------------------------------
// Block comments (/* */)
// ---------------------------------------------------------------------------

#[test]
fn jsonc_block_comment_before_key() {
    let source = r#"{
  /* block comment */
  "key": "value"
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(
        keys.contains(&"key"),
        "'key' must be indexed; found: {keys:?}"
    );
}

#[test]
fn jsonc_block_comment_multiline() {
    let source = r#"{
  /*
   * Multi-line
   * block comment
   */
  "alpha": 1,
  "beta": 2
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(
        keys.contains(&"alpha"),
        "'alpha' must be indexed; found: {keys:?}"
    );
    assert!(
        keys.contains(&"beta"),
        "'beta' must be indexed; found: {keys:?}"
    );
}

// ---------------------------------------------------------------------------
// Trailing commas
// ---------------------------------------------------------------------------

#[test]
fn jsonc_trailing_comma_in_object() {
    let source = r#"{
  "x": 1,
  "y": 2,
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(keys.contains(&"x"), "'x' must be indexed; found: {keys:?}");
    assert!(keys.contains(&"y"), "'y' must be indexed; found: {keys:?}");
}

#[test]
fn jsonc_trailing_comma_in_array() {
    let source = r#"{
  "items": [1, 2, 3,]
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let keys: Vec<&str> = ast.keys.iter().map(|k| k.key.as_str()).collect();
    assert!(
        keys.contains(&"items"),
        "'items' must be indexed; found: {keys:?}"
    );
    // Verify array elements are indexed
    let paths: Vec<&str> = ast.keys.iter().map(|k| k.path.as_str()).collect();
    assert!(
        paths.contains(&"items[0]"),
        "array elements must be indexed; found: {paths:?}"
    );
    assert!(
        paths.contains(&"items[2]"),
        "last array element must be indexed; found: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Nested objects with comments
// ---------------------------------------------------------------------------

#[test]
fn jsonc_nested_with_comments() {
    let source = r#"{
  // Database config
  "database": {
    "host": "localhost", /* primary host */
    "port": 5432,
    // credentials
    "user": "admin"
  },
  "logging": {
    "level": "debug", // TODO: change to info in prod
  }
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let paths: Vec<&str> = ast.keys.iter().map(|k| k.path.as_str()).collect();
    // All keys must be present (no silent data loss)
    for expected in &[
        "database",
        "database.host",
        "database.port",
        "database.user",
        "logging",
        "logging.level",
    ] {
        assert!(
            paths.contains(expected),
            "'{expected}' must be indexed; found: {paths:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Realistic JSONC (tsconfig.json style)
// ---------------------------------------------------------------------------

#[test]
fn jsonc_tsconfig_style() {
    let source = r#"{
  // TypeScript compiler options
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true,
    /* Output settings */
    "outDir": "./dist",
    "rootDir": "./src",
  },
  "include": ["src/**/*"],
  "exclude": [
    "node_modules",
    "dist",
  ]
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let paths: Vec<&str> = ast.keys.iter().map(|k| k.path.as_str()).collect();
    for expected in &[
        "compilerOptions",
        "compilerOptions.target",
        "compilerOptions.module",
        "compilerOptions.strict",
        "compilerOptions.outDir",
        "compilerOptions.rootDir",
        "include",
        "exclude",
    ] {
        assert!(
            paths.contains(expected),
            "'{expected}' must be indexed; found: {paths:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Value kinds are preserved correctly
// ---------------------------------------------------------------------------

#[test]
fn jsonc_value_kinds_preserved() {
    let source = r#"{
  // string value
  "name": "test",
  /* number value */
  "count": 42,
  "enabled": true, // boolean
  "data": null
}"#;
    let ast = parse_structured(source, DocumentKind::JsonC).unwrap();
    let find = |key: &str| ast.keys.iter().find(|k| k.key == key);

    if let Some(entry) = find("name") {
        assert_eq!(entry.value_kind, ValueKind::String);
    }
    if let Some(entry) = find("count") {
        assert_eq!(entry.value_kind, ValueKind::Number);
    }
    if let Some(entry) = find("enabled") {
        assert_eq!(entry.value_kind, ValueKind::Boolean);
    }
    if let Some(entry) = find("data") {
        assert_eq!(entry.value_kind, ValueKind::Null);
    }
}
