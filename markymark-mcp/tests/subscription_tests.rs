//! Integration tests for MCP resource subscription tracking.
//!
//! Tests the subscription tracker directly and the subscribe/unsubscribe
//! ServerHandler methods via MarkymarkMcp.

use std::path::Path;
use std::sync::Arc;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_mcp::MarkymarkMcp;

struct MockEngine;

impl CoreEngine for MockEngine {
    fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { .. } => {
                CoreOperationResult::Outline(vec!["Test".to_string()])
            }
            CoreOperation::SearchSymbols { query, .. } => CoreOperationResult::Symbols(vec![(
                format!("match-{query}"),
                DocumentUri::from_file_path(Path::new("/vault/a.md")),
                Range::new(Position::new(0, 0), Position::new(0, 10)),
            )]),
            _ => CoreOperationResult::Error(CoreError::Message("not mocked".to_string())),
        }
    }
}

fn make_mcp() -> MarkymarkMcp {
    MarkymarkMcp::new(Arc::new(MockEngine))
}

// --- SubscriptionTracker unit-style tests via MarkymarkMcp public API ---

#[test]
fn initially_has_no_subscriptions() {
    let mcp = make_mcp();
    assert_eq!(mcp.subscription_count(), 0);
}

#[test]
fn subscribe_adds_uri_to_tracked_set() {
    let mcp = make_mcp();
    mcp.track_subscription("markymark://symbols?query=test".to_string());
    assert!(mcp.is_subscribed("markymark://symbols?query=test"));
    assert_eq!(mcp.subscription_count(), 1);
}

#[test]
fn unsubscribe_removes_uri_from_tracked_set() {
    let mcp = make_mcp();
    mcp.track_subscription("markymark://symbols?query=test".to_string());
    assert!(mcp.untrack_subscription("markymark://symbols?query=test"));
    assert!(!mcp.is_subscribed("markymark://symbols?query=test"));
    assert_eq!(mcp.subscription_count(), 0);
}

#[test]
fn unsubscribe_returns_false_for_unknown_uri() {
    let mcp = make_mcp();
    assert!(!mcp.untrack_subscription("markymark://unknown"));
}

#[test]
fn multiple_subscriptions_tracked_independently() {
    let mcp = make_mcp();
    mcp.track_subscription("markymark://symbols?query=a".to_string());
    mcp.track_subscription("markymark://outline/file:///a.md".to_string());
    mcp.track_subscription("markymark://dependency-graph?realm=default&format=json".to_string());
    assert_eq!(mcp.subscription_count(), 3);

    mcp.untrack_subscription("markymark://outline/file:///a.md");
    assert_eq!(mcp.subscription_count(), 2);
    assert!(mcp.is_subscribed("markymark://symbols?query=a"));
    assert!(!mcp.is_subscribed("markymark://outline/file:///a.md"));
}

#[test]
fn duplicate_subscribe_is_idempotent() {
    let mcp = make_mcp();
    mcp.track_subscription("markymark://symbols?query=test".to_string());
    mcp.track_subscription("markymark://symbols?query=test".to_string());
    assert_eq!(mcp.subscription_count(), 1);
}

// --- ServerHandler capability tests ---

#[test]
fn server_info_advertises_subscribe_capability() {
    use rmcp::ServerHandler;
    let mcp = make_mcp();
    let info = mcp.get_info();
    let resources = info
        .capabilities
        .resources
        .expect("resources capability should be present");
    assert_eq!(
        resources.subscribe,
        Some(true),
        "server should advertise subscribe capability"
    );
}
