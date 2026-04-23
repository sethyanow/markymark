//! Tests for the `find-references` engine operation.
//!
//! Covers named-realm routing and structured-document edge cases
//! (JSON/YAML keys have no cross-doc refs; off-key positions error).

use super::*;

#[tokio::test]
async fn find_references_uses_named_realm() {
    let dir = make_temp_realm_dir();
    // A heading with a wiki-link reference in the same file
    fs::write(
        dir.path().join("doc.md"),
        "# My Heading\n\n[[My Heading]]\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let position = markymark_core::Range {
        start: Position {
            line: 0,
            character: 2,
        },
        end: Position {
            line: 0,
            character: 12,
        },
    };

    // Default realm has no such doc
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: uri.clone(),
            position,
            realm: None,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should find the references
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            position,
            realm: Some("refs-realm".to_string()),
        })
        .await;
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
}

#[tokio::test]
async fn find_references_structured_doc_key_returns_empty_locations() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("config.json"),
        "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-structured", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            position: Range::new(Position::new(2, 5), Position::new(2, 5)),
            realm: Some("refs-structured".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                locations.is_empty(),
                "structured keys have no cross-doc refs"
            )
        }
        other => panic!("expected empty Locations result, got {other:?}"),
    }
}

#[tokio::test]
async fn find_references_structured_doc_off_key_returns_error() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("config.json"),
        "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-structured-off-key", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            // Cursor on value text ("localhost"), not on a key.
            position: Range::new(Position::new(2, 15), Position::new(2, 15)),
            realm: Some("refs-structured-off-key".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Error(err) => {
            assert!(
                err.to_string()
                    .contains("no referenceable symbol at position"),
                "expected no-symbol error, got {err:?}"
            );
        }
        other => panic!("expected Error result, got {other:?}"),
    }
}
