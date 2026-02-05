# markymark: High-Performance Markdown LSP

**Date:** 2026-02-05
**Status:** Draft
**Author:** Samson + Bender

## Overview

markymark is a memory-efficient, high-performance Markdown Language Server Protocol (LSP) implementation in Rust. It replaces Marksman for use cases requiring extreme resource efficiency, multi-tenant workspace isolation, and full anchor link rename support.

### Motivation

1. **Multi-project infrastructure** - forge, floatilla, and future projects need robust markdown LSP
2. **Constrained environments** - Must run lean on limited hardware
3. **Long-running daemon** - No memory creep over days/weeks of operation
4. **Token efficiency** - LSP state must be compact for AI context serialization
5. **Marksman limitations** - F# codebase limits contribution; rename doesn't update anchor links

### Goals

- Sub-5MB static binary
- O(1) realm creation/destruction (arena allocation)
- Full Obsidian and Logseq markdown flavor support
- Rename that actually updates anchor links
- Multi-tenant isolation with shared and isolated realm modes

---

## Architecture

### High-Level Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                      LSP Transport (tower-lsp)                  │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                        Realm Router                             │
│               (dispatches requests to correct realm)            │
└─────────────────────────────┬───────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
    ┌───────────┐       ┌───────────┐       ┌───────────┐
    │  Realm A  │       │  Realm B  │       │  Realm C  │
    │  (shared) │       │ (isolated)│       │ (isolated)│
    ├───────────┤       ├───────────┤       ├───────────┤
    │ - Arena   │       │ - Arena   │       │ - Arena   │
    │ - Index   │       │ - Index   │       │ - Index   │
    │ - Graph   │       │ - Graph   │       │ - Graph   │
    │ - Roots[] │       │ - Roots[] │       │ - Roots[] │
    └───────────┘       └───────────┘       └───────────┘
```

### Key Components

- **Realm Router**: Maps document URIs to owning realm. O(1) lookup via prefix tree.
- **Realm**: Isolated unit with own arena allocator, symbol index, and connection graph.
- **Root**: Directory (real or virtual) within a realm. Multiple roots enable cross-references.
- **Arena**: Per-realm bump allocator. Realm destruction = instant memory cleanup.

---

## Realm Management

### Configuration

```rust
struct RealmConfig {
    id: RealmId,
    mode: RealmMode,                // Shared | Isolated
    roots: Vec<RootConfig>,
    settings: RealmSettings,
}

struct RealmSettings {
    wiki_style: WikiStyle,          // [[double-bracket]] | [single-bracket]
    slug_mode: SlugMode,            // GitHub | GitLab | Custom
    file_extensions: Vec<String>,
    ignore_patterns: Vec<Glob>,
    max_file_size: usize,
}

enum RealmMode {
    Shared,     // Cross-root references enabled
    Isolated,   // Single-root sandbox, no cross-references
}
```

### Custom LSP Methods

```
markymark/createRealm    { id, mode, settings? }        → RealmId
markymark/destroyRealm   { id }                         → ()
markymark/listRealms     {}                             → [RealmInfo]
markymark/getRealm       { id }                         → RealmInfo | null

markymark/addRoot        { realm_id, path, virtual_fs? } → RootId
markymark/removeRoot     { realm_id, root_id }           → ()
markymark/listRoots      { realm_id }                    → [RootInfo]

markymark/realmStats     { id }  → { doc_count, symbol_count, mem_bytes }
markymark/compact        { id }  → ()
```

### Lifecycle Hooks

```
markymark/onRealmCreated     → notification with realm_id
markymark/onRealmDestroyed   → notification with realm_id
markymark/onRootIndexed      → notification when initial scan completes
markymark/onIndexUpdated     → notification on incremental updates (debounced)
```

---

## Document Model

### Markdown Flavors

```rust
enum MarkdownFlavor {
    Standard,           // CommonMark + GFM
    Obsidian {
        callouts: bool,
        embeds: bool,
        block_ids: bool,
    },
    Logseq {
        block_refs: bool,
        block_embeds: bool,
        properties: bool,
        query_blocks: bool,
    },
    Custom(FlavorConfig),
}
```

### Link Types (Unified)

```rust
struct WikiLink<'arena> {
    target: WikiTarget<'arena>,
    alias: Option<&'arena str>,
    range: Range,
}

