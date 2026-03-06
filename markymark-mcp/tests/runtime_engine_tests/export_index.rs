use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::DocumentUri;
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

#[tokio::test]
async fn export_index_returns_full_document_data() {
    let ws = TempWorkspace::new("export-index");
    let doc = ws.root().join("notes.md");
    fs::write(
        &doc,
        "# Introduction\n\n## Details\n\n<agent>stuff</agent>\n\n[[other-page#section]]\n\n[Click](https://example.com)\n",
    )
    .expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: false,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport {
            uri: result_uri,
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
            ..
        } => {
            assert_eq!(result_uri.as_str(), uri.as_str());
            assert_eq!(headings.len(), 2);
            assert_eq!(headings[0].0, "Introduction");
            assert_eq!(headings[0].1, 1); // level
            assert_eq!(headings[1].0, "Details");
            assert_eq!(headings[1].1, 2); // level
            assert!(!xml_tags.is_empty(), "expected at least 1 XML tag");
            assert_eq!(xml_tags[0].0, "agent");
            assert_eq!(wiki_links.len(), 1);
            assert_eq!(wiki_links[0].0, "other-page");
            assert_eq!(wiki_links[0].1, Some("section".to_string()));
            assert_eq!(markdown_links.len(), 1);
            assert_eq!(markdown_links[0].0, "Click");
            assert_eq!(markdown_links[0].1, "https://example.com");
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_errors_for_unindexed_document() {
    let ws = TempWorkspace::new("export-index-missing");
    fs::write(ws.root().join("a.md"), "# A\n").expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: DocumentUri::from_file_path(&ws.root().join("nonexistent.md")),
            realm: None,
            include_blocks: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error result, got: {other:?}"),
    }
}

#[tokio::test]
async fn workspace_with_mixed_formats_indexes_all_supported_types() {
    let ws = TempWorkspace::new("mixed-formats");
    fs::write(ws.root().join("notes.md"), "# Notes\n").expect("md should be created");
    fs::write(ws.root().join("config.json"), r#"{"key": "val"}"#).expect("json should be created");
    fs::write(ws.root().join("settings.yaml"), "key: val\n").expect("yaml should be created");
    fs::write(ws.root().join(".env"), "DB_HOST=localhost\n").expect("env should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::RealmStats {
            realm: "default".to_string(),
            check_duplicates: false,
            include_token_counts: false,
        })
        .await;

    match result {
        CoreOperationResult::RealmStats {
            document_count,
            heading_count,
            structured_doc_count,
            key_path_count,
            ..
        } => {
            assert_eq!(document_count, 4, "1 markdown + 3 structured docs");
            assert_eq!(heading_count, 1, "one heading from notes.md");
            assert_eq!(structured_doc_count, 3, "json + yaml + .env");
            assert!(key_path_count >= 3, "at least one key per structured doc");
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_returns_empty_lists_for_minimal_document() {
    let ws = TempWorkspace::new("export-index-minimal");
    let doc = ws.root().join("empty.md");
    fs::write(&doc, "Just some text with no structure.\n").expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: false,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport {
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
            ..
        } => {
            assert!(headings.is_empty());
            assert!(xml_tags.is_empty());
            assert!(wiki_links.is_empty());
            assert!(markdown_links.is_empty());
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_includes_frontmatter() {
    let ws = TempWorkspace::new("export-index-frontmatter");
    let doc = ws.root().join("with-frontmatter.md");
    fs::write(
        &doc,
        "---\nstatus: active\ntags: [rust, mcp]\n---\n# My Doc\n",
    )
    .expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: false,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport {
            frontmatter,
            headings,
            ..
        } => {
            // Verify the heading is present (sanity check)
            assert_eq!(headings.len(), 1);
            assert_eq!(headings[0].0, "My Doc");

            // Verify frontmatter is populated
            assert!(
                !frontmatter.is_empty(),
                "frontmatter should not be empty for a document with YAML frontmatter"
            );

            // Find the 'status' key — scalar value wrapped as single-element vec
            let status_entry = frontmatter
                .iter()
                .find(|(k, _)| k == "status")
                .expect("frontmatter should contain 'status' key");
            assert_eq!(status_entry.1, vec!["active"]);

            // Find the 'tags' key — list value preserved as multi-element vec
            let tags_entry = frontmatter
                .iter()
                .find(|(k, _)| k == "tags")
                .expect("frontmatter should contain 'tags' key");
            assert_eq!(tags_entry.1, vec!["rust", "mcp"]);
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_with_include_blocks_returns_content_blocks() {
    let ws = TempWorkspace::new("export-index-include-blocks");
    let doc = ws.root().join("blocks.md");
    fs::write(
        &doc,
        "# Heading\n\nA paragraph under the heading.\n\n- List item one\n- List item two\n",
    )
    .expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: true,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport {
            content_blocks,
            headings,
            ..
        } => {
            assert_eq!(headings.len(), 1, "should have one heading");
            let blocks = content_blocks.expect("include_blocks=true should produce Some(blocks)");
            assert!(
                !blocks.is_empty(),
                "document with paragraphs and lists should have content blocks"
            );
            // Verify block kinds are present
            let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
            assert!(
                kinds.contains(&"paragraph"),
                "should contain a paragraph block, got: {kinds:?}"
            );
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_without_include_blocks_omits_content_blocks() {
    let ws = TempWorkspace::new("export-index-no-blocks");
    let doc = ws.root().join("blocks.md");
    fs::write(&doc, "# Heading\n\nA paragraph.\n").expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: false,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport { content_blocks, .. } => {
            assert!(
                content_blocks.is_none(),
                "include_blocks=false should produce None, got: {content_blocks:?}"
            );
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[tokio::test]
async fn export_index_structured_doc_with_include_blocks_returns_none() {
    let ws = TempWorkspace::new("export-index-structured-blocks");
    let doc = ws.root().join("config.json");
    fs::write(&doc, r#"{"key": "value", "nested": {"a": 1}}"#).expect("doc should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: None,
            include_blocks: true,
        })
        .await;

    match result {
        CoreOperationResult::DocumentExport { content_blocks, .. } => {
            assert!(
                content_blocks.is_none(),
                "structured documents should return None for content_blocks, got: {content_blocks:?}"
            );
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}
