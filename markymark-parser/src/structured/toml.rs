//! TOML parser using tree-sitter-toml-ng.
//!
//! Walks the CST to extract [`KeyEntry`] items with byte-accurate ranges.
//! Handles standard tables, arrays of tables, dotted keys, inline tables,
//! and all TOML value types.

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Position, Range};
use tree_sitter::{Node, Parser as TSParser};

/// Parse a TOML document into a [`StructuredAst`].
pub fn parse_toml(source: &str) -> Result<StructuredAst, CoreError> {
    let mut parser = TSParser::new();
    let language: tree_sitter::Language = tree_sitter_toml_ng::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| CoreError::Message(format!("failed to set TOML language: {e}")))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CoreError::Message("failed to parse TOML".to_string()))?;

    let root = tree.root_node();
    if root.has_error() {
        return Err(CoreError::Message(
            "TOML document contains syntax errors".to_string(),
        ));
    }

    let mut keys = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "pair" => extract_pair(child, source, &[], 0, &mut keys),
            "table" => extract_table(child, source, &mut keys),
            "table_array_element" => extract_table_array(child, source, &mut keys),
            _ => {}
        }
    }

    Ok(StructuredAst {
        source: source.to_string(),
        kind: DocumentKind::Toml,
        keys,
    })
}

/// Extract the key segments from a key node (bare_key, quoted_key, or dotted_key).
fn extract_key_parts(node: Node, source: &str) -> Vec<String> {
    match node.kind() {
        "bare_key" => {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            vec![text.to_string()]
        }
        "quoted_key" => {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            // Strip surrounding quotes (both single and double)
            let trimmed = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(text);
            vec![trimmed.to_string()]
        }
        "dotted_key" => {
            let mut parts = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "bare_key" | "quoted_key" | "dotted_key" => {
                        parts.extend(extract_key_parts(child, source));
                    }
                    _ => {} // skip dots
                }
            }
            parts
        }
        _ => vec![],
    }
}

/// Extract a key-value pair.
fn extract_pair(
    node: Node,
    source: &str,
    parent_path: &[String],
    base_depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut key_node = None;
    let mut value_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "bare_key" | "quoted_key" | "dotted_key" if key_node.is_none() => {
                key_node = Some(child);
            }
            "=" | "comment" => {}
            _ if key_node.is_some() && value_node.is_none() => {
                value_node = Some(child);
            }
            _ => {}
        }
    }

    let (key_nd, val_nd) = match (key_node, value_node) {
        (Some(k), Some(v)) => (k, v),
        _ => return,
    };

    let key_parts = extract_key_parts(key_nd, source);
    if key_parts.is_empty() {
        return;
    }

    // For dotted keys like `a.b.c = val`, we create intermediate Object entries
    // and a final entry for the leaf value.
    let mut current_path = parent_path.to_vec();

    for (i, part) in key_parts.iter().enumerate() {
        current_path.push(part.clone());
        let depth = base_depth + i;

        if i < key_parts.len() - 1 {
            // Intermediate dotted key segment — emit as Object
            keys.push(KeyEntry {
                path: current_path.join("."),
                key: part.clone(),
                depth,
                value_kind: ValueKind::Object,
                key_range: node_to_range(key_nd, source),
                value_range: node_to_range(key_nd, source),
            });
        } else {
            // Final key segment — emit with actual value
            let value_kind = classify_value(&val_nd);
            keys.push(KeyEntry {
                path: current_path.join("."),
                key: part.clone(),
                depth,
                value_kind,
                key_range: node_to_range(key_nd, source),
                value_range: node_to_range(val_nd, source),
            });

            // Recurse into inline tables and arrays
            walk_value(val_nd, source, &current_path, depth + 1, keys);
        }
    }
}

/// Extract a standard table `[header]` and its contained pairs.
fn extract_table(node: Node, source: &str, keys: &mut Vec<KeyEntry>) {
    let (header_parts, header_node) = extract_table_header(node, source);
    if header_parts.is_empty() {
        return;
    }

    // Emit the table header as an Object entry
    let header_range = header_node
        .map(|n| node_to_range(n, source))
        .unwrap_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0)));
    emit_table_path(&header_parts, 0, header_range, keys);

    // Extract child pairs
    let depth = header_parts.len();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            extract_pair(child, source, &header_parts, depth, keys);
        }
    }
}

