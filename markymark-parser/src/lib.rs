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

    /// Parse markdown text into an AST
    pub fn parse(&mut self, source: &str) -> CoreResult<Ast> {
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
            .parse(parse_source.as_bytes(), None)
            .ok_or_else(|| CoreError::Message("Failed to parse".to_string()))?;

        // Store the parse source (with newline) so node byte ranges remain valid
        Ast::from_markdown_tree(md_tree, parse_source)
    }

    /// Parse with incremental update
    pub fn parse_incremental(
        &mut self,
        _old_ast: &Ast,
        new_source: &str,
        _start_byte: usize,
        _old_end_byte: usize,
        _new_end_byte: usize,
        _new_end_position: usize,
    ) -> CoreResult<Ast> {
        // For now, just re-parse from scratch
        // TODO: Implement true incremental parsing using MarkdownTree.edit()
        self.parse(new_source)
    }
}

/// Convenience function to parse markdown text.
pub fn parse(source: &str) -> CoreResult<Ast> {
    Parser::new()?.parse(source)
}
