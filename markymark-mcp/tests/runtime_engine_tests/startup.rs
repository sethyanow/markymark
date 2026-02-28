use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::DocumentUri;
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

#[tokio::test]
async fn rejects_empty_workspace_roots() {
    let err = match RuntimeEngine::from_workspace_roots(Vec::new()).await {
        Ok(_) => panic!("empty workspace roots should fail"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .contains("at least one workspace root is required"));
}

#[tokio::test]
async fn rejects_missing_workspace_root() {
    let dir = tempfile::TempDir::new().expect("temp dir should be created");
    let missing = dir.path().to_path_buf();
    drop(dir); // delete the directory so `missing` is a non-existent path

    let err = match RuntimeEngine::from_workspace_roots(vec![missing.clone()]).await {
        Ok(_) => panic!("missing workspace root should fail"),
        Err(err) => err,
    };
    assert!(err.to_string().contains(&format!(
        "workspace root does not exist: {}",
        missing.display()
    )));
}

#[tokio::test]
async fn rejects_workspace_root_file_path() {
    let ws = TempWorkspace::new("root-file");
    let file_path = ws.root().join("not-a-directory.md");
    fs::write(&file_path, "# Heading").expect("test file should be created");

    let err = match RuntimeEngine::from_workspace_roots(vec![file_path.clone()]).await {
        Ok(_) => panic!("workspace root file should fail"),
        Err(err) => err,
    };
    assert!(err.to_string().contains(&format!(
        "workspace root is not a directory: {}",
        file_path.display()
    )));
}

#[tokio::test]
async fn indexes_markdown_and_returns_deterministic_symbols() {
    let ws = TempWorkspace::new("indexed");
    let first = ws.root().join("a.md");
    let second = ws.root().join("b.md");
    fs::write(&first, "# Zebra\n## Alpha\n").expect("first markdown should be created");
    fs::write(&second, "# Beta\n").expect("second markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let outline = engine
        .execute(CoreOperation::GetOutline {
            uri: DocumentUri::from_file_path(&first),
            realm: None,
        })
        .await;
    match outline {
        CoreOperationResult::Outline(headings) => {
            assert_eq!(headings, vec!["Zebra".to_string(), "Alpha".to_string()]);
        }
        other => panic!("expected outline result, got: {other:?}"),
    }

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "a".to_string(),
            realm: None,
        })
        .await;
    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(
                names,
                vec!["Alpha".to_string(), "Beta".to_string(), "Zebra".to_string()]
            );
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}
