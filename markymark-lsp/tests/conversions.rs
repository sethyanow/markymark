//! Tests for type conversions between LSP types and markymark-core types.

use markymark_core::{DocumentUri, Position, Range};
use markymark_lsp::convert;
use tower_lsp_server::ls_types;

// ---------------------------------------------------------------------------
// Type conversion: Position
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_position_zero() {
    let lsp_pos = ls_types::Position::new(0, 0);
    let core_pos = convert::from_lsp_position(lsp_pos);
    assert_eq!(core_pos.line, 0);
    assert_eq!(core_pos.character, 0);
}

#[test]
fn test_from_lsp_position_nonzero() {
    let lsp_pos = ls_types::Position::new(42, 17);
    let core_pos = convert::from_lsp_position(lsp_pos);
    assert_eq!(core_pos.line, 42);
    assert_eq!(core_pos.character, 17);
}

#[test]
fn test_to_lsp_position() {
    let core_pos = Position::new(10, 5);
    let lsp_pos = convert::to_lsp_position(core_pos);
    assert_eq!(lsp_pos.line, 10);
    assert_eq!(lsp_pos.character, 5);
}

#[test]
fn test_position_roundtrip_lsp_to_core_to_lsp() {
    let original = ls_types::Position::new(99, 55);
    let core = convert::from_lsp_position(original);
    let roundtrip = convert::to_lsp_position(core);
    assert_eq!(roundtrip.line, original.line);
    assert_eq!(roundtrip.character, original.character);
}

#[test]
fn test_position_roundtrip_core_to_lsp_to_core() {
    let original = Position::new(7, 23);
    let lsp = convert::to_lsp_position(original);
    let roundtrip = convert::from_lsp_position(lsp);
    assert_eq!(roundtrip.line, original.line);
    assert_eq!(roundtrip.character, original.character);
}

// ---------------------------------------------------------------------------
// Type conversion: Range
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_range() {
    let lsp_range = ls_types::Range::new(
        ls_types::Position::new(1, 0),
        ls_types::Position::new(1, 10),
    );
    let core_range = convert::from_lsp_range(lsp_range);
    assert_eq!(core_range.start.line, 1);
    assert_eq!(core_range.start.character, 0);
    assert_eq!(core_range.end.line, 1);
    assert_eq!(core_range.end.character, 10);
}

#[test]
fn test_to_lsp_range() {
    let core_range = Range::new(Position::new(3, 5), Position::new(3, 15));
    let lsp_range = convert::to_lsp_range(core_range);
    assert_eq!(lsp_range.start.line, 3);
    assert_eq!(lsp_range.start.character, 5);
    assert_eq!(lsp_range.end.line, 3);
    assert_eq!(lsp_range.end.character, 15);
}

#[test]
fn test_range_roundtrip() {
    let original = ls_types::Range::new(
        ls_types::Position::new(5, 3),
        ls_types::Position::new(8, 20),
    );
    let core = convert::from_lsp_range(original);
    let roundtrip = convert::to_lsp_range(core);
    assert_eq!(roundtrip.start.line, original.start.line);
    assert_eq!(roundtrip.start.character, original.start.character);
    assert_eq!(roundtrip.end.line, original.end.line);
    assert_eq!(roundtrip.end.character, original.end.character);
}

// ---------------------------------------------------------------------------
// Type conversion: URI
// ---------------------------------------------------------------------------

#[test]
fn test_from_lsp_uri_file() {
    let uri: ls_types::Uri = "file:///home/user/notes/readme.md".parse().unwrap();
    let doc_uri = convert::from_lsp_uri(&uri).expect("should convert file URI");
    assert_eq!(doc_uri.as_str(), "file:///home/user/notes/readme.md");
}

#[test]
fn test_to_lsp_uri_file() {
    let doc_uri = DocumentUri::new("file:///tmp/test.md").unwrap();
    let uri = convert::to_lsp_uri(&doc_uri).expect("should convert to URI");
    assert_eq!(uri.as_str(), "file:///tmp/test.md");
}

#[test]
fn test_uri_roundtrip() {
    let original: ls_types::Uri = "file:///workspace/docs/index.md".parse().unwrap();
    let doc_uri = convert::from_lsp_uri(&original).expect("from_lsp_uri");
    let roundtrip = convert::to_lsp_uri(&doc_uri).expect("to_lsp_uri");
    assert_eq!(roundtrip.as_str(), original.as_str());
}
