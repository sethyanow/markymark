//! Type conversions between `ls_types` (used by tower-lsp-server) and `markymark_core` types.

use markymark_core::{DocumentUri, Position, Range};
use tower_lsp_server::ls_types;

/// Convert an `ls_types::Position` to a `markymark_core::Position`.
pub fn from_lsp_position(pos: ls_types::Position) -> Position {
    Position::new(pos.line, pos.character)
}

/// Convert a `markymark_core::Position` to an `ls_types::Position`.
pub fn to_lsp_position(pos: Position) -> ls_types::Position {
    ls_types::Position::new(pos.line, pos.character)
}

/// Convert an `ls_types::Range` to a `markymark_core::Range`.
pub fn from_lsp_range(range: ls_types::Range) -> Range {
    Range::new(from_lsp_position(range.start), from_lsp_position(range.end))
}

/// Convert a `markymark_core::Range` to an `ls_types::Range`.
pub fn to_lsp_range(range: Range) -> ls_types::Range {
    ls_types::Range::new(to_lsp_position(range.start), to_lsp_position(range.end))
}

/// Convert an `ls_types::Uri` to a `markymark_core::DocumentUri`.
pub fn from_lsp_uri(uri: &ls_types::Uri) -> markymark_core::CoreResult<DocumentUri> {
    DocumentUri::new(uri.as_str())
}

/// Convert a `markymark_core::DocumentUri` to an `ls_types::Uri`.
pub fn to_lsp_uri(uri: &DocumentUri) -> Result<ls_types::Uri, String> {
    uri.as_str()
        .parse::<ls_types::Uri>()
        .map_err(|e| e.to_string())
}

/// Build an `ls_types::Location` from a `DocumentUri` and a `Range`.
pub fn to_lsp_location(uri: &DocumentUri, range: Range) -> Result<ls_types::Location, String> {
    let lsp_uri = to_lsp_uri(uri)?;
    Ok(ls_types::Location::new(lsp_uri, to_lsp_range(range)))
}
