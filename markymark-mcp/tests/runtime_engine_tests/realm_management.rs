use std::fs;
use std::path::PathBuf;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

#[tokio::test]
async fn create_realm_returns_realm_info() {
    let ws = TempWorkspace::new("create-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::CreateRealm {
            name: "test-realm".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "test-realm");
            assert_eq!(root_count, 0);
            assert_eq!(document_count, 0);
        }
        other => panic!("expected RealmInfo, got: {other:?}"),
    }
}

#[tokio::test]
async fn create_realm_rejects_duplicate_name() {
    let ws = TempWorkspace::new("create-dup-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "my-realm".to_string(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::CreateRealm {
            name: "my-realm".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for duplicate realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn create_realm_rejects_empty_name() {
    let ws = TempWorkspace::new("create-empty-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::CreateRealm {
            name: "".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for empty realm name, got: {other:?}"),
    }
}

#[tokio::test]
async fn destroy_realm_removes_realm() {
    let ws = TempWorkspace::new("destroy-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "temp-realm".to_string(),
        })
        .await;
    let result = engine
        .execute(CoreOperation::DestroyRealm {
            name: "temp-realm".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::Ok => { /* expected */ }
        other => panic!("expected Ok for destroy, got: {other:?}"),
    }

    let result = engine
        .execute(CoreOperation::CreateRealm {
            name: "temp-realm".to_string(),
        })
        .await;
    match result {
        CoreOperationResult::RealmInfo { name, .. } => {
            assert_eq!(name, "temp-realm");
        }
        other => panic!("expected RealmInfo after re-create, got: {other:?}"),
    }
}

#[tokio::test]
async fn destroy_realm_rejects_unknown_name() {
    let ws = TempWorkspace::new("destroy-unknown-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::DestroyRealm {
            name: "nonexistent".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn destroy_realm_rejects_default_realm() {
    let ws = TempWorkspace::new("destroy-default-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::DestroyRealm {
            name: "default".to_string(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected: cannot destroy default realm */ }
        other => panic!("expected error for destroying default realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn add_root_indexes_markdown_files_in_realm() {
    let ws = TempWorkspace::new("add-root");
    let sub = ws.root().join("docs");
    fs::create_dir_all(&sub).expect("subdirectory should be created");
    fs::write(sub.join("a.md"), "# Alpha\n").expect("a.md should be created");
    fs::write(sub.join("b.md"), "# Beta\n").expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "docs-realm".to_string(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::AddRoot {
            realm: "docs-realm".to_string(),
            root: sub,
        })
        .await;

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "docs-realm");
            assert_eq!(root_count, 1);
            assert_eq!(document_count, 2);
        }
        other => panic!("expected RealmInfo, got: {other:?}"),
    }
}

#[tokio::test]
async fn add_root_rejects_unknown_realm() {
    let ws = TempWorkspace::new("add-root-unknown");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::AddRoot {
            realm: "nonexistent".to_string(),
            root: ws.root(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn add_root_rejects_invalid_path() {
    let ws = TempWorkspace::new("add-root-invalid");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: PathBuf::from("/nonexistent/path/to/nowhere"),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for invalid path, got: {other:?}"),
    }
}

#[tokio::test]
async fn add_root_rejects_duplicate_root() {
    let ws = TempWorkspace::new("add-root-dup");
    fs::write(ws.root().join("a.md"), "# A\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        })
        .await;
    let _ = engine
        .execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: ws.root(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: ws.root(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for duplicate root, got: {other:?}"),
    }
}

#[tokio::test]
async fn remove_root_unindexes_documents() {
    let ws = TempWorkspace::new("remove-root");
    let docs = ws.root().join("docs");
    fs::create_dir_all(&docs).expect("docs dir should be created");
    fs::write(docs.join("x.md"), "# X\n").expect("x.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        })
        .await;
    let _ = engine
        .execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: docs.clone(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::RemoveRoot {
            realm: "r".to_string(),
            root: docs,
        })
        .await;

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "r");
            assert_eq!(root_count, 0);
            assert_eq!(document_count, 0);
        }
        other => panic!("expected RealmInfo after remove, got: {other:?}"),
    }
}

#[tokio::test]
async fn remove_root_rejects_unknown_realm() {
    let ws = TempWorkspace::new("remove-root-unknown-realm");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::RemoveRoot {
            realm: "nonexistent".to_string(),
            root: ws.root(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn remove_root_rejects_untracked_root() {
    let ws = TempWorkspace::new("remove-root-untracked");
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let _ = engine
        .execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        })
        .await;

    let result = engine
        .execute(CoreOperation::RemoveRoot {
            realm: "r".to_string(),
            root: ws.root(),
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for untracked root, got: {other:?}"),
    }
}

#[tokio::test]
async fn skips_non_utf8_documents_without_failing_startup() {
    let ws = TempWorkspace::new("invalid-utf8");
    let good = ws.root().join("good.md");
    let bad = ws.root().join("bad.md");
    fs::write(&good, "# Intro\n").expect("valid markdown should be created");
    fs::write(&bad, [0xFFu8, 0xFEu8, 0xFDu8]).expect("invalid utf8 markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let outline = engine
        .execute(CoreOperation::GetOutline {
            uri: markymark_core::DocumentUri::from_file_path(&good),
            realm: None,
        })
        .await;
    match outline {
        CoreOperationResult::Outline(headings) => assert_eq!(headings, vec!["Intro"]),
        other => panic!("expected outline result, got: {other:?}"),
    }
}
