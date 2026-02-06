# Crate Relationship Map

<agent>
<goal>Understand how markymark crates relate to each other and when to use each.</goal>
<when_to_use>When planning implementation or understanding crate dependencies.</when_to_use>
<contains>Dependency graph, data flow, decision trees, integration patterns</contains>
<see_also>README.md, AGENTS.md</see_also>
<routing>
<rule>Rust workspace / ownership / async baseline -> core.md</rule>
<rule>LSP server / requests / capabilities -> tower-lsp.md</rule>
<rule>MCP server / tools / resources / prompts -> rmcp.md</rule>
<rule>Incremental parsing / edits / syntax nodes -> tree-sitter.md</rule>
<rule>Graph algorithms / backrefs / cycles -> petgraph.md</rule>
<rule>Arena allocation / bulk lifetimes -> bumpalo.md</rule>
<rule>Typed errors / anyhow boundary -> error-handling.md</rule>
<rule>Snapshots / proptest -> testing.md</rule>
</routing>
</agent>

## Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      markymark Architecture (Dual-Transport)             │
└─────────────────────────────────────────────────────────────────────────┘

┌──────────────────┐                              ┌──────────────────┐
│  tower-lsp-      │                              │     rmcp         │
│  server (LSP)    │                              │  (MCP SDK)       │
└────────┬─────────┘                              └────────┬─────────┘
         │                                                 │
         ▼                                                 ▼
┌─────────────────┐                              ┌─────────────────┐
│  markymark-lsp  │                              │  markymark-mcp  │
│  (LSP transport)│                              │  (MCP transport)│
└────────┬────────┘                              └────────┬────────┘
         │                                                │
         └──────────────────┬─────────────────────────────┘
                            │
                            ▼
                   ┌─────────────────┐
                   │  markymark-core │◀─────────── bumpalo (arena)
                   │  (CoreEngine +  │◀─────────── thiserror (errors)
                   │   types)        │
                   └────────┬────────┘
                            │
                            ▼
                   ┌─────────────────┐
                   │ markymark-index │◀─────────── petgraph (graph)
                   │  (symbols +     │
                   │   graph + realm)│
                   └────────┬────────┘
                            │
                            ▼
                   ┌─────────────────┐
                   │markymark-parser │
                   │ (tree-sitter)   │
                   └────────┬────────┘
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
│  markymark-core (CoreEngine)        │  Execute operations
│  → CoreOperation → CoreResult       │  Transport-agnostic
└─────────────────────────────────────┘
     │
     │ CoreResult
     ├────────────────────────────────────────┐
     ▼                                        ▼
┌─────────────────────────────┐  ┌─────────────────────────────┐
│  markymark-lsp              │  │  markymark-mcp              │
│  → tower-lsp-server handlers│  │  → rmcp ServerHandler       │
│  → LSP Response             │  │  → MCP Response             │
└──────────────┬──────────────┘  └──────────────┬──────────────┘
               │                                │
               ▼                                ▼
          Editor/IDE                     AI Assistant
```

## When to Use Each Crate

### tower-lsp-server
| Scenario | Use When |
|----------|----------|
| Building LSP server | Always - main LSP framework |
| Custom LSP methods | `$/createRealm`, etc. |
| Sending notifications | Diagnostics, progress |
| State management | Document sync, capabilities |

**Don't use for:** Parsing, indexing, graph operations, MCP transport

### rmcp
| Scenario | Use When |
|----------|----------|
| Building MCP server | Always - official MCP SDK |
| Defining tools | `#[tool]` macro on async methods |
| Exposing resources | Symbol data, outlines, graphs |
| AI prompts | explain-link, suggest-references |

**Don't use for:** Editor integrations (use tower-lsp-server), parsing, indexing

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
    ├── Need to handle MCP request (AI assistant)?
    │   ├── Yes → rmcp.md
    │   │   └── Need error response? → error-handling.md (thiserror)
    │   └── No
    │       │
    │       ├── Need to parse markdown?
    │       │   ├── Yes → tree-sitter.md
    │       │   └── No
    │       │       │
    │       │       ├── Need document/symbol relationships?
    │       │       │   ├── Yes → petgraph.md
    │       │       │   └── No
    │       │       │       │
    │       │       │       ├── Need fast allocation with bulk free?
    │       │       │       │   ├── Yes → bumpalo.md
    │       │       │       │   └── No → Use standard allocation
    │       │       │       │
    │       │       │       └── Writing tests?
    │       │       │           ├── Complex output comparison → testing.md (insta)
    │       │       │           ├── Invariant testing → testing.md (proptest)
    │       │       │           └── Both → testing.md (combined)
    │       │       │
    │       │       └── Defining error types?
    │       │           └── error-handling.md
```

## Version Compatibility

| Crate | Version | Notes |
|-------|---------|-------|
| tower-lsp-server | 0.23.x | Community fork, edition 2024, Rust 1.85+ |
| rmcp | 0.13.x | Official MCP SDK, pre-1.0 (API may evolve) |
| tree-sitter | 0.26.x | Current stable |
| tree-sitter-md | 0.5.x | Markdown grammar |
| petgraph | 0.8.x | Stable |
| bumpalo | 3.x | Stable, needs `collections` feature |
| thiserror | 1.x | Stable |
| anyhow | 1.x | Stable |
| insta | 1.x | Stable, use `yaml`/`json` features |
| proptest | 1.x | Stable |

## Related

- Individual crate docs: See README.md
- markymark design: `../plans/2026-02-05-markymark-design.md`
- Rust guidelines: `../rust_guidelines/`
