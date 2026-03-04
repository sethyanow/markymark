use markymark_core::{DocumentUri, EdgeKind, GraphNode};
use markymark_index::{ConnectionGraph, RefKind, SymbolId};

// ---------------------------------------------------------------------------
// Empty graph
// ---------------------------------------------------------------------------

#[test]
fn test_empty_graph() {
    let graph = ConnectionGraph::new();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.unresolved_references().is_empty());
}

// ---------------------------------------------------------------------------
// Adding symbols
// ---------------------------------------------------------------------------

#[test]
fn test_add_document_symbol() {
    let mut graph = ConnectionGraph::new();
    let uri = DocumentUri::new("file:///notes/page-a.md").unwrap();
    let _id = graph.add_document(uri);
    assert_eq!(graph.node_count(), 1);
}

#[test]
fn test_add_heading_symbol() {
    let mut graph = ConnectionGraph::new();
    let uri = DocumentUri::new("file:///notes/page-a.md").unwrap();
    let _doc = graph.add_document(uri.clone());
    let _heading = graph.add_heading(uri, "introduction", "Introduction");
    assert_eq!(graph.node_count(), 2);
}

// ---------------------------------------------------------------------------
// References
// ---------------------------------------------------------------------------

#[test]
fn test_add_wiki_link_reference() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/page-a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/page-b.md").unwrap();

    let doc_a = graph.add_document(uri_a);
    let doc_b = graph.add_document(uri_b);

    graph.add_reference(doc_a, doc_b, RefKind::WikiLink);

    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn test_forward_references() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/b.md").unwrap();
    let uri_c = DocumentUri::new("file:///notes/c.md").unwrap();

    let a = graph.add_document(uri_a);
    let b = graph.add_document(uri_b);
    let c = graph.add_document(uri_c);

    graph.add_reference(a, b, RefKind::WikiLink);
    graph.add_reference(a, c, RefKind::MarkdownLink);

    let refs = graph.references(a);
    assert_eq!(refs.len(), 2);

    // Verify targets are b and c (order may vary)
    let targets: Vec<SymbolId> = refs.iter().map(|(id, _)| *id).collect();
    assert!(targets.contains(&b));
    assert!(targets.contains(&c));
}

#[test]
fn test_backward_references() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/b.md").unwrap();
    let uri_c = DocumentUri::new("file:///notes/c.md").unwrap();

    let a = graph.add_document(uri_a);
    let b = graph.add_document(uri_b);
    let c = graph.add_document(uri_c);

    graph.add_reference(a, c, RefKind::WikiLink);
    graph.add_reference(b, c, RefKind::Embed);

    let backs = graph.backrefs(c);
    assert_eq!(backs.len(), 2);

    let sources: Vec<SymbolId> = backs.iter().map(|(id, _)| *id).collect();
    assert!(sources.contains(&a));
    assert!(sources.contains(&b));
}

// ---------------------------------------------------------------------------
// Unresolved references
// ---------------------------------------------------------------------------

#[test]
fn test_unresolved_references() {
    let mut graph = ConnectionGraph::new();

    let uri = DocumentUri::new("file:///notes/page.md").unwrap();
    let doc = graph.add_document(uri);

    graph.add_unresolved(doc, "NonExistentPage", RefKind::WikiLink);
    graph.add_unresolved(doc, "AnotherMissing", RefKind::Embed);

    let unresolved = graph.unresolved_references();
    assert_eq!(unresolved.len(), 2);
    assert_eq!(unresolved[0].target, "NonExistentPage");
    assert_eq!(unresolved[0].kind, RefKind::WikiLink);
    assert_eq!(unresolved[1].target, "AnotherMissing");
    assert_eq!(unresolved[1].kind, RefKind::Embed);
}

// ---------------------------------------------------------------------------
// Removal
// ---------------------------------------------------------------------------

#[test]
fn test_remove_document_cleans_edges() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/b.md").unwrap();

    let a = graph.add_document(uri_a.clone());
    let b = graph.add_document(uri_b);

    // a has a heading
    let heading = graph.add_heading(uri_a.clone(), "intro", "Introduction");

    // References: a -> b, heading -> b
    graph.add_reference(a, b, RefKind::WikiLink);
    graph.add_reference(heading, b, RefKind::MarkdownLink);

    assert_eq!(graph.node_count(), 3); // a, b, heading
    assert_eq!(graph.edge_count(), 2);

    // Remove document a (should remove a + its heading + all edges)
    graph.remove_document(&uri_a);

    assert_eq!(graph.node_count(), 1); // only b remains
    assert_eq!(graph.edge_count(), 0);

    // b should have no backrefs now
    let backs = graph.backrefs(b);
    assert!(backs.is_empty());
}

