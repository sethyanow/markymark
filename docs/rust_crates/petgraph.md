# petgraph - Graph Data Structures

<agent>
<goal>Build and traverse connection graphs for document relationships and dependencies.</goal>
<when_to_use>When you need graph data structures, traversals, or algorithms (BFS, DFS, cycles, topological sort).</when_to_use>
<contains>DiGraph setup, node/edge operations, traversals, algorithms, generational indices</contains>
<see_also>tower-lsp.md, bumpalo.md</see_also>
</agent>

**TL;DR:** petgraph provides efficient graph types. Use `DiGraph<N, E>` for directed graphs, `NodeIndex`/`EdgeIndex` for stable handles. Nodes and edges can store arbitrary data.

**Checklist:**
- [ ] Choose graph type: `DiGraph`, `UnGraph`, `StableGraph`
- [ ] Use `NodeIndex` for node handles, `EdgeIndex` for edges
- [ ] Prefer `StableGraph` if removing nodes/edges frequently
- [ ] Use algorithm functions from `petgraph::algo`

---

## Setup

### Cargo.toml

```toml
[dependencies]
petgraph = "0.6"
```

### Basic Graph Operations

```rust
use petgraph::graph::{DiGraph, NodeIndex, EdgeIndex};
use petgraph::Direction;

fn main() {
    // Create directed graph with String nodes and &str edge labels
    let mut graph: DiGraph<String, &str> = DiGraph::new();

    // Add nodes - returns NodeIndex
    let doc_a = graph.add_node("doc_a.md".to_string());
    let doc_b = graph.add_node("doc_b.md".to_string());
    let doc_c = graph.add_node("doc_c.md".to_string());

    // Add edges - returns EdgeIndex
    let _link1 = graph.add_edge(doc_a, doc_b, "links_to");
    let _link2 = graph.add_edge(doc_a, doc_c, "links_to");
    let _link3 = graph.add_edge(doc_b, doc_c, "links_to");

    // Access node data
    println!("Node: {}", graph[doc_a]);

    // Iterate neighbors
    for neighbor in graph.neighbors(doc_a) {
        println!("  -> {}", graph[neighbor]);
    }

    // Iterate incoming edges
    for neighbor in graph.neighbors_directed(doc_c, Direction::Incoming) {
        println!("  <- {}", graph[neighbor]);
    }
}
```

---

## Patterns

### Connection Graph for Documents

```rust
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::has_path_connecting;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Symbol {
    uri: String,
    kind: SymbolKind,
    name: String,
}

#[derive(Debug, Clone)]
enum SymbolKind {
    Document,
    Heading { level: u8 },
    Block { id: String },
}

#[derive(Debug, Clone)]
enum EdgeKind {
    WikiLink,
    MarkdownLink,
    BlockRef,
    Embed,
}

struct ConnectionGraph {
    graph: DiGraph<Symbol, EdgeKind>,
    symbol_to_node: HashMap<String, NodeIndex>,  // Symbol key -> NodeIndex
}

impl ConnectionGraph {
    fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            symbol_to_node: HashMap::new(),
        }
    }

    fn add_symbol(&mut self, symbol: Symbol) -> NodeIndex {
        let key = symbol_key(&symbol);
        if let Some(&idx) = self.symbol_to_node.get(&key) {
            return idx;
        }

        let idx = self.graph.add_node(symbol);
        self.symbol_to_node.insert(key, idx);
        idx
    }

    fn add_reference(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) {
        // Avoid duplicate edges
        if !self.graph.contains_edge(from, to) {
            self.graph.add_edge(from, to, kind);
        }
    }

    fn get_references(&self, symbol: NodeIndex) -> Vec<(NodeIndex, &EdgeKind)> {
        self.graph
            .edges(symbol)
            .map(|e| (e.target(), e.weight()))
            .collect()
    }

    fn get_backrefs(&self, symbol: NodeIndex) -> Vec<(NodeIndex, &EdgeKind)> {
        self.graph
            .edges_directed(symbol, Direction::Incoming)
            .map(|e| (e.source(), e.weight()))
            .collect()
    }

    fn is_connected(&self, from: NodeIndex, to: NodeIndex) -> bool {
        has_path_connecting(&self.graph, from, to, None)
    }
}

fn symbol_key(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Document => symbol.uri.clone(),
        SymbolKind::Heading { .. } => format!("{}#{}", symbol.uri, symbol.name),
        SymbolKind::Block { id } => format!("{}^{}", symbol.uri, id),
    }
}
```

