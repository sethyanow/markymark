//! YAML parser using tree-sitter-yaml.
//!
//! Walks the CST to extract [`KeyEntry`] items with byte-accurate ranges.
//! Implements YAML 1.2 specification.
//!
//! # Multi-Document Support
//! YAML allows multiple documents in one file separated by `---`.
//! Currently only the first document is parsed.
//! TODO(marky-XXX): Add multi-document support if needed.

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Range};
use tree_sitter::{Node, Parser as TSParser};

use super::byte_to_position;

/// Parse a YAML document into a [`StructuredAst`].
///
/// # YAML 1.2
/// This parser implements YAML 1.2 as supported by tree-sitter-yaml.
/// Only `true`/`false` are booleans, only `null`/`~` are null (no y/n/on/off).
pub fn parse_yaml(source: &str) -> Result<StructuredAst, CoreError> {
    let mut parser = TSParser::new();
    let language: tree_sitter::Language = tree_sitter_yaml::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| CoreError::Message(format!("failed to set YAML language: {e}")))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CoreError::Message("failed to parse YAML".to_string()))?;

    let root = tree.root_node();

    // Check for ERROR nodes in the parse tree (malformed YAML).
    // tree-sitter's has_error() is O(1) — checks any descendant.
    if root.has_error() {
        return Err(CoreError::Message("malformed YAML syntax".to_string()));
    }

    let mut keys = Vec::new();

    // The root node is "stream" which contains "document" nodes.
    // For v1, we only parse the first document.
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "document" {
            walk_document(child, source, &mut keys);
            break; // Only process first document
        }
    }

    Ok(StructuredAst {
        source: source.to_string(),
        kind: DocumentKind::Yaml,
        keys,
    })
}

/// Walk a YAML document node.
fn walk_document(node: Node, source: &str, keys: &mut Vec<KeyEntry>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_value(child, source, &[], 0, keys);
    }
}

/// Recursively walk a YAML value node, extracting key entries.
///
/// Unwraps `block_node`/`flow_node` wrappers before dispatching to handlers.
fn walk_value(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    // Unwrap wrapper nodes (document -> block_node -> block_mapping)
    let actual = unwrap_node(node);
    match actual.kind() {
        "block_mapping" => walk_block_mapping(actual, source, path, depth, keys),
        "flow_mapping" => walk_flow_mapping(actual, source, path, depth, keys),
        "block_sequence" => walk_block_sequence(actual, source, path, depth, keys),
        "flow_sequence" => walk_flow_sequence(actual, source, path, depth, keys),
        _ => {} // Scalar values are recorded by their parent mapping/sequence context
    }
}

/// Walk a block-style mapping (indentation-based).
fn walk_block_mapping(
    node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "block_mapping_pair" {
            extract_mapping_pair(child, source, path, depth, keys);
        }
    }
}

/// Walk a flow-style mapping (`{key: value}`).
fn walk_flow_mapping(
    node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "flow_pair" {
            extract_flow_pair(child, source, path, depth, keys);
        }
    }
}

/// Extract key text from a node, unwrapping wrappers and handling scalars.
fn extract_key_text(node: Node, source: &str) -> String {
    let unwrapped = unwrap_node(node);
    let text = unwrapped.utf8_text(source.as_bytes()).unwrap_or("");

    // Strip quotes if present
    let trimmed = text.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Unwrap flow_node/block_node/anchor/sequence_item wrappers to get the actual value node.
fn unwrap_node(node: Node) -> Node {
    match node.kind() {
        "flow_node" | "block_node" => {
            // These nodes wrap a single meaningful child
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if !child.kind().contains("comment") && child.kind() != "tag" {
                    return unwrap_node(child);
                }
            }
            node
        }
        "block_sequence_item" => {
            // Sequence items contain "-" token + the actual value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "-" {
                    return unwrap_node(child);
                }
            }
            node
        }
        "anchor" => {
            // anchor contains: "&" + anchor_name + the actual value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let kind = child.kind();
                if kind != "anchor_name" && kind != "&" {
                    return unwrap_node(child);
                }
            }
            node
        }
        "alias" => node, // alias (*x) is a leaf - keep as-is
        _ => node,
    }
}

