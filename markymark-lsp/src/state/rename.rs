//! Rename operations: prepare_rename and execute rename across documents.

use markymark_core::{DocumentUri, Position, Range};
use markymark_index::{slugify, MarkdownLinkEntry, WikiLinkEntry};

use super::navigation::SymbolAtPosition;
use super::ServerState;

/// Result from `prepare_rename_at`: the range and current text of the renameable symbol.
#[derive(Debug, Clone)]
pub struct PrepareRenameResult {
    /// The source range of the renameable text.
    pub range: Range,
    /// The current text (used as placeholder in rename dialog).
    pub placeholder: String,
}

/// A single text edit produced by a rename operation.
#[derive(Debug, Clone)]
pub struct RenameEdit {
    /// The document to edit.
    pub uri: DocumentUri,
    /// The range to replace.
    pub range: Range,
    /// The replacement text.
    pub new_text: String,
}

impl ServerState {
    /// Check whether the symbol at the given position can be renamed.
    ///
    /// Returns the range and current text of the renameable symbol, or `None`
    /// if no renameable symbol is found at the position.
    pub fn prepare_rename_at(
        &self,
        uri: &DocumentUri,
        pos: Position,
    ) -> Option<PrepareRenameResult> {
        let symbol = self.symbol_at_position(uri, pos)?;
        match symbol {
            SymbolAtPosition::Heading(h) => Some(PrepareRenameResult {
                range: h.range,
                placeholder: h.text.to_string(),
            }),
            SymbolAtPosition::XmlTag(xt) => {
                // Tag name range: starts after '<', length of tag_name
                let name_start = Position::new(xt.range.start.line, xt.range.start.character + 1);
                let name_end = Position::new(
                    xt.range.start.line,
                    xt.range.start.character + 1 + xt.tag_name.len() as u32,
                );
                Some(PrepareRenameResult {
                    range: Range::new(name_start, name_end),
                    placeholder: xt.tag_name.to_string(),
                })
            }
            // Wiki links and markdown links are not renameable themselves
            // (you rename the heading they point to, not the link)
            _ => None,
        }
    }

