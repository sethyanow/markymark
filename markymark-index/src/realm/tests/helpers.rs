use super::super::*;
use crate::document::{CodeSpanOwned, DocumentIndex};
use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst};
use std::path::PathBuf;

pub(super) fn make_md_index(source: &str) -> DocumentIndex {
    DocumentIndex::from_text(source)
}

/// Build a markdown index whose code_spans contain the given identifiers.
///
/// Constructs a source string with backtick code spans so the engine
/// extracts them naturally.
pub(super) fn make_md_index_with_code_spans(code_spans: Vec<CodeSpanOwned>) -> DocumentIndex {
    // Build source text: heading + one backtick code span per entry
    let mut source = String::from("# Intro\n\n");
    for cs in &code_spans {
        source.push('`');
        source.push_str(&cs.text);
        source.push_str("` ");
    }
    source.push('\n');
    DocumentIndex::from_text(&source)
}

pub(super) fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}")))
}

pub(super) fn make_structured_index(
    kind: DocumentKind,
    keys: Vec<KeyEntry>,
) -> StructuredDocumentIndex {
    let ast = StructuredAst {
        source: String::new(),
        kind,
        keys,
    };
    StructuredDocumentIndex::from_ast(ast)
}

pub(super) fn test_key(path: &str, key_name: &str, depth: usize, vk: ValueKind) -> KeyEntry {
    KeyEntry {
        path: path.to_string(),
        key: key_name.to_string(),
        depth,
        value_kind: vk,
        key_range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
        value_range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
    }
}

pub(super) fn code_span(text: &str) -> CodeSpanOwned {
    CodeSpanOwned {
        text: text.to_string(),
        range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
        start_byte: 0,
        end_byte: text.len(),
    }
}