enum WikiTarget<'arena> {
    Page { path: &'arena str },
    PageHeading { path: &'arena str, heading: &'arena str },
    PageBlock { path: &'arena str, block_id: &'arena str },
    CurrentPageHeading { heading: &'arena str },
    CurrentPageBlock { block_id: &'arena str },
}

enum BlockRef<'arena> {
    Obsidian { page: Option<&'arena str>, block_id: &'arena str, range: Range },
    Logseq { uuid: &'arena str, range: Range },
}

enum Embed<'arena> {
    File { path: &'arena str, anchor: Option<EmbedAnchor<'arena>>, range: Range },
    Page { path: &'arena str, range: Range },
    Block { uuid: &'arena str, range: Range },
}
```

### Properties & Frontmatter

```rust
enum Frontmatter<'arena> {
    Yaml { raw: &'arena str, properties: &'arena [Property<'arena>], range: Range },
    LogseqPageProps { properties: &'arena [Property<'arena>], range: Range },
}

struct Property<'arena> {
    key: &'arena str,
    value: PropertyValue<'arena>,
    range: Range,
}

enum PropertyValue<'arena> {
    String(&'arena str),
    Number(f64),
    Bool(bool),
    Date(Date),
    List(&'arena [&'arena str]),
    PageRef(&'arena str),
    BlockRef(&'arena str),
    Tags(&'arena [&'arena str]),
}
```

### Task States

```rust
enum TaskState {
    // Markdown checkbox
    Unchecked, Checked, InProgress, Cancelled,
    // Logseq keywords
    Todo, Doing, Done, Later, Now, Waiting, Cancelled_KW,
    Custom(&'static str),
}
```

### List Items (Logseq Deep Nesting)

```rust
struct ListItem<'arena> {
    block_id: Option<&'arena str>,
    task: Option<Task<'arena>>,
    properties: &'arena [InlineProperty<'arena>],
    content: &'arena [Inline<'arena>],
    children: &'arena [ListItem<'arena>],  // Recursive, unlimited depth
    depth: u8,
    range: Range,
    collapsed: bool,
}
```

---

## Indexing & Connection Graph

### Per-Document Index

```rust
struct DocumentIndex<'arena> {
    headings: HashMap<&'arena str, &'arena Heading>,
    block_ids: HashMap<&'arena str, BlockLocation<'arena>>,
    link_defs: HashMap<&'arena str, &'arena LinkDefinition>,
    wiki_links: Vec<&'arena WikiLink<'arena>>,
    md_links: Vec<&'arena MdLink<'arena>>,
    block_refs: Vec<&'arena BlockRef<'arena>>,
    embeds: Vec<&'arena Embed<'arena>>,
    tags: Vec<&'arena Tag<'arena>>,
    properties: HashMap<&'arena str, Vec<&'arena Property<'arena>>>,
    tasks: Vec<&'arena Task<'arena>>,
    toc: Vec<TocEntry<'arena>>,
    outline: &'arena OutlineNode<'arena>,
}
```

### Connection Graph

```rust
struct ConnectionGraph<'arena> {
    resolved: petgraph::DiGraph<Symbol<'arena>, EdgeKind>,
    unresolved: Vec<UnresolvedRef<'arena>>,
    backrefs: HashMap<Symbol<'arena>, Vec<Symbol<'arena>>>,
    ref_deps: DependencyGraph<'arena>,
    last_touched: HashSet<Symbol<'arena>>,
}

