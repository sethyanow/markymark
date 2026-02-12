//! Integration tests for MarkymarkMcp prompt handlers.
//!
//! These tests validate the MCP prompt handler layer using a MockEngine
//! that returns canned responses for each CoreOperation variant.

use std::path::Path;
use std::sync::Arc;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_mcp::MarkymarkMcp;
use rmcp::model::{PromptMessageContent, PromptMessageRole};
use rmcp::ServerHandler;
use serde_json::json;

struct MockEngine;

impl CoreEngine for MockEngine {
    fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { .. } => {
                CoreOperationResult::Outline(vec!["Introduction".to_string(), "Setup".to_string()])
            }
            CoreOperation::SearchSymbols { query } => CoreOperationResult::Symbols(vec![(
                format!("{query}-match"),
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                Range::new(Position::new(0, 0), Position::new(0, 10)),
            )]),
            CoreOperation::FindReferences { .. } => CoreOperationResult::Locations(vec![
                (
                    DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                    Range::new(Position::new(1, 0), Position::new(1, 5)),
                ),
                (
                    DocumentUri::from_file_path(Path::new("/vault/other.md")),
                    Range::new(Position::new(3, 2), Position::new(3, 7)),
                ),
            ]),
            CoreOperation::ExportIndex { uri } => CoreOperationResult::DocumentExport {
                uri,
                headings: vec![
                    (
                        "Introduction".to_string(),
                        1,
                        Range::new(Position::new(0, 0), Position::new(0, 16)),
                    ),
                    (
                        "Setup".to_string(),
                        2,
                        Range::new(Position::new(5, 0), Position::new(5, 9)),
                    ),
                ],
                xml_tags: vec![],
                wiki_links: vec![(
                    "other-page".to_string(),
                    Some("section".to_string()),
                    Range::new(Position::new(2, 0), Position::new(2, 25)),
                )],
                markdown_links: vec![(
                    "example".to_string(),
                    "https://example.com".to_string(),
                    Range::new(Position::new(3, 0), Position::new(3, 30)),
                )],
            },
            _ => CoreOperationResult::Error(CoreError::NotImplemented(
                "not needed for prompt tests".to_string(),
            )),
        }
    }
}

fn make_mcp() -> MarkymarkMcp {
    MarkymarkMcp::new(Arc::new(MockEngine))
}

// ---------------------------------------------------------------------------
// prompts/list tests
// ---------------------------------------------------------------------------

#[test]
fn list_prompts_returns_two_prompts() {
    let mcp = make_mcp();
    let prompts = mcp.list_prompt_definitions();
    assert_eq!(prompts.len(), 2, "expected exactly 2 prompts");

    let names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();
    assert!(
        names.contains(&"explain-link"),
        "missing explain-link prompt"
    );
    assert!(
        names.contains(&"suggest-references"),
        "missing suggest-references prompt"
    );
}

#[test]
fn explain_link_prompt_has_required_arguments() {
    let mcp = make_mcp();
    let prompts = mcp.list_prompt_definitions();
    let explain = prompts
        .iter()
        .find(|p| p.name == "explain-link")
        .expect("explain-link not found");

    let args = explain.arguments.as_ref().expect("should have arguments");
    assert_eq!(args.len(), 2);

    let uri_arg = args.iter().find(|a| a.name == "uri").expect("missing uri");
    assert_eq!(uri_arg.required, Some(true));

    let target_arg = args
        .iter()
        .find(|a| a.name == "target")
        .expect("missing target");
    assert_eq!(target_arg.required, Some(true));
}

#[test]
fn suggest_references_prompt_has_required_arguments() {
    let mcp = make_mcp();
    let prompts = mcp.list_prompt_definitions();
    let suggest = prompts
        .iter()
        .find(|p| p.name == "suggest-references")
        .expect("suggest-references not found");

    let args = suggest.arguments.as_ref().expect("should have arguments");
    assert_eq!(args.len(), 3);

    let uri_arg = args.iter().find(|a| a.name == "uri").expect("missing uri");
    assert_eq!(uri_arg.required, Some(true));

    let line_arg = args
        .iter()
        .find(|a| a.name == "line")
        .expect("missing line");
    assert_eq!(line_arg.required, Some(true));

    let char_arg = args
        .iter()
        .find(|a| a.name == "character")
        .expect("missing character");
    assert_eq!(char_arg.required, Some(true));
}

// ---------------------------------------------------------------------------
// prompts/get: explain-link
// ---------------------------------------------------------------------------

