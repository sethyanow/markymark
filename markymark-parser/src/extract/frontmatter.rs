use crate::types::arena_alloc_str;
use crate::types::*;
use markymark_core::arena::new_arena_hashmap;

/// Extract frontmatter
pub fn extract_frontmatter<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Option<Frontmatter<'a>> {
    // Check if document starts with --- followed by a newline (LF or CRLF)
    let rest = if let Some(r) = source.strip_prefix("---\r\n") {
        r
    } else if let Some(r) = source.strip_prefix("---\n") {
        r
    } else {
        return None;
    };

    // Handle empty frontmatter: closing --- at start of rest (no preceding newline)
    if rest.starts_with("---\r\n") {
        return Some(parse_simple_yaml("", arena));
    }
    if rest.starts_with("---\n") {
        return Some(parse_simple_yaml("", arena));
    }

    // Find the earliest closing --- (handle both LF and CRLF, pick min position)
    let end_pos = [rest.find("\n---\r\n"), rest.find("\n---\n")]
        .into_iter()
        .flatten()
        .min();
    if let Some(end_pos) = end_pos {
        let yaml_content = &rest[..end_pos];
        return Some(parse_simple_yaml(yaml_content, arena));
    }

    None
}

/// Extract page properties
pub fn extract_page_properties<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Option<Properties<'a>> {
    // Logseq properties: key:: value at start of document
    let mut data = new_arena_hashmap(arena);
    let mut found_any = false;

    for line in source.lines() {
        // Stop at first non-property line (blank or heading)
        if line.is_empty() || line.starts_with('#') {
            break;
        }

        // Check for property: key:: value
        if let Some(double_colon_pos) = line.find("::") {
            let key = arena_alloc_str(arena, line[..double_colon_pos].trim());
            let value_str = line[double_colon_pos + 2..].trim();

            let value = if value_str.contains("[[") {
                // Check if it's multiple page refs (list)
                if value_str.matches("[[").count() > 1 {
                    let mut values = bumpalo::collections::Vec::new_in(arena);
                    for item in value_str.split(',') {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            values.push(arena_alloc_str(arena, trimmed));
                        }
                    }
                    PropertyValue::List(values.into_bump_slice())
                } else {
                    // Single page reference
                    PropertyValue::PageRef(arena_alloc_str(arena, value_str))
                }
            } else if value_str.contains(',') {
                let mut values = bumpalo::collections::Vec::new_in(arena);
                for item in value_str.split(',') {
                    let trimmed = item.trim();
                    if !trimmed.is_empty() {
                        values.push(arena_alloc_str(arena, trimmed));
                    }
                }
                PropertyValue::List(values.into_bump_slice())
            } else {
                // String
                PropertyValue::String(arena_alloc_str(arena, value_str))
            };

            data.insert(key, value);
            found_any = true;
        } else {
            break;
        }
    }

    if found_any {
        Some(Properties::new(data))
    } else {
        None
    }
}

/// Hint for what type a YAML scalar value should become.
///
/// Used by both the parser crate's `parse_simple_yaml` and the index crate's
/// `parse_frontmatter_owned` to ensure consistent type detection across
/// the two YAML parsing paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YamlScalarHint {
    /// null, Null, NULL, ~, or empty string
    Null,
    /// true/false/yes/no/on/off (case-insensitive per YAML 1.1)
    Boolean(bool),
    /// Fits in i64
    Integer,
    /// Fits in f64 (excluding NaN/inf)
    Float,
    /// Quoted or unrecognized scalar — keep as string
    Str,
}

/// Strip matching outer quotes from a YAML scalar.
///
/// Returns `(stripped_value, was_quoted)`. Single or double quotes are
/// removed if they form a matching pair; otherwise the value is returned
/// unchanged.
pub fn strip_yaml_quotes(value: &str) -> (&str, bool) {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return (&value[1..value.len() - 1], true);
        }
    }
    (value, false)
}

/// Detect the YAML scalar type for an unquoted value.
///
/// Priority: Null → Boolean → Integer → Float → Str.
/// Quoted strings always return `Str` (caller should use `strip_yaml_quotes`
/// first and pass `was_quoted = true` to skip detection).
pub fn detect_yaml_scalar(value: &str) -> YamlScalarHint {
    if value.is_empty() {
        return YamlScalarHint::Null;
    }

    // Null variants
    match value {
        "null" | "Null" | "NULL" | "~" => return YamlScalarHint::Null,
        _ => {}
    }

    // Boolean variants (YAML 1.1 compatible)
    match value {
        "true" | "True" | "TRUE" | "yes" | "Yes" | "YES" | "on" | "On" | "ON" => {
            return YamlScalarHint::Boolean(true);
        }
        "false" | "False" | "FALSE" | "no" | "No" | "NO" | "off" | "Off" | "OFF" => {
            return YamlScalarHint::Boolean(false);
        }
        _ => {}
    }

    // Integer (i64)
    if value.parse::<i64>().is_ok() {
        return YamlScalarHint::Integer;
    }

    // Float (f64) — reject NaN, inf, -inf
    if let Ok(f) = value.parse::<f64>() {
        if f.is_finite() {
            return YamlScalarHint::Float;
        }
    }

    YamlScalarHint::Str
}

