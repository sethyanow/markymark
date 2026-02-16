//! markymark-parser: Tree-sitter based markdown parser
//!
//! Parses markdown documents with support for Obsidian and Logseq flavors.

#![warn(missing_docs)]
#![warn(clippy::all)]

use markymark_core::prelude::*;
use tree_sitter_md::MarkdownParser;

mod ast;
mod extract;
pub mod structured;
mod types;

pub use ast::Ast;
pub use extract::*;
pub use tree_sitter::InputEdit;
pub use tree_sitter::Point;
pub use tree_sitter_md::MarkdownTree;
pub use types::*;

/// Markdown parser using tree-sitter
pub struct Parser {
    parser: MarkdownParser,
}

impl Parser {
    /// Create a new parser instance
    pub fn new() -> CoreResult<Self> {
        let parser = MarkdownParser::default();
        Ok(Self { parser })
    }

    /// Parse markdown text into an AST (full reparse, no tree reuse).
    pub fn parse(&mut self, source: &str) -> CoreResult<Ast> {
        self.parse_with_old_tree(source, None)
    }

    /// Parse markdown text, optionally reusing an old parse tree for incremental updates.
    ///
    /// When `old_tree` is `Some`, tree-sitter reuses unchanged subtrees from the
    /// old tree, making the parse O(edit_size) instead of O(document_size).
    ///
    /// The old tree **must** have been updated via [`MarkdownTree::edit()`] with all
    /// changes since it was last parsed. Failing to do so produces incorrect results.
    pub fn parse_with_old_tree(
        &mut self,
        source: &str,
        old_tree: Option<&MarkdownTree>,
    ) -> CoreResult<Ast> {
        // tree-sitter-md requires a trailing newline for valid block parsing.
        // Normalize input to avoid ERROR nodes for content without one.
        let needs_newline = !source.is_empty() && !source.ends_with('\n');
        let normalized;
        let parse_source = if needs_newline {
            normalized = format!("{source}\n");
            normalized.as_str()
        } else {
            source
        };

        let md_tree = self
            .parser
            .parse(parse_source.as_bytes(), old_tree)
            .ok_or_else(|| CoreError::Message("Failed to parse".to_string()))?;

        // Store the parse source (with newline) so node byte ranges remain valid
        Ast::from_markdown_tree(md_tree, parse_source)
    }
}

/// Convenience function to parse markdown text.
pub fn parse(source: &str) -> CoreResult<Ast> {
    Parser::new()?.parse(source)
}

/// Convert a byte offset within a source string to a tree-sitter [`Point`] (row, column).
///
/// Both `row` and `column` are zero-based. Column is in bytes (not characters),
/// matching tree-sitter's convention.
pub fn byte_to_point(source: &str, byte_offset: usize) -> Point {
    let clamped = byte_offset.min(source.len());
    let prefix = &source[..clamped];
    let row = prefix.matches('\n').count();
    let column = prefix
        .rfind('\n')
        .map(|pos| clamped - pos - 1)
        .unwrap_or(clamped);
    Point { row, column }
}
