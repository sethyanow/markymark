//! Flat file parser for `.env`, `.ini`, and `.cfg` files.
//!
//! Handles key=value pairs with optional `[section]` headers (INI/CFG).
//! Comments start with `#` or `;`. Values are always [`ValueKind::String`].

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::{CoreError, Range};

use super::byte_to_position;

/// Parse a flat config file (.env, .ini, .cfg) into a [`StructuredAst`].
///
/// - `.env`: key=value pairs, no sections
/// - `.ini`/`.cfg`: optional `[section]` headers, key=value or key: value pairs
pub fn parse_flat(source: &str, kind: DocumentKind) -> Result<StructuredAst, CoreError> {
    let mut keys = Vec::new();
    let mut current_section: Option<String> = None;
    let mut line_byte_offset = 0usize;

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            line_byte_offset += line.len() + 1;
            continue;
        }

        // Check for section header [section]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = trimmed[1..trimmed.len() - 1].trim().to_string();
            if !section_name.is_empty() {
                let section_start_byte = line_byte_offset + line.find('[').unwrap_or(0);
                let section_end_byte =
                    line_byte_offset + line.rfind(']').map(|i| i + 1).unwrap_or(line.len());
                let key_range = Range::new(
                    byte_to_position(source, section_start_byte),
                    byte_to_position(source, section_end_byte),
                );

                keys.push(KeyEntry {
                    path: section_name.clone(),
                    key: section_name.clone(),
                    depth: 0,
                    value_kind: ValueKind::Object,
                    key_range,
                    value_range: key_range,
                });

                current_section = Some(section_name);
            }
            line_byte_offset += line.len() + 1;
            continue;
        }

        // Parse key=value or key: value
        let (key, value, sep_pos) = if let Some(eq_pos) = trimmed.find('=') {
            let k = trimmed[..eq_pos].trim();
            let v = trimmed[eq_pos + 1..].trim();
            (k, v, eq_pos)
        } else if let Some(colon_pos) = trimmed.find(':') {
            let k = trimmed[..colon_pos].trim();
            let v = trimmed[colon_pos + 1..].trim();
            (k, v, colon_pos)
        } else {
            line_byte_offset += line.len() + 1;
            continue;
        };

        if key.is_empty() {
            line_byte_offset += line.len() + 1;
            continue;
        }

        // Compute ranges using raw value length (before quote stripping)
        let raw_value = value;

        // Strip surrounding quotes from value if present
        let _value = raw_value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| {
                raw_value
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''))
            })
            .unwrap_or(raw_value);

        // Compute ranges
        let leading_whitespace = line.len() - line.trim_start().len();
        let key_start_byte = line_byte_offset + leading_whitespace;
        let key_end_byte = key_start_byte + key.len();
        let val_start_byte = line_byte_offset + leading_whitespace + sep_pos + 1;
        // Skip whitespace after separator for value range
        let val_trimmed_offset =
            trimmed[sep_pos + 1..].len() - trimmed[sep_pos + 1..].trim_start().len();
        let val_start_byte = val_start_byte + val_trimmed_offset;
        let val_end_byte = val_start_byte + raw_value.len();

        let key_range = Range::new(
            byte_to_position(source, key_start_byte),
            byte_to_position(source, key_end_byte),
        );
        let value_range = Range::new(
            byte_to_position(source, val_start_byte),
            byte_to_position(source, val_end_byte),
        );

        let (path, depth) = match &current_section {
            Some(section) => (format!("{section}.{key}"), 1),
            None => (key.to_string(), 0),
        };

        keys.push(KeyEntry {
            path,
            key: key.to_string(),
            depth,
            value_kind: ValueKind::String,
            key_range,
            value_range,
        });

        line_byte_offset += line.len() + 1;
    }

    Ok(StructuredAst {
        source: source.to_string(),
        kind,
        keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::Position;

    #[test]
    fn test_parse_env_empty() {
        let ast = parse_flat("", DocumentKind::DotEnv).unwrap();
        assert_eq!(ast.kind, DocumentKind::DotEnv);
        assert!(ast.keys.is_empty());
    }

    #[test]
    fn test_parse_env_basic() {
        let source = "DATABASE_URL=postgres://localhost\nPORT=3000";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "DATABASE_URL");
        assert_eq!(ast.keys[0].path, "DATABASE_URL");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::String);

        assert_eq!(ast.keys[1].key, "PORT");
        assert_eq!(ast.keys[1].path, "PORT");
    }

    #[test]
    fn test_parse_env_comments() {
        let source = "# Database config\nDB_HOST=localhost\n# Port\nDB_PORT=5432";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "DB_HOST");
        assert_eq!(ast.keys[1].key, "DB_PORT");
    }

    #[test]
    fn test_parse_env_quoted_values() {
        let source = "NAME=\"John Doe\"\nPATH='some/path'";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        // Quotes should be stripped from values (not from key ranges)
        assert_eq!(ast.keys.len(), 2);
    }

    #[test]
    fn test_parse_ini_sections() {
        let source = "[database]\nhost = localhost\nport = 5432\n\n[logging]\nlevel = info";
        let ast = parse_flat(source, DocumentKind::Ini).unwrap();

        // [database] (Object, d0), database.host (String, d1), database.port (String, d1)
        // [logging] (Object, d0), logging.level (String, d1)
        assert_eq!(ast.keys[0].key, "database");
        assert_eq!(ast.keys[0].depth, 0);
        assert_eq!(ast.keys[0].value_kind, ValueKind::Object);

        assert_eq!(ast.keys[1].key, "host");
        assert_eq!(ast.keys[1].path, "database.host");
        assert_eq!(ast.keys[1].depth, 1);

        assert_eq!(ast.keys[2].key, "port");
        assert_eq!(ast.keys[2].path, "database.port");

        assert_eq!(ast.keys[3].key, "logging");
        assert_eq!(ast.keys[3].depth, 0);

        assert_eq!(ast.keys[4].key, "level");
        assert_eq!(ast.keys[4].path, "logging.level");
        assert_eq!(ast.keys[4].depth, 1);
    }

    #[test]
    fn test_parse_ini_colon_separator() {
        let source = "[section]\nkey: value";
        let ast = parse_flat(source, DocumentKind::Ini).unwrap();

        assert_eq!(ast.keys[1].key, "key");
        assert_eq!(ast.keys[1].path, "section.key");
    }

    #[test]
    fn test_parse_ini_semicolon_comments() {
        let source = "; comment\n[section]\nkey = value";
        let ast = parse_flat(source, DocumentKind::Ini).unwrap();

        assert_eq!(ast.keys.len(), 2); // section + key
    }

    #[test]
    fn test_parse_flat_position_accuracy() {
        let source = "key = value";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        let entry = &ast.keys[0];
        assert_eq!(entry.key_range.start, Position::new(0, 0));
        assert_eq!(entry.key_range.end, Position::new(0, 3));
    }

    #[test]
    fn test_parse_flat_no_value() {
        let source = "just_a_line_without_equals";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();
        assert!(ast.keys.is_empty()); // No separator = skip
    }

    #[test]
    fn test_parse_cfg_as_ini() {
        let source = "[section]\nkey = value";
        let ast = parse_flat(source, DocumentKind::Ini).unwrap();

        assert_eq!(ast.keys.len(), 2);
        assert_eq!(ast.keys[0].key, "section");
    }

    #[test]
    fn test_parse_env_blank_lines() {
        let source = "A=1\n\n\nB=2";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        assert_eq!(ast.keys.len(), 2);
    }

    #[test]
    fn test_parse_env_empty_value() {
        let source = "EMPTY=";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "EMPTY");
    }

    #[test]
    fn test_parse_env_value_with_equals() {
        let source = "URL=postgres://host?opt=val";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "URL");
        // Value should include everything after first =
    }

    #[test]
    fn test_parse_env_root_keys() {
        let source = "A=1\nB=2\nC=3";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        let roots = ast.root_keys();
        assert_eq!(roots.len(), 3);
    }

    #[test]
    fn test_parse_ini_root_keys() {
        let source = "[a]\nx = 1\n[b]\ny = 2";
        let ast = parse_flat(source, DocumentKind::Ini).unwrap();

        let roots = ast.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].key, "a");
        assert_eq!(roots[1].key, "b");
    }

    #[test]
    fn test_parse_env_quoted_value_range_includes_quotes() {
        // Value range should span the full raw value including quotes,
        // not just the stripped inner text.
        let source = "NAME=\"John Doe\"";
        let ast = parse_flat(source, DocumentKind::DotEnv).unwrap();

        let entry = &ast.keys[0];
        assert_eq!(entry.key, "NAME");
        // Value "John Doe" with quotes: starts at byte 5 (the opening "), ends at byte 15
        assert_eq!(entry.value_range.start, Position::new(0, 5));
        assert_eq!(entry.value_range.end, Position::new(0, 15)); // includes closing quote
    }

    #[test]
    fn test_parse_flat_large() {
        let mut source = String::new();
        for i in 0..1000 {
            source.push_str(&format!("KEY_{i}=VALUE_{i}\n"));
        }
        let ast = parse_flat(&source, DocumentKind::DotEnv).unwrap();
        assert_eq!(ast.keys.len(), 1000);
    }
}
