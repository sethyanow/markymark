//! JSON parser using tree-sitter-json.
//!
//! Walks the CST to extract [`KeyEntry`] items with byte-accurate ranges.

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Position, Range};
use tree_sitter::{Node, Parser as TSParser};

/// Parse a JSON document into a [`StructuredAst`].
pub fn parse_json(source: &str) -> Result<StructuredAst, CoreError> {
    let mut parser = TSParser::new();
    let language = tree_sitter_json::language();
    parser
        .set_language(language)
        .map_err(|e| CoreError::Message(format!("failed to set JSON language: {e}")))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CoreError::Message("failed to parse JSON".to_string()))?;

    let root = tree.root_node();
    let mut keys = Vec::new();

    // The root node is "document" which contains the top-level value.
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        walk_value(child, source, &[], 0, &mut keys);
    }

    Ok(StructuredAst {
        source: source.to_string(),
        kind: DocumentKind::Json,
        keys,
    })
}

/// Recursively walk a JSON value node, extracting key entries.
fn walk_value(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    match node.kind() {
        "object" => walk_object(node, source, path, depth, keys),
        "array" => walk_array(node, source, path, depth, keys),
        _ => {} // Scalar values are recorded by their parent pair/array context
    }
}

/// Walk an object node, extracting key entries for each pair.
fn walk_object(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            extract_pair(child, source, path, depth, keys);
        }
    }
}

/// Extract a key-value pair from a "pair" node.
fn extract_pair(
    pair_node: Node,
    source: &str,
    parent_path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut key_node = None;
    let mut value_node = None;

    let mut cursor = pair_node.walk();
    for child in pair_node.children(&mut cursor) {
        match child.kind() {
            "string" if key_node.is_none() => key_node = Some(child),
            ":" => {}
            _ if key_node.is_some() && value_node.is_none() => value_node = Some(child),
            _ => {}
        }
    }

    let (key_nd, val_nd) = match (key_node, value_node) {
        (Some(k), Some(v)) => (k, v),
        _ => return,
    };

    // Extract the key text (strip surrounding quotes).
    let key_text = key_nd
        .utf8_text(source.as_bytes())
        .unwrap_or("")
        .trim_matches('"')
        .to_string();

    let mut full_path = parent_path.to_vec();
    full_path.push(key_text.clone());
    let path_str = full_path.join(".");

    let value_kind = classify_value(&val_nd);

    keys.push(KeyEntry {
        path: path_str,
        key: key_text,
        depth,
        value_kind,
        key_range: node_to_range(key_nd, source),
        value_range: node_to_range(val_nd, source),
    });

    // Recurse into nested objects/arrays.
    walk_value(val_nd, source, &full_path, depth + 1, keys);
}

/// Walk an array node, extracting entries for each element.
fn walk_array(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    let mut index = 0usize;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip brackets and commas.
        if child.kind() == "[" || child.kind() == "]" || child.kind() == "," {
            continue;
        }

        let index_key = format!("[{index}]");
        let mut full_path = path.to_vec();

        // Build path like "items[0]" by appending index to last segment.
        if let Some(last) = full_path.last_mut() {
            *last = format!("{last}{index_key}");
        } else {
            full_path.push(index_key.clone());
        }

        let value_kind = classify_value(&child);

        keys.push(KeyEntry {
            path: full_path.join("."),
            key: index_key,
            depth,
            value_kind,
            key_range: node_to_range(child, source),
            value_range: node_to_range(child, source),
        });

        // Recurse into nested objects/arrays within the array element.
        if child.kind() == "object" || child.kind() == "array" {
            walk_value(child, source, &full_path, depth + 1, keys);
        }

        index += 1;
    }
}

/// Classify a tree-sitter node into a [`ValueKind`].
fn classify_value(node: &Node) -> ValueKind {
    match node.kind() {
        "string" => ValueKind::String,
        "number" => ValueKind::Number,
        "true" | "false" => ValueKind::Boolean,
        "null" => ValueKind::Null,
        "array" => ValueKind::Array,
        "object" => ValueKind::Object,
        _ => ValueKind::Null, // Fallback for unexpected node types
    }
}

/// Convert a tree-sitter node's position to a markymark [`Range`].
fn node_to_range(node: Node, source: &str) -> Range {
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    Range::new(
        byte_to_position(source, start_byte),
        byte_to_position(source, end_byte),
    )
}

/// Convert a byte offset in source text to a [`Position`] (line, character).
fn byte_to_position(source: &str, byte_offset: usize) -> Position {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let col = match prefix.rfind('\n') {
        Some(nl) => (offset - nl - 1) as u32,
        None => offset as u32,
    };
    Position::new(line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_empty_object() {
        let ast = parse_json("{}").unwrap();
        assert_eq!(ast.kind, DocumentKind::Json);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_json_flat() {
        let source = r#"{"a": 1, "b": "hello"}"#;
        let ast = parse_json(source).unwrap();
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
    fn test_parse_json_nested() {
        let source = r#"{"db": {"host": "localhost", "port": 5432}}"#;
        let ast = parse_json(source).unwrap();

        // "db" at depth 0, then "db.host" and "db.port" at depth 1
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
    fn test_parse_json_array() {
        let source = r#"{"items": [1, 2, 3]}"#;
        let ast = parse_json(source).unwrap();

        // "items" at depth 0, then items[0], items[1], items[2] at depth 1
        assert_eq!(ast.keys[0].key, "items");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "items[0]");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Number);

        assert_eq!(ast.keys[2].path, "items[1]");
        assert_eq!(ast.keys[3].path, "items[2]");
    }

    #[test]
    fn test_parse_json_nested_array_of_objects() {
        let source = r#"{"servers": [{"host": "a"}, {"host": "b"}]}"#;
        let ast = parse_json(source).unwrap();

        // servers (Array, d0) -> servers[0] (Object, d1) -> servers[0].host (String, d2)
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
    fn test_parse_json_value_kinds() {
        let source =
            r#"{"s": "text", "n": 42, "b": true, "f": false, "x": null, "a": [], "o": {}}"#;
        let ast = parse_json(source).unwrap();

        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Number);
        assert_eq!(ast.keys[2].value_kind, ValueKind::Boolean);
        assert_eq!(ast.keys[3].value_kind, ValueKind::Boolean);
        assert_eq!(ast.keys[4].value_kind, ValueKind::Null);
        assert_eq!(ast.keys[5].value_kind, ValueKind::Array);
        assert_eq!(ast.keys[6].value_kind, ValueKind::Object);
    }

    #[test]
    fn test_parse_json_position_accuracy() {
        // Each character is one byte here (ASCII).
        let source = "{\n  \"name\": \"Alice\"\n}";
        let ast = parse_json(source).unwrap();

        assert_eq!(ast.keys.len(), 1);
        let entry = &ast.keys[0];
        assert_eq!(entry.key, "name");

        // "name" starts at byte 4 (line 1, char 2), key node includes quotes: "name"
        // Key range: line 1, col 2 to line 1, col 8
        assert_eq!(entry.key_range.start, Position::new(1, 2));
        assert_eq!(entry.key_range.end, Position::new(1, 8));

        // "Alice" starts at byte 11, value node includes quotes: "Alice"
        assert_eq!(entry.value_range.start, Position::new(1, 10));
        assert_eq!(entry.value_range.end, Position::new(1, 17));
    }

    #[test]
    fn test_parse_json_root_keys() {
        let source = r#"{"top": {"nested": 1}, "other": 2}"#;
        let ast = parse_json(source).unwrap();

        let roots = ast.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].key, "top");
        assert_eq!(roots[1].key, "other");
    }
}
