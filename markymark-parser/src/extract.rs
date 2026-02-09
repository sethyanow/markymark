use std::collections::HashMap;

use crate::types::*;
use markymark_core::prelude::*;
use regex::Regex;

/// Extract wiki links from elements
pub fn extract_wiki_links(_elements: &[Element], source: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();

    // Regex for wiki links: [[target]] or [[target|alias]] or [[target#heading]] or [[target#^blockid]]
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();

    for captures in re.captures_iter(source) {
        if let Some(content_match) = captures.get(1) {
            let content = content_match.as_str();
            let start = content_match.start() - 2; // Account for [[
            let end = content_match.end() + 2; // Account for ]]

            // Parse the content to extract components
            let (target, alias, heading, block_id) = parse_wiki_link_content(content);

            // Calculate position in source
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;

            links.push(WikiLink::new(
                target.unwrap_or_default(),
                alias,
                heading,
                block_id,
                Range::new(
                    Position::new(line, start_char),
                    Position::new(line, end_char),
                ),
            ));
        }
    }

    links
}

/// Parse wiki link content into components
fn parse_wiki_link_content(
    content: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    // Check for alias first: [[target|alias]]
    if let Some(pipe_pos) = content.find('|') {
        let target_part = &content[..pipe_pos];
        let alias = Some(content[pipe_pos + 1..].to_string());
        let (target, heading, block_id) = parse_target_part(target_part);
        return (target, alias, heading, block_id);
    }

    // No alias, parse target
    let (target, heading, block_id) = parse_target_part(content);
    (target, None, heading, block_id)
}

/// Parse target part: [[Page#heading]] or [[Page#^blockid]] or [[#heading]]
fn parse_target_part(target: &str) -> (Option<String>, Option<String>, Option<String>) {
    if let Some(hash_pos) = target.find('#') {
        let page_part = &target[..hash_pos];
        let after_hash = &target[hash_pos + 1..];

        // Check if it's a block ID (starts with ^)
        if let Some(stripped) = after_hash.strip_prefix('^') {
            let block_id = Some(stripped.to_string());
            let page = if page_part.is_empty() {
                None
            } else {
                Some(page_part.to_string())
            };
            return (page, None, block_id);
        }

        // It's a heading
        let heading = Some(after_hash.to_string());
        let page = if page_part.is_empty() {
            None
        } else {
            Some(page_part.to_string())
        };
        return (page, heading, None);
    }

    // Just a plain page link
    (Some(target.to_string()), None, None)
}

/// Extract markdown links
pub fn extract_markdown_links(_elements: &[Element], source: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();

    // Regex for inline links: [text](url) or [text](url#anchor)
    let inline_re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    // Regex for reference links: [text][ref]
    let ref_re = Regex::new(r"\[([^\]]+)\]\[([^\]]+)\]").unwrap();

    // Extract inline links
    for captures in inline_re.captures_iter(source) {
        if let (Some(text_match), Some(url_match)) = (captures.get(1), captures.get(2)) {
            let text = text_match.as_str().to_string();
            let url_str = url_match.as_str();

            // Check for anchor
            let (url, anchor) = if let Some(hash_pos) = url_str.find('#') {
                (
                    url_str[..hash_pos].to_string(),
                    Some(url_str[hash_pos + 1..].to_string()),
                )
            } else {
                (url_str.to_string(), None)
            };

            links.push(MarkdownLink::new(text, url, anchor, None));
        }
    }

    // Extract reference links
    for captures in ref_re.captures_iter(source) {
        if let (Some(text_match), Some(ref_match)) = (captures.get(1), captures.get(2)) {
            let text = text_match.as_str().to_string();
            let reference = ref_match.as_str().to_string();
            links.push(MarkdownLink::new(
                text,
                String::new(),
                None,
                Some(reference),
            ));
        }
    }

    links
}

