//! [`DocumentIndex::from_ast`] — construct index from a parsed AST.
//!
//! Post-Phase C, this delegates to the Zig scan backend for all markdown-content
//! extraction. Frontmatter is extracted via tree-sitter (the Ast), then the source
//! is passed to `from_scan_with_frontmatter` for everything else.

use markymark_core::scanner::Md4cScanBackend;
use markymark_parser::Ast;

use super::{helpers, DocumentIndex};

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    ///
    /// Extracts frontmatter from the tree-sitter AST, then delegates all
    /// markdown-content extraction (headings, links, tags, code spans, etc.)
    /// to the Zig scan backend via [`from_scan_with_frontmatter`].
    pub fn from_ast(ast: Ast) -> Self {
        let source_text = ast.source().to_string();
        let (fm, aliases) = helpers::extract_frontmatter_from_ast(&ast);
        // Release tree-sitter arena memory before from_scan allocates a new arena.
        drop(ast);
        let masked = helpers::mask_frontmatter(&source_text);
        Self::from_scan_with_frontmatter(&masked, &Md4cScanBackend, fm, aliases)
    }
}