### Finding Orphans (No Incoming Links)

```rust
fn find_orphan_documents(graph: &ConnectionGraph) -> Vec<NodeIndex> {
    graph.graph
        .node_indices()
        .filter(|&idx| {
            // Only consider document nodes
            matches!(graph.graph[idx].kind, SymbolKind::Document)
        })
        .filter(|&idx| {
            // No incoming edges
            graph.graph
                .edges_directed(idx, Direction::Incoming)
                .next()
                .is_none()
        })
        .collect()
}
```

### Topological Sort (Dependency Order)

```rust
use petgraph::algo::toposort;

fn get_build_order(graph: &ConnectionGraph) -> Result<Vec<NodeIndex>, ()> {
    match toposort(&graph.graph, None) {
        Ok(order) => Ok(order),
        Err(_cycle) => Err(()), // Graph has cycles
    }
}
```

### Cycle Detection

```rust
use petgraph::algo::is_cyclic_directed;

fn has_circular_references(graph: &ConnectionGraph) -> bool {
    is_cyclic_directed(&graph.graph)
}

// Find nodes involved in cycles
use petgraph::algo::tarjan_scc;

fn find_cycles(graph: &ConnectionGraph) -> Vec<Vec<NodeIndex>> {
    tarjan_scc(&graph.graph)
        .into_iter()
        .filter(|scc| scc.len() > 1) // SCCs with >1 node are cycles
        .collect()
}
```

### BFS/DFS Traversals

```rust
use petgraph::visit::{Bfs, Dfs};

fn bfs_from(graph: &ConnectionGraph, start: NodeIndex) -> Vec<NodeIndex> {
    let mut bfs = Bfs::new(&graph.graph, start);
    let mut visited = Vec::new();

    while let Some(node) = bfs.next(&graph.graph) {
        visited.push(node);
    }

    visited
}

fn dfs_from(graph: &ConnectionGraph, start: NodeIndex) -> Vec<NodeIndex> {
    let mut dfs = Dfs::new(&graph.graph, start);
    let mut visited = Vec::new();

    while let Some(node) = dfs.next(&graph.graph) {
        visited.push(node);
    }

    visited
}
```

### Incremental Updates

```rust
impl ConnectionGraph {
    fn remove_document(&mut self, uri: &str) {
        // Find all nodes belonging to this document
        let nodes_to_remove: Vec<_> = self.symbol_to_node
            .iter()
            .filter(|(key, _)| key.starts_with(uri))
            .map(|(key, &idx)| (key.clone(), idx))
            .collect();

        for (key, idx) in nodes_to_remove {
            self.graph.remove_node(idx);
            self.symbol_to_node.remove(&key);
        }
    }

    fn update_document(&mut self, uri: &str, new_symbols: Vec<Symbol>, new_refs: Vec<(String, String, EdgeKind)>) {
        // Remove old data
        self.remove_document(uri);

        // Add new symbols
        for symbol in new_symbols {
            self.add_symbol(symbol);
        }

        // Add new references
        for (from_key, to_key, kind) in new_refs {
            if let (Some(&from), Some(&to)) = (
                self.symbol_to_node.get(&from_key),
                self.symbol_to_node.get(&to_key),
            ) {
                self.add_reference(from, to, kind);
            }
        }
    }
}
```

### Using StableGraph for Frequent Removals

```rust
use petgraph::stable_graph::StableGraph;

// StableGraph preserves indices after removal
// DiGraph may invalidate indices on removal

struct StableConnectionGraph {
    graph: StableGraph<Symbol, EdgeKind>,
    symbol_to_node: HashMap<String, NodeIndex>,
}

impl StableConnectionGraph {
    fn remove_node_stable(&mut self, key: &str) {
        if let Some(idx) = self.symbol_to_node.remove(key) {
            self.graph.remove_node(idx);
            // Other NodeIndex values remain valid!
        }
    }
}
```

### Custom Edge/Node Types

