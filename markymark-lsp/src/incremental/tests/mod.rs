use super::*;
use markymark_core::{Position, Range};
use markymark_parser::Point;

mod adjust;
mod blocks;
mod markdown_links;
mod parity;
mod regression;
mod wiki_links;
mod xml_tags;

// ─── Shared helpers ──────────────────────────────────────────────────────────

fn make_edit(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> InputEdit {
    InputEdit {
        start_byte: 0,
        old_end_byte: 1,
        new_end_byte: 1,
        start_position: Point {
            row: start_line as usize,
            column: start_col as usize,
        },
        old_end_position: Point {
            row: end_line as usize,
            column: end_col as usize,
        },
        new_end_position: Point {
            row: end_line as usize,
            column: end_col as usize,
        },
    }
}

fn make_ml(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> MarkdownLinkOwned {
    make_ml_bytes(start_line, start_col, end_line, end_col, 0, 0)
}

fn make_ml_bytes(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    start_byte: usize,
    end_byte: usize,
) -> MarkdownLinkOwned {
    MarkdownLinkOwned {
        text: "link".to_string(),
        url: "https://example.com".to_string(),
        anchor: None,
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
        start_byte,
        end_byte,
    }
}

fn make_xt(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    tag_name: &str,
) -> XmlTagOwned {
    make_xt_bytes(start_line, start_col, end_line, end_col, tag_name, 0, 0)
}

fn make_xt_bytes(
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    tag_name: &str,
    start_byte: usize,
    end_byte: usize,
) -> XmlTagOwned {
    XmlTagOwned {
        tag_name: tag_name.to_string(),
        attributes: vec![("key".to_string(), "val".to_string())],
        is_self_closing: false,
        is_unclosed: false,
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
        start_byte,
        end_byte,
    }
}

fn make_block_owned(
    id: &str,
    start_line: u32,
    start_col: u32,
    end_col: u32,
    start_byte: usize,
    end_byte: usize,
) -> BlockOwned {
    BlockOwned {
        id: id.to_string(),
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(start_line, end_col),
        ),
        start_byte,
        end_byte,
    }
}
