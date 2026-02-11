//! Connection graph: tracks inter-document relationships using petgraph.

use markymark_core::DocumentUri;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashMap;

/// Opaque handle to a symbol in the connection graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(NodeIndex);

/// The kind of reference between two symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    /// `[[target]]` wiki-style link.
    WikiLink,
    /// `[text](url)` markdown link.
    MarkdownLink,
    /// `((block-ref))` Logseq block reference.
    BlockRef,
    /// `![[embed]]` transclusion.
    Embed,
    /// `#tag` reference.
    TagRef,
    /// `<tag>` XML tag reference.
    XmlTagRef,
}

/// An unresolved reference (target symbol not found in the graph).
#[derive(Debug, Clone)]
pub struct UnresolvedRef {
    /// The symbol that contains the reference.
    pub from: SymbolId,
    /// The target string that could not be resolved.
    pub target: String,
    /// The kind of reference.
    pub kind: RefKind,
}

/// Data stored for each node in the graph.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum SymbolData {
    Document {
        uri: DocumentUri,
    },
    Heading {
        uri: DocumentUri,
        slug: String,
        text: String,
    },
    XmlTag {
        uri: DocumentUri,
        tag_name: String,
    },
}

/// A directed graph of document symbols and their references.
///
/// Uses [`petgraph::stable_graph::StableGraph`] so that node indices
/// remain valid after removals.
pub struct ConnectionGraph {
    graph: StableGraph<SymbolData, RefKind>,
    uri_to_nodes: HashMap<String, Vec<NodeIndex>>,
    unresolved: Vec<UnresolvedRef>,
}

impl ConnectionGraph {
    /// Create an empty connection graph.
    pub fn new() -> Self {
        Self {
            graph: StableGraph::new(),
            uri_to_nodes: HashMap::new(),
            unresolved: Vec::new(),
        }
    }

    /// Add a document symbol and return its handle.
    pub fn add_document(&mut self, uri: DocumentUri) -> SymbolId {
        let key = uri.as_str().to_string();
        let idx = self.graph.add_node(SymbolData::Document { uri });
        self.uri_to_nodes.entry(key).or_default().push(idx);
        SymbolId(idx)
    }

    /// Add a heading symbol belonging to a document.
    pub fn add_heading(&mut self, uri: DocumentUri, slug: &str, text: &str) -> SymbolId {
        let key = uri.as_str().to_string();
        let idx = self.graph.add_node(SymbolData::Heading {
            uri,
            slug: slug.to_string(),
            text: text.to_string(),
        });
        self.uri_to_nodes.entry(key).or_default().push(idx);
        SymbolId(idx)
    }

    /// Add an XML tag symbol belonging to a document.
    pub fn add_xml_tag(&mut self, uri: DocumentUri, tag_name: &str) -> SymbolId {
        let key = uri.as_str().to_string();
        let idx = self.graph.add_node(SymbolData::XmlTag {
            uri,
            tag_name: tag_name.to_string(),
        });
        self.uri_to_nodes.entry(key).or_default().push(idx);
        SymbolId(idx)
    }

    /// Record a reference from one symbol to another.
    pub fn add_reference(&mut self, from: SymbolId, to: SymbolId, kind: RefKind) {
        // Check for existing edge with same kind (dedup)
        let has_duplicate = self
            .graph
            .edges(from.0)
            .any(|e| e.target() == to.0 && *e.weight() == kind);

        if !has_duplicate {
            self.graph.add_edge(from.0, to.0, kind);
        }
    }

    /// Get all outgoing references from a symbol.
    pub fn references(&self, symbol: SymbolId) -> Vec<(SymbolId, RefKind)> {
        self.graph
            .edges(symbol.0)
            .map(|e| (SymbolId(e.target()), e.weight().clone()))
            .collect()
    }

    /// Get all incoming references (back-references) to a symbol.
    pub fn backrefs(&self, symbol: SymbolId) -> Vec<(SymbolId, RefKind)> {
        self.graph
            .edges_directed(symbol.0, Direction::Incoming)
            .map(|e| (SymbolId(e.source()), e.weight().clone()))
            .collect()
    }

    /// List references whose targets could not be resolved.
    pub fn unresolved_references(&self) -> &[UnresolvedRef] {
        &self.unresolved
    }

    /// Record an unresolved reference.
    pub fn add_unresolved(&mut self, from: SymbolId, target: &str, kind: RefKind) {
        self.unresolved.push(UnresolvedRef {
            from,
            target: target.to_string(),
            kind,
        });
    }

    /// Remove a document and all of its symbols and edges.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        let key = uri.as_str();
        if let Some(nodes) = self.uri_to_nodes.remove(key) {
            for idx in &nodes {
                self.graph.remove_node(*idx);
            }
            // Remove unresolved refs from this document's nodes
            let node_set: std::collections::HashSet<NodeIndex> = nodes.into_iter().collect();
            self.unresolved.retain(|ur| !node_set.contains(&ur.from.0));
        }
    }

    /// Total number of symbols (nodes) in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total number of reference edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for ConnectionGraph {
    fn default() -> Self {
        Self::new()
    }
}
