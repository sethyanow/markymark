//! Tests for the `get-outline` engine operation.
//!
//! Covers flat + tree formats, include_text toggling, structured doc
//! fallback, unicode, and named-realm routing (marky-bgtt).

use super::*;

#[tokio::test]
async fn get_outline_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
    let engine = make_engine_with_custom_realm("my-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    // Should fail without realm (default realm has no such doc)
    let result = engine
        .execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: None,
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error when querying default realm, got {result:?}"
    );

    // Should succeed with the correct realm
    let result = engine
        .execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: Some("my-realm".to_string()),
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "expected Outline from named realm, got {result:?}"
    );
}

#[tokio::test]
async fn outline_flat_format_backward_compat() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
    let engine = make_engine_with_custom_realm("flat-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("flat-realm".to_string()),
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::Outline(headings) => {
            assert_eq!(headings, vec!["Hello World", "Section"]);
        }
        other => panic!("expected flat Outline, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_nested_hierarchy() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# Root\n\n## Child\n\n### Grandchild\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("tree-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("tree-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            // Root node (level 0, no heading)
            assert_eq!(tree.title, "");
            assert_eq!(tree.level, 0);
            assert_eq!(tree.children.len(), 1, "root should have 1 h1 child");

            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Root");
            assert_eq!(h1.level, 1);
            assert_eq!(h1.children.len(), 1, "h1 should have 1 h2 child");

            let h2 = &h1.children[0];
            assert_eq!(h2.title, "Child");
            assert_eq!(h2.level, 2);
            assert_eq!(h2.children.len(), 1, "h2 should have 1 h3 child");

            let h3 = &h2.children[0];
            assert_eq!(h3.title, "Grandchild");
            assert_eq!(h3.level, 3);
            assert!(h3.children.is_empty());
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_skipped_levels() {
    // h1 followed by h3 (no h2) should still produce correct hierarchy
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# Top\n\n### Deep\n\n## Middle\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("skip-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("skip-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.children.len(), 1, "root should have 1 h1 child");
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Top");
            // h3 is child of h1, h2 is sibling of h3 (comes after, higher level)
            assert_eq!(h1.children.len(), 2, "h1 should have h3 and h2 as children");
            assert_eq!(h1.children[0].title, "Deep");
            assert_eq!(h1.children[0].level, 3);
            assert_eq!(h1.children[1].title, "Middle");
            assert_eq!(h1.children[1].level, 2);
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_no_headings() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "Just some text, no headings.\n").unwrap();
    let engine = make_engine_with_custom_realm("empty-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("empty-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.title, "");
            assert_eq!(tree.level, 0);
            assert!(tree.children.is_empty(), "no headings = no children");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_root_node_no_heading() {
    // Root OutlineNode has heading=None; verify it serializes correctly
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Title\n").unwrap();
    let engine = make_engine_with_custom_realm("root-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("root-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.title, "", "root node should have empty title");
            assert_eq!(tree.level, 0, "root node should be level 0");
            assert!(tree.text.is_none(), "no include_text = no text field");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_include_text_false_omits_field() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Title\n\nSome content here.\n").unwrap();
    let engine = make_engine_with_custom_realm("notext-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("notext-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert!(tree.text.is_none(), "root should have no text");
            let h1 = &tree.children[0];
            assert!(
                h1.text.is_none(),
                "h1 should have no text when include_text=false"
            );
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_include_text_true_inlines_content() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# Title\n\nParagraph one.\n\n## Section\n\nParagraph two.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("text-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("text-realm".to_string()),
            format: "tree".to_string(),
            include_text: true,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Title");
            let text = h1.text.as_ref().expect("h1 should have text");
            assert!(
                text.contains("Paragraph one."),
                "h1 text should contain paragraph one, got: {text}"
            );
            // h1 text should NOT contain h2's content (section boundary)
            assert!(
                !text.contains("Paragraph two."),
                "h1 text should not contain paragraph two"
            );

            let h2 = &h1.children[0];
            assert_eq!(h2.title, "Section");
            let text2 = h2.text.as_ref().expect("h2 should have text");
            assert!(
                text2.contains("Paragraph two."),
                "h2 text should contain paragraph two, got: {text2}"
            );
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_structured_doc_tree_fallback_to_flat() {
    // JSON/YAML documents should return flat format even when tree requested
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("config.json"),
        r#"{"key": "value", "nested": {"inner": 42}}"#,
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("struct-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("struct-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    // Structured docs should fall back to flat format
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "structured doc with format=tree should fall back to flat, got {result:?}"
    );
}

#[tokio::test]
async fn outline_unicode_headings() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# 日本語タイトル\n\n## Über-docs\n\n### 🎉 Emoji\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("unicode-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("unicode-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.children.len(), 1);
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "日本語タイトル");
            assert_eq!(h1.children[0].title, "Über-docs");
            assert_eq!(h1.children[0].children[0].title, "🎉 Emoji");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}
