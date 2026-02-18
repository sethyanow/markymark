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
                        if wl.heading == Some(old_slug) {
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
                                let close_name_start = Position::new(
                                    xml.range.end.line,
                                    xml.range.end.character - 1 - xml.tag_name.len() as u32,
                                );
                                let close_name_end =
                                    Position::new(xml.range.end.line, xml.range.end.character - 1);
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: Range::new(close_name_start, close_name_end),
                                    new_text: new_name.to_string(),
                                });
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