// ---------------------------------------------------------------------------
// Edge deduplication
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_ref_kinds() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/b.md").unwrap();

    let a = graph.add_document(uri_a);
    let b = graph.add_document(uri_b);

    graph.add_reference(a, b, RefKind::WikiLink);
    graph.add_reference(a, b, RefKind::Embed);
    graph.add_reference(a, b, RefKind::TagRef);

    // Different kinds should all be stored
    assert_eq!(graph.edge_count(), 3);

    let refs = graph.references(a);
    assert_eq!(refs.len(), 3);

    let kinds: Vec<&RefKind> = refs.iter().map(|(_, k)| k).collect();
    assert!(kinds.contains(&&RefKind::WikiLink));
    assert!(kinds.contains(&&RefKind::Embed));
    assert!(kinds.contains(&&RefKind::TagRef));
}

#[test]
fn test_no_duplicate_edges() {
    let mut graph = ConnectionGraph::new();

    let uri_a = DocumentUri::new("file:///notes/a.md").unwrap();
    let uri_b = DocumentUri::new("file:///notes/b.md").unwrap();

    let a = graph.add_document(uri_a);
    let b = graph.add_document(uri_b);

    // Add the same reference twice
    graph.add_reference(a, b, RefKind::WikiLink);
    graph.add_reference(a, b, RefKind::WikiLink);

    // Should only have one edge (deduplication)
    assert_eq!(graph.edge_count(), 1);
}

// ---------------------------------------------------------------------------
// Generic graph tests with custom types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TestNode {
    id: String,
}

impl GraphNode for TestNode {
    type Key = String;
    fn key(&self) -> String {
        self.id.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum TestEdge {
    Blocks,
    Related,
}

impl EdgeKind for TestEdge {
    fn is_blocking(&self) -> bool {
        matches!(self, TestEdge::Blocks)
    }
}

#[test]
fn test_generic_graph_custom_types() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode {
        id: "task-a".into(),
    });
    let b = graph.add_node(TestNode {
        id: "task-b".into(),
    });
    graph.add_reference(a, b, TestEdge::Blocks);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);
    let refs = graph.references(a);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].0, b);
    assert_eq!(refs[0].1, TestEdge::Blocks);
}

// ---------------------------------------------------------------------------
// Cycle detection
// ---------------------------------------------------------------------------

#[test]
fn test_has_cycle_no_cycle() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Blocks);
    assert!(!graph.has_cycle());
}

#[test]
fn test_has_cycle_with_cycle() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Blocks);
    graph.add_reference(c, a, TestEdge::Blocks);
    assert!(graph.has_cycle());
}

#[test]
fn test_has_cycle_self_loop() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    graph.add_reference(a, a, TestEdge::Blocks);
    assert!(graph.has_cycle());
}

#[test]
fn test_has_cycle_empty_graph() {
    let graph = ConnectionGraph::<TestNode, TestEdge>::default();
    assert!(!graph.has_cycle());
}

// ---------------------------------------------------------------------------
// Topological sort
// ---------------------------------------------------------------------------

#[test]
fn test_topological_sort_linear() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Blocks);
    let sorted = graph.topological_sort().unwrap();
    let pos_a = sorted.iter().position(|s| *s == a).unwrap();
    let pos_b = sorted.iter().position(|s| *s == b).unwrap();
    let pos_c = sorted.iter().position(|s| *s == c).unwrap();
    assert!(pos_a < pos_b);
    assert!(pos_b < pos_c);
}

#[test]
fn test_topological_sort_cyclic_returns_none() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, a, TestEdge::Blocks);
    assert!(graph.topological_sort().is_none());
}