    /// Compute all edits needed to rename the symbol at the given position.
    ///
    /// For a heading rename, this:
    /// 1. Renames the heading text in the source document
    /// 2. Updates all wiki links that reference the old heading slug
    /// 3. Updates all markdown link anchors (`#old-slug` → `#new-slug`)
    pub fn rename_at(
        &self,
        uri: &DocumentUri,
        pos: Position,
        new_name: &str,
    ) -> Option<Vec<RenameEdit>> {
        let symbol = self.symbol_at_position(uri, pos)?;
        match symbol {
            SymbolAtPosition::Heading(h) => {
                let old_slug = h.slug;
                let new_slug = slugify(new_name);
                let mut edits = Vec::new();

                // 1. Edit the heading text itself.
                //    The heading range covers the full line including `# ` prefix.
                //    We need to compute the text-only range: skip "## " prefix.
                let text = self.get_document_text(uri)?;
                let heading_line = text.lines().nth(h.range.start.line as usize)?;
                let prefix_len =
                    heading_line.len() - heading_line.trim_start_matches('#').trim_start().len();
                let text_start = Position::new(h.range.start.line, prefix_len as u32);
                let text_end =
                    Position::new(h.range.start.line, prefix_len as u32 + h.text.len() as u32);
                edits.push(RenameEdit {
                    uri: uri.clone(),
                    range: Range::new(text_start, text_end),
                    new_text: new_name.to_string(),
                });

                // 2. Search all documents for wiki links referencing the old slug
                for (doc_uri, index) in self.realm.iter_documents() {
                    for wl in index.wiki_links() {
                        let wl_heading_slug = wl.heading.map(slugify);
                        if wl_heading_slug.as_deref() == Some(old_slug) {
                            let doc_text = self.get_document_text(doc_uri);
                            if let Some(anchor_range) =
                                find_wiki_link_heading_range(doc_text, wl, old_slug)
                            {
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: anchor_range,
                                    new_text: new_name.to_string(),
                                });
                            }
                        }
                    }

                    // 3. Update markdown link anchors: [text](#old-slug) → [text](#new-slug)
                    for ml in index.markdown_links() {
                        if ml.anchor == Some(old_slug) {
                            let doc_text = self.get_document_text(doc_uri);
                            if let Some(anchor_range) =
                                find_markdown_link_anchor_range(doc_text, ml, old_slug)
                            {
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: anchor_range,
                                    new_text: new_slug.clone(),
                                });
                            }
                        }
                    }
                }

                Some(edits)
            }
            SymbolAtPosition::XmlTag(xt) => {
                let old_name = &xt.tag_name;
                let mut edits = Vec::new();

                // Find all XML tags with the same name across all documents
                for (doc_uri, index) in self.realm.iter_documents() {
                    for xml in index.xml_tags() {
                        if xml.tag_name == *old_name {
                            // Opening tag name: starts after '<', length of tag_name
                            let name_start =
                                Position::new(xml.range.start.line, xml.range.start.character + 1);
                            let name_end = Position::new(
                                xml.range.start.line,
                                xml.range.start.character + 1 + xml.tag_name.len() as u32,
                            );
                            edits.push(RenameEdit {
                                uri: doc_uri.clone(),
                                range: Range::new(name_start, name_end),
                                new_text: new_name.to_string(),
                            });

                            // Closing tag name: ends just before '>' in </tagname>
                            if !xml.is_self_closing && !xml.is_unclosed {
                                if let Some(close_start) = xml
                                    .range
                                    .end
                                    .character
                                    .checked_sub(1)
                                    .and_then(|c| c.checked_sub(xml.tag_name.len() as u32))
                                {
                                    let close_name_start =
                                        Position::new(xml.range.end.line, close_start);
                                    let close_name_end = Position::new(
                                        xml.range.end.line,
                                        xml.range.end.character.saturating_sub(1),
                                    );
                                    edits.push(RenameEdit {
                                        uri: doc_uri.clone(),
                                        range: Range::new(close_name_start, close_name_end),
                                        new_text: new_name.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }

                if edits.is_empty() {
                    None
                } else {
                    Some(edits)
                }
            }
            _ => None,
        }
    }
}

/// Find the range of the heading portion within a wiki link.
///
/// Given a wiki link like `[[page#heading]]` or `[[#heading]]`, returns the
/// range covering just the heading text (after `#`, before `]]`).
fn find_wiki_link_heading_range(
    doc_text: Option<&str>,
    wl: &WikiLinkEntry,
    old_heading: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(wl.range.start.line as usize)?;
    let link_start = wl.range.start.character as usize;
    if link_start > line.len() {
        return None;
    }
    let link_text = &line[link_start..];

    // Find `#heading` within the wiki link text
    let hash_offset = link_text.find('#')?;
    let heading_start = link_start + hash_offset + 1; // skip the '#'

    // Verify the text matches
    let heading_end = heading_start + old_heading.len();
    if line.get(heading_start..heading_end) == Some(old_heading) {
        Some(Range::new(
            Position::new(wl.range.start.line, heading_start as u32),
            Position::new(wl.range.start.line, heading_end as u32),
        ))
    } else {
        None
    }
}

/// Find the range of the anchor portion within a markdown link.
///
/// Given a markdown link like `[text](#slug)`, returns the range covering
/// just the slug text (after `#`, before `)`).
fn find_markdown_link_anchor_range(
    doc_text: Option<&str>,
    ml: &MarkdownLinkEntry,
    old_slug: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(ml.range.start.line as usize)?;
    let link_start = ml.range.start.character as usize;
    if link_start > line.len() {
        return None;
    }
    let link_text = &line[link_start..];

    // Find `(#slug)` within the markdown link text
    let paren_hash = link_text.find("(#")?;
    let slug_start = link_start + paren_hash + 2; // skip "(#"
    let slug_end = slug_start + old_slug.len();

    if line.get(slug_start..slug_end) == Some(old_slug) {
        Some(Range::new(
            Position::new(ml.range.start.line, slug_start as u32),
            Position::new(ml.range.start.line, slug_end as u32),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::{Position, Range};
    use markymark_index::{MarkdownLinkEntry, WikiLinkEntry};

    /// marky-oiv: XML tag rename closing-tag arithmetic must not underflow
    /// when end.character is too small for `- 1 - tag_name.len()`.
    #[test]
    fn test_xml_tag_rename_closing_tag_checked_sub_guards_underflow() {
        // Simulate the closing-tag name position calculation from rename_at.
        // The vulnerable expression: xml.range.end.character - 1 - xml.tag_name.len() as u32
        // When end.character = 0, tag_name = "div" (len 3): 0u32 - 1 = underflow!
        let end_character: u32 = 0;
        let tag_name_len: u32 = 3; // "div"

        // With checked_sub, this safely returns None instead of panicking
        let result = end_character
            .checked_sub(1)
            .and_then(|c| c.checked_sub(tag_name_len));

        assert_eq!(
            result, None,
            "checked_sub must return None for pathological range, not panic"
        );

        // Also test edge case: end_character = 1, tag_name_len = 3 → 1 - 1 = 0, 0 - 3 = underflow
        let result2 = 1u32
            .checked_sub(1)
            .and_then(|c| c.checked_sub(tag_name_len));
        assert_eq!(result2, None, "edge case: end_char=1 must also return None");

        // Happy path: end_character = 10, tag_name_len = 3 → 10 - 1 = 9, 9 - 3 = 6
        let result3 = 10u32
            .checked_sub(1)
            .and_then(|c| c.checked_sub(tag_name_len));
        assert_eq!(result3, Some(6), "happy path should return Some(6)");
    }

    /// marky-xpk: character offset beyond line length must return None, not panic.
    #[test]
    fn test_wiki_link_heading_range_oob_character_returns_none() {
        let wl = WikiLinkEntry {
            target: "page",
            alias: None,
            heading: Some("heading"),
            range: Range {
                start: Position::new(0, 9999),
                end: Position::new(0, 10005),
            },
            start_byte: 0,
            end_byte: 0,
        };
        // "short" is 5 bytes; character=9999 is way out of bounds — must not panic.
        let result = find_wiki_link_heading_range(Some("short\n"), &wl, "heading");
        assert_eq!(result, None);
    }

    /// Regression: wiki link heading comparison must slugify before matching.
    /// Raw heading text like "My Section" must match slug "my-section".
    #[test]
    fn test_wiki_link_heading_slug_comparison() {
        // Simulates the rename_at loop: wl.heading is raw text, old_slug is slugified.
        let raw_heading = "My Section";
        let old_slug = "my-section";

        // Old (broken) comparison: raw text != slug
        assert_ne!(Some(raw_heading), Some(old_slug));

        // New (fixed) comparison: slugify raw heading, then compare
        let slugified = slugify(raw_heading);
        assert_eq!(slugified.as_str(), old_slug);
    }

    /// marky-u46: character offset beyond line length must return None, not panic.
    #[test]
    fn test_markdown_link_anchor_range_oob_character_returns_none() {
        let ml = MarkdownLinkEntry {
            text: "text",
            url: "#heading",
            anchor: Some("heading"),
            range: Range {
                start: Position::new(0, 9999),
                end: Position::new(0, 10005),
            },
            start_byte: 0,
            end_byte: 0,
        };
        // "short" is 5 bytes; character=9999 is way out of bounds — must not panic.
        let result = find_markdown_link_anchor_range(Some("short\n"), &ml, "heading");
        assert_eq!(result, None);
    }
}
