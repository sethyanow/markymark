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

    /// Parse markdown and return only the block tree, skipping all inline grammar parsing.
    ///
    /// The dual-grammar `MarkdownParser` runs N inline parses (one per paragraph/inline node).
    /// This method runs only the block grammar, making it O(1) in number of inline nodes.
    /// Useful for measuring how much of the parse cost comes from inline vs. block parsing.
    pub fn parse_block_tree_only(
        &mut self,
        source: &str,
        old_block_tree: Option<&tree_sitter::Tree>,
    ) -> Option<tree_sitter::Tree> {
        let needs_newline = !source.is_empty() && !source.ends_with('\n');
        let normalized;
        let parse_source: &[u8] = if needs_newline {
            normalized = format!("{source}\n");
            normalized.as_bytes()
        } else {
            source.as_bytes()
        };
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .expect("block language load");
        ts_parser.parse(parse_source, old_block_tree)
    }

    /// Parse markdown text and return only the MarkdownTree, skipping AST element collection.
    ///
    /// This is a diagnostic/optimization path: it performs the tree-sitter parse (with optional
    /// old-tree reuse) but skips `collect_elements`, which builds the arena-allocated Element vec.
    /// Useful when measuring parse-only cost vs. full AST construction cost.
    pub fn parse_tree_only(
        &mut self,
        source: &str,
        old_tree: Option<&MarkdownTree>,
    ) -> Option<MarkdownTree> {
        let needs_newline = !source.is_empty() && !source.ends_with('\n');
        let normalized;
        let parse_source = if needs_newline {
            normalized = format!("{source}\n");
            normalized.as_str()
        } else {
            source
        };
        self.parser.parse(parse_source.as_bytes(), old_tree)
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

/// Find a byte offset for a prose edit near the middle of `content`.
///
/// Returns the midpoint of the line, closest to the document midpoint, that:
/// - is at least 30 characters long
/// - does not start with `#`, `` ` ``, `~`, `-`, `*`, or `>`
/// - does not contain wiki-link syntax (`[[` / `]]`)
///
/// Returns `None` if no qualifying line exists.
///
/// # Note
///
/// This is a benchmarking utility exposed for use by criterion benches.
/// It is not part of the stable public API.
#[doc(hidden)]
pub fn find_prose_edit_pos(content: &str) -> Option<usize> {
    let target = content.len() / 2;
    let mut best: Option<usize> = None;
    let mut best_dist = usize::MAX;
    let mut offset = 0usize;

    for line in content.lines() {
        let line_mid = offset + line.len() / 2;
        let is_prose = line.len() >= 30
            && !line.starts_with('#')
            && !line.starts_with("```")
            && !line.starts_with("~~~")
            && !line.starts_with('-')
            && !line.starts_with('*')
            && !line.starts_with('>')
            && !line.contains("[[")
            && !line.contains("]]");

        if is_prose {
            let dist = line_mid.abs_diff(target);
            if dist < best_dist {
                best_dist = dist;
                best = Some(line_mid);
            }
        }
        // Advance by the line length plus the line terminator width.
        // `str::lines()` strips both '\n' and '\r\n', so we check the original
        // byte after the line content to pick the correct terminator width.
        let terminator_len = if content.as_bytes().get(offset + line.len()) == Some(&b'\r') {
            2 // \r\n
        } else {
            1 // \n (or end-of-file)
        };
        offset += line.len() + terminator_len;
    }

    best
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

#[cfg(test)]
mod bench_helpers_tests {
    use super::find_prose_edit_pos;

    #[test]
    fn empty_returns_none() {
        assert_eq!(find_prose_edit_pos(""), None);
    }

    #[test]
    fn only_headings_returns_none() {
        let doc = "# Heading One\n## Heading Two\n### Heading Three\n";
        assert_eq!(find_prose_edit_pos(doc), None);
    }

    #[test]
    fn finds_prose_line() {
        let doc = concat!(
            "# Title\n\n",
            "This is a prose paragraph with plenty of text to be found.\n\n",
            "Another prose paragraph also with sufficient length here.\n",
        );
        let pos = find_prose_edit_pos(doc).expect("should find a prose position");
        assert!(pos < doc.len(), "position must be within document bounds");
        let ch = doc.as_bytes()[pos] as char;
        assert!(
            ch != '#' && ch != '\n',
            "not on heading or newline, got {ch:?}"
        );
    }

    #[test]
    fn skips_wiki_links() {
        let doc = "# Head\n\n[[This is a wiki link that is definitely longer than thirty chars]]\n";
        assert_eq!(find_prose_edit_pos(doc), None);
    }

    #[test]
    fn skips_short_lines() {
        let doc = "# Head\n\nShort line.\n\nAnother short one.\n";
        assert_eq!(find_prose_edit_pos(doc), None);
    }

    #[test]
    fn crlf_line_endings_produce_correct_offset() {
        // Regression: find_prose_edit_pos used to advance by line.len()+1,
        // undercounting by 1 byte per CRLF-terminated line.
        let line1 = "# Title\r\n";
        let line2 = "\r\n";
        let prose = "This is a prose paragraph with plenty of text to be found here.\r\n";
        let doc = format!("{line1}{line2}{prose}");

        let pos = find_prose_edit_pos(&doc).expect("should find prose in CRLF doc");
        // The prose line starts at offset = len(line1) + len(line2) = 9 + 2 = 11
        let prose_start = line1.len() + line2.len();
        let prose_end = prose_start + prose.len();
        assert!(
            pos >= prose_start && pos < prose_end,
            "offset {pos} should be within prose range [{prose_start}..{prose_end})"
        );
        // Verify the byte at this position is actual prose content
        assert!(
            doc.as_bytes()[pos].is_ascii_alphabetic(),
            "byte at offset {pos} should be alphabetic, got {:?}",
            doc.as_bytes()[pos] as char,
        );
    }

    #[test]
    fn prefers_midpoint() {
        // Two qualifying lines; one near start, one near end.
        // Separate them with blank lines (empty lines are not qualifying, length < 30).
        // The line whose midpoint is closest to the document midpoint should win.
        let near_start = "This is the first qualifying prose paragraph with plenty of text.\n";
        // 1000 blank lines push the midpoint to roughly (near_start.len + 1000) / 2
        let filler = "\n".repeat(1000);
        let near_end = "This is the second qualifying prose paragraph with plenty of text.\n";
        let doc = format!("{near_start}{filler}{near_end}");

        let pos = find_prose_edit_pos(&doc).expect("should find a prose position");
        // near_end midpoint (~offset 1099) is closer to target (~566) than
        // near_start midpoint (~32), so near_end wins.
        assert!(
            pos > near_start.len() + filler.len() / 2,
            "should prefer the line closer to the document midpoint"
        );
    }
}
