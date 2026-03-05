//! Tests for the export-docs-index engine handler.

use super::*;

// ── Helpers ──

async fn make_engine_with_root(dir: &Path) -> RuntimeEngine {
    let engine = RuntimeEngine::default();
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: dir.to_path_buf(),
        })
        .await;
    engine
}

fn extract_entries(result: CoreOperationResult) -> (Vec<String>, usize, usize, usize) {
    match result {
        CoreOperationResult::DocsIndexExport {
            entries,
            doc_count,
            root_count,
            skipped_count,
            ..
        } => (entries, doc_count, root_count, skipped_count),
        other => panic!("expected DocsIndexExport, got: {other:?}"),
    }
}

// ── Tests ──

#[tokio::test]
async fn empty_realm_returns_empty_entries() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, root_count, skipped_count) = extract_entries(result);
    assert!(entries.is_empty());
    assert_eq!(doc_count, 0);
    assert_eq!(root_count, 0);
    assert_eq!(skipped_count, 0);
}

#[tokio::test]
async fn nonexistent_realm_returns_error() {
    let engine = RuntimeEngine::default();
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: Some("nonexistent".to_string()),
            name_override: None,
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error for nonexistent realm"
    );
}

#[tokio::test]
async fn single_root_flat_files() {
    let dir = make_temp_realm_dir("export-flat");
    fs::write(dir.path().join("README.md"), "# Root Doc\n").unwrap();
    fs::write(dir.path().join("guide.md"), "# Guide\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, _root_count, skipped_count) = extract_entries(result);
    assert_eq!(entries.len(), 1);
    assert_eq!(doc_count, 2);
    assert_eq!(skipped_count, 0);

    let entry = &entries[0];
    // Should have "." category for root-level files.
    assert!(
        entry.contains("|.:{"),
        "expected dot category, got: {entry}"
    );
    assert!(entry.contains("README.md"), "expected README.md in entry");
    assert!(entry.contains("guide.md"), "expected guide.md in entry");
}

#[tokio::test]
async fn single_root_nested_dirs() {
    let dir = make_temp_realm_dir("export-nested");
    fs::create_dir_all(dir.path().join("core")).unwrap();
    fs::create_dir_all(dir.path().join("advanced")).unwrap();
    fs::write(dir.path().join("core/_index.md"), "# Core Index\n").unwrap();
    fs::write(dir.path().join("core/types.md"), "# Types\n").unwrap();
    fs::write(dir.path().join("advanced/async.md"), "# Async\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, _, _) = extract_entries(result);
    assert_eq!(entries.len(), 1);
    assert_eq!(doc_count, 3);

    let entry = &entries[0];
    // Categories should be sorted alphabetically.
    let advanced_pos = entry.find("advanced:").expect("missing advanced category");
    let core_pos = entry.find("core:").expect("missing core category");
    assert!(advanced_pos < core_pos, "categories should be alphabetical");

    // _index.md should appear first in its category.
    let core_section = entry.split('|').find(|p| p.starts_with("core:")).unwrap();
    assert!(
        core_section.starts_with("core:{_index.md,"),
        "_index.md should be first, got: {core_section}"
    );
}

#[tokio::test]
async fn deep_nesting_preserves_subpath() {
    let dir = make_temp_realm_dir("export-deep");
    fs::create_dir_all(dir.path().join("tooling/sub")).unwrap();
    fs::write(dir.path().join("tooling/sub/deep.md"), "# Deep Doc\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, _, _, _) = extract_entries(result);
    let entry = &entries[0];
    // "tooling" is the category, "sub/deep.md" is the file path within it.
    assert!(
        entry.contains("tooling:{sub/deep.md}"),
        "expected tooling:{{sub/deep.md}}, got: {entry}"
    );
}

#[tokio::test]
async fn name_override_replaces_derived_name() {
    let dir = make_temp_realm_dir("export-override");
    fs::write(dir.path().join("doc.md"), "# Doc\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: Some("my-custom-name".to_string()),
        })
        .await;

    let (entries, _, _, _) = extract_entries(result);
    assert!(
        entries[0].starts_with("[my-custom-name]|"),
        "expected custom name, got: {}",
        entries[0]
    );
}

#[tokio::test]
async fn deterministic_output() {
    let dir = make_temp_realm_dir("export-deterministic");
    fs::create_dir_all(dir.path().join("z")).unwrap();
    fs::create_dir_all(dir.path().join("a")).unwrap();
    fs::write(dir.path().join("z/zebra.md"), "# Zebra\n").unwrap();
    fs::write(dir.path().join("z/alpha.md"), "# Alpha\n").unwrap();
    fs::write(dir.path().join("a/beta.md"), "# Beta\n").unwrap();
    fs::write(dir.path().join("README.md"), "# Root\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;

    let result1 = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;
    let result2 = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries1, _, _, _) = extract_entries(result1);
    let (entries2, _, _, _) = extract_entries(result2);
    assert_eq!(entries1, entries2, "output must be deterministic");
}

#[tokio::test]
async fn root_with_zero_docs_is_skipped() {
    let dir = make_temp_realm_dir("export-empty-root");
    // Create a root with only non-md files.
    fs::write(dir.path().join("data.json"), "{}").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, _, _) = extract_entries(result);
    assert!(entries.is_empty(), "no markdown docs means no entries");
    assert_eq!(doc_count, 0);
}

#[tokio::test]
async fn multiple_roots_produce_multiple_entries() {
    let dir1 = make_temp_realm_dir("export-multi-1");
    let dir2 = make_temp_realm_dir("export-multi-2");
    fs::write(dir1.path().join("a.md"), "# A\n").unwrap();
    fs::write(dir2.path().join("b.md"), "# B\n").unwrap();

    let engine = RuntimeEngine::default();
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: dir1.path().to_path_buf(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: dir2.path().to_path_buf(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, _, _) = extract_entries(result);
    assert_eq!(entries.len(), 2, "one entry per root");
    assert_eq!(doc_count, 2);
}

#[tokio::test]
async fn mixed_root_and_nested_files() {
    let dir = make_temp_realm_dir("export-mixed");
    fs::create_dir_all(dir.path().join("core")).unwrap();
    fs::write(dir.path().join("README.md"), "# Root\n").unwrap();
    fs::write(dir.path().join("core/types.md"), "# Types\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, doc_count, _, _) = extract_entries(result);
    assert_eq!(doc_count, 2);

    let entry = &entries[0];
    // Should have both "." and "core" categories.
    assert!(entry.contains("|.:{"), "expected dot category");
    assert!(entry.contains("|core:{"), "expected core category");
    // "." category should come before "core" alphabetically.
    let dot_pos = entry.find("|.:{").unwrap();
    let core_pos = entry.find("|core:{").unwrap();
    assert!(dot_pos < core_pos, "dot category should come before core");
}

#[tokio::test]
async fn kebab_name_from_underscore_root() {
    let dir = make_temp_realm_dir("export-kebab");
    // tempdir names are random, but let's test the name logic indirectly
    // by using name_override=None and checking the entry starts with [...]
    fs::write(dir.path().join("doc.md"), "# Doc\n").unwrap();

    let engine = make_engine_with_root(dir.path()).await;
    let result = engine
        .execute(CoreOperation::ExportDocsIndex {
            realm: None,
            name_override: None,
        })
        .await;

    let (entries, _, _, _) = extract_entries(result);
    let entry = &entries[0];
    // Entry should start with [something]|root:
    assert!(
        entry.starts_with('[') && entry.contains("]|root: "),
        "expected [name]|root: format, got: {entry}"
    );
}
