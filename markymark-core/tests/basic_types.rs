use std::path::PathBuf;

use markymark_core::{DocumentUri, Position, Range};

#[test]
fn position_new_sets_line_and_character() {
    let pos = Position::new(4, 12);
    assert_eq!(pos.line, 4);
    assert_eq!(pos.character, 12);
}

#[test]
fn range_new_preserves_start_and_end() {
    let start = Position::new(1, 0);
    let end = Position::new(3, 8);
    let range = Range::new(start, end);

    assert_eq!(range.start.line, 1);
    assert_eq!(range.end.character, 8);
}

#[test]
fn range_contains_position_inside_bounds() {
    let range = Range::new(Position::new(2, 4), Position::new(2, 10));
    assert!(range.contains(Position::new(2, 6)));
    assert!(!range.contains(Position::new(2, 11)));
}

#[test]
fn document_uri_requires_a_scheme() {
    assert!(DocumentUri::new("notes/today.md").is_err());
    assert!(DocumentUri::new("file:///tmp/today.md").is_ok());
}

#[test]
fn document_uri_round_trips_file_path() {
    let original = PathBuf::from("/tmp/My Notes/today.md");
    let uri = DocumentUri::from_file_path(&original);

    assert_eq!(uri.as_str(), "file:///tmp/My%20Notes/today.md");
    assert_eq!(uri.to_file_path(), Some(original));
}
