use std::path::PathBuf;

use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};

// ---------------------------------------------------------------------------
// CoreOperation enum variants
// ---------------------------------------------------------------------------

#[test]
fn test_core_operation_get_outline_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let op = CoreOperation::GetOutline { uri, realm: None };
    // Verify it's the correct variant via pattern matching
    match op {
        CoreOperation::GetOutline { uri, .. } => {
            assert_eq!(uri.as_str(), "file:///vault/test.md");
        }
        _ => panic!("expected GetOutline variant"),
    }
}

#[test]
fn test_core_operation_find_references_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let position = Range::new(Position::new(5, 10), Position::new(5, 20));
    let op = CoreOperation::FindReferences {
        uri,
        position,
        realm: None,
    };
    match op {
        CoreOperation::FindReferences {
            uri,
            position,
            realm: None,
        } => {
            assert_eq!(uri.as_str(), "file:///vault/test.md");
            assert_eq!(position.start.line, 5);
            assert_eq!(position.start.character, 10);
        }
        _ => panic!("expected FindReferences variant"),
    }
}

#[test]
fn test_core_operation_rename_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let position = Range::new(Position::new(2, 0), Position::new(2, 10));
    let op = CoreOperation::Rename {
        uri,
        position,
        new_name: "NewHeading".to_string(),
        realm: None,
    };
    match op {
        CoreOperation::Rename {
            uri,
            position,
            new_name,
            realm: None,
        } => {
            assert_eq!(uri.as_str(), "file:///vault/test.md");
            assert_eq!(position.start.line, 2);
            assert_eq!(new_name, "NewHeading");
        }
        _ => panic!("expected Rename variant"),
    }
}

#[test]
fn test_core_operation_search_symbols_variant() {
    let op = CoreOperation::SearchSymbols {
        query: "introduction".to_string(),
        realm: None,
    };
    match op {
        CoreOperation::SearchSymbols { query, .. } => {
            assert_eq!(query, "introduction");
        }
        _ => panic!("expected SearchSymbols variant"),
    }
}

// ---------------------------------------------------------------------------
// CoreOperationResult enum variants
// ---------------------------------------------------------------------------

#[test]
fn test_core_result_outline_variant() {
    let result =
        CoreOperationResult::Outline(vec!["Heading 1".to_string(), "Heading 2".to_string()]);
    match result {
        CoreOperationResult::Outline(headings) => {
            assert_eq!(headings.len(), 2);
            assert_eq!(headings[0], "Heading 1");
        }
        _ => panic!("expected Outline variant"),
    }
}

#[test]
fn test_core_result_locations_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let range = Range::new(Position::new(0, 0), Position::new(0, 10));
    let result = CoreOperationResult::Locations(vec![(uri, range)]);
    match result {
        CoreOperationResult::Locations(locs) => {
            assert_eq!(locs.len(), 1);
            assert_eq!(locs[0].0.as_str(), "file:///vault/test.md");
        }
        _ => panic!("expected Locations variant"),
    }
}

#[test]
fn test_core_result_workspace_edit_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let range = Range::new(Position::new(1, 2), Position::new(1, 12));
    let edits = vec![(uri, vec![(range, "NewText".to_string())])];
    let result = CoreOperationResult::WorkspaceEdit(edits);
    match result {
        CoreOperationResult::WorkspaceEdit(we) => {
            assert_eq!(we.len(), 1);
            assert_eq!(we[0].1.len(), 1);
            assert_eq!(we[0].1[0].1, "NewText");
        }
        _ => panic!("expected WorkspaceEdit variant"),
    }
}

#[test]
fn test_core_result_symbols_variant() {
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/test.md"));
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    let result = CoreOperationResult::Symbols(vec![("intro".to_string(), uri, range)]);
    match result {
        CoreOperationResult::Symbols(symbols) => {
            assert_eq!(symbols.len(), 1);
            assert_eq!(symbols[0].0, "intro");
        }
        _ => panic!("expected Symbols variant"),
    }
}

#[test]
fn test_core_result_ok_variant() {
    let result = CoreOperationResult::Ok;
    match result {
        CoreOperationResult::Ok => {} // success
        _ => panic!("expected Ok variant"),
    }
}

#[test]
fn test_core_result_error_variant() {
    let err = CoreError::Message("something went wrong".to_string());
    let result = CoreOperationResult::Error(err);
    match result {
        CoreOperationResult::Error(e) => {
            let msg = format!("{}", e);
            assert_eq!(msg, "something went wrong");
        }
        _ => panic!("expected Error variant"),
    }
}

// ---------------------------------------------------------------------------
// CoreError Display formatting
// ---------------------------------------------------------------------------

#[test]
fn test_core_error_display_message() {
    let err = CoreError::Message("custom error".to_string());
    assert_eq!(format!("{}", err), "custom error");
}

#[test]
fn test_core_error_display_invalid_uri() {
    let err = CoreError::InvalidUri("missing scheme".to_string());
    assert_eq!(format!("{}", err), "Invalid URI: missing scheme");
}

#[test]
fn test_core_error_display_not_implemented() {
    let err = CoreError::NotImplemented("feature X".to_string());
    assert_eq!(format!("{}", err), "Not implemented: feature X");
}

// ---------------------------------------------------------------------------
// CoreEngine trait contract
// ---------------------------------------------------------------------------

struct MockEngine;

#[async_trait]
impl CoreEngine for MockEngine {
    async fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { .. } => {
                CoreOperationResult::Outline(vec!["Heading".to_string()])
            }
            CoreOperation::GetContentBlocks { uri, .. } => CoreOperationResult::ContentBlocks {
                uri,
                blocks: vec![],
            },
            _ => CoreOperationResult::Ok,
        }
    }
}

#[tokio::test]
async fn test_core_engine_executes_operation() {
    let engine = MockEngine;
    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/notes.md"));
    let result = engine
        .execute(CoreOperation::GetOutline { uri, realm: None })
        .await;

    match result {
        CoreOperationResult::Outline(items) => {
            assert_eq!(items, vec!["Heading".to_string()]);
        }
        _ => panic!("expected outline result"),
    }
}