/// Convert a trimmed scalar string to a typed `FrontmatterValue`.
fn scalar_to_value<'a>(raw: &str, arena: &'a bumpalo::Bump) -> FrontmatterValue<'a> {
    let (stripped, was_quoted) = strip_yaml_quotes(raw);
    if was_quoted {
        return FrontmatterValue::String(arena_alloc_str(arena, stripped));
    }
    match detect_yaml_scalar(stripped) {
        YamlScalarHint::Null => FrontmatterValue::Null,
        YamlScalarHint::Boolean(b) => FrontmatterValue::Boolean(b),
        YamlScalarHint::Integer => match stripped.parse::<i64>() {
            Ok(n) => FrontmatterValue::Integer(n),
            Err(_) => FrontmatterValue::String(arena_alloc_str(arena, stripped)),
        },
        YamlScalarHint::Float => match stripped.parse::<f64>() {
            Ok(f) if f.is_finite() => FrontmatterValue::Float(f),
            _ => FrontmatterValue::String(arena_alloc_str(arena, stripped)),
        },
        YamlScalarHint::Str => FrontmatterValue::String(arena_alloc_str(arena, stripped)),
    }
}

/// Simple YAML parser for frontmatter
fn parse_simple_yaml<'a>(content: &str, arena: &'a bumpalo::Bump) -> Frontmatter<'a> {
    let mut data = new_arena_hashmap(arena);

    for line in content.lines() {
        // Use splitn(2, ':') so values containing colons (e.g. URLs) are preserved.
        let mut parts = line.splitn(2, ':');
        if let (Some(raw_key), Some(raw_value)) = (parts.next(), parts.next()) {
            let key_str = raw_key.trim();
            if key_str.is_empty() {
                continue;
            }
            let key = arena_alloc_str(arena, key_str);
            let value_str = raw_value.trim();

            let value = if value_str.starts_with('[') && value_str.ends_with(']') {
                let inner = &value_str[1..value_str.len() - 1];
                let mut items = bumpalo::collections::Vec::new_in(arena);
                for item in inner.split(',') {
                    let trimmed = item.trim();
                    if !trimmed.is_empty() {
                        items.push(scalar_to_value(trimmed, arena));
                    }
                }
                FrontmatterValue::List(items.into_bump_slice())
            } else {
                scalar_to_value(value_str, arena)
            };

            data.insert(key, value);
        }
    }

    Frontmatter::new(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn property_scan_stops_at_non_property_line() {
        let arena = Bump::new();
        let source =
            "title:: My Page\ntags:: rust, code\nThis is body text.\nlater:: not-a-property\n";
        let props = extract_page_properties(&[], source, &arena).unwrap();
        let keys: Vec<_> = props.iter().map(|(k, _)| k).collect();
        assert_eq!(keys.len(), 2);
        assert!(props.get("title").is_some());
        assert!(props.get("tags").is_some());
        assert!(props.get("later").is_none());
    }

    #[test]
    fn property_scan_stops_at_blank_line() {
        let arena = Bump::new();
        let source = "title:: My Page\n\nbody:: not-a-property\n";
        let props = extract_page_properties(&[], source, &arena).unwrap();
        assert!(props.get("title").is_some());
        assert!(props.get("body").is_none());
    }

    #[test]
    fn property_scan_stops_at_heading() {
        let arena = Bump::new();
        let source = "title:: My Page\n# Heading\nother:: value\n";
        let props = extract_page_properties(&[], source, &arena).unwrap();
        assert!(props.get("title").is_some());
        assert!(props.get("other").is_none());
    }

    #[test]
    fn no_properties_returns_none() {
        let arena = Bump::new();
        let source = "Just normal text\nNo properties here\n";
        assert!(extract_page_properties(&[], source, &arena).is_none());
    }

    // ---- Typed frontmatter value parsing tests ----

    #[test]
    fn frontmatter_typed_integer() {
        let arena = Bump::new();
        let source = "---\npriority: 42\nnegative: -3\nzero: 0\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert_eq!(fm.get_integer("priority"), Some(42));
        assert_eq!(fm.get_integer("negative"), Some(-3));
        assert_eq!(fm.get_integer("zero"), Some(0));
        // Integer keys should NOT be returned by get_string
        assert!(fm.get_string("priority").is_none());
    }

    #[test]
    fn frontmatter_typed_float() {
        let arena = Bump::new();
        let source = "---\nweight: 2.75\nneg: -0.5\nsci: 1e10\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert_eq!(fm.get_float("weight"), Some(2.75));
        assert_eq!(fm.get_float("neg"), Some(-0.5));
        assert!(fm.get_float("sci").is_some());
    }

    #[test]
    fn frontmatter_typed_boolean() {
        let arena = Bump::new();
        let source = "---\ndraft: true\npublished: false\nyes_val: yes\nno_val: no\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert_eq!(fm.get_boolean("draft"), Some(true));
        assert_eq!(fm.get_boolean("published"), Some(false));
        assert_eq!(fm.get_boolean("yes_val"), Some(true));
        assert_eq!(fm.get_boolean("no_val"), Some(false));
    }

    #[test]
    fn frontmatter_typed_null() {
        let arena = Bump::new();
        let source = "---\nempty:\nnull_val: null\ntilde: ~\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        // Empty value after colon
        assert!(fm
            .iter()
            .find(|(k, _)| *k == "empty")
            .map(|(_, v)| v.is_null())
            .unwrap_or(false));
        assert!(fm
            .iter()
            .find(|(k, _)| *k == "null_val")
            .map(|(_, v)| v.is_null())
            .unwrap_or(false));
        assert!(fm
            .iter()
            .find(|(k, _)| *k == "tilde")
            .map(|(_, v)| v.is_null())
            .unwrap_or(false));
    }

    #[test]
    fn frontmatter_quoted_string_not_coerced() {
        let arena = Bump::new();
        let source = "---\nnum: \"42\"\nbool: 'true'\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        // Quoted values should remain strings
        assert_eq!(fm.get_string("num"), Some("42"));
        assert_eq!(fm.get_string("bool"), Some("true"));
        // Should NOT be detected as typed
        assert!(fm.get_integer("num").is_none());
        assert!(fm.get_boolean("bool").is_none());
    }

    #[test]
    fn frontmatter_typed_inline_list() {
        let arena = Bump::new();
        let source = "---\nmixed: [42, true, hello]\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        let mixed = fm.iter().find(|(k, _)| *k == "mixed").map(|(_, v)| v);
        assert!(mixed.is_some());
        if let Some(FrontmatterValue::List(items)) = mixed {
            assert_eq!(items.len(), 3);
            assert!(matches!(items[0], FrontmatterValue::Integer(42)));
            assert!(matches!(items[1], FrontmatterValue::Boolean(true)));
            assert!(matches!(items[2], FrontmatterValue::String("hello")));
        } else {
            panic!("expected List");
        }
    }

    #[test]
    fn frontmatter_nan_inf_stay_string() {
        let arena = Bump::new();
        let source = "---\nnan_val: NaN\ninf_val: inf\n---\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert_eq!(fm.get_string("nan_val"), Some("NaN"));
        assert_eq!(fm.get_string("inf_val"), Some("inf"));
        assert!(fm.get_float("nan_val").is_none());
        assert!(fm.get_float("inf_val").is_none());
    }

    #[test]
    fn frontmatter_with_lf() {
        let arena = Bump::new();
        let source = "---\ntitle: Hello\n---\nBody\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert!(fm.get_string("title").is_some());
    }

    #[test]
    fn frontmatter_with_crlf() {
        let arena = Bump::new();
        let source = "---\r\ntitle: Hello\r\n---\r\nBody\r\n";
        let fm = extract_frontmatter(&[], source, &arena).unwrap();
        assert!(fm.get_string("title").is_some());
    }

    #[test]
    fn frontmatter_no_delimiter_returns_none() {
        let arena = Bump::new();
        let source = "No frontmatter here\n";
        assert!(extract_frontmatter(&[], source, &arena).is_none());
    }

    #[test]
    fn frontmatter_unclosed_returns_none() {
        let arena = Bump::new();
        let source = "---\ntitle: Hello\nNo closing delimiter\n";
        assert!(extract_frontmatter(&[], source, &arena).is_none());
    }

    // ---- YamlScalarHint / strip_yaml_quotes tests ----

    #[test]
    fn detect_null_variants() {
        assert_eq!(detect_yaml_scalar(""), YamlScalarHint::Null);
        assert_eq!(detect_yaml_scalar("null"), YamlScalarHint::Null);
        assert_eq!(detect_yaml_scalar("Null"), YamlScalarHint::Null);
        assert_eq!(detect_yaml_scalar("NULL"), YamlScalarHint::Null);
        assert_eq!(detect_yaml_scalar("~"), YamlScalarHint::Null);
    }

    #[test]
    fn detect_boolean_true_variants() {
        for v in &[
            "true", "True", "TRUE", "yes", "Yes", "YES", "on", "On", "ON",
        ] {
            assert_eq!(
                detect_yaml_scalar(v),
                YamlScalarHint::Boolean(true),
                "failed for {v}"
            );
        }
    }

    #[test]
    fn detect_boolean_false_variants() {
        for v in &[
            "false", "False", "FALSE", "no", "No", "NO", "off", "Off", "OFF",
        ] {
            assert_eq!(
                detect_yaml_scalar(v),
                YamlScalarHint::Boolean(false),
                "failed for {v}"
            );
        }
    }

    #[test]
    fn detect_integers() {
        assert_eq!(detect_yaml_scalar("0"), YamlScalarHint::Integer);
        assert_eq!(detect_yaml_scalar("42"), YamlScalarHint::Integer);
        assert_eq!(detect_yaml_scalar("-3"), YamlScalarHint::Integer);
        assert_eq!(
            detect_yaml_scalar("9223372036854775807"),
            YamlScalarHint::Integer
        ); // i64::MAX
    }

    #[test]
    fn detect_integer_overflow_falls_to_float_or_string() {
        // Larger than i64::MAX but finite f64 → Float
        assert_eq!(
            detect_yaml_scalar("99999999999999999999"),
            YamlScalarHint::Float
        );
        // Truly unparseable number → Str
        assert_eq!(detect_yaml_scalar("12.34.56"), YamlScalarHint::Str);
    }

    #[test]
    fn detect_floats() {
        assert_eq!(detect_yaml_scalar("3.14"), YamlScalarHint::Float);
        assert_eq!(detect_yaml_scalar("-0.5"), YamlScalarHint::Float);
        assert_eq!(detect_yaml_scalar("1e10"), YamlScalarHint::Float);
        assert_eq!(detect_yaml_scalar("1.0"), YamlScalarHint::Float);
    }

    #[test]
    fn detect_nan_inf_rejected_as_string() {
        assert_eq!(detect_yaml_scalar("NaN"), YamlScalarHint::Str);
        assert_eq!(detect_yaml_scalar("inf"), YamlScalarHint::Str);
        assert_eq!(detect_yaml_scalar("-inf"), YamlScalarHint::Str);
        assert_eq!(detect_yaml_scalar("Inf"), YamlScalarHint::Str);
    }

    #[test]
    fn detect_plain_strings() {
        assert_eq!(detect_yaml_scalar("hello"), YamlScalarHint::Str);
        assert_eq!(detect_yaml_scalar("hello world"), YamlScalarHint::Str);
        assert_eq!(
            detect_yaml_scalar("https://example.com"),
            YamlScalarHint::Str
        );
    }

    #[test]
    fn strip_double_quotes() {
        let (v, q) = strip_yaml_quotes("\"42\"");
        assert_eq!(v, "42");
        assert!(q);
    }

    #[test]
    fn strip_single_quotes() {
        let (v, q) = strip_yaml_quotes("'true'");
        assert_eq!(v, "true");
        assert!(q);
    }

    #[test]
    fn no_quotes_unchanged() {
        let (v, q) = strip_yaml_quotes("hello");
        assert_eq!(v, "hello");
        assert!(!q);
    }

    #[test]
    fn mismatched_quotes_unchanged() {
        let (v, q) = strip_yaml_quotes("\"hello'");
        assert_eq!(v, "\"hello'");
        assert!(!q);
    }

    #[test]
    fn empty_string_no_strip() {
        let (v, q) = strip_yaml_quotes("");
        assert_eq!(v, "");
        assert!(!q);
    }

    #[test]
    fn single_char_no_strip() {
        let (v, q) = strip_yaml_quotes("\"");
        assert_eq!(v, "\"");
        assert!(!q);
    }

    #[test]
    fn extract_frontmatter_mixed_endings_picks_earliest_close() {
        // LF close comes first, but CRLF "---" appears later in body.
        // Bug: find(CRLF).or_else(find(LF)) picks CRLF at 19 instead of LF at 8,
        //      treating "bogus: B" as a frontmatter entry.
        let source = "---\ntitle: A\n---\nbogus: B\r\n---\r\nMore\n";
        let arena = Bump::new();
        let fm = extract_frontmatter(&[], source, &arena);
        assert!(fm.is_some(), "should parse frontmatter");
        let fm = fm.unwrap();
        assert!(fm.get_string("title").is_some(), "should find 'title'");
        assert!(
            fm.get_string("bogus").is_none(),
            "body content must not leak into frontmatter"
        );
    }
}
