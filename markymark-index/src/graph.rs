//! Connection graph: tracks inter-document relationships using petgraph.
//!
//! Generic over node type ([`GraphNode`]) and edge type ([`EdgeKind`]).
//! Defaults to `SymbolData` / `RefKind` for backward compatibility.

use markymark_core::{DocumentUri, EdgeKind, GraphNode};
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::{HashMap, HashSet};

/// Opaque handle to a symbol in the connection graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub(crate) NodeIndex);

/// The kind of reference between two symbols.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
}

impl EdgeKind for RefKind {
    fn is_blocking(&self) -> bool {
        false // document links never block
    }
}

/// An unresolved reference (target symbol not found in the graph).
#[derive(Debug, Clone)]
pub struct UnresolvedRef<E: EdgeKind = RefKind> {
    /// The symbol that contains the reference.
    pub from: SymbolId,
    /// The target string that could not be resolved.
    pub target: String,
    /// The kind of reference.
    pub kind: E,
}

/// Data stored for each node in the default connection graph.
///
/// Variants track document-level and heading-level symbols.
/// The `slug` and `text` fields on `Heading` are stored for future
/// cross-document navigation features.
#[derive(Debug, Clone)]
pub enum SymbolData {
    /// A document node.
    Document {
        /// The document URI.
        uri: DocumentUri,
    },
    /// A heading within a document.
    Heading {
        /// The document URI this heading belongs to.
        uri: DocumentUri,
        /// The heading slug (anchor).
        #[allow(dead_code)]
        slug: String,
        /// The heading text.
        #[allow(dead_code)]
        text: String,
    },
}

impl GraphNode for SymbolData {
    type Key = String;

    fn key(&self) -> String {
        match self {
            SymbolData::Document { uri } => uri.as_str().to_string(),
            SymbolData::Heading { uri, .. } => uri.as_str().to_string(),
        }
    }
}

/// A directed graph of symbols and their references.
///
/// Generic over node type `N` ([`GraphNode`]) and edge type `E` ([`EdgeKind`]).
/// Defaults to `SymbolData` and `RefKind` for backward compatibility with
/// existing markymark code.
///
/// Uses [`petgraph::stable_graph::StableGraph`] so that indices of
/// remaining nodes stay valid when other nodes are removed.
pub struct ConnectionGraph<N: GraphNode = SymbolData, E: EdgeKind = RefKind> {
    graph: StableGraph<N, E>,
    key_to_nodes: HashMap<N::Key, Vec<NodeIndex>>,
    unresolved: Vec<UnresolvedRef<E>>,
}

// ---------------------------------------------------------------------------
// Generic methods (available on all ConnectionGraph<N, E>)
// ---------------------------------------------------------------------------

impl<N: GraphNode, E: EdgeKind> ConnectionGraph<N, E> {
    /// Check whether a SymbolId's underlying node still exists in the graph.
    fn has_node(&self, id: SymbolId) -> bool {
        self.graph.node_weight(id.0).is_some()
    }

    /// Add a node and return its handle.
    pub fn add_node(&mut self, data: N) -> SymbolId {
        let key = data.key();
        let idx = self.graph.add_node(data);
        self.key_to_nodes.entry(key).or_default().push(idx);
        SymbolId(idx)
    }

    /// Record a reference from one symbol to another.
    ///
    /// Deduplicates: if an edge with the same target and kind already exists,
    /// it is not added again.
    pub fn add_reference(&mut self, from: SymbolId, to: SymbolId, kind: E) {
        if !self.has_node(from) || !self.has_node(to) {
            return;
        }
        let has_duplicate = self
            .graph
            .edges(from.0)
            .any(|e| e.target() == to.0 && *e.weight() == kind);

        if !has_duplicate {
            self.graph.add_edge(from.0, to.0, kind);
        }
    }

