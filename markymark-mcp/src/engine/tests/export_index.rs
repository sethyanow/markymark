//! Tests for the `export-index` engine operation (per-document export;
//! distinct from `export-docs-index` which is batch).
//!
//! Open-at-execution decision (marky-n1h step 8): standalone file rather
//! than absorbed into `export_docs_index.rs`. Reason: `ExportIndex` and
//! `ExportDocsIndex` are different `CoreOperation` variants with different
//! result shapes (`DocumentExport` vs `DocsIndexExport`). Grouping by
//! operation, not by name similarity.

use super::*;

#[tokio::test]
async fn export_index_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Title\n").unwrap();
    let engine = make_engine_with_custom_realm("export-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: Some("export-realm".to_string()),
            include_blocks: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::DocumentExport { .. }),
        "expected DocumentExport from named realm, got {result:?}"
    );

    let result_default = engine
        .execute(CoreOperation::ExportIndex {
            uri,
            realm: None,
            include_blocks: false,
        })
        .await;
    assert!(
        matches!(result_default, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result_default:?}"
    );
}
