//! [`DocumentIndex::from_ast`] — construct index from a parsed AST.
//!
//! Extracts frontmatter and content blocks from the tree-sitter AST, then
//! delegates markdown-content extraction (headings, links, tags, code spans,
//! etc.) to the Zig scan backend via `from_scan_inner`.

use markymark_core::scanner::Md4cScanBackend;
use markymark_parser::Ast;

use super::types::BlockKind;
use super::{helpers, DocumentIndex};

/// Intermediate representation of a content block extracted from tree-sitter.
///
/// Carries the block kind and byte range from the AST walk to `from_scan_inner`
/// where it gets arena-allocated as a [`ContentBlock`].
pub(super) struct RawBlock {
    pub kind: BlockKind,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    ///
    /// Extracts frontmatter and content blocks from the tree-sitter AST, then
    /// delegates all markdown-content extraction (headings, links, tags, code
    /// spans, etc.) to the Zig scan backend.
    pub fn from_ast(mut ast: Ast) -> Self {
        let source_text = ast.source().to_string();
        let (fm, aliases) = helpers::extract_frontmatter_from_ast(&ast);
        let fm_end = helpers::frontmatter_byte_end(&source_text);
        let raw_blocks = extract_content_blocks(&mut ast, &source_text, fm_end);
        // Release tree-sitter arena memory before from_scan allocates a new arena.
        drop(ast);
        let masked = helpers::mask_frontmatter(&source_text);
        Self::from_scan_inner(&masked, &Md4cScanBackend, fm, aliases, raw_blocks)
    }
}

/// Extract content blocks from the tree-sitter AST before it's dropped.
///
/// Walks the block tree root, descending into section nodes (tree-sitter-md's
/// structural wrapper), and collects paragraph, list_item, fenced_code_block,
/// indented_code_block, block_quote, thematic_break, and pipe_table nodes.
///
/// Blocks whose `start_byte` falls within the frontmatter region (< `fm_end`)
/// are excluded. Logseq-style heading list items are also excluded.
fn extract_content_blocks(ast: &mut Ast, source: &str, fm_end: usize) -> Vec<RawBlock> {
    let md_tree = match ast.take_md_tree() {
        Some(t) => t,
        None => return Vec::new(),
    };

    let root = md_tree.block_tree().root_node();
    let mut blocks = Vec::new();
    collect_content_blocks(root, source, fm_end, &mut blocks);

    // Ensure document-order by start_byte (tree-sitter walk should already be
    // in order, but sort defensively).
    blocks.sort_by_key(|b| b.start_byte);
    blocks
}

/// Recursively walk tree-sitter nodes, descending into sections and lists.
fn collect_content_blocks(
    node: tree_sitter::Node,
    source: &str,
    fm_end: usize,
    blocks: &mut Vec<RawBlock>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind_str = child.kind();
        let start = child.start_byte();
        let end = child.end_byte();

        // Skip anything in the frontmatter region
        if start < fm_end {
            continue;
        }

        match kind_str {
            // Structural wrapper — recurse without collecting
            "section" | "document" => {
                collect_content_blocks(child, source, fm_end, blocks);
            }

            "paragraph" => {
                blocks.push(RawBlock {
                    kind: BlockKind::Paragraph,
                    start_byte: start,
                    end_byte: end,
                });
            }

            "list" => {
                // Descend into list to extract flat list_item entries
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    if list_child.kind() == "list_item" {
                        // Skip Logseq-style headings (- # Heading)
                        if is_logseq_heading(list_child, source) {
                            continue;
                        }
                        blocks.push(RawBlock {
                            kind: BlockKind::ListItem,
                            start_byte: list_child.start_byte(),
                            end_byte: list_child.end_byte(),
                        });
                    }
                }
            }

            "fenced_code_block" | "indented_code_block" => {
                blocks.push(RawBlock {
                    kind: BlockKind::CodeBlock,
                    start_byte: start,
                    end_byte: end,
                });
            }

            "block_quote" => {
                // Top-level only — do NOT descend into inner paragraphs
                blocks.push(RawBlock {
                    kind: BlockKind::BlockQuote,
                    start_byte: start,
                    end_byte: end,
                });
            }

            "thematic_break" => {
                blocks.push(RawBlock {
                    kind: BlockKind::ThematicBreak,
                    start_byte: start,
                    end_byte: end,
                });
            }

            "pipe_table" => {
                blocks.push(RawBlock {
                    kind: BlockKind::Table,
                    start_byte: start,
                    end_byte: end,
                });
            }

            // ATX headings and other node types — skip (headings are HeadingEntry)
            _ => {}
        }
    }
}

/// Check if a list_item node is a Logseq-style heading (e.g., `- # Heading`).
///
/// Mirrors the logic in `markymark_parser::ast::try_logseq_heading`.
fn is_logseq_heading(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() != "list_item" {
        return false;
    }
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let first_line = match text.lines().next() {
        Some(l) => l,
        None => return false,
    };
    let trimmed = first_line.trim_start();
    let after_marker = if trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
    {
        &trimmed[2..]
    } else {
        return false;
    };
    // Must start with 1-6 '#' followed by a space
    if !after_marker.starts_with('#') {
        return false;
    }
    let level = after_marker.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return false;
    }
    let rest = &after_marker[level..];
    rest.starts_with(' ') && !rest[1..].trim().is_empty()
}