/// Extract an array-of-tables `[[header]]` element.
fn extract_table_array(node: Node, source: &str, keys: &mut Vec<KeyEntry>) {
    let (header_parts, header_node) = extract_table_header(node, source);
    if header_parts.is_empty() {
        return;
    }

    // Emit the table array header as an Array entry
    let header_range = header_node
        .map(|n| node_to_range(n, source))
        .unwrap_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0)));

    // Emit path segments leading to the array
    for (i, part) in header_parts.iter().enumerate() {
        let path: Vec<_> = header_parts[..=i].to_vec();
        let value_kind = if i < header_parts.len() - 1 {
            ValueKind::Object
        } else {
            ValueKind::Array
        };
        keys.push(KeyEntry {
            path: path.join("."),
            key: part.clone(),
            depth: i,
            value_kind,
            key_range: header_range,
            value_range: header_range,
        });
    }

    // Extract child pairs under the array element
    let depth = header_parts.len();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            extract_pair(child, source, &header_parts, depth, keys);
        }
    }
}

/// Extract the header key segments from a table or table_array_element node.
fn extract_table_header<'a>(node: Node<'a>, source: &str) -> (Vec<String>, Option<Node<'a>>) {
    let mut parts = Vec::new();
    let mut header_node = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "bare_key" | "quoted_key" | "dotted_key" => {
                if header_node.is_none() {
                    header_node = Some(child);
                }
                parts.extend(extract_key_parts(child, source));
            }
            _ => {}
        }
    }
    (parts, header_node)
}

/// Emit Object entries for each segment of a table path.
fn emit_table_path(parts: &[String], base_depth: usize, range: Range, keys: &mut Vec<KeyEntry>) {
    for (i, part) in parts.iter().enumerate() {
        let path: Vec<_> = parts[..=i].to_vec();
        keys.push(KeyEntry {
            path: path.join("."),
            key: part.clone(),
            depth: base_depth + i,
            value_kind: ValueKind::Object,
            key_range: range,
            value_range: range,
        });
    }
}

/// Recursively walk a TOML value for inline tables and arrays.
fn walk_value(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    match node.kind() {
        "inline_table" => walk_inline_table(node, source, path, depth, keys),
        "array" => walk_array(node, source, path, depth, keys),
        _ => {}
    }
}

/// Walk an inline table, extracting child pairs.
fn walk_inline_table(
    node: Node,
    source: &str,
    path: &[String],
    depth: usize,
    keys: &mut Vec<KeyEntry>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            extract_pair(child, source, path, depth, keys);
        }
    }
}

/// Walk an array, extracting indexed elements.
fn walk_array(node: Node, source: &str, path: &[String], depth: usize, keys: &mut Vec<KeyEntry>) {
    let mut index = 0usize;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip brackets, commas, comments
        if child.kind() == "["
            || child.kind() == "]"
            || child.kind() == ","
            || child.kind() == "comment"
        {
            continue;
        }
        // Skip whitespace/newline tokens
        if !child.is_named() {
            continue;
        }

        let index_key = format!("[{index}]");
        let mut full_path = path.to_vec();

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

        // Recurse into nested inline tables or arrays
        walk_value(child, source, &full_path, depth + 1, keys);

        index += 1;
    }
}