/// Extract a key-value pair from a block_mapping_pair node.
fn extract_mapping_pair(
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
            // The key is usually a flow_node or plain_scalar
            "flow_node" | "plain_scalar" | "double_quote_scalar" | "single_quote_scalar"
                if key_node.is_none() =>
            {
                key_node = Some(child);
            }
            // After the key, we hit the value (could be flow_node, block_node, etc.)
            "flow_node" | "block_node" if key_node.is_some() && value_node.is_none() => {
                value_node = Some(child);
            }
            _ => {}
        }
    }

    let (key_nd, val_nd) = match (key_node, value_node) {
        (Some(k), Some(v)) => (k, v),
        (Some(k), None) => {
            // Key with no value (null)
            let key_text = extract_key_text(k, source);
            let mut full_path = parent_path.to_vec();
            full_path.push(key_text.clone());

            keys.push(KeyEntry {
                path: full_path.join("."),
                key: key_text,
                depth,
                value_kind: ValueKind::Null,
                key_range: node_to_range(k, source),
                value_range: node_to_range(k, source),
            });
            return;
        }
        _ => return,
    };

    let key_text = extract_key_text(key_nd, source);
    let mut full_path = parent_path.to_vec();
    full_path.push(key_text.clone());
    let path_str = full_path.join(".");

    // Get the actual value node (unwrap flow_node/block_node wrappers)
    let actual_value = unwrap_node(val_nd);
    let value_kind = classify_value(&actual_value, source);

    keys.push(KeyEntry {
        path: path_str,
        key: key_text,
        depth,
        value_kind,
        key_range: node_to_range(key_nd, source),
        value_range: node_to_range(actual_value, source),
    });

    // Recurse into nested structures
    walk_value(actual_value, source, &full_path, depth + 1, keys);
}

/// Extract a key-value pair from a flow_pair node.
fn extract_flow_pair(
    pair_node: Node,
    source: &str,
    parent_path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    // Flow pairs use similar structure to block pairs
    extract_mapping_pair(pair_node, source, parent_path, depth, keys);
}

/// Walk a block-style sequence (indentation-based array).
fn walk_block_sequence(
    node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut index = 0usize;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "block_sequence_item" {
            walk_sequence_item(child, source, path, depth, index, keys);
            index += 1;
        }
    }
}

/// Walk a flow-style sequence (`[item1, item2]`).
fn walk_flow_sequence(
    node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut index = 0usize;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip brackets and commas
        if child.kind() == "[" || child.kind() == "]" || child.kind() == "," {
            continue;
        }
        if child.kind() == "flow_node" {
            walk_sequence_item(child, source, path, depth, index, keys);
            index += 1;
        }
    }
}

/// Walk a sequence item (common logic for block and flow sequences).
fn walk_sequence_item(
    item_node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    index: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let index_key = format!("[{index}]");
    let mut full_path = path.to_vec();

    // Build path like "items[0]" by appending index to last segment
    if let Some(last) = full_path.last_mut() {
        *last = format!("{last}{index_key}");
    } else {
        full_path.push(index_key.clone());
    }

    // Unwrap the item node to get the actual value
    let actual_value = unwrap_node(item_node);
    let value_kind = classify_value(&actual_value, source);

    keys.push(KeyEntry {
        path: full_path.join("."),
        key: index_key,
        depth,
        value_kind,
        key_range: node_to_range(actual_value, source),
        value_range: node_to_range(actual_value, source),
    });

    // Recurse into nested structures
    walk_value(actual_value, source, &full_path, depth + 1, keys);
}