enum Symbol<'arena> {
    Document(DocumentUri),
    Heading { doc: DocumentUri, slug: &'arena str },
    Block { doc: DocumentUri, id: &'arena str },
    LinkDef { doc: DocumentUri, label: &'arena str },
    Tag(&'arena str),
}
```

### Incremental Updates

Four-phase update algorithm:
1. **Remove** deleted symbols, queue dependents for re-resolution
2. **Add** new symbols, queue references for resolution
3. **Resolve** queued references via oracle
4. **Track** last_touched for diagnostics

---

## LSP Features

### Standard Operations

| Method | Description |
|--------|-------------|
| `textDocument/definition` | Navigate to link target |
| `textDocument/references` | Find all references to symbol |
| `textDocument/completion` | Wiki links, block refs, tags, properties |
| `textDocument/rename` | Rename heading + update ALL anchor references |
| `textDocument/diagnostic` | Broken links, ambiguous references |
| `textDocument/documentSymbol` | Document outline |
| `workspace/symbol` | Cross-document symbol search |

### Key Improvement: Rename

Unlike Marksman, markymark's rename updates anchor links because the connection graph tracks precise source ranges:

```rust
async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    // Edit the definition
    edits.push(TextEdit { range: heading.range, new_text: new_name });

    // Edit ALL backrefs (this is where Marksman fails)
    for ref_sym in realm.graph.backrefs.get(&symbol) {
        let new_slug = compute_slug(new_name);
        edits.push(TextEdit {
            range: ref_element.anchor_range(),  // Just the #anchor portion
            new_text: new_slug
        });
    }
}
```

### Extended Methods

| Method | Description |
|--------|-------------|
| `markymark/getOutline` | Full document outline with depth |
| `markymark/getDependencyGraph` | Cross-document dependency graph |
| `markymark/findOrphans` | Documents with no incoming links |
| `markymark/exportIndex` | Export index for external consumers |
| `markymark/subscribeUpdates` | Stream symbol updates |

---

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Memory safety, performance, ecosystem |
| LSP Framework | tower-lsp | Mature, async, well-maintained |
| Parser | tree-sitter-markdown | Incremental parsing, battle-tested |
| Graph | petgraph | Generational indices, algorithms |
| Arena | bumpalo | Fast bump allocation, bulk dealloc |
| Testing | insta + proptest | Snapshots + property-based |

---

## Project Structure

```
markymark/
├── Cargo.toml
├── crates/
│   ├── markymark-core/           # Core types, no dependencies
│   ├── markymark-parser/         # tree-sitter integration
│   ├── markymark-index/          # Indexing & connection graph
│   ├── markymark-lsp/            # LSP server implementation
│   └── markymark-cli/            # CLI binary
├── tests/
│   ├── golden/                   # Ported Marksman snapshots
│   └── integration/
└── benches/
```

---

## Testing Strategy

### Test Pyramid

| Layer | Count | Scope |
|-------|-------|-------|
| Unit | 500+ | Per-function, fast |
| Integration | 100-200 | Multi-component, in-memory |
| E2E | 10-20 | Real LSP client, real files |

### Key Test Categories

1. **Parser tests** - Port all Marksman ParserTests.fs as golden files
2. **Graph tests** - Incremental update consistency
3. **LSP tests** - Full request/response cycles
4. **Property tests** - Parse doesn't panic, incremental = full rebuild
5. **Performance tests** - Incremental edit latency < 50ms

---

## CI/CD Pipeline

### Jobs

1. **check** - Format, clippy, docs
2. **unit-tests** - Parallel per-crate
3. **integration-tests** - Multi-component
4. **property-tests** - Extended proptest runs
5. **e2e-tests** - Cross-platform (Linux, macOS, Windows)
6. **coverage** - llvm-cov + Codecov
7. **benchmarks** - Criterion + PR comparison
8. **build** - Release binaries for all targets
9. **release** - GitHub releases on tags

### Targets

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

---

## Future Considerations

- **Semantic indexing integration** - Export hooks for embedding systems
- **Bidirectional sync** - Forge integration for metadata sync
- **Plugin system** - Custom link types, resolvers
- **WASM build** - Browser-based editing support

---

## References

- [Marksman](https://github.com/artempyanykh/marksman) - Original F# implementation
- [tower-lsp](https://github.com/ebkalderon/tower-lsp) - LSP framework
- [tree-sitter-markdown](https://github.com/tree-sitter-grammars/tree-sitter-markdown) - Parser
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)