/// Classify a tree-sitter node into a [`ValueKind`].
fn classify_value(node: &Node) -> ValueKind {
    match node.kind() {
        "string" => ValueKind::String,
        "integer" => ValueKind::Number,
        "float" => ValueKind::Number,
        "boolean" => ValueKind::Boolean,
        "array" => ValueKind::Array,
        "inline_table" => ValueKind::Object,
        "offset_date_time" | "local_date_time" | "local_date" | "local_time" => ValueKind::String,
        _ => ValueKind::Null,
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
    fn test_parse_toml_empty() {
        let ast = parse_toml("").unwrap();
        assert_eq!(ast.kind, DocumentKind::Toml);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_toml_flat_pairs() {
        let source = "name = \"Alice\"\nage = 30";
        let ast = parse_toml(source).unwrap();
        assert_eq!(ast.keys.len(), 2);

        assert_eq!(ast.keys[0].key, "name");
        assert_eq!(ast.keys[0].path, "name");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);

        assert_eq!(ast.keys[1].key, "age");
        assert_eq!(ast.keys[1].path, "age");
        assert_eq!(ast.keys[1].depth, 0);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Number);
    }

    #[test]
    fn test_parse_toml_table() {
        let source = "[database]\nhost = \"localhost\"\nport = 5432";
        let ast = parse_toml(source).unwrap();

        // "database" (Object, d0), "database.host" (String, d1), "database.port" (Number, d1)
        assert_eq!(ast.keys[0].key, "database");
        assert_eq!(ast.keys[0].path, "database");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "host");
        assert_eq!(ast.keys[1].path, "database.host");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::String);

        assert_eq!(ast.keys[2].key, "port");
        assert_eq!(ast.keys[2].path, "database.port");
        assert_eq!(ast.keys[2].depth, 1);
        assert_eq!(ast.keys[2].value_kind, ValueKind::Number);
    }

    #[test]
    fn test_parse_toml_nested_tables() {
        let source = "[servers.alpha]\nip = \"10.0.0.1\"";
        let ast = parse_toml(source).unwrap();

        // "servers" (Object, d0), "servers.alpha" (Object, d1), "servers.alpha.ip" (String, d2)
        assert_eq!(ast.keys[0].key, "servers");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "alpha");
        assert_eq!(ast.keys[1].path, "servers.alpha");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[2].key, "ip");
        assert_eq!(ast.keys[2].path, "servers.alpha.ip");
        assert_eq!(ast.keys[2].depth, 2);
        assert_eq!(ast.keys[2].value_kind, ValueKind::String);
    }

    #[test]
    fn test_parse_toml_dotted_key_pair() {
        let source = "physical.color = \"orange\"";
        let ast = parse_toml(source).unwrap();

        // "physical" (Object, d0), "physical.color" (String, d1)
        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "physical");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "color");
        assert_eq!(ast.keys[1].path, "physical.color");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::String);
    }

    #[test]
    fn test_parse_toml_inline_table() {
        let source = "point = { x = 1, y = 2 }";
        let ast = parse_toml(source).unwrap();

        // "point" (Object, d0), "point.x" (Number, d1), "point.y" (Number, d1)
        assert_eq!(ast.keys[0].key, "point");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "x");
        assert_eq!(ast.keys[1].path, "point.x");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Number);

        assert_eq!(ast.keys[2].key, "y");
        assert_eq!(ast.keys[2].path, "point.y");
        assert_eq!(ast.keys[2].depth, 1);
    }

    #[test]
    fn test_parse_toml_array() {
        let source = "ports = [8001, 8001, 8002]";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys[0].key, "ports");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "ports[0]");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[1].value_kind, ValueKind::Number);

        assert_eq!(ast.keys[2].path, "ports[1]");
        assert_eq!(ast.keys[3].path, "ports[2]");
    }

    #[test]
    fn test_parse_toml_array_of_tables() {
        let source = "[[products]]\nname = \"Hammer\"\n\n[[products]]\nname = \"Nail\"";
        let ast = parse_toml(source).unwrap();

        // First [[products]]: "products" (Array, d0), "products.name" (String, d1)
        // Second [[products]]: "products" (Array, d0), "products.name" (String, d1)
        let product_entries: Vec<_> = ast.keys.iter().filter(|k| k.key == "products").collect();
        assert_eq!(product_entries.len(), 2);
        assert_eq!(product_entries[0].value_kind, ValueKind::Array);

        let name_entries: Vec<_> = ast.keys.iter().filter(|k| k.key == "name").collect();
        assert_eq!(name_entries.len(), 2);
        assert_eq!(name_entries[0].path, "products.name");
        assert_eq!(name_entries[0].depth, 1);
    }

    #[test]
    fn test_parse_toml_value_kinds() {
        let source = r#"
s = "text"
i = 42
f = 3.14
b = true
bf = false
d = 1979-05-27
dt = 1979-05-27T07:32:00Z
lt = 07:32:00
a = [1, 2]
o = { k = "v" }
"#;
        let ast = parse_toml(source).unwrap();

        let kinds: Vec<_> = ast
            .keys
            .iter()
            .filter(|k| k.depth == 0)
            .map(|k| (&k.key, k.value_kind))
            .collect();
        assert_eq!(kinds[0], (&"s".to_string(), ValueKind::String));
        assert_eq!(kinds[1], (&"i".to_string(), ValueKind::Number));
        assert_eq!(kinds[2], (&"f".to_string(), ValueKind::Number));
        assert_eq!(kinds[3], (&"b".to_string(), ValueKind::Boolean));
        assert_eq!(kinds[4], (&"bf".to_string(), ValueKind::Boolean));
        assert_eq!(kinds[5], (&"d".to_string(), ValueKind::String)); // dates → String
        assert_eq!(kinds[6], (&"dt".to_string(), ValueKind::String));
        assert_eq!(kinds[7], (&"lt".to_string(), ValueKind::String));
        assert_eq!(kinds[8], (&"a".to_string(), ValueKind::Array));
        assert_eq!(kinds[9], (&"o".to_string(), ValueKind::Object));
    }

    #[test]
    fn test_parse_toml_position_accuracy() {
        let source = "name = \"Alice\"";
        let ast = parse_toml(source).unwrap();

        let entry = &ast.keys[0];
        assert_eq!(entry.key, "name");

        // "name" is at bytes 0..4 (line 0, col 0..4)
        assert_eq!(entry.key_range.start, Position::new(0, 0));
        assert_eq!(entry.key_range.end, Position::new(0, 4));

        // "Alice" is at bytes 7..14 (line 0, col 7..14, includes quotes)
        assert_eq!(entry.value_range.start, Position::new(0, 7));
        assert_eq!(entry.value_range.end, Position::new(0, 14));
    }

    #[test]
    fn test_parse_toml_quoted_keys() {
        let source = "\"127.0.0.1\" = \"value\"";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys[0].key, "127.0.0.1");
        assert_eq!(ast.keys[0].path, "127.0.0.1");
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
    }

    #[test]
    fn test_parse_toml_mixed_tables_and_pairs() {
        let source = "title = \"My Config\"\n\n[database]\nhost = \"localhost\"\n\n[logging]\nlevel = \"info\"";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys[0].path, "title");
        assert_eq!(ast.keys[0].depth, 0);

        assert_eq!(ast.keys[1].path, "database");
        assert_eq!(ast.keys[1].depth, 0);

        assert_eq!(ast.keys[2].path, "database.host");
        assert_eq!(ast.keys[2].depth, 1);

        assert_eq!(ast.keys[3].path, "logging");
        assert_eq!(ast.keys[3].depth, 0);

        assert_eq!(ast.keys[4].path, "logging.level");
        assert_eq!(ast.keys[4].depth, 1);
    }

    #[test]
    fn test_parse_toml_comments_ignored() {
        let source = "# This is a comment\nkey = \"value\" # inline comment";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "key");
    }

    #[test]
    fn test_parse_toml_multiline_string() {
        let source = "bio = \"\"\"\nHello\nWorld\"\"\"";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "bio");
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);
    }

    #[test]
    fn test_parse_toml_root_keys() {
        let source = "[a]\nx = 1\n[b]\ny = 2";
        let ast = parse_toml(source).unwrap();

        let roots = ast.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].key, "a");
        assert_eq!(roots[1].key, "b");
    }

    #[test]
    fn test_parse_toml_malformed_returns_error() {
        let result = parse_toml("= no key");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_toml_nested_inline_table() {
        let source = "config = { db = { host = \"localhost\", port = 5432 } }";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys[0].key, "config");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "db");
        assert_eq!(ast.keys[1].path, "config.db");
        assert_eq!(ast.keys[1].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[2].path, "config.db.host");
        assert_eq!(ast.keys[2].depth, 2);

        assert_eq!(ast.keys[3].path, "config.db.port");
        assert_eq!(ast.keys[3].depth, 2);
    }

    #[test]
    fn test_parse_toml_array_of_inline_tables() {
        let source = "points = [{ x = 1, y = 2 }, { x = 3, y = 4 }]";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys[0].key, "points");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Array);

        assert_eq!(ast.keys[1].path, "points[0]");
        assert_eq!(ast.keys[1].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[2].path, "points[0].x");
        assert_eq!(ast.keys[3].path, "points[0].y");

        assert_eq!(ast.keys[4].path, "points[1]");
        assert_eq!(ast.keys[5].path, "points[1].x");
        assert_eq!(ast.keys[6].path, "points[1].y");
    }

    #[test]
    fn test_parse_toml_integer_formats() {
        let source = "dec = 42\nhex = 0xDEAD\noct = 0o755\nbin = 0b11010110";
        let ast = parse_toml(source).unwrap();

        for entry in &ast.keys {
            assert_eq!(entry.value_kind, ValueKind::Number);
        }
    }

    #[test]
    fn test_parse_toml_large_document() {
        let mut source = String::new();
        for i in 0..1000 {
            source.push_str(&format!("key_{i} = {i}\n"));
        }
        let ast = parse_toml(&source).unwrap();
        assert_eq!(ast.keys.len(), 1000);
    }

    #[test]
    fn test_parse_toml_unicode_keys_and_values() {
        let source = "\"ключ\" = \"значение\"\n\"名前\" = \"太郎\"";
        let ast = parse_toml(source).unwrap();

        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "ключ");
        assert_eq!(ast.keys[1].key, "名前");
    }
}
