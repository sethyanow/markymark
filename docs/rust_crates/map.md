# Crate Relationship Map

<agent>
<goal>Understand how markymark crates relate to each other and when to use each.</goal>
<when_to_use>When planning implementation or understanding crate dependencies.</when_to_use>
<contains>Dependency graph, data flow, decision trees, integration patterns</contains>
<see_also>README.md, AGENTS.md</see_also>
</agent>

## Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          markymark Architecture                          │
└─────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────┐
                              │  tower-lsp  │
                              │  (LSP I/O)  │
                              └──────┬──────┘
                                     │
                                     ▼
┌──────────────┐            ┌─────────────────┐            ┌──────────────┐
│   bumpalo    │───────────▶│  markymark-lsp  │◀───────────│  thiserror   │
│   (arena)    │            │   (handlers)    │            │  (errors)    │
└──────────────┘            └────────┬────────┘            └──────────────┘
       │                             │
       │                             ▼
       │                    ┌─────────────────┐
       │                    │ markymark-index │
       └───────────────────▶│  (symbols +     │◀───────────┐
                            │   graph)        │            │
                            └────────┬────────┘            │
                                     │                     │
                                     ▼                     │
                            ┌─────────────────┐     ┌──────┴──────┐
                            │markymark-parser │     │  petgraph   │
                            │ (tree-sitter)   │     │  (graph)    │
                            └────────┬────────┘     └─────────────┘
                                     │
                                     ▼
                            ┌─────────────────┐
                            │  tree-sitter +  │
                            │ tree-sitter-md  │
                            └─────────────────┘

Testing layer (all crates):
┌─────────────┐     ┌─────────────┐
│    insta    │     │  proptest   │
│ (snapshots) │     │ (property)  │
└─────────────┘     └─────────────┘
```

## Data Flow

```
Input Document
     │
     ▼
┌─────────────────────────────────────┐
│  tree-sitter-markdown               │  Parse markdown text to CST
│  → tree-sitter queries              │
└─────────────────────────────────────┘
     │
     │ Node / Tree
     ▼