/// Extract link definitions
pub fn extract_link_definitions(_elements: &[Element], source: &str) -> Vec<LinkDefinition> {
    let mut defs = Vec::new();

    // Regex for link definitions: [label]: url "optional title"
    let re = Regex::new(r#"(?m)^\[([^\]]+)\]:\s+(\S+)(?:\s+"([^"]+)")?"#).unwrap();

    for captures in re.captures_iter(source) {
        if let (Some(label_match), Some(url_match)) = (captures.get(1), captures.get(2)) {
            let label = label_match.as_str().to_string();
            let url = url_match.as_str().to_string();
            let title = captures.get(3).map(|m| m.as_str().to_string());
            defs.push(LinkDefinition::new(label, url, title));
        }
    }

    defs
}

/// Extract block IDs
pub fn extract_block_ids(_elements: &[Element], source: &str) -> Vec<BlockId> {
    let mut blocks = Vec::new();

    // Regex for block IDs: ^blockid at end of line
    let re = Regex::new(r"\^([a-zA-Z0-9_-]+)\s*$").unwrap();

    for captures in re.captures_iter(source) {
        if let Some(id_match) = captures.get(1) {
            blocks.push(BlockId::new(id_match.as_str().to_string()));
        }
    }

    blocks
}

/// Extract block refs
pub fn extract_block_refs(_elements: &[Element], source: &str) -> Vec<BlockRef> {
    let mut refs = Vec::new();

    // Regex for Logseq block refs: ((uuid))
    let re = Regex::new(r"\(\(([0-9a-f-]{36})\)\)").unwrap();

    for captures in re.captures_iter(source) {
        if let Some(uuid_match) = captures.get(1) {
            refs.push(BlockRef::new(uuid_match.as_str().to_string()));
        }
    }

    refs
}

/// Extract tags
pub fn extract_tags(_elements: &[Element], source: &str) -> Vec<Tag> {
    let mut tags = Vec::new();

    // Regex for simple tags: #tag or #nested/tag/path
    let simple_re = Regex::new(r"#([a-zA-Z0-9_/-]+)").unwrap();
    // Regex for multi-word tags: #[[multi word tag]]
    let multi_re = Regex::new(r"#\[\[([^\]]+)\]\]").unwrap();

    // Extract multi-word tags first (they're more specific)
    for captures in multi_re.captures_iter(source) {
        if let Some(name_match) = captures.get(1) {
            tags.push(Tag::new(name_match.as_str().to_string()));
        }
    }

    // Extract simple tags
    for captures in simple_re.captures_iter(source) {
        if let Some(name_match) = captures.get(1) {
            let name = name_match.as_str().to_string();
            // Skip if it's the start of a wiki link (already captured as multi-word)
            if !source[name_match.start()..].starts_with("[[") {
                tags.push(Tag::new(name));
            }
        }
    }

    tags
}

/// Extract embeds
pub fn extract_embeds(_elements: &[Element], source: &str) -> Vec<Embed> {
    let mut embeds = Vec::new();

    // Regex for embeds: ![[target]]
    let re = Regex::new(r"!\[\[([^\]]+)\]\]").unwrap();

    for captures in re.captures_iter(source) {
        if let Some(content_match) = captures.get(1) {
            let target = content_match.as_str().to_string();
            embeds.push(Embed::new(target));
        }
    }

    embeds
}

/// Extract list items
pub fn extract_list_items(elements: &[Element], _source: &str) -> Vec<ListItem> {
    elements
        .iter()
        .filter_map(|e| match e {
            Element::ListItem(item) => Some(item.clone()),
            _ => None,
        })
        .collect()
}

/// Extract tasks
pub fn extract_tasks(_elements: &[Element], source: &str) -> Vec<Task> {
    let mut tasks = Vec::new();

    // Regex for task checkboxes: - [ ], - [x], - [/]
    let checkbox_re = Regex::new(r"(?m)^-\s+\[([x /])\]\s+").unwrap();
    // Regex for TODO/DONE markers
    let marker_re = Regex::new(r"(?m)^-\s+(TODO|DONE)\s+").unwrap();

    // Extract checkbox tasks
    for captures in checkbox_re.captures_iter(source) {
        if let Some(state_match) = captures.get(1) {
            let state_str = match state_match.as_str().trim() {
                "" => "unchecked",
                "x" => "checked",
                "/" => "in_progress",
                _ => "unchecked",
            };
            tasks.push(Task::new(TaskState::new(state_str.to_string())));
        }
    }

    // Extract marker tasks
    for captures in marker_re.captures_iter(source) {
        if let Some(marker_match) = captures.get(1) {
            let state_str = marker_match.as_str().to_lowercase();
            tasks.push(Task::new(TaskState::new(state_str)));
        }
    }

    tasks
}

/// Extract callouts
pub fn extract_callouts(_elements: &[Element], source: &str) -> Vec<Callout> {
    let mut callouts = Vec::new();

    // Regex for Obsidian callouts: > [!type] title
    let re = Regex::new(r"(?m)^\u003e\s+\[!([a-z]+)\]\s+(.*)$").unwrap();

    for captures in re.captures_iter(source) {
        if let (Some(type_match), title_match) = (captures.get(1), captures.get(2)) {
            let callout_type = type_match.as_str().to_string();
            let title = title_match.map(|m| m.as_str().trim().to_string());
            callouts.push(Callout::new(callout_type, title));
        }
    }

    callouts
}

/// Extract query blocks
pub fn extract_query_blocks(_elements: &[Element], source: &str) -> Vec<QueryBlock> {
    let mut queries = Vec::new();

    // Regex for Logseq query blocks: {{query ...}}
    let re = Regex::new(r"\{\{query\s+([^}]+)\}\}").unwrap();

    for captures in re.captures_iter(source) {
        if let Some(query_match) = captures.get(1) {
            let query = query_match.as_str().trim().to_string();
            queries.push(QueryBlock::new(query));
        }
    }

    queries
}

/// Extract frontmatter
pub fn extract_frontmatter(_elements: &[Element], source: &str) -> Option<Frontmatter> {
    // Check if document starts with ---
    if !source.starts_with("---\n") {
        return None;
    }

    // Find the closing ---
    let rest = &source[4..];
    if let Some(end_pos) = rest.find("\n---\n") {
        let yaml_content = &rest[..end_pos];
        // Simple YAML parsing - just extract key: value pairs
        return Some(parse_simple_yaml(yaml_content));
    }

    None
}

/// Extract page properties
pub fn extract_page_properties(_elements: &[Element], source: &str) -> Option<Properties> {
    // Logseq properties: key:: value at start of document
    let mut data = std::collections::HashMap::new();
    let mut found_any = false;

    for line in source.lines() {
        // Stop at first non-property line (blank or heading)
        if line.is_empty() || line.starts_with('#') {
            break;
        }

        // Check for property: key:: value
        if let Some(double_colon_pos) = line.find("::") {
            let key = line[..double_colon_pos].trim().to_string();
            let value_str = line[double_colon_pos + 2..].trim();

            let value = if value_str.contains("[[") {
                // Check if it's multiple page refs (list)
                if value_str.matches("[[").count() > 1 {
                    // Multiple page refs - it's a list
                    let items: Vec<String> = value_str
                        .split("[[")
                        .skip(1)
                        .filter_map(|s| s.split("]]").next())
                        .map(|s| s.trim().to_string())
                        .collect();
                    PropertyValue::List(items)
                } else {
                    // Single page reference
                    PropertyValue::PageRef(value_str.to_string())
                }
            } else if value_str.contains(',') {
                // List
                let items: Vec<String> =
                    value_str.split(',').map(|s| s.trim().to_string()).collect();
                PropertyValue::List(items)
            } else {
                // String
                PropertyValue::String(value_str.to_string())
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

/// Extract XML/HTML tags from the document source
pub fn extract_xml_tags(_elements: &[Element], source: &str) -> Vec<XmlTag> {
    let mut tags = Vec::new();

    // HTML void elements that are self-closing even without />
    const VOID_ELEMENTS: &[&str] = &[
        "br", "hr", "img", "input", "meta", "link", "source", "track", "wbr", "area", "base",
        "col", "embed", "param",
    ];

    // Regex for self-closing tags: <tag ... />
    let self_closing_re = Regex::new(r#"<([a-zA-Z][a-zA-Z0-9-]*)(\s[^>]*)?\s*/>"#).unwrap();

    // Regex for opening tags: <tag ...> (but not self-closing or closing)
    let open_re = Regex::new(r#"<([a-zA-Z][a-zA-Z0-9-]*)(\s[^>]*)?\s*>"#).unwrap();

    // Regex for attributes: key="value"
    let attr_re = Regex::new(r#"([a-zA-Z_:][a-zA-Z0-9_.:-]*)\s*=\s*"([^"]*)""#).unwrap();

    // Helper to parse attributes from attribute string
    let parse_attrs = |attr_str: &str| -> HashMap<String, String> {
        let mut attrs = HashMap::new();
        for cap in attr_re.captures_iter(attr_str) {
            if let (Some(key), Some(val)) = (cap.get(1), cap.get(2)) {
                attrs.insert(key.as_str().to_string(), val.as_str().to_string());
            }
        }
        attrs
    };

    // Helper to compute Range from byte offset
    let compute_range = |start: usize, end: usize| -> Range {
        let start_line = source[..start].matches('\n').count() as u32;
        let start_line_offset = source[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let start_char = (start - start_line_offset) as u32;

        let end_line = source[..end].matches('\n').count() as u32;
        let end_line_offset = source[..end].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let end_char = (end - end_line_offset) as u32;

        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    };

    // 1) Find self-closing tags: <tag ... />
    for cap in self_closing_re.captures_iter(source) {
        let full = cap.get(0).unwrap();
        let tag_name = cap.get(1).unwrap().as_str().to_string();
        let attr_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let attrs = parse_attrs(attr_str);
        let range = compute_range(full.start(), full.end());

        tags.push(XmlTag::new(tag_name, attrs, true, None, range));
    }

    // 2) Find opening tags and match with closing tags (or void elements)
    for cap in open_re.captures_iter(source) {
        let full = cap.get(0).unwrap();
        let tag_name = cap.get(1).unwrap().as_str();
        let attr_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let attrs = parse_attrs(attr_str);

        let is_void = VOID_ELEMENTS.contains(&tag_name.to_lowercase().as_str());

        if is_void {
            let range = compute_range(full.start(), full.end());
            tags.push(XmlTag::new(tag_name.to_string(), attrs, true, None, range));
        } else {
            // Find matching closing tag, handling nesting
            let after_open = full.end();
            let closing_tag = format!("</{}>", tag_name);
            let opening_pattern = format!("<{}", tag_name);

            let mut depth = 1;
            let mut search_pos = after_open;

            while depth > 0 {
                // Find next opening or closing of same tag
                let next_close = source[search_pos..].find(&closing_tag);
                let next_open = source[search_pos..].find(&opening_pattern).and_then(|pos| {
                    // Verify it's actually an opening tag (followed by > or space)
                    let after_name = search_pos + pos + opening_pattern.len();
                    if after_name < source.len() {
                        let ch = source.as_bytes()[after_name];
                        if ch == b'>' || ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'/' {
                            Some(pos)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                match (next_close, next_open) {
                    (Some(close_pos), Some(open_pos)) if open_pos < close_pos => {
                        // Another opening tag found before closing
                        depth += 1;
                        search_pos = search_pos + open_pos + opening_pattern.len();
                    }
                    (Some(close_pos), _) => {
                        depth -= 1;
                        if depth == 0 {
                            let content_start = after_open;
                            let content_end = search_pos + close_pos;
                            let tag_end = content_end + closing_tag.len();

                            let content_str = &source[content_start..content_end];
                            let content = if content_str.is_empty() {
                                None
                            } else {
                                Some(content_str.to_string())
                            };

                            let range = compute_range(full.start(), tag_end);
                            tags.push(XmlTag::new(
                                tag_name.to_string(),
                                attrs.clone(),
                                false,
                                content,
                                range,
                            ));
                        } else {
                            search_pos = search_pos + close_pos + closing_tag.len();
                        }
                    }
                    _ => {
                        // No closing tag found — treat as unclosed, skip
                        break;
                    }
                }
            }
        }
    }

    // Sort by position in source for consistent ordering
    tags.sort_by_key(|t| {
        let r = t.range();
        (r.start.line, r.start.character)
    });

    tags
}

/// Simple YAML parser for frontmatter
fn parse_simple_yaml(content: &str) -> Frontmatter {
    use std::collections::HashMap;

    let mut data = HashMap::new();

    for line in content.lines() {
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_string();
            let value_str = line[colon_pos + 1..].trim();

            let value = if value_str.starts_with('[') && value_str.ends_with(']') {
                // List value
                let inner = &value_str[1..value_str.len() - 1];
                let items: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
                FrontmatterValue::List(items)
            } else {
                // String value
                FrontmatterValue::String(value_str.to_string())
            };

            data.insert(key, value);
        }
    }

    Frontmatter::new(data)
}
