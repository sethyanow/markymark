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

            // Calculate range: from '[' (1 before text_match) to ')' (1 after url_match)
            let full_match = captures.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );

            links.push(MarkdownLink::new(text, url, anchor, None, range));
        }
    }

    // Extract reference links
    for captures in ref_re.captures_iter(source) {
        if let (Some(text_match), Some(ref_match)) = (captures.get(1), captures.get(2)) {
            let text = text_match.as_str().to_string();
            let reference = ref_match.as_str().to_string();

            let full_match = captures.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();
            let line = source[..start].matches('\n').count() as u32;
            let line_start = source[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let start_char = (start - line_start) as u32;
            let end_char = start_char + (end - start) as u32;
            let range = Range::new(
                Position::new(line, start_char),
                Position::new(line, end_char),
            );

            links.push(MarkdownLink::new(
                text,
                String::new(),
                None,
                Some(reference),
                range,
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
    let re = Regex::new(r"(?m)\^([a-zA-Z0-9_-]+)\s*$").unwrap();

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

/// Extract XML/HTML tags from the document source.
///
/// Uses a single-pass stack-based tokenizer for O(n) performance.
/// Handles self-closing tags, void HTML elements, nested same-name tags,
/// and attribute values containing `>`.
pub fn extract_xml_tags(_elements: &[Element], source: &str) -> Vec<XmlTag> {
    let mut tags = Vec::new();

    // HTML void elements that are self-closing even without />
    const VOID_ELEMENTS: &[&str] = &[
        "br", "hr", "img", "input", "meta", "link", "source", "track", "wbr", "area", "base",
        "col", "embed", "param",
    ];

    // Regex for attributes: key="value"
    let attr_re = Regex::new(r#"([a-zA-Z_:][a-zA-Z0-9_.:-]*)\s*=\s*"([^"]*)""#).unwrap();

    let parse_attrs = |attr_str: &str| -> HashMap<String, String> {
        let mut attrs = HashMap::new();
        for cap in attr_re.captures_iter(attr_str) {
            if let (Some(key), Some(val)) = (cap.get(1), cap.get(2)) {
                attrs.insert(key.as_str().to_string(), val.as_str().to_string());
            }
        }
        attrs
    };

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

    /// Frame on the tag-matching stack for open tags awaiting their close.
    struct StackFrame {
        tag_name: String,
        attrs: HashMap<String, String>,
        tag_start: usize,
        content_start: usize,
    }

    // Find the end of a tag starting at `<`, respecting quoted attribute values
    // that may contain `>`. Returns the byte index *after* the closing `>`.
    let find_tag_end = |from: usize| -> Option<usize> {
        let bytes = source.as_bytes();
        let mut i = from;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    // Skip to closing quote
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += 1;
                    }
                }
                b'\'' => {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'\'' {
                        i += 1;
                    }
                }
                b'>' => return Some(i + 1),
                _ => {}
            }
            i += 1;
        }
        None
    };

    let mut stack: Vec<StackFrame> = Vec::new();
    let bytes = source.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes[pos] != b'<' {
            pos += 1;
            continue;
        }

        // We found a '<'. Determine what kind of tag it is.
        let tag_start = pos;

        // Find the end of this tag (quote-aware)
        let tag_end = match find_tag_end(tag_start) {
            Some(end) => end,
            None => break, // Malformed, no closing >
        };

        let tag_str = &source[tag_start..tag_end];

        // Is this a closing tag?  </name>
        if tag_str.starts_with("</") {
            let name_start = 2;
            let name_end = tag_str[name_start..]
                .find(|c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != '_')
                .map(|p| name_start + p)
                .unwrap_or(tag_str.len() - 1);
            let tag_name = &tag_str[name_start..name_end];

            if !tag_name.is_empty() {
                // Walk the stack backwards to find the matching open tag
                let mut idx = stack.len();
                while idx > 0 {
                    idx -= 1;
                    if stack[idx].tag_name == tag_name {
                        let frame = stack.remove(idx);
                        let content_str = &source[frame.content_start..tag_start];
                        let content = if content_str.is_empty() {
                            None
                        } else {
                            Some(content_str.to_string())
                        };
                        let range = compute_range(frame.tag_start, tag_end);
                        tags.push(XmlTag::new(
                            frame.tag_name,
                            frame.attrs,
                            false,
                            content,
                            range,
                        ));
                        break;
                    }
                }
            }

            pos = tag_end;
            continue;
        }

        // Not a closing tag — extract the tag name
        let name_start = 1; // skip '<'
        let name_end = tag_str[name_start..]
            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != '_')
            .map(|p| name_start + p)
            .unwrap_or(tag_str.len() - 1);
        let tag_name = &tag_str[name_start..name_end];

        if tag_name.is_empty() || !tag_name.as_bytes()[0].is_ascii_alphabetic() {
            pos = tag_end;
            continue;
        }

        // Extract attribute region (between tag name and closing > or />)
        let attr_region = &tag_str[name_end..tag_str.len() - 1]; // strip trailing >
        let is_self_closing = tag_str.ends_with("/>") || attr_region.trim_end().ends_with('/');
        let is_void = VOID_ELEMENTS
            .iter()
            .any(|v| v.eq_ignore_ascii_case(tag_name));

        let attrs = parse_attrs(attr_region);

        if is_self_closing || is_void {
            let range = compute_range(tag_start, tag_end);
            tags.push(XmlTag::new(tag_name.to_string(), attrs, true, None, range));
        } else {
            // Regular opening tag — push onto stack
            stack.push(StackFrame {
                tag_name: tag_name.to_string(),
                attrs,
                tag_start,
                content_start: tag_end,
            });
        }

        pos = tag_end;
    }

    // Emit remaining unclosed tags from the stack
    for frame in stack {
        let range = compute_range(frame.tag_start, frame.content_start);
        tags.push(XmlTag::unclosed(frame.tag_name, frame.attrs, range));
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
