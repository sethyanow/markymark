# markymark Implementation Plan

**Date:** 2026-02-05
**Design Doc:** [markymark-design.md](./2026-02-05-markymark-design.md)

## Phase 0: Project Setup (Foundation)

### 0.1 Repository & Tooling
- [ ] Create new repository `markymark`
- [ ] Initialize Cargo workspace with crate structure
- [ ] Set up CI/CD pipeline (GitHub Actions)
- [ ] Configure rustfmt, clippy, pre-commit hooks
- [ ] Add LICENSE, README, CONTRIBUTING

### 0.2 Dependencies & Scaffolding
- [ ] Add workspace dependencies (tower-lsp-server, rmcp, tree-sitter, petgraph, bumpalo)
- [ ] Create empty crate shells with pub interfaces
- [ ] Set up test infrastructure (insta, proptest)
- [ ] Create test harness crate

**Deliverable:** Empty but buildable workspace with CI green

---

## Phase 1: Core Types (markymark-core)

### 1.1 Basic Types
- [ ] `Range`, `Position`, `Location` types
- [ ] `DocumentUri` with path utilities
- [ ] `RealmId`, `RootId` identifiers

### 1.2 Configuration Types
- [ ] `RealmConfig`, `RealmSettings`, `RealmMode`
- [ ] `MarkdownFlavor` enum with Obsidian/Logseq variants
- [ ] `SlugMode` and slug generation functions
- [ ] `WikiStyle` configuration

### 1.3 Element Types
- [ ] `WikiLink`, `WikiTarget` variants
- [ ] `MdLink` with anchor parsing
- [ ] `BlockRef` (Obsidian + Logseq)
- [ ] `Embed` variants
- [ ] `Tag`, `TagStyle`
- [ ] `Property`, `PropertyValue`
- [ ] `Frontmatter` (YAML + Logseq)
- [ ] `Task`, `TaskState`, `TaskPriority`
- [ ] `ListItem` with recursive children
- [ ] `SpecialBlock` (Callout, Query, OrgBlock, Code, Math)
- [ ] Unified `Element` enum

### 1.4 Symbol Types
- [ ] `Symbol` enum (Document, Heading, Block, LinkDef, Tag)
- [ ] `BlockLocation`, `BlockPath`, `BlockAddress`

**Deliverable:** Core types compile, unit tests for slug generation

---

## Phase 2: Parser (markymark-parser)

### 2.1 Tree-sitter Integration
- [ ] Integrate tree-sitter-markdown
- [ ] Create CST traversal utilities
- [ ] Implement incremental edit support (`tree.edit()`)

### 2.2 Element Extraction
- [ ] Extract headings with slug computation
- [ ] Extract wiki links (all variants)
- [ ] Extract markdown links with anchor parsing
- [ ] Extract block references
- [ ] Extract embeds
- [ ] Extract tags (simple, nested, multi-word)
- [ ] Extract link definitions

### 2.3 Obsidian-Specific
- [ ] Parse callouts (`> [!type]`)
- [ ] Parse block IDs (`^id`)
- [ ] Parse file embeds (`![[file]]`)
- [ ] Parse comments (`%%comment%%`)

### 2.4 Logseq-Specific
- [ ] Parse block refs (`((uuid))`)
- [ ] Parse block embeds (`{{embed ((uuid))}}`)
- [ ] Parse inline properties (`property:: value`)
- [ ] Parse query blocks (`{{query ...}}`)
- [ ] Parse org-mode blocks (`#+BEGIN_`)
- [ ] Handle deep list nesting (10+ levels)

### 2.5 Frontmatter
- [ ] Parse YAML frontmatter
- [ ] Parse Logseq page properties
- [ ] Extract typed property values

### 2.6 Port Marksman Tests
- [ ] Port ParserTests.fs as golden files
- [ ] Add property tests (parse never panics)

**Deliverable:** Parser handles all Obsidian/Logseq syntax, golden tests pass

---

## Phase 3: Indexing (markymark-index)

