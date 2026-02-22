use crate::types::arena_alloc_str;
use crate::types::*;
use markymark_core::arena::new_arena_hashmap;

/// Extract frontmatter
pub fn extract_frontmatter<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Option<Frontmatter<'a>> {
    // Check if document starts with ---
    if !source.starts_with("---\n") {
        return None;
    }

    // Find the closing ---
    let rest = &source[4..];
    if let Some(end_pos) = rest.find("\n---\n") {
        let yaml_content = &rest[..end_pos];
        // Simple YAML parsing - just extract key: value pairs
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
