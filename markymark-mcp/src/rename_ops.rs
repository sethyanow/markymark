//! Rename operation helpers for heading and XML tag renames.
//!
//! Extracted from `runtime_engine.rs` to keep file sizes manageable.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;

use markymark_core::engine::CoreOperationResult;
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_index::{slugify, HeadingEntry, MarkdownLinkEntry, RealmIndex, WikiLinkEntry};

/// Read the source text for a document from disk.
fn read_document_text(uri: &DocumentUri) -> Option<String> {
    let path = uri.to_file_path()?;
    fs::read_to_string(path).ok()
}

/// Rename a heading and all references to it across a realm.
pub(crate) fn rename_heading(
    realm: &RealmIndex,
    uri: &DocumentUri,
    heading: HeadingEntry,
    new_name: &str,
) -> CoreOperationResult {
    let old_slug = heading.slug;
    let new_slug = slugify(new_name);
    let mut doc_edits: HashMap<DocumentUri, Vec<(Range, String)>> = HashMap::new();

    // 1. Edit the heading text itself.
    //    Skip the "## " prefix to find the text-only range.
    if let Some(text) = read_document_text(uri) {
        if let Some(heading_line) = text.lines().nth(heading.range.start.line as usize) {
            let prefix_len =
                heading_line.len() - heading_line.trim_start_matches('#').trim_start().len();
            let text_start = Position::new(heading.range.start.line, prefix_len as u32);
            let text_end = Position::new(
                heading.range.start.line,
                prefix_len as u32 + heading.text.len() as u32,
            );
            doc_edits
                .entry(uri.clone())
                .or_default()
                .push((Range::new(text_start, text_end), new_name.to_string()));
        }
    }

    // 2. Update wiki links and markdown link anchors across all documents.
    for (doc_uri, doc_index) in realm.iter_documents() {
        let doc_text = read_document_text(doc_uri);

        for wl in doc_index.wiki_links() {
            if wl.heading.as_deref() == Some(&old_slug) {
                if let Some(anchor_range) =
                    find_wiki_link_heading_range(doc_text.as_deref(), wl, &old_slug)
                {
                    doc_edits
                        .entry(doc_uri.clone())
                        .or_default()
                        .push((anchor_range, new_name.to_string()));
                }
            }
        }

        for ml in doc_index.markdown_links() {
            if ml.anchor.as_deref() == Some(&old_slug) {
                if let Some(anchor_range) =
                    find_markdown_link_anchor_range(doc_text.as_deref(), ml, &old_slug)
                {
                    doc_edits
                        .entry(doc_uri.clone())
                        .or_default()
                        .push((anchor_range, new_slug.clone()));
                }
            }
        }
    }

    build_workspace_edit(doc_edits)
}

/// Rename an XML tag across all documents in a realm.
pub(crate) fn rename_xml_tag(
    realm: &RealmIndex,
    old_name: &str,
    new_name: &str,
) -> CoreOperationResult {
    let mut doc_edits: HashMap<DocumentUri, Vec<(Range, String)>> = HashMap::new();

    for (doc_uri, doc_index) in realm.iter_documents() {
        for xml in doc_index.xml_tags() {
            if xml.tag_name == old_name {
                // Opening tag name: starts after '<'
                let name_start = Position::new(xml.range.start.line, xml.range.start.character + 1);
                let name_end = Position::new(
                    xml.range.start.line,
                    xml.range.start.character + 1 + xml.tag_name.len() as u32,
                );
                doc_edits
                    .entry(doc_uri.clone())
                    .or_default()
                    .push((Range::new(name_start, name_end), new_name.to_string()));

                // Closing tag name (skip for self-closing and unclosed)
                if !xml.is_self_closing && !xml.is_unclosed {
                    let close_name_start = Position::new(
                        xml.range.end.line,
                        xml.range.end.character - 1 - xml.tag_name.len() as u32,
                    );
                    let close_name_end =
                        Position::new(xml.range.end.line, xml.range.end.character - 1);
                    doc_edits.entry(doc_uri.clone()).or_default().push((
                        Range::new(close_name_start, close_name_end),
                        new_name.to_string(),
                    ));
                }
            }
        }
    }

    if doc_edits.is_empty() {
        return CoreOperationResult::Error(CoreError::Message(
            "no renameable symbol at position".to_string(),
        ));
    }

    build_workspace_edit(doc_edits)
}

/// Sort and build a deterministic WorkspaceEdit from collected document edits.
pub(crate) fn build_workspace_edit(
    doc_edits: HashMap<DocumentUri, Vec<(Range, String)>>,
) -> CoreOperationResult {
    let mut result: Vec<(DocumentUri, Vec<(Range, String)>)> = doc_edits
        .into_iter()
        .map(|(uri, mut edits)| {
            edits.sort_by(|(a, _), (b, _)| compare_ranges(*a, *b));
            (uri, edits)
        })
        .collect();
    result.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    CoreOperationResult::WorkspaceEdit(result)
}

/// Compare two ranges for deterministic sorting.
pub(crate) fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
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

    let hash_offset = link_text.find('#')?;
    let heading_start = link_start + hash_offset + 1; // skip the '#'
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