### 3.1 Document Index
- [ ] `DocumentIndex` structure
- [ ] Build index from parsed elements
- [ ] Heading lookup by slug
- [ ] Block lookup by ID
- [ ] TOC generation
- [ ] Outline tree construction

### 3.2 Realm Index
- [ ] `RealmIndex` structure
- [ ] Document registry with URI mapping
- [ ] Global heading table
- [ ] Global block ID table
- [ ] Global tag table with usage tracking

### 3.3 Connection Graph
- [ ] `ConnectionGraph` with petgraph
- [ ] Forward edges (reference → definition)
- [ ] Backward edges (backrefs)
- [ ] Unresolved reference tracking
- [ ] Ambiguous reference detection

### 3.4 Reference Resolution
- [ ] Wiki link resolution (page, heading, block)
- [ ] Markdown link resolution
- [ ] Block ref resolution (Obsidian + Logseq)
- [ ] Embed resolution
- [ ] Cross-root resolution (shared mode)
- [ ] Isolation enforcement (isolated mode)

### 3.5 Incremental Updates
- [ ] Symbol diff computation
- [ ] Four-phase update algorithm
- [ ] Dependency tracking for cascading updates
- [ ] `last_touched` tracking for diagnostics

### 3.6 Port Marksman Tests
- [ ] Port ConnTest.fs cases
- [ ] Port RefsTests.fs cases
- [ ] Add property tests (incremental = full rebuild)

**Deliverable:** Index builds correctly, incremental updates work, resolution tests pass

---

## Phase 3.5: Transport Abstraction (markymark-core)

- [ ] Define `CoreOperation` enum with all transport-agnostic operations
- [ ] Define `CoreResult` enum for operation results
- [ ] Define `CoreError` type with thiserror
- [ ] Define `CoreEngine` trait (async execute method)
- [ ] Implement `CoreEngine` for the indexing layer (bridges to RealmIndex/ConnectionGraph)
- [ ] Write tests for CoreEngine with mock realm data

**Deliverable:** Core engine processes all operations, transport crates can depend on it

---

## Phase 4: LSP Transport (markymark-lsp)

### 4.1 Server Scaffolding
- [ ] tower-lsp-server integration (community fork)
- [ ] Initialize/shutdown handlers
- [ ] Document sync (open/change/close)
- [ ] LSP handlers convert to CoreOperation internally

### 4.2 Realm Router
- [ ] URI to realm mapping
- [ ] Prefix tree for fast lookup
- [ ] Dynamic realm/root management

### 4.3 Navigation
- [ ] `textDocument/definition`
- [ ] `textDocument/references`
- [ ] `textDocument/hover`

### 4.4 Completion
- [ ] Context detection (wiki link, block ref, tag, property)
- [ ] Wiki link completion
- [ ] Block ref completion
- [ ] Tag completion
- [ ] Property key completion

### 4.5 Rename (Critical Path)
- [ ] `textDocument/prepareRename`
- [ ] `textDocument/rename`
- [ ] **Anchor link updates** (the Marksman fix)
- [ ] Cross-file rename propagation

### 4.6 Diagnostics
- [ ] Broken link detection
- [ ] Ambiguous reference warnings
- [ ] Duplicate heading slugs

### 4.7 Symbols
- [ ] `textDocument/documentSymbol`
- [ ] `workspace/symbol`

### 4.8 Extended Methods
- [ ] `markymark/createRealm`
- [ ] `markymark/destroyRealm`
- [ ] `markymark/addRoot`
- [ ] `markymark/removeRoot`
- [ ] `markymark/realmStats`
- [ ] `markymark/exportIndex`

### 4.9 Port Marksman Tests
- [ ] Port ComplTests.fs cases
- [ ] Port SymbolsTests.fs cases
- [ ] Port RefactorTests.fs cases

**Deliverable:** Full LSP functionality, rename updates anchors

---

## Phase 4.5: MCP Transport (markymark-mcp)