/// Classify a tree-sitter node into a [`ValueKind`].
fn classify_value(node: &Node, _source: &str) -> ValueKind {
    match node.kind() {
        "string_scalar" | "double_quote_scalar" | "single_quote_scalar" => ValueKind::String,
        "integer_scalar" | "float_scalar" => ValueKind::Number,
        "boolean_scalar" => ValueKind::Boolean,
        "null_scalar" => ValueKind::Null,
        "block_scalar" => ValueKind::String, // | and > multiline strings
        "block_sequence" | "flow_sequence" => ValueKind::Array,
        "block_mapping" | "flow_mapping" => ValueKind::Object,
        "plain_scalar" => {
            // plain_scalar wraps typed children: string_scalar, integer_scalar, etc.
            if let Some(child) = node.child(0) {
                match child.kind() {
                    "string_scalar" => ValueKind::String,
                    "integer_scalar" | "float_scalar" => ValueKind::Number,
                    "boolean_scalar" => ValueKind::Boolean,
                    "null_scalar" => ValueKind::Null,
                    _ => ValueKind::String,
                }
            } else {
                ValueKind::String
            }
        }
        "anchor" => {
            // Anchors (&x value) - classify the value part
            let unwrapped = unwrap_node(*node);
            if unwrapped.id() == node.id() {
                ValueKind::String
            } else {
                classify_value(&unwrapped, _source)
            }
        }
        "alias" => ValueKind::String, // aliases (*x) reference another value
        _ => ValueKind::Null,         // Fallback for unexpected node types
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

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::Position;

    // ===== BASIC PARSING =====

    #[test]
    fn test_parse_yaml_empty_document() {
        let ast = parse_yaml("").unwrap();
        assert_eq!(ast.kind, DocumentKind::Yaml);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_yaml_flat() {
        let source = "key: value";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "key");
        assert_eq!(ast.keys[0].path, "key");
        assert_eq!(ast.keys[0].depth, 0);
    }

    #[test]
    fn test_parse_yaml_nested() {
        let source = "database:\n  host: localhost";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys.len(), 2);

        assert_eq!(ast.keys[0].key, "database");
        assert_eq!(ast.keys[0].path, "database");
        assert_eq!(ast.keys[0].depth, 0);

        assert_eq!(ast.keys[1].key, "host");
        assert_eq!(ast.keys[1].path, "database.host");
        assert_eq!(ast.keys[1].depth, 1);
    }

    #[test]
    fn test_parse_yaml_nested_flow_mapping() {
        let source = "db: {host: localhost, port: 5432}";
        let ast = parse_yaml(source).unwrap();
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

    // ===== SEQUENCES (ARRAYS) =====

    #[test]
    fn test_parse_yaml_block_sequence() {
        let source = "items:\n  - a\n  - b";
        let ast = parse_yaml(source).unwrap();

        assert_eq!(ast.keys[0].path, "items");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "items[0]");
        assert_eq!(ast.keys[1].depth, 1);

        assert_eq!(ast.keys[2].path, "items[1]");
        assert_eq!(ast.keys[2].depth, 1);
    }

    #[test]
    fn test_parse_yaml_flow_sequence() {
        let source = "items: [a, b]";
        let ast = parse_yaml(source).unwrap();

        assert_eq!(ast.keys[0].path, "items");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "items[0]");
        assert_eq!(ast.keys[2].path, "items[1]");
    }

    #[test]
    fn test_parse_yaml_nested_sequence() {
        let source = "servers:\n  - host: a";
        let ast = parse_yaml(source).unwrap();

        // servers (Array, d0) -> servers[0] (Object, d1) -> servers[0].host (String, d2)
        assert_eq!(ast.keys[0].path, "servers");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "servers[0]");
        assert_eq!(ast.keys[1].value_kind, ValueKind::Object);
        assert_eq!(ast.keys[1].depth, 1);

        assert_eq!(ast.keys[2].path, "servers[0].host");
        assert_eq!(ast.keys[2].value_kind, ValueKind::String);
        assert_eq!(ast.keys[2].depth, 2);
    }

    // ===== VALUE KINDS =====

    #[test]
    fn test_parse_yaml_value_kinds_string() {
        let ast = parse_yaml("key: value").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
    }

    #[test]
    fn test_parse_yaml_value_kinds_number() {
        let ast = parse_yaml("port: 8080").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Number);
    }

    #[test]
    fn test_parse_yaml_value_kinds_boolean() {
        let ast = parse_yaml("enabled: true").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Boolean);
    }

    #[test]
    fn test_parse_yaml_value_kinds_null() {
        let ast = parse_yaml("value: null").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Null);
    }

    #[test]
    fn test_parse_yaml_value_kinds_tilde() {
        // YAML 1.2: ~ is alternate null syntax
        let ast = parse_yaml("value: ~").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Null);
    }

    #[test]
    fn test_parse_yaml_value_kinds_array() {
        let ast = parse_yaml("items: [1, 2]").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);
    }

    #[test]
    fn test_parse_yaml_value_kinds_object() {
        let ast = parse_yaml("db: {host: x}").unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);
    }

    // ===== POSITION ACCURACY =====

    #[test]
    fn test_parse_yaml_position_accuracy() {
        let source = "key: value";
        let ast = parse_yaml(source).unwrap();

        assert_eq!(ast.keys.len(), 1);
        let entry = &ast.keys[0];

        // "key" starts at byte 0 (line 0, char 0)
        assert_eq!(entry.key_range.start, Position::new(0, 0));
        assert_eq!(entry.key_range.end, Position::new(0, 3));

        // "value" starts at byte 5
        assert_eq!(entry.value_range.start, Position::new(0, 5));
        assert_eq!(entry.value_range.end, Position::new(0, 10));
    }

    // ===== MULTILINE STRINGS =====

    #[test]
    fn test_parse_yaml_multiline_string_pipe() {
        // Pipe | preserves newlines (literal)
        let source = "text: |\n  line1\n  line2";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
        assert_eq!(ast.keys[0].key, "text");
    }

    #[test]
    fn test_parse_yaml_multiline_string_gt() {
        // Greater-than > folds to single line (folded)
        let source = "text: >\n  line1\n  line2";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
        assert_eq!(ast.keys[0].key, "text");
    }

    // ===== YAML FEATURES =====

    #[test]
    fn test_parse_yaml_anchors_aliases() {
        // &x creates anchor, *x references it
        // Both should be indexed as separate keys
        let source = "a: &x value\nb: *x";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "a");
        assert_eq!(ast.keys[1].key, "b");
    }

    #[test]
    fn test_parse_yaml_merge_keys() {
        // << operator is indexed as a regular key in v1 (no alias resolution)
        // TODO(marky-XXX): Add merge key resolution for full << support
        let source = "base:\n  x: 1\nderived:\n  <<: value\n  y: 2";
        let ast = parse_yaml(source).unwrap();

        let paths: Vec<_> = ast.keys.iter().map(|k| k.path.as_str()).collect();
        assert!(paths.contains(&"base"));
        assert!(paths.contains(&"base.x"));
        assert!(paths.contains(&"derived"));
        assert!(paths.contains(&"derived.<<"));
        assert!(paths.contains(&"derived.y"));
    }

    // ===== ERROR HANDLING =====

    #[test]
    fn test_parse_yaml_malformed_syntax() {
        // Unclosed flow mapping is malformed
        let source = "{key: value";
        let result = parse_yaml(source);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("malformed"));
    }

    #[test]
    fn test_parse_yaml_tab_indentation() {
        // Tabs are forbidden in YAML 1.2 spec
        let source = "key:\n\tvalue";
        let result = parse_yaml(source);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("malformed"));
    }

    // ===== STRESS TESTS =====

    #[test]
    fn test_parse_yaml_deep_nesting() {
        // Build 200-level deep nesting
        let mut source = String::new();
        for i in 0..200 {
            source.push_str(&"  ".repeat(i));
            source.push_str(&format!("level{i}:\n"));
        }
        source.push_str(&"  ".repeat(200));
        source.push_str("value: final");

        // Should not stack overflow
        let result = parse_yaml(&source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_yaml_large_document() {
        // Build 1,000-key document (debug builds are ~3-7x slower than release)
        let mut source = String::new();
        for i in 0..1_000 {
            source.push_str(&format!("key{i}: value{i}\n"));
        }

        let start = std::time::Instant::now();
        let ast = parse_yaml(&source).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(ast.keys.len(), 1_000);
        assert!(elapsed.as_secs() < 2, "took {elapsed:?}, expected <2s");
    }

    // ===== UNICODE =====

    #[test]
    fn test_parse_yaml_unicode_keys() {
        let source = "日本語: value";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys[0].key, "日本語");
    }

    #[test]
    fn test_parse_yaml_unicode_values() {
        let source = "key: 🎉";
        let ast = parse_yaml(source).unwrap();
        assert_eq!(ast.keys[0].key, "key");
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
    }
}
