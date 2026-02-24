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
                        items.push(arena_alloc_str(arena, trimmed));
                    }
                }
                FrontmatterValue::List(items.into_bump_slice())
            } else {
                // String value
                FrontmatterValue::String(arena_alloc_str(arena, value_str))
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
        let source = "title:: My Page\ntags:: rust, code\nThis is body text.\nlater:: not-a-property\n";
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