#[test]
fn test_topological_sort_disconnected_components() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let x = graph.add_node(TestNode { id: "x".into() });
    let y = graph.add_node(TestNode { id: "y".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(x, y, TestEdge::Blocks);
    let sorted = graph.topological_sort().unwrap();
    assert_eq!(sorted.len(), 4);
    let pos_a = sorted.iter().position(|s| *s == a).unwrap();
    let pos_b = sorted.iter().position(|s| *s == b).unwrap();
    let pos_x = sorted.iter().position(|s| *s == x).unwrap();
    let pos_y = sorted.iter().position(|s| *s == y).unwrap();
    assert!(pos_a < pos_b);
    assert!(pos_x < pos_y);
}

// ---------------------------------------------------------------------------
// Reachable from
// ---------------------------------------------------------------------------

#[test]
fn test_reachable_from_linear() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    let d = graph.add_node(TestNode { id: "d".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Blocks);
    let reachable = graph.reachable_from(a);
    assert!(reachable.contains(&b));
    assert!(reachable.contains(&c));
    assert!(!reachable.contains(&a));
    assert!(!reachable.contains(&d));
}

#[test]
fn test_reachable_from_in_cycle_terminates() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Blocks);
    graph.add_reference(c, a, TestEdge::Blocks);
    let reachable = graph.reachable_from(a);
    assert!(reachable.contains(&b));
    assert!(reachable.contains(&c));
    assert!(!reachable.contains(&a));
}

// ---------------------------------------------------------------------------
// Blocking predecessors
// ---------------------------------------------------------------------------

#[test]
fn test_blocking_predecessors_mixed_edges() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    let c = graph.add_node(TestNode { id: "c".into() });
    graph.add_reference(a, c, TestEdge::Blocks);
    graph.add_reference(b, c, TestEdge::Related);
    let blockers = graph.blocking_predecessors(c);
    assert_eq!(blockers.len(), 1);
    assert!(blockers.contains(&a));
    assert!(!blockers.contains(&b));
}

#[test]
fn test_blocking_predecessors_no_predecessors() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    assert!(graph.blocking_predecessors(a).is_empty());
}

// ---------------------------------------------------------------------------
// Remove by key
// ---------------------------------------------------------------------------

#[test]
fn test_remove_by_key_removes_nodes_and_edges() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    graph.add_reference(a, b, TestEdge::Blocks);
    graph.remove_by_key(&"a".to_string());
    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.backrefs(b).is_empty());
}

#[test]
fn test_remove_by_key_cleans_unresolved() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    graph.add_unresolved(a, "missing", TestEdge::Blocks);
    assert_eq!(graph.unresolved_references().len(), 1);
    graph.remove_by_key(&"a".to_string());
    assert!(graph.unresolved_references().is_empty());
}

#[test]
fn test_remove_by_key_nonexistent_is_noop() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let _a = graph.add_node(TestNode { id: "a".into() });
    graph.remove_by_key(&"nonexistent".to_string());
    assert_eq!(graph.node_count(), 1);
}

// ---------------------------------------------------------------------------
// Stale SymbolId guards (regression tests for marky-dr0t)
// ---------------------------------------------------------------------------

#[test]
fn test_stale_symbol_id_after_removal() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });
    graph.add_reference(a, b, TestEdge::Related);

    // Remove node A — its SymbolId is now stale
    graph.remove_by_key(&"a".to_string());

    // All operations with stale id should return empty, not panic
    assert!(graph.references(a).is_empty());
    assert!(graph.backrefs(a).is_empty());
    assert!(graph.reachable_from(a).is_empty());
    assert!(graph.blocking_predecessors(a).is_empty());
}

#[test]
fn test_add_reference_one_stale_endpoint() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let a = graph.add_node(TestNode { id: "a".into() });
    let b = graph.add_node(TestNode { id: "b".into() });

    // Remove A, then try to add edge from stale A to valid B
    graph.remove_by_key(&"a".to_string());
    let edge_count_before = graph.edge_count();
    graph.add_reference(a, b, TestEdge::Related);
    assert_eq!(
        graph.edge_count(),
        edge_count_before,
        "edge should not be added with stale source"
    );
}

#[test]
fn test_add_unresolved_stale_symbol_ignored() {
    let mut graph = ConnectionGraph::<TestNode, TestEdge>::default();
    let id = graph.add_node(TestNode { id: "test".into() });
    graph.remove_by_key(&"test".to_string());
    graph.add_unresolved(id, "target.md", TestEdge::Related);
    assert!(
        graph.unresolved_references().is_empty(),
        "unresolved ref from stale node should be silently ignored"
    );
}
