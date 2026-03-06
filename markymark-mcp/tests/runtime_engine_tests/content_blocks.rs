//! Integration tests for `get-content-blocks` MCP tool.
//!
//! Part of epic marky-z7uc: expose ContentBlock model via MCP tools.
//! These tests exercise the RuntimeEngine dispatch path end-to-end.

use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::DocumentUri;
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

/// The most basic test: index a markdown file with paragraphs and verify
/// that content blocks are returned via the engine path.
///
/// This test deliberately targets the from_scan indexing gap — if the MCP
/// engine's markdown indexing path doesn't populate content blocks, this
/// test will fail with an empty blocks vec.
#[tokio::test]
async fn get_content_blocks_returns_blocks_for_markdown_document() {
    let ws = TempWorkspace::new("content-blocks");
    let doc = ws.root().join("test.md");
    fs::write(
        &doc,
        "# Introduction\n\nThis is a paragraph under the introduction.\n\n## Details\n\nAnother paragraph here.\n\n- List item one\n- List item two\n",
    )
    .expect("write test document");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { uri: result_uri, blocks } => {
            assert_eq!(result_uri.as_str(), uri.as_str());
            assert!(
                !blocks.is_empty(),
                "content blocks should be populated for a markdown document with paragraphs and lists"
            );
        }
        CoreOperationResult::Error(e) => {
            panic!("expected ContentBlocks result, got error: {e}");
        }
        other => panic!("expected ContentBlocks result, got: {other:?}"),
    }
}

/// Verify that include_text=true returns block text content.
#[tokio::test]
async fn get_content_blocks_with_include_text_returns_text() {
    let ws = TempWorkspace::new("content-blocks-text");
    let doc = ws.root().join("test.md");
    fs::write(&doc, "# Heading\n\nHello world paragraph.\n")
        .expect("write test document");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(!blocks.is_empty(), "should have at least one block");
            let has_text = blocks.iter().any(|b| b.text.is_some());
            assert!(has_text, "include_text=true should populate text field");
            let para = blocks.iter().find(|b| b.kind == "paragraph");
            assert!(para.is_some(), "should have a paragraph block");
            assert_eq!(
                para.unwrap().text.as_deref(),
                Some("Hello world paragraph.\n"),
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

/// Verify that include_text=false omits block text.
#[tokio::test]
async fn get_content_blocks_without_include_text_omits_text() {
    let ws = TempWorkspace::new("content-blocks-no-text");
    let doc = ws.root().join("test.md");
    fs::write(&doc, "# Heading\n\nParagraph content.\n")
        .expect("write test document");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(!blocks.is_empty(), "should have at least one block");
            let all_none = blocks.iter().all(|b| b.text.is_none());
            assert!(all_none, "include_text=false should omit all text fields");
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

/// Verify error for a document URI that isn't indexed.
#[tokio::test]
async fn get_content_blocks_errors_for_unknown_uri() {
    let ws = TempWorkspace::new("content-blocks-unknown");
    let doc = ws.root().join("exists.md");
    fs::write(&doc, "# Hello\n").expect("write test document");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let bogus_uri = DocumentUri::from_file_path(&ws.root().join("nonexistent.md"));
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: bogus_uri,
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("not found") || msg.contains("not indexed"),
                "error should indicate document not found, got: {msg}"
            );
        }
        other => panic!("expected Error result for unknown URI, got: {other:?}"),
    }
}

/// Verify kind_filter restricts returned blocks.
#[tokio::test]
async fn get_content_blocks_filters_by_kind() {
    let ws = TempWorkspace::new("content-blocks-kind-filter");
    let doc = ws.root().join("test.md");
    fs::write(
        &doc,
        "# Heading\n\nA paragraph.\n\n- A list item\n\n```rust\nfn main() {}\n```\n",
    )
    .expect("write test document");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: Some("paragraph".to_string()),
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(!blocks.is_empty(), "should have paragraph blocks");
            assert!(
                blocks.iter().all(|b| b.kind == "paragraph"),
                "all returned blocks should be paragraphs"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}
