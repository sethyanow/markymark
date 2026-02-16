//! Document symbol conversion helpers for LSP responses.
//!
//! Converts markymark index data (outline trees, XML tags, structured keys)
//! into LSP `DocumentSymbol` hierarchies for `textDocument/documentSymbol`.

use markymark_core::Range as CoreRange;
use markymark_index::{OutlineNode, StructuredDocumentIndex, XmlTagEntry};
use tower_lsp_server::ls_types::*;

/// Convert outline children to `DocumentSymbol` entries.
pub(crate) fn outline_children_to_symbols(children: &[OutlineNode]) -> Vec<DocumentSymbol> {
    children
        .iter()
        .filter_map(|node| {
            let heading = node.heading.as_ref()?;
            let range = crate::convert::to_lsp_range(heading.range);
            #[expect(deprecated, reason = "DocumentSymbol.deprecated field is deprecated by LSP spec but struct still required")]
            Some(DocumentSymbol {
                name: heading.text.to_string(),
                detail: None,
                kind: SymbolKind::STRING,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: Some(outline_children_to_symbols(node.children)),
            })
        })
        .collect()
}

#[derive(Debug)]
struct XmlSymbolNode {
    name: String,
    range: CoreRange,
    children: Vec<XmlSymbolNode>,
}

/// Convert XML tags into nested `DocumentSymbol` entries using containment.
pub(crate) fn xml_tags_to_symbols(xml_tags: &[XmlTagEntry]) -> Vec<DocumentSymbol> {
    let mut roots: Vec<XmlSymbolNode> = Vec::new();

    for tag in xml_tags {
        let node = XmlSymbolNode {
            name: format!("<{}>", tag.tag_name),
            range: tag.range,
            children: Vec::new(),
        };
        insert_xml_node(&mut roots, node);
    }

    roots.into_iter().map(xml_node_to_document_symbol).collect()
}

fn insert_xml_node(nodes: &mut Vec<XmlSymbolNode>, node: XmlSymbolNode) {
    for existing in nodes.iter_mut().rev() {
        if core_range_strictly_contains(existing.range, node.range) {
            insert_xml_node(&mut existing.children, node);
            return;
        }
    }

    nodes.push(node);
}

fn core_range_strictly_contains(parent: CoreRange, child: CoreRange) -> bool {
    parent.start <= child.start
        && child.end <= parent.end
        && (parent.start < child.start || child.end < parent.end)
}

fn xml_node_to_document_symbol(node: XmlSymbolNode) -> DocumentSymbol {
    let range = crate::convert::to_lsp_range(node.range);
    let children: Vec<DocumentSymbol> = node
        .children
        .into_iter()
        .map(xml_node_to_document_symbol)
        .collect();

    #[expect(
        deprecated,
        reason = "DocumentSymbol.deprecated field is deprecated by LSP spec but struct still required"
    )]
    DocumentSymbol {
        name: node.name,
        detail: None,
        kind: SymbolKind::OBJECT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

/// Convert structured document key entries into nested LSP DocumentSymbol items.
///
/// Reconstructs the tree hierarchy from the flat key list using depth information,
/// and maps each `ValueKind` to an appropriate LSP `SymbolKind`.
pub(crate) fn key_entries_to_symbols(index: &StructuredDocumentIndex) -> Vec<DocumentSymbol> {
    use markymark_core::structured::ValueKind;

    fn value_kind_to_symbol_kind(vk: ValueKind) -> SymbolKind {
        match vk {
            ValueKind::Object => SymbolKind::OBJECT,
            ValueKind::Array => SymbolKind::ARRAY,
            ValueKind::String => SymbolKind::STRING,
            ValueKind::Number => SymbolKind::NUMBER,
            ValueKind::Boolean => SymbolKind::BOOLEAN,
            ValueKind::Null => SymbolKind::NULL,
        }
    }

    /// A tree node built from the flat key list.
    struct SymbolNode {
        name: String,
        detail: Option<String>,
        kind: SymbolKind,
        range: tower_lsp_server::ls_types::Range,
        selection_range: tower_lsp_server::ls_types::Range,
        children: Vec<SymbolNode>,
    }

    fn to_document_symbol(node: SymbolNode) -> DocumentSymbol {
        let children: Vec<DocumentSymbol> =
            node.children.into_iter().map(to_document_symbol).collect();

        #[expect(
            deprecated,
            reason = "DocumentSymbol.deprecated field is deprecated by LSP spec but struct still required"
        )]
        DocumentSymbol {
            name: node.name,
            detail: node.detail,
            kind: node.kind,
            tags: None,
            deprecated: None,
            range: node.range,
            selection_range: node.selection_range,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    let keys = index.keys();
    if keys.is_empty() {
        return Vec::new();
    }

    // Build tree using a stack of (depth, node) to track nesting.
    let mut roots: Vec<SymbolNode> = Vec::new();
    // Stack holds mutable references by index path into roots.
    // Simpler approach: use a stack of Vec<SymbolNode> per depth level.
    let mut stack: Vec<(usize, Vec<SymbolNode>)> = Vec::new();

    for entry in keys {
        let range = crate::convert::to_lsp_range(entry.key_range);
        let value_range = crate::convert::to_lsp_range(entry.value_range);

        // The range should span from key start to value end for full coverage.
        let full_range = tower_lsp_server::ls_types::Range {
            start: range.start,
            end: if value_range.end > range.end {
                value_range.end
            } else {
                range.end
            },
        };

        let node = SymbolNode {
            name: entry.key.clone(),
            detail: Some(format!("{:?}", entry.value_kind)),
            kind: value_kind_to_symbol_kind(entry.value_kind),
            range: full_range,
            selection_range: range,
            children: Vec::new(),
        };

        let depth = entry.depth;

        // Pop stack levels that are at or deeper than current depth,
        // folding their children into the parent.
        while let Some((d, _)) = stack.last() {
            if *d >= depth {
                let (_, children) = stack.pop().unwrap();
                if let Some((_, parent_children)) = stack.last_mut() {
                    if let Some(parent) = parent_children.last_mut() {
                        parent.children = children;
                    }
                } else {
                    // These are root-level nodes being finalized
                    if let Some(root) = roots.last_mut() {
                        root.children = children;
                    }
                }
            } else {
                break;
            }
        }

        if depth == 0 {
            roots.push(node);
        } else if let Some((_, children)) = stack.last_mut() {
            children.push(node);
        } else {
            // Shouldn't happen with well-formed data, but handle gracefully
            roots.push(node);
        }

        // If this is a container type, push a new stack level for its children
        if entry.value_kind == ValueKind::Object || entry.value_kind == ValueKind::Array {
            stack.push((depth, Vec::new()));
        }
    }

    // Drain remaining stack levels
    while let Some((_, children)) = stack.pop() {
        if let Some((_, parent_children)) = stack.last_mut() {
            if let Some(parent) = parent_children.last_mut() {
                parent.children = children;
            }
        } else if let Some(root) = roots.last_mut() {
            root.children = children;
        }
    }

    roots.into_iter().map(to_document_symbol).collect()
}