```rust
use petgraph::graph::DiGraph;

#[derive(Debug, Clone)]
struct DocumentNode {
    uri: String,
    title: Option<String>,
    headings: Vec<String>,
    last_modified: u64,
}

#[derive(Debug, Clone)]
struct LinkEdge {
    kind: LinkKind,
    source_range: Range<usize>,
    anchor: Option<String>,
}

#[derive(Debug, Clone)]
enum LinkKind {
    Wiki { alias: Option<String> },
    Markdown { title: Option<String> },
    BlockRef,
    Embed,
}

type DocGraph = DiGraph<DocumentNode, LinkEdge>;

fn create_doc_graph() -> DocGraph {
    let mut graph = DocGraph::new();

    let doc1 = graph.add_node(DocumentNode {
        uri: "note1.md".into(),
        title: Some("First Note".into()),
        headings: vec!["Introduction".into(), "Details".into()],
        last_modified: 1234567890,
    });

    let doc2 = graph.add_node(DocumentNode {
        uri: "note2.md".into(),
        title: Some("Second Note".into()),
        headings: vec!["Overview".into()],
        last_modified: 1234567900,
    });

    graph.add_edge(doc1, doc2, LinkEdge {
        kind: LinkKind::Wiki { alias: None },
        source_range: 10..25,
        anchor: Some("overview".into()),
    });

    graph
}
```

---

## Pitfalls

### NodeIndex Invalidation

<pitfall>
**Problem:** `DiGraph::remove_node` may invalidate other NodeIndex values.

```rust
// BAD: NodeIndex can become invalid
let a = graph.add_node("a");
let b = graph.add_node("b");
let c = graph.add_node("c");

graph.remove_node(a);
// WARNING: 'c' may now have a different index!
println!("{}", graph[c]); // Might panic or return wrong data
```

**Solution:** Use `StableGraph` or rebuild index mappings:

```rust
// GOOD: StableGraph preserves indices
use petgraph::stable_graph::StableGraph;

let mut graph: StableGraph<&str, ()> = StableGraph::new();
let a = graph.add_node("a");
let b = graph.add_node("b");
let c = graph.add_node("c");

graph.remove_node(a);
println!("{}", graph[c]); // Safe! Returns "c"
```
</pitfall>

### Edge Direction Confusion

<pitfall>
**Problem:** `neighbors()` vs `neighbors_directed()` behavior differs.

```rust
// `neighbors()` returns OUTGOING neighbors by default
for n in graph.neighbors(node) { } // Only outgoing edges

// To get incoming:
for n in graph.neighbors_directed(node, Direction::Incoming) { }
```

**Solution:** Be explicit about direction:

```rust
// GOOD: Clear intent
fn get_outgoing(graph: &DiGraph<N, E>, node: NodeIndex) -> Vec<NodeIndex> {
    graph.neighbors_directed(node, Direction::Outgoing).collect()
}

fn get_incoming(graph: &DiGraph<N, E>, node: NodeIndex) -> Vec<NodeIndex> {
    graph.neighbors_directed(node, Direction::Incoming).collect()
}
```
</pitfall>

### Weight Access Patterns

<pitfall>
**Problem:** `graph[idx]` borrows graph, preventing mutation during iteration.

```rust
// BAD: Cannot mutate while iterating
for node in graph.node_indices() {
    graph[node].count += 1; // Borrow error!
}
```

**Solution:** Collect indices first or use `node_weight_mut`:

```rust
// GOOD: Collect then mutate
let indices: Vec<_> = graph.node_indices().collect();
for idx in indices {
    graph[idx].count += 1;
}

// Or use raw_nodes for bulk access (if supported)
```
</pitfall>

### Empty Graph Edge Cases

<pitfall>
**Problem:** Algorithms may behave unexpectedly on empty graphs.

```rust
// toposort on empty graph returns empty vec (ok)
// has_path_connecting with invalid indices may panic
```

**Solution:** Check for empty graphs:

```rust
fn safe_has_path(graph: &DiGraph<N, E>, from: NodeIndex, to: NodeIndex) -> bool {
    if graph.node_count() == 0 {
        return false;
    }
    if !graph.contains_node(from) || !graph.contains_node(to) {
        return false;
    }
    has_path_connecting(graph, from, to, None)
}
```
</pitfall>

---

## Related

- LSP with graph state: `tower-lsp.md`
- Memory-efficient graphs: `bumpalo.md`
- petgraph docs: https://docs.rs/petgraph/
- petgraph algorithms: https://docs.rs/petgraph/latest/petgraph/algo/index.html
