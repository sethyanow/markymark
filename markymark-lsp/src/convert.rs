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

/// Convert an LSP position (line number + UTF-16 character offset) to a byte
/// offset within a UTF-8 string.
///
/// LSP positions use zero-based line numbers and UTF-16 code units for the
/// character offset. This function correctly handles multi-byte Unicode
/// characters including emoji (surrogate pairs = 2 UTF-16 code units) and
/// accented characters (1 UTF-16 code unit, 2+ UTF-8 bytes).
pub fn lsp_position_to_byte_offset(text: &str, line: u32, character: u32) -> usize {
    let target_line = line as usize;
    let target_char = character as usize;
    let mut byte_offset = 0;

    for (idx, line_text) in text.split('\n').enumerate() {
        if idx == target_line {
            // Strip trailing \r for character offset calculation (CRLF line endings)
            let content = line_text.strip_suffix('\r').unwrap_or(line_text);
            return byte_offset + utf16_offset_to_byte_offset(content, target_char);
        }
        byte_offset += line_text.len() + 1; // +1 for the '\n'
    }

    text.len()
}

/// Convert a UTF-16 character offset to a byte offset within a single line.
pub(crate) fn utf16_offset_to_byte_offset(line: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }
    line.len()
}

/// Convert an iterator of markymark diagnostics to a vec of LSP diagnostics.
pub fn to_lsp_diagnostics(
    diagnostics: impl IntoIterator<Item = crate::diagnostics::MarkyDiagnostic>,
) -> Vec<ls_types::Diagnostic> {
    use crate::diagnostics::DiagnosticSeverity;
    diagnostics
        .into_iter()
        .map(|d| ls_types::Diagnostic {
            range: to_lsp_range(d.range),
            severity: Some(match d.severity {
                DiagnosticSeverity::Error => ls_types::DiagnosticSeverity::ERROR,
                DiagnosticSeverity::Warning => ls_types::DiagnosticSeverity::WARNING,
            }),
            source: Some("markymark".to_string()),
            message: d.message,
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{DiagnosticSeverity, MarkyDiagnostic};
    use markymark_core::{Position, Range};

    fn make_diagnostic(severity: DiagnosticSeverity, message: &str) -> MarkyDiagnostic {
        MarkyDiagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            severity,
            message: message.to_string(),
        }
    }

    #[test]
    fn test_error_severity_maps_to_lsp_error() {
        let result = to_lsp_diagnostics([make_diagnostic(DiagnosticSeverity::Error, "err")]);
        assert_eq!(
            result[0].severity,
            Some(ls_types::DiagnosticSeverity::ERROR)
        );
    }

    #[test]
    fn test_warning_severity_maps_to_lsp_warning() {
        let result = to_lsp_diagnostics([make_diagnostic(DiagnosticSeverity::Warning, "warn")]);
        assert_eq!(
            result[0].severity,
            Some(ls_types::DiagnosticSeverity::WARNING)
        );
    }

    #[test]
    fn test_empty_iterator_returns_empty_vec() {
        let result = to_lsp_diagnostics(Vec::<MarkyDiagnostic>::new());
        assert!(result.is_empty());
    }

    #[test]
    fn test_source_field_is_markymark() {
        let result = to_lsp_diagnostics([make_diagnostic(DiagnosticSeverity::Error, "x")]);
        assert_eq!(result[0].source, Some("markymark".to_string()));
    }

    #[test]
    fn test_message_and_range_preserved() {
        let diag = MarkyDiagnostic {
            range: Range::new(Position::new(3, 2), Position::new(3, 10)),
            severity: DiagnosticSeverity::Error,
            message: "test error".to_string(),
        };
        let result = to_lsp_diagnostics([diag]);
        assert_eq!(result[0].message, "test error");
        let expected_range = ls_types::Range::new(
            ls_types::Position::new(3, 2),
            ls_types::Position::new(3, 10),
        );
        assert_eq!(result[0].range, expected_range);
    }
}
