use markymark_core::DocumentUri;
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
