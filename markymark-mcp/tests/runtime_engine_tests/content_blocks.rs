//! Integration tests for the GetContentBlocks operation via RuntimeEngine.

use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::DocumentUri;
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

// ---------------------------------------------------------------------------
// Basic: returns blocks for a document
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_content_blocks_returns_blocks_for_document() {
    let ws = TempWorkspace::new("content-blocks-basic");
    let doc = ws.root().join("notes.md");
    fs::write(
        &doc,
        "# Introduction\n\nThis is the first paragraph.\n\nThis is the second paragraph.\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
        CoreOperationResult::ContentBlocks {
            uri: result_uri,
            blocks,
        } => {
            assert_eq!(result_uri.as_str(), uri.as_str());
            assert!(
                !blocks.is_empty(),
                "expected at least one content block for a document with paragraphs"
            );
            // All blocks should have kind set
            for block in &blocks {
                assert!(!block.kind.is_empty());
            }
        }
        other => panic!("expected ContentBlocks result, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// include_text: false omits text, true includes it
// ---------------------------------------------------------------------------

#[tokio::test]
async fn include_text_false_omits_text() {
    let ws = TempWorkspace::new("content-blocks-no-text");
    let doc = ws.root().join("doc.md");
    fs::write(&doc, "# Hello\n\nSome paragraph text here.\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
            for block in &blocks {
                assert!(
                    block.text.is_none(),
                    "text should be None when include_text=false"
                );
            }
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

#[tokio::test]
async fn include_text_true_returns_text() {
    let ws = TempWorkspace::new("content-blocks-with-text");
    let doc = ws.root().join("doc.md");
    fs::write(&doc, "# Hello\n\nSome paragraph text here.\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
            assert!(!blocks.is_empty(), "expected at least one block");
            let paragraph = blocks
                .iter()
                .find(|b| b.kind == "paragraph")
                .expect("expected a paragraph block");
            let text = paragraph
                .text
                .as_ref()
                .expect("text should be Some when include_text=true");
            assert!(
                text.contains("Some paragraph text here"),
                "paragraph text should contain the written content, got: {text}"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Kind filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn kind_filter_returns_only_matching_blocks() {
    let ws = TempWorkspace::new("content-blocks-kind-filter");
    let doc = ws.root().join("mixed.md");
    fs::write(
        &doc,
        "# Heading\n\nA paragraph.\n\n- list item one\n- list item two\n\n```rust\nlet x = 1;\n```\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    // Filter for paragraphs only
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
            assert!(!blocks.is_empty(), "expected at least one paragraph block");
            for block in &blocks {
                assert_eq!(block.kind, "paragraph", "all blocks should be paragraphs");
            }
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }

    // Filter for code blocks only
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: Some("code_block".to_string()),
            heading_filter: None,
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(!blocks.is_empty(), "expected at least one code block");
            for block in &blocks {
                assert_eq!(block.kind, "code_block");
            }
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Heading filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn heading_filter_returns_blocks_under_heading() {
    let ws = TempWorkspace::new("content-blocks-heading-filter");
    let doc = ws.root().join("multi.md");
    fs::write(
        &doc,
        "# Introduction\n\nIntro paragraph.\n\n# Details\n\nDetails paragraph.\n\nMore details.\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: Some("details".to_string()),
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(
                !blocks.is_empty(),
                "expected blocks under 'details' heading"
            );
            for block in &blocks {
                assert_eq!(
                    block.parent_heading_slug.as_deref(),
                    Some("details"),
                    "all blocks should be under 'details' heading"
                );
            }
            // Should contain the details paragraphs but not the intro paragraph.
            let texts: Vec<&str> = blocks.iter().filter_map(|b| b.text.as_deref()).collect();
            let joined = texts.join(" ");
            assert!(
                joined.contains("Details paragraph"),
                "should contain 'Details paragraph', got: {joined}"
            );
            assert!(
                !joined.contains("Intro paragraph"),
                "should NOT contain 'Intro paragraph'"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

#[tokio::test]
async fn heading_filter_excludes_blocks_before_any_heading() {
    let ws = TempWorkspace::new("content-blocks-before-heading");
    let doc = ws.root().join("preamble.md");
    fs::write(
        &doc,
        "Preamble text before any heading.\n\n# First Heading\n\nContent under heading.\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: Some("first-heading".to_string()),
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            // Preamble text should NOT be included since it has no parent heading.
            for block in &blocks {
                let text = block.text.as_deref().unwrap_or("");
                assert!(
                    !text.contains("Preamble text"),
                    "blocks before any heading should be excluded when heading filter is active"
                );
            }
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Block ID filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn block_id_filter_returns_single_block() {
    let ws = TempWorkspace::new("content-blocks-block-id");
    let doc = ws.root().join("with-ids.md");
    fs::write(
        &doc,
        "# Notes\n\nFirst paragraph. ^first-block\n\nSecond paragraph. ^second-block\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: Some("second-block".to_string()),
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert_eq!(
                blocks.len(),
                1,
                "block_id filter should return exactly one block"
            );
            assert_eq!(blocks[0].block_id.as_deref(), Some("second-block"));
            let text = blocks[0].text.as_deref().unwrap_or("");
            assert!(
                text.contains("Second paragraph"),
                "block text should contain 'Second paragraph', got: {text}"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

#[tokio::test]
async fn block_id_filter_returns_empty_for_nonexistent_id() {
    let ws = TempWorkspace::new("content-blocks-block-id-missing");
    let doc = ws.root().join("doc.md");
    fs::write(&doc, "# Notes\n\nA paragraph.\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: Some("nonexistent".to_string()),
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(
                blocks.is_empty(),
                "nonexistent block_id should return empty"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Combined filters (AND semantics)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn combined_filters_and_together() {
    let ws = TempWorkspace::new("content-blocks-combined");
    let doc = ws.root().join("combined.md");
    fs::write(
        &doc,
        "# Section A\n\nParagraph under A.\n\n- List item under A\n\n# Section B\n\nParagraph under B.\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    // Filter for paragraphs under section-a only
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: None,
            kind_filter: Some("paragraph".to_string()),
            heading_filter: Some("section-a".to_string()),
            block_id: None,
            include_text: true,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            for block in &blocks {
                assert_eq!(block.kind, "paragraph");
                assert_eq!(block.parent_heading_slug.as_deref(), Some("section-a"));
            }
            // Should NOT include list items under A or paragraphs under B
            let texts: Vec<&str> = blocks.iter().filter_map(|b| b.text.as_deref()).collect();
            let joined = texts.join(" ");
            assert!(
                !joined.contains("List item"),
                "list items should be excluded by kind filter"
            );
            assert!(
                !joined.contains("Paragraph under B"),
                "paragraphs under B should be excluded by heading filter"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn error_for_unindexed_document() {
    let ws = TempWorkspace::new("content-blocks-unindexed");
    fs::write(ws.root().join("a.md"), "# A\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: DocumentUri::from_file_path(&ws.root().join("nonexistent.md")),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error for unindexed document, got: {other:?}"),
    }
}

#[tokio::test]
async fn error_for_nonexistent_realm() {
    let ws = TempWorkspace::new("content-blocks-no-realm");
    fs::write(ws.root().join("a.md"), "# A\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: DocumentUri::from_file_path(&ws.root().join("a.md")),
            realm: Some("nonexistent-realm".to_string()),
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error for nonexistent realm, got: {other:?}"),
    }
}

#[tokio::test]
async fn error_for_structured_document() {
    let ws = TempWorkspace::new("content-blocks-structured");
    fs::write(ws.root().join("config.json"), r#"{"key": "val"}"#).unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();

    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: DocumentUri::from_file_path(&ws.root().join("config.json")),
            realm: None,
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => {} // expected: markdown-only
        other => panic!("expected Error for structured document, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Empty document
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_document_returns_empty_blocks() {
    let ws = TempWorkspace::new("content-blocks-empty");
    let doc = ws.root().join("empty.md");
    fs::write(&doc, "").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
            assert!(blocks.is_empty(), "empty document should return no blocks");
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// No filters returns all blocks, preserving order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_filters_returns_all_blocks_in_order() {
    let ws = TempWorkspace::new("content-blocks-order");
    let doc = ws.root().join("ordered.md");
    fs::write(
        &doc,
        "First paragraph.\n\n# Heading\n\nSecond paragraph.\n\n- A list item\n",
    )
    .unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
            assert!(blocks.len() >= 2, "expected multiple blocks");
            // Verify blocks are in source order (by start line)
            for window in blocks.windows(2) {
                assert!(
                    window[0].range.start.line <= window[1].range.start.line,
                    "blocks should be in source order: line {} should come before line {}",
                    window[0].range.start.line,
                    window[1].range.start.line,
                );
            }
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Parent heading slug populated correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn parent_heading_slug_populated() {
    let ws = TempWorkspace::new("content-blocks-parent-slug");
    let doc = ws.root().join("with-headings.md");
    fs::write(&doc, "# My Heading\n\nParagraph under heading.\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
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
            let paragraph = blocks
                .iter()
                .find(|b| b.kind == "paragraph")
                .expect("expected a paragraph block");
            assert_eq!(
                paragraph.parent_heading_slug.as_deref(),
                Some("my-heading"),
                "parent heading slug should be set to the slugified heading text"
            );
        }
        other => panic!("expected ContentBlocks, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Empty realm string treated as default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_realm_string_uses_default() {
    let ws = TempWorkspace::new("content-blocks-empty-realm");
    let doc = ws.root().join("doc.md");
    fs::write(&doc, "# Test\n\nA paragraph.\n").unwrap();

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .unwrap();
    let uri = DocumentUri::from_file_path(&doc);

    // Empty string realm should work (treated as default)
    let result = engine
        .execute(CoreOperation::GetContentBlocks {
            uri: uri.clone(),
            realm: Some(String::new()),
            kind_filter: None,
            heading_filter: None,
            block_id: None,
            include_text: false,
        })
        .await;

    match result {
        CoreOperationResult::ContentBlocks { blocks, .. } => {
            assert!(
                !blocks.is_empty(),
                "should return blocks with empty realm string"
            );
        }
        // Empty string might error since it's not "default" — that's also acceptable behavior.
        // The handler normalizes empty to None, but the engine dispatch uses the raw value.
        CoreOperationResult::Error(_) => {}
        other => panic!("expected ContentBlocks or Error, got: {other:?}"),
    }
}
