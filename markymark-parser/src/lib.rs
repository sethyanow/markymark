//! markymark-parser: Tree-sitter based markdown parser
//!
//! Parses markdown documents with support for Obsidian and Logseq flavors.

#![warn(missing_docs)]
#![warn(clippy::all)]

use markymark_core::prelude::*;
use tree_sitter::Parser as TSParser;

mod ast;
mod extract;
mod types;

pub use ast::Ast;
pub use extract::*;
pub use types::*;

/// Markdown parser using tree-sitter
pub struct Parser {
    parser: TSParser,
}

impl Parser {
    /// Create a new parser instance
    pub fn new() -> CoreResult<Self> {
        let mut parser = TSParser::new();
        let language = tree_sitter_markdown::language();

        parser
            .set_language(language)
            .map_err(|e| CoreError::Message(format!("Failed to set language: {}", e)))?;

        Ok(Self { parser })
    }

    /// Parse markdown text into an AST
    pub fn parse(&mut self, source: &str) -> CoreResult<Ast> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| CoreError::Message("Failed to parse".to_string()))?;

        Ast::from_tree(tree, source)
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
        // TODO: Implement true incremental parsing using tree.edit()
        self.parse(new_source)
    }
}

/// Convenience function to parse markdown text.
pub fn parse(source: &str) -> CoreResult<Ast> {
    Parser::new()?.parse(source)
}
