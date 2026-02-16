//! JSON Lines (.jsonl) parser.
//!
//! Each line is a separate JSON document, indexed as `[n].key.path`.
//! Uses the existing JSON parser for per-line extraction.

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Position, Range};

use super::{byte_to_position, json};

/// Parse a JSONL document into a [`StructuredAst`].
///
/// Each non-empty line is parsed as a JSON value and indexed as `[n]`.
/// Keys within each line are prefixed with the line index, e.g. `[0].name`.
pub fn parse_jsonl(source: &str) -> Result<StructuredAst, CoreError> {
    let mut keys = Vec::new();
    let mut line_byte_offset = 0usize;

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line_byte_offset += line.len() + 1; // +1 for newline
            continue;
        }

        let leading_ws = line.len() - line.trim_start().len();
        let index_key = format!("[{line_idx}]");
        let line_start = byte_to_position(source, line_byte_offset);
        let line_end = byte_to_position(source, line_byte_offset + line.len());
        let line_range = Range::new(line_start, line_end);

        // Quick validation: JSON values must start with {, [, ", a digit, t, f, or n
        let is_plausible_json = trimmed.starts_with('{')
            || trimmed.starts_with('[')
            || trimmed.starts_with('"')
            || trimmed.starts_with('-')
            || trimmed
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_digit())
            || trimmed == "true"
            || trimmed == "false"
            || trimmed == "null";

        // Try to parse this line as JSON
        match if is_plausible_json {
            json::parse_json(trimmed)
        } else {
            Err(CoreError::Message("not valid JSON".to_string()))
        } {
            Ok(line_ast) => {
                // Classify the root value kind from the first key or fallback
                let root_kind = if line_ast.keys.is_empty() {
                    // Scalar value or empty object/array
                    classify_root_value(trimmed)
                } else {
                    // Has keys — must be object or array
                    let first = &line_ast.keys[0];
                    if first.depth == 0 && first.key.starts_with('[') {
                        ValueKind::Array
                    } else {
                        ValueKind::Object
                    }
                };

                // Emit the line-level entry
                keys.push(KeyEntry {
                    path: index_key.clone(),
                    key: index_key.clone(),
                    depth: 0,
                    value_kind: root_kind,
                    key_range: line_range,
                    value_range: line_range,
                });

                // Re-prefix all child keys with the line index
                for entry in &line_ast.keys {
                    let prefixed_path = format!("{index_key}.{}", entry.path);
                    keys.push(KeyEntry {
                        path: prefixed_path,
                        key: entry.key.clone(),
                        depth: entry.depth + 1,
                        value_kind: entry.value_kind,
                        key_range: offset_range(
                            entry.key_range,
                            line_byte_offset + leading_ws,
                            source,
                        ),
                        value_range: offset_range(
                            entry.value_range,
                            line_byte_offset + leading_ws,
                            source,
                        ),
                    });
                }
            }
            Err(_) => {
                // Skip malformed lines — index what we can
                keys.push(KeyEntry {
                    path: index_key.clone(),
                    key: index_key.clone(),
                    depth: 0,
                    value_kind: ValueKind::Null,
                    key_range: line_range,
                    value_range: line_range,
                });
            }
        }

        line_byte_offset += line.len() + 1; // +1 for newline
    }

    Ok(StructuredAst {
        source: source.to_string(),
        kind: DocumentKind::JsonLines,
        keys,
    })
}

/// Classify a scalar JSON value from its text.
fn classify_root_value(text: &str) -> ValueKind {
    if text.starts_with('"') {
        ValueKind::String
    } else if text == "true" || text == "false" {
        ValueKind::Boolean
    } else if text == "null" {
        ValueKind::Null
    } else if text.starts_with('{') {
        ValueKind::Object
    } else if text.starts_with('[') {
        ValueKind::Array
    } else {
        ValueKind::Number
    }
}

/// Offset a range by the line's byte offset in the source.
///
/// The JSON parser produces ranges relative to the line; we need
/// to convert them to absolute positions in the full source.
fn offset_range(range: Range, line_byte_offset: usize, source: &str) -> Range {
    let start_byte = position_to_byte(range.start) + line_byte_offset;
    let end_byte = position_to_byte(range.end) + line_byte_offset;
    Range::new(
        byte_to_position(source, start_byte),
        byte_to_position(source, end_byte),
    )
}

