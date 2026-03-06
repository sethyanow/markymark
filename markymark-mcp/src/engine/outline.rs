//! GetOutline operation handler.

use markymark_core::engine::{CoreOperationResult, OutlineTreeNode};
use markymark_core::sidecar::{self as sidecar_types, DocumentSidecar, DEFAULT_SIDECAR_DIR};
use markymark_core::{CoreError, DocumentUri, Range};
use markymark_index::{HeadingEntry, OutlineNode, RealmIndex};

use super::enrich;

pub(crate) fn handle_get_outline(
    realm: &RealmIndex,
    roots: &[std::path::PathBuf],
    uri: &DocumentUri,
    format: &str,
    include_text: bool,
) -> CoreOperationResult {
    match realm.get_any_document(uri) {
        Some(markymark_index::AnyDocumentIndex::Markdown(index)) => {
            if format == "tree" {
                let source = if include_text {
                    uri.to_file_path()
                        .and_then(|p| std::fs::read_to_string(p).ok())
                } else {
                    None
                };
                let mut tree =
                    outline_node_to_owned(index.outline(), source.as_deref(), index.headings());

                // Try to inject sidecar summaries if available.
                if let Some(sidecar) = try_load_sidecar(uri, roots) {
                    enrich::inject_summaries(&mut tree, &sidecar);
                }

                CoreOperationResult::OutlineTree(tree)
            } else {
                CoreOperationResult::Outline(
                    index
                        .headings()
                        .iter()
                        .map(|heading| heading.text.to_string())
                        .collect(),
                )
            }
        }
        Some(markymark_index::AnyDocumentIndex::Structured(index)) => {
            // Structured documents don't have outline trees — always return flat.
            CoreOperationResult::Outline(
                index
                    .keys()
                    .iter()
                    .map(|k| {
                        let indent = "  ".repeat(k.depth);
                        format!("{indent}{}: {:?}", k.path, k.value_kind)
                    })
                    .collect(),
            )
        }
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed: {}",
            uri.as_str()
        ))),
    }
}

/// Convert an arena-allocated `OutlineNode` tree into an owned `OutlineTreeNode`.
///
/// When `source` is provided, each node's `text` field is filled with the section
/// content between this heading and the next sibling/parent heading.
fn outline_node_to_owned(
    node: &OutlineNode<'_>,
    source: Option<&str>,
    all_headings: &[HeadingEntry<'_>],
) -> OutlineTreeNode {
    // Split source lines once and pass the slice to the recursive inner function,
    // avoiding O(headings * lines) repeated allocations.
    let lines: Option<Vec<&str>> = source.map(|s| s.lines().collect());
    outline_node_to_owned_inner(node, lines.as_deref(), all_headings)
}

/// Recursive inner function that receives pre-split source lines.
fn outline_node_to_owned_inner(
    node: &OutlineNode<'_>,
    lines: Option<&[&str]>,
    all_headings: &[HeadingEntry<'_>],
) -> OutlineTreeNode {
    let (title, level, range) = match &node.heading {
        Some(h) => (h.text.to_string(), h.level, h.range),
        None => (
            String::new(),
            0,
            Range::new(
                markymark_core::Position::new(0, 0),
                markymark_core::Position::new(0, 0),
            ),
        ),
    };

    let text = match (lines, &node.heading) {
        (Some(l), Some(heading)) => extract_section_text(l, heading, all_headings),
        _ => None,
    };

    let children = node
        .children
        .iter()
        .map(|child| outline_node_to_owned_inner(child, lines, all_headings))
        .collect();

    OutlineTreeNode {
        title,
        level,
        range,
        text,
        summary: None,
        children,
    }
}

/// Extract section text for a heading: from the line after the heading
/// to the start of the next heading at any level (or EOF).
///
/// This returns the "direct" content of a node — content between this heading
/// and its first child heading (or next sibling). The tree structure captures
/// the hierarchy, so each node's text is just its own prose.
fn extract_section_text(
    lines: &[&str],
    heading: &HeadingEntry<'_>,
    all_headings: &[HeadingEntry<'_>],
) -> Option<String> {
    let heading_line = heading.range.start.line as usize;

    // Content starts on the line after the heading
    let content_start = heading_line + 1;
    if content_start >= lines.len() {
        return Some(String::new());
    }

    // Find the very next heading at any level — section text is the content
    // between this heading and the next one (child or sibling).
    let content_end = all_headings
        .iter()
        .filter(|h| h.range.start.line as usize > heading_line)
        .map(|h| h.range.start.line as usize)
        .next()
        .unwrap_or(lines.len());

    let section: String = lines[content_start..content_end].join("\n");
    Some(section.trim_end().to_string())
}

/// Try to load a sidecar file for the given document URI.
///
/// Checks each workspace root for a `.markymark/<relative_path>.json` sidecar.
/// Returns None if no sidecar exists or the sidecar is stale (we don't validate
/// staleness here — the caller gets whatever exists; enrichment validates freshness).
pub(crate) fn try_load_sidecar(
    uri: &DocumentUri,
    roots: &[std::path::PathBuf],
) -> Option<DocumentSidecar> {
    let file_path = uri.to_file_path()?;

    for root in roots {
        if let Ok(relative) = file_path.strip_prefix(root) {
            let sidecar_dir = root.join(DEFAULT_SIDECAR_DIR);
            let sidecar_file = sidecar_types::sidecar_path(&sidecar_dir, relative);
            if let Ok(json) = std::fs::read_to_string(&sidecar_file) {
                if let Ok(sidecar) = serde_json::from_str::<DocumentSidecar>(&json) {
                    // Validate content hash to avoid stale summaries.
                    if let Ok(source) = std::fs::read(&file_path) {
                        let current_hash = sidecar_types::content_hash(&source);
                        if !sidecar.is_stale(&current_hash) {
                            return Some(sidecar);
                        }
                    }
                }
            }
        }
    }

    None
}