#[test]
fn explain_link_returns_user_message_with_document_context() {
    let mcp = make_mcp();
    let args = json!({
        "uri": "file:///vault/notes.md",
        "target": "other-page#section"
    });
    let result = mcp
        .get_prompt_by_name("explain-link", Some(args.as_object().unwrap().clone()))
        .expect("prompt should succeed");

    assert!(result.description.is_some());
    assert!(
        !result.messages.is_empty(),
        "should return at least one message"
    );

    // First message should be user role
    assert_eq!(result.messages[0].role, PromptMessageRole::User);

    // Message text should contain the URI and target
    let text = prompt_text(&result.messages[0].content);
    assert!(
        text.contains("file:///vault/notes.md"),
        "should include document URI in prompt"
    );
    assert!(
        text.contains("other-page#section"),
        "should include link target in prompt"
    );
    // Should include document context from export-index
    assert!(
        text.contains("Introduction"),
        "should include heading context from document"
    );
}

#[test]
fn explain_link_fails_on_missing_uri() {
    let mcp = make_mcp();
    let args = json!({ "target": "other-page" });
    let result = mcp.get_prompt_by_name("explain-link", Some(args.as_object().unwrap().clone()));
    assert!(result.is_err(), "should fail without uri argument");
}

#[test]
fn explain_link_fails_on_missing_target() {
    let mcp = make_mcp();
    let args = json!({ "uri": "file:///vault/notes.md" });
    let result = mcp.get_prompt_by_name("explain-link", Some(args.as_object().unwrap().clone()));
    assert!(result.is_err(), "should fail without target argument");
}

#[test]
fn explain_link_fails_on_non_file_uri() {
    let mcp = make_mcp();
    let args = json!({
        "uri": "https://example.com/notes.md",
        "target": "heading"
    });
    let result = mcp.get_prompt_by_name("explain-link", Some(args.as_object().unwrap().clone()));
    assert!(result.is_err(), "should fail with non-file URI");
}

// ---------------------------------------------------------------------------
// prompts/get: suggest-references
// ---------------------------------------------------------------------------

#[test]
fn suggest_references_returns_user_message_with_symbol_context() {
    let mcp = make_mcp();
    let args = json!({
        "uri": "file:///vault/notes.md",
        "line": 0,
        "character": 5
    });
    let result = mcp
        .get_prompt_by_name(
            "suggest-references",
            Some(args.as_object().unwrap().clone()),
        )
        .expect("prompt should succeed");

    assert!(result.description.is_some());
    assert!(!result.messages.is_empty());

    assert_eq!(result.messages[0].role, PromptMessageRole::User);

    let text = prompt_text(&result.messages[0].content);
    assert!(
        text.contains("file:///vault/notes.md"),
        "should include document URI"
    );
    // Should include reference locations from find-references
    assert!(
        text.contains("notes.md") || text.contains("other.md"),
        "should include reference context from the realm"
    );
}

#[test]
fn suggest_references_fails_on_missing_uri() {
    let mcp = make_mcp();
    let args = json!({ "line": 0, "character": 5 });
    let result = mcp.get_prompt_by_name(
        "suggest-references",
        Some(args.as_object().unwrap().clone()),
    );
    assert!(result.is_err(), "should fail without uri argument");
}

#[test]
fn suggest_references_fails_on_missing_line() {
    let mcp = make_mcp();
    let args = json!({
        "uri": "file:///vault/notes.md",
        "character": 5
    });
    let result = mcp.get_prompt_by_name(
        "suggest-references",
        Some(args.as_object().unwrap().clone()),
    );
    assert!(result.is_err(), "should fail without line argument");
}

#[test]
fn suggest_references_fails_on_non_file_uri() {
    let mcp = make_mcp();
    let args = json!({
        "uri": "https://example.com",
        "line": 0,
        "character": 0
    });
    let result = mcp.get_prompt_by_name(
        "suggest-references",
        Some(args.as_object().unwrap().clone()),
    );
    assert!(result.is_err(), "should fail with non-file URI");
}

// ---------------------------------------------------------------------------
// unknown prompt
// ---------------------------------------------------------------------------

#[test]
fn get_prompt_fails_on_unknown_name() {
    let mcp = make_mcp();
    let result = mcp.get_prompt_by_name("nonexistent", None);
    assert!(result.is_err(), "should fail on unknown prompt name");
}

// ---------------------------------------------------------------------------
// capabilities
// ---------------------------------------------------------------------------

#[test]
fn server_capabilities_include_prompts() {
    let mcp = make_mcp();
    let info = mcp.get_info();
    assert!(
        info.capabilities.prompts.is_some(),
        "server capabilities should include prompts"
    );
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn prompt_text(content: &PromptMessageContent) -> String {
    match content {
        PromptMessageContent::Text { text } => text.clone(),
        _ => panic!("expected text content in prompt message"),
    }
}
