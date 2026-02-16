//! JSON5 parser with byte-accurate source ranges.
//!
//! Uses the `json5` crate for structural parsing (via serde) and a source
//! scanner for byte-accurate position tracking. Handles JSON5 features:
//! unquoted keys, single-quoted strings, trailing commas, and comments.

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Range};

use super::byte_to_position;

/// Parse a JSON5 document into a [`StructuredAst`].
pub fn parse_json5(source: &str) -> Result<StructuredAst, CoreError> {
    let value: serde_json::Value = json5::from_str(source)
        .map_err(|e| CoreError::Message(format!("failed to parse JSON5: {e}")))?;

    let mut keys = Vec::new();
    let mut scanner = Scanner::new(source);

    // Advance past leading whitespace/comments to the root value.
    scanner.skip_whitespace_and_comments();

    walk_value(&value, source, &[], 0, &mut scanner, &mut keys);

    Ok(StructuredAst {
        source: source.to_string(),
        kind: DocumentKind::Json5,
        keys,
    })
}

/// A byte-offset scanner over JSON5 source text.
struct Scanner<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            src: source.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.pos < self.src.len() {
            self.pos += 1;
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }

            if self.pos + 1 < self.src.len() && self.src[self.pos] == b'/' {
                if self.src[self.pos + 1] == b'/' {
                    // Single-line comment: skip to end of line
                    self.pos += 2;
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                    continue;
                } else if self.src[self.pos + 1] == b'*' {
                    // Block comment: skip to */
                    self.pos += 2;
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                    continue;
                }
            }

            break;
        }
    }

    /// Expect and consume a specific byte. Panics on mismatch (internal invariant).
    fn expect(&mut self, ch: u8) {
        debug_assert_eq!(
            self.peek(),
            Some(ch),
            "expected '{}' at pos {}, found {:?}",
            ch as char,
            self.pos,
            self.peek().map(|b| b as char)
        );
        self.advance();
    }

    /// Read the current position as a key token (quoted or unquoted identifier).
    /// Returns `(key_text, key_start_byte, key_end_byte)`.
    fn read_key(&mut self) -> (String, usize, usize) {
        self.skip_whitespace_and_comments();
        let start = self.pos;

        match self.peek() {
            Some(b'"') => {
                let text = self.read_quoted_string(b'"');
                (text, start, self.pos)
            }
            Some(b'\'') => {
                let text = self.read_quoted_string(b'\'');
                (text, start, self.pos)
            }
            _ => {
                // Unquoted identifier
                let text = self.read_identifier();
                (text, start, self.pos)
            }
        }
    }

    /// Read a quoted string (single or double), consuming the quotes.
    /// Returns the inner text (without quotes), with escape sequences decoded
    /// to match serde/json5 output (required for key lookup in the serde map).
    fn read_quoted_string(&mut self, quote: u8) -> String {
        self.advance(); // opening quote
        let mut text = String::new();
        while self.pos < self.src.len() {
            let ch = self.src[self.pos];
            if ch == b'\\' {
                self.advance();
                if self.pos < self.src.len() {
                    let escaped = self.src[self.pos];
                    match escaped {
                        b'"' => text.push('"'),
                        b'\'' => text.push('\''),
                        b'\\' => text.push('\\'),
                        b'/' => text.push('/'),
                        b'n' => text.push('\n'),
                        b'r' => text.push('\r'),
                        b't' => text.push('\t'),
                        b'b' => text.push('\u{0008}'),
                        b'f' => text.push('\u{000C}'),
                        b'u' => {
                            // \uXXXX unicode escape
                            self.advance();
                            let hex_start = self.pos;
                            let hex_end = (self.pos + 4).min(self.src.len());
                            let hex = &self.src[hex_start..hex_end];
                            if hex.len() == 4 {
                                if let Ok(s) = std::str::from_utf8(hex) {
                                    if let Ok(code) = u32::from_str_radix(s, 16) {
                                        if let Some(c) = char::from_u32(code) {
                                            text.push(c);
                                            self.pos += 4;
                                            continue;
                                        }
                                    }
                                }
                            }
                            // Invalid unicode escape — push literal
                            text.push('u');
                            continue;
                        }
                        // JSON5 spec: any other char after \ is itself
                        other => text.push(other as char),
                    }
                    self.advance();
                }
            } else if ch == quote {
                self.advance(); // closing quote
                break;
            } else {
                text.push(ch as char);
                self.advance();
            }
        }
        text
    }

    /// Read an unquoted identifier (ECMAScript IdentifierName subset).
    fn read_identifier(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.src.len() {
            let ch = self.src[self.pos];
            if ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$' {
                self.pos += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).to_string()
    }

    /// Read and skip a value in the source, returning `(value_start_byte, value_end_byte)`.
    /// Does NOT recurse into children — just identifies the byte span of the value.
    fn scan_value_span(&mut self) -> (usize, usize) {
        self.skip_whitespace_and_comments();
        let start = self.pos;

        match self.peek() {
            Some(b'{') => {
                self.skip_balanced(b'{', b'}');
                (start, self.pos)
            }
            Some(b'[') => {
                self.skip_balanced(b'[', b']');
                (start, self.pos)
            }
            Some(b'"') => {
                self.read_quoted_string(b'"');
                (start, self.pos)
            }
            Some(b'\'') => {
                self.read_quoted_string(b'\'');
                (start, self.pos)
            }
            _ => {
                // Number, boolean, null, or unquoted literal
                self.skip_literal();
                (start, self.pos)
            }
        }
    }

    /// Skip a balanced pair of delimiters (handling nesting, strings, comments).
    fn skip_balanced(&mut self, open: u8, close: u8) {
        let mut depth = 0;
        loop {
            match self.peek() {
                None => break,
                Some(ch) if ch == open => {
                    depth += 1;
                    self.advance();
                }
                Some(ch) if ch == close => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                Some(b'"') => {
                    self.read_quoted_string(b'"');
                }
                Some(b'\'') => {
                    self.read_quoted_string(b'\'');
                }
                Some(b'/') if self.pos + 1 < self.src.len() => {
                    let next = self.src[self.pos + 1];
                    if next == b'/' || next == b'*' {
                        self.skip_whitespace_and_comments();
                    } else {
                        self.advance();
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Skip a literal token (number, boolean, null, identifier).
    fn skip_literal(&mut self) {
        while self.pos < self.src.len() {
            let ch = self.src[self.pos];
            // Literals end at commas, braces, brackets, whitespace, colons, comments
            if ch == b','
                || ch == b'}'
                || ch == b']'
                || ch == b':'
                || ch.is_ascii_whitespace()
                || ch == b'/'
            {
                break;
            }
            self.pos += 1;
        }
    }
}

/// Recursively walk a serde Value, extracting KeyEntry items with source positions.
fn walk_value(
    value: &serde_json::Value,
    source: &str,
    path: &[String],
    depth: usize,
    scanner: &mut Scanner,
    keys: &mut Vec<KeyEntry>,
) {
    match value {
        serde_json::Value::Object(map) => {
            walk_object(map, source, path, depth, scanner, keys);
        }
        serde_json::Value::Array(arr) => {
            walk_array(arr, source, path, depth, scanner, keys);
        }
        _ => {
            // Scalar — just scan past it
            scanner.scan_value_span();
        }
    }
}

/// Walk a JSON5 object, extracting key entries.
///
/// Iterates keys in **source order** (driven by the scanner), looking up each
/// key in the serde map for type information. This avoids depending on
/// `serde_json::Map` iteration order (which may be sorted, not insertion-ordered).
fn walk_object(
    map: &serde_json::Map<String, serde_json::Value>,
    source: &str,
    path: &[String],
    depth: usize,
    scanner: &mut Scanner,
    keys: &mut Vec<KeyEntry>,
) {
    scanner.skip_whitespace_and_comments();
    scanner.expect(b'{');

    let num_keys = map.len();
    for _ in 0..num_keys {
        // Read the key token from source (in source order)
        let (key_text, key_start, key_end) = scanner.read_key();

        // Look up the scanned key in the serde map for its value/type
        let value = map.get(&key_text).unwrap_or(&serde_json::Value::Null);

        // Skip colon
        scanner.skip_whitespace_and_comments();
        scanner.expect(b':');
        scanner.skip_whitespace_and_comments();

        let val_start = scanner.pos;

        let mut full_path = path.to_vec();
        full_path.push(key_text.clone());
        let path_str = full_path.join(".");

        let value_kind = classify_serde_value(value);

        match value {
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                // Peek to get full span, then recurse
                let saved = scanner.pos;
                let (_, val_end) = scanner.scan_value_span();
                scanner.pos = saved;

                keys.push(KeyEntry {
                    path: path_str,
                    key: key_text,
                    depth,
                    value_kind,
                    key_range: Range::new(
                        byte_to_position(source, key_start),
                        byte_to_position(source, key_end),
                    ),
                    value_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                });

                walk_value(value, source, &full_path, depth + 1, scanner, keys);
            }
            _ => {
                let (_, val_end) = scanner.scan_value_span();

                keys.push(KeyEntry {
                    path: path_str,
                    key: key_text,
                    depth,
                    value_kind,
                    key_range: Range::new(
                        byte_to_position(source, key_start),
                        byte_to_position(source, key_end),
                    ),
                    value_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                });
            }
        }

        // Skip comma if present (trailing comma is valid in JSON5)
        scanner.skip_whitespace_and_comments();
        if scanner.peek() == Some(b',') {
            scanner.advance();
        }
    }

    scanner.skip_whitespace_and_comments();
    scanner.expect(b'}');
}

/// Walk a JSON5 array, extracting entries for each element.
fn walk_array(
    arr: &[serde_json::Value],
    source: &str,
    path: &[String],
    depth: usize,
    scanner: &mut Scanner,
    keys: &mut Vec<KeyEntry>,
) {
    scanner.skip_whitespace_and_comments();
    scanner.expect(b'[');

    for (i, value) in arr.iter().enumerate() {
        scanner.skip_whitespace_and_comments();

        let index_key = format!("[{i}]");
        let mut full_path = path.to_vec();

        if let Some(last) = full_path.last_mut() {
            *last = format!("{last}{index_key}");
        } else {
            full_path.push(index_key.clone());
        }

        let value_kind = classify_serde_value(value);
        let val_start = scanner.pos;

        match value {
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                // Peek to get span, then recurse
                let saved = scanner.pos;
                let (_, val_end) = scanner.scan_value_span();
                scanner.pos = saved;

                keys.push(KeyEntry {
                    path: full_path.join("."),
                    key: index_key,
                    depth,
                    value_kind,
                    key_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                    value_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                });

                walk_value(value, source, &full_path, depth + 1, scanner, keys);
            }
            _ => {
                let (_, val_end) = scanner.scan_value_span();

                keys.push(KeyEntry {
                    path: full_path.join("."),
                    key: index_key,
                    depth,
                    value_kind,
                    key_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                    value_range: Range::new(
                        byte_to_position(source, val_start),
                        byte_to_position(source, val_end),
                    ),
                });
            }
        }

        // Skip comma if present
        scanner.skip_whitespace_and_comments();
        if scanner.peek() == Some(b',') {
            scanner.advance();
        }
    }

    scanner.skip_whitespace_and_comments();
    scanner.expect(b']');
}

/// Classify a serde_json::Value into a ValueKind.
fn classify_serde_value(value: &serde_json::Value) -> ValueKind {
    match value {
        serde_json::Value::String(_) => ValueKind::String,
        serde_json::Value::Number(_) => ValueKind::Number,
        serde_json::Value::Bool(_) => ValueKind::Boolean,
        serde_json::Value::Null => ValueKind::Null,
        serde_json::Value::Array(_) => ValueKind::Array,
        serde_json::Value::Object(_) => ValueKind::Object,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json5_empty_object() {
        let ast = parse_json5("{}").unwrap();
        assert_eq!(ast.kind, DocumentKind::Json5);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_json5_flat() {
        let source = r#"{"a": 1, "b": "hello"}"#;
        let ast = parse_json5(source).unwrap();
        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "a");
        assert_eq!(ast.keys[1].key, "b");
    }

    #[test]
    fn test_scanner_skip_line_comment() {
        let src = "// comment\n42";
        let mut scanner = Scanner::new(src);
        scanner.skip_whitespace_and_comments();
        assert_eq!(scanner.pos, 11); // past the newline
    }

    #[test]
    fn test_scanner_skip_block_comment() {
        let src = "/* block */ 42";
        let mut scanner = Scanner::new(src);
        scanner.skip_whitespace_and_comments();
        assert_eq!(scanner.pos, 12); // past the space after */
    }

    #[test]
    fn test_scanner_read_unquoted_key() {
        let src = "myKey: 42";
        let mut scanner = Scanner::new(src);
        let (text, start, end) = scanner.read_key();
        assert_eq!(text, "myKey");
        assert_eq!(start, 0);
        assert_eq!(end, 5);
    }

    #[test]
    fn test_scanner_read_quoted_key() {
        let src = r#""myKey": 42"#;
        let mut scanner = Scanner::new(src);
        let (text, start, end) = scanner.read_key();
        assert_eq!(text, "myKey");
        assert_eq!(start, 0);
        assert_eq!(end, 7);
    }

    #[test]
    fn test_scanner_read_single_quoted_key() {
        let src = "'myKey': 42";
        let mut scanner = Scanner::new(src);
        let (text, start, end) = scanner.read_key();
        assert_eq!(text, "myKey");
        assert_eq!(start, 0);
        assert_eq!(end, 7);
    }
}