/// Approximate byte offset from a Position within a single line.
/// Since the JSON parser processes trimmed single-line text,
/// the line number is always 0 and character is the byte offset.
fn position_to_byte(pos: Position) -> usize {
    // JSON parser produces positions relative to single-line input
    // line is always 0, character is byte offset within the line
    pos.character as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jsonl_empty() {
        let ast = parse_jsonl("").unwrap();
        assert_eq!(ast.kind, DocumentKind::JsonLines);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_jsonl_single_line() {
        let source = r#"{"name": "Alice", "age": 30}"#;
        let ast = parse_jsonl(source).unwrap();

        // [0] (Object), [0].name (String), [0].age (Number)
        assert_eq!(ast.keys[0].path, "[0]");
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].path, "[0].name");
        assert_eq!(ast.keys[1].depth, 1);

        assert_eq!(ast.keys[2].path, "[0].age");
    }

    #[test]
    fn test_parse_jsonl_multiple_lines() {
        let source = "{\"a\": 1}\n{\"b\": 2}\n{\"c\": 3}";
        let ast = parse_jsonl(source).unwrap();

        let line_entries: Vec<_> = ast.keys.iter().filter(|k| k.depth == 0).collect();
        assert_eq!(line_entries.len(), 3);
        assert_eq!(line_entries[0].path, "[0]");
        assert_eq!(line_entries[1].path, "[1]");
        assert_eq!(line_entries[2].path, "[2]");
    }

    #[test]
    fn test_parse_jsonl_nested_objects() {
        let source = "{\"user\": {\"name\": \"Alice\"}}";
        let ast = parse_jsonl(source).unwrap();

        assert_eq!(ast.keys[0].path, "[0]");
        assert_eq!(ast.keys[1].path, "[0].user");
        assert_eq!(ast.keys[1].depth, 1);
        assert_eq!(ast.keys[2].path, "[0].user.name");
        assert_eq!(ast.keys[2].depth, 2);
    }

    #[test]
    fn test_parse_jsonl_blank_lines_skipped() {
        let source = "{\"a\": 1}\n\n{\"b\": 2}";
        let ast = parse_jsonl(source).unwrap();

        let line_entries: Vec<_> = ast.keys.iter().filter(|k| k.depth == 0).collect();
        assert_eq!(line_entries.len(), 2);
        assert_eq!(line_entries[0].path, "[0]");
        assert_eq!(line_entries[1].path, "[2]"); // line index 2 (skipped 1)
    }

    #[test]
    fn test_parse_jsonl_malformed_line_indexed_as_null() {
        let source = "{\"a\": 1}\nnot json\n{\"b\": 2}";
        let ast = parse_jsonl(source).unwrap();

        let line_entries: Vec<_> = ast.keys.iter().filter(|k| k.depth == 0).collect();
        assert_eq!(line_entries.len(), 3);
        assert_eq!(line_entries[1].value_kind, ValueKind::Null); // malformed
    }

    #[test]
    fn test_parse_jsonl_scalar_lines() {
        let source = "42\n\"hello\"\ntrue\nnull";
        let ast = parse_jsonl(source).unwrap();

        let entries: Vec<_> = ast.keys.iter().filter(|k| k.depth == 0).collect();
        assert_eq!(entries[0].value_kind, ValueKind::Number);
        assert_eq!(entries[1].value_kind, ValueKind::String);
        assert_eq!(entries[2].value_kind, ValueKind::Boolean);
        assert_eq!(entries[3].value_kind, ValueKind::Null);
    }

    #[test]
    fn test_parse_jsonl_position_accuracy() {
        let source = "{\"a\": 1}\n{\"b\": 2}";
        let ast = parse_jsonl(source).unwrap();

        // Line 0 starts at (0,0), line 1 starts at (1,0)
        assert_eq!(ast.keys[0].key_range.start, Position::new(0, 0));
        let line1_entry: Vec<_> = ast.keys.iter().filter(|k| k.path == "[1]").collect();
        assert_eq!(line1_entry[0].key_range.start, Position::new(1, 0));
    }

    #[test]
    fn test_parse_jsonl_root_keys() {
        let source = "{\"a\": 1}\n{\"b\": 2}";
        let ast = parse_jsonl(source).unwrap();

        let roots = ast.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].path, "[0]");
        assert_eq!(roots[1].path, "[1]");
    }

    #[test]
    fn test_parse_jsonl_large_file() {
        let mut lines = Vec::new();
        for i in 0..500 {
            lines.push(format!("{{\"id\": {i}}}"));
        }
        let source = lines.join("\n");
        let ast = parse_jsonl(&source).unwrap();

        let line_entries: Vec<_> = ast.keys.iter().filter(|k| k.depth == 0).collect();
        assert_eq!(line_entries.len(), 500);
    }

    #[test]
    fn test_parse_jsonl_indented_line_ranges() {
        // Lines with leading whitespace: key ranges must account for the
        // indentation, not just the trimmed content.
        let source = "  {\"name\": \"Alice\"}\n  {\"name\": \"Bob\"}";
        let ast = parse_jsonl(source).unwrap();

        // Line 0: "  {\"name\": \"Alice\"}" — "name" key starts at column 3
        let name_entries: Vec<_> = ast.keys.iter().filter(|k| k.key == "name").collect();
        assert_eq!(name_entries.len(), 2);

        // First "name" on line 0: leading 2 spaces + { + " = byte 3 for the quote
        assert_eq!(name_entries[0].key_range.start.line, 0);
        assert_eq!(name_entries[0].key_range.start.character, 3);

        // Second "name" on line 1: leading 2 spaces + { + " = byte 3 for the quote
        assert_eq!(name_entries[1].key_range.start.line, 1);
        assert_eq!(name_entries[1].key_range.start.character, 3);
    }
}
