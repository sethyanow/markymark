//! Tests for the `rename` engine operation.
//!
//! Covers named-realm routing and the "structured documents don't support
//! rename" guard (JSON/YAML/TOML).

use super::*;

#[tokio::test]
async fn rename_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Old Name\n").unwrap();
    let engine = make_engine_with_custom_realm("rename-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let position = markymark_core::Range {
        start: Position {
            line: 0,
            character: 2,
        },
        end: Position {
            line: 0,
            character: 10,
        },
    };

    // Default realm has no such doc
    let result = engine
        .execute(CoreOperation::Rename {
            uri: uri.clone(),
            position,
            new_name: "New Name".to_string(),
            realm: None,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should work
    let result = engine
        .execute(CoreOperation::Rename {
            uri,
            position,
            new_name: "New Name".to_string(),
            realm: Some("rename-realm".to_string()),
        })
        .await;
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
}

#[tokio::test]
async fn rename_structured_doc_returns_not_supported_error() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("config.toml"), "host = \"localhost\"\n").unwrap();
    let engine = make_engine_with_custom_realm("rename-structured", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.toml"));

    let result = engine
        .execute(CoreOperation::Rename {
            uri,
            position: Range::new(Position::new(0, 1), Position::new(0, 1)),
            new_name: "server_host".to_string(),
            realm: Some("rename-structured".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Error(err) => {
            assert!(
                err.to_string()
                    .contains("rename is not supported for structured documents"),
                "expected structured rename unsupported error, got {err:?}"
            );
        }
        other => panic!("expected Error result, got {other:?}"),
    }
}