    /// Get all outgoing references from a symbol.
    pub fn references(&self, symbol: SymbolId) -> Vec<(SymbolId, E)> {
        if !self.has_node(symbol) {
            return Vec::new();
        }
        self.graph
            .edges(symbol.0)
            .map(|e| (SymbolId(e.target()), e.weight().clone()))
            .collect()
    }

    /// Get all incoming references (back-references) to a symbol.
    pub fn backrefs(&self, symbol: SymbolId) -> Vec<(SymbolId, E)> {
        if !self.has_node(symbol) {
            return Vec::new();
        }
        self.graph
            .edges_directed(symbol.0, Direction::Incoming)
            .map(|e| (SymbolId(e.source()), e.weight().clone()))
            .collect()
    }

    /// Record an unresolved reference.
    pub fn add_unresolved(&mut self, from: SymbolId, target: &str, kind: E) {
        self.unresolved.push(UnresolvedRef {
            from,
            target: target.to_string(),
            kind,
        });
    }

    /// List references whose targets could not be resolved.
    pub fn unresolved_references(&self) -> &[UnresolvedRef<E>] {
        &self.unresolved
    }

    /// Remove all nodes with the given key and their edges.
    ///
    /// Also removes any unresolved references originating from those nodes.
    /// No-op if the key does not exist.
    pub fn remove_by_key(&mut self, key: &N::Key) {
        if let Some(nodes) = self.key_to_nodes.remove(key) {
            let node_set: HashSet<NodeIndex> = nodes.iter().copied().collect();
            for idx in &nodes {
                self.graph.remove_node(*idx);
            }
            self.unresolved.retain(|ur| !node_set.contains(&ur.from.0));
        }
    }

    /// Total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns true if the graph contains a cycle.
    pub fn has_cycle(&self) -> bool {
        petgraph::algo::is_cyclic_directed(&self.graph)
    }

    /// Returns a topological ordering if the graph is acyclic, `None` otherwise.
    pub fn topological_sort(&self) -> Option<Vec<SymbolId>> {
        petgraph::algo::toposort(&self.graph, None)
            .ok()
            .map(|order| order.into_iter().map(SymbolId).collect())
    }

    /// Returns all nodes reachable from `start` via outgoing edges (excluding start).
    pub fn reachable_from(&self, start: SymbolId) -> HashSet<SymbolId> {
        if !self.has_node(start) {
            return HashSet::new();
        }
        let mut dfs = petgraph::visit::Dfs::new(&self.graph, start.0);
        let mut result = HashSet::new();
        while let Some(node) = dfs.next(&self.graph) {
            if node != start.0 {
                result.insert(SymbolId(node));
            }
        }
        result
    }

    /// Returns predecessor nodes connected via blocking edges only.
    pub fn blocking_predecessors(&self, node: SymbolId) -> Vec<SymbolId> {
        if !self.has_node(node) {
            return Vec::new();
        }
        self.graph
            .edges_directed(node.0, Direction::Incoming)
            .filter(|e| e.weight().is_blocking())
            .map(|e| SymbolId(e.source()))
            .collect()
    }
}

impl<N: GraphNode, E: EdgeKind> Default for ConnectionGraph<N, E> {
    fn default() -> Self {
        Self {
            graph: StableGraph::new(),
            key_to_nodes: HashMap::new(),
            unresolved: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// SymbolData-specific convenience methods (backward compatibility)
// ---------------------------------------------------------------------------

impl ConnectionGraph<SymbolData, RefKind> {
    /// Create an empty connection graph with default types.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document symbol and return its handle.
    pub fn add_document(&mut self, uri: DocumentUri) -> SymbolId {
        self.add_node(SymbolData::Document { uri })
    }

    /// Add a heading symbol belonging to a document.
    pub fn add_heading(&mut self, uri: DocumentUri, slug: &str, text: &str) -> SymbolId {
        self.add_node(SymbolData::Heading {
            uri,
            slug: slug.to_string(),
            text: text.to_string(),
        })
    }

    /// Remove a document and all of its symbols and edges.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        self.remove_by_key(&uri.as_str().to_string());
    }
}