### 4.5.1 Server Scaffolding
- [ ] rmcp `ServerHandler` implementation
- [ ] `#[tool]` macro setup for tool definitions
- [ ] Transport configuration (stdio, SSE)

### 4.5.2 Resources
- [ ] `markymark/symbol` resource
- [ ] `markymark/outline` resource
- [ ] `markymark/dependency-graph` resource (json/dot formats)
- [ ] `markymark/updates` resource (streaming)

### 4.5.3 Tools
- [ ] `markymark/rename` tool
- [ ] `markymark/find-references` tool
- [ ] `markymark/create-realm` / `markymark/destroy-realm` tools
- [ ] `markymark/search-symbols` tool

### 4.5.4 Prompts
- [ ] `markymark/explain-link` prompt
- [ ] `markymark/suggest-references` prompt

**Deliverable:** MCP transport exposes full markymark capabilities to AI assistants

---

## Phase 5: CLI & Distribution (markymark-cli)

### 5.1 CLI Binary
- [ ] Argument parsing (clap)
- [ ] Transport selector (--lsp / --mcp flags)
- [ ] Stdio transport mode
- [ ] TCP transport mode (optional)
- [ ] Config file support
- [ ] Transport selection via CLI flags or config

### 5.2 Build Optimization
- [ ] LTO configuration
- [ ] Strip symbols
- [ ] Binary size verification (<5MB)

### 5.3 Distribution
- [ ] Multi-platform builds in CI
- [ ] GitHub releases
- [ ] Homebrew formula
- [ ] Cargo install support

**Deliverable:** Distributable binaries for all targets

---

## Phase 6: Integration & Polish

### 6.1 E2E Testing
- [ ] Real LSP client tests
- [ ] Neovim integration test
- [ ] VS Code extension smoke test

### 6.2 Performance
- [ ] Benchmark suite (Criterion)
- [ ] Memory profiling
- [ ] Incremental edit latency verification

### 6.3 Documentation
- [ ] API documentation
- [ ] User guide
- [ ] Editor integration guides

### 6.4 Virtual FS Support
- [ ] Abstract file system trait
- [ ] In-memory FS for testing
- [ ] Forge integration hooks

**Deliverable:** Production-ready release

---

## Dependencies Between Phases

```
Phase 0 (Setup)
    │
    ▼
Phase 1 (Core Types)
    │
    ▼
Phase 2 (Parser) ──────┐
    │                  │
    ▼                  ▼
Phase 3 (Index) ◄──────┘
    │
    ▼
Phase 3.5 (Transport Abstraction)
    │
    ├──────────────────┐
    ▼                  ▼
Phase 4 (LSP)    Phase 4.5 (MCP)
    │                  │
    └────────┬─────────┘
             ▼
Phase 5 (CLI)
    │
    ▼
Phase 6 (Polish)
```

---

## Estimated Scope

| Phase | Tasks | Complexity |
|-------|-------|------------|
| 0 | 10 | Low |
| 1 | 15 | Low-Medium |
| 2 | 20 | Medium-High |
| 3 | 18 | High |
| 3.5 | 6 | Medium |
| 4 | 25 | High |
| 4.5 | 15 | Medium-High |
| 5 | 8 | Low |
| 6 | 12 | Medium |

**Total:** ~129 discrete tasks

---

## Risk Areas

1. **tree-sitter-markdown coverage** - May need custom extensions for Obsidian/Logseq
2. **Incremental update correctness** - Complex state machine, needs thorough testing
3. **Performance under churn** - High-frequency realm creation/destruction
4. **Cross-platform builds** - Windows and ARM targets
5. **MCP SDK maturity** - rmcp is v0.13, API may evolve before 1.0

---

## Success Criteria

- [ ] All Marksman golden tests pass
- [ ] Rename updates anchor links correctly
- [ ] Realm creation/destruction < 10ms
- [ ] Incremental edit < 50ms on 1000-heading document
- [ ] Binary size < 5MB
- [ ] Memory stable over 24hr stress test