┌─────────────────────────────────────┐
│  markymark-parser                   │  Extract Element types
│  → Element enum (Heading, Link...)  │  Arena-allocated
│  → bumpalo for allocation           │
└─────────────────────────────────────┘
     │
     │ &[Element<'arena>]
     ▼
┌─────────────────────────────────────┐
│  markymark-index                    │  Build document index
│  → DocumentIndex (headings, links)  │
│  → RealmIndex (cross-doc lookup)    │
│  → ConnectionGraph (petgraph)       │
└─────────────────────────────────────┘
     │
     │ Index + Graph
     ▼
┌─────────────────────────────────────┐
│  markymark-lsp                      │  Handle LSP requests
│  → tower-lsp handlers               │
│  → thiserror for responses          │
└─────────────────────────────────────┘
     │
     │ LSP Response
     ▼
Editor/Client
```

## When to Use Each Crate

### tower-lsp
| Scenario | Use When |
|----------|----------|
| Building LSP server | Always - main framework |
| Custom LSP methods | `markymark/createRealm`, etc. |
| Sending notifications | Diagnostics, progress |
| State management | Document sync, capabilities |

**Don't use for:** Parsing, indexing, graph operations

### tree-sitter
| Scenario | Use When |
|----------|----------|
| Parsing markdown | Initial parse, incremental updates |
| Node traversal | Finding elements by position |
| Syntax queries | Pattern matching on AST |

**Don't use for:** Semantic analysis (that's markymark-index)

### petgraph
| Scenario | Use When |
|----------|----------|
| Connection tracking | Links between documents |
| Backref lookup | "Who links to this?" |
| Orphan detection | Documents with no incoming links |
| Cycle detection | Circular reference warnings |

**Don't use for:** Simple key-value lookups (use HashMap)

### bumpalo
| Scenario | Use When |
|----------|----------|
| Per-realm allocation | All parsed data for a realm |
| Temporary parsing state | Intermediate structures |
| Bulk deallocation | Realm destruction |

**Don't use for:** Long-lived mutable data, cross-realm sharing

### thiserror + anyhow
| Scenario | Use |
|----------|-----|
| Library error types | `thiserror` |
| Application errors | `anyhow` |
| LSP error responses | `thiserror` → `jsonrpc::Error` |
| CLI error handling | `anyhow` |

### insta + proptest
| Scenario | Use |
|----------|-----|
| Parser output tests | `insta` (snapshot) |
| Format/display tests | `insta` (snapshot) |
| "Never panics" tests | `proptest` (property) |
| Roundtrip tests | `proptest` (property) |
| Golden file tests | `insta` (bulk snapshots) |
| Edge case discovery | `proptest` (fuzzing) |

## Integration Patterns

### Realm Lifecycle

```rust
// Create realm with arena
let realm = Realm::new(id);

// Parse documents into realm's arena
realm.add_document(uri, content);

// Query via index and graph
let refs = realm.find_references(symbol);

// Destroy realm - O(1) cleanup
realm.destroy();
```

### LSP Request Flow

```rust
// 1. tower-lsp receives request
async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<...> {

    // 2. Look up document in realm
    let realm = self.get_realm(&params.uri)?;

    // 3. Find symbol at position (tree-sitter + index)
    let symbol = realm.find_symbol_at(position)?;

    // 4. Resolve definition via graph (petgraph)
    let definition = realm.graph.resolve(&symbol)?;

    // 5. Return LSP response
    Ok(Some(definition.to_location()))
}
```

### Testing Pattern

```rust
// 1. Property test: parser doesn't panic
proptest! {
    #[test]
    fn parser_no_panic(input in ".*") {
        let arena = Bump::new();
        let _ = parse(&arena, &input);
    }
}

// 2. Snapshot test: known inputs
#[test]
fn test_heading_parsing() {
    let arena = Bump::new();
    let result = parse(&arena, "# Hello");
    assert_debug_snapshot!(result);
}

// 3. Roundtrip test: parse → format → parse
proptest! {
    #[test]
    fn roundtrip(input in valid_markdown_strategy()) {
        let arena = Bump::new();
        let doc = parse(&arena, &input).unwrap();
        let formatted = format(&doc);
        let reparsed = parse(&arena, &formatted).unwrap();
        prop_assert_eq!(doc.headings.len(), reparsed.headings.len());
    }
}
```

## Decision Tree

```
Need to handle LSP request?
├── Yes → tower-lsp.md
│   └── Need error response? → error-handling.md (thiserror)
└── No
    │
    ├── Need to parse markdown?
    │   ├── Yes → tree-sitter.md
    │   └── No
    │       │
    │       ├── Need document/symbol relationships?
    │       │   ├── Yes → petgraph.md
    │       │   └── No
    │       │       │
    │       │       ├── Need fast allocation with bulk free?
    │       │       │   ├── Yes → bumpalo.md
    │       │       │   └── No → Use standard allocation
    │       │       │
    │       │       └── Writing tests?
    │       │           ├── Complex output comparison → testing.md (insta)
    │       │           ├── Invariant testing → testing.md (proptest)
    │       │           └── Both → testing.md (combined)
    │       │
    │       └── Defining error types?
    │           └── error-handling.md
```

## Version Compatibility

| Crate | Version | Notes |
|-------|---------|-------|
| tower-lsp | 0.20.x | Stable, async-trait |
| tree-sitter | 0.22.x | Breaking changes from 0.21 |
| tree-sitter-markdown | git | Use tree-sitter-grammars fork |
| petgraph | 0.6.x | Stable |
| bumpalo | 3.x | Stable, needs `collections` feature |
| thiserror | 1.x | Stable |
| anyhow | 1.x | Stable |
| insta | 1.x | Stable, use `yaml`/`json` features |
| proptest | 1.x | Stable |

## Related

- Individual crate docs: See README.md
- markymark design: `../plans/2026-02-05-markymark-design.md`
- Rust guidelines: `../rust_guidelines/`
