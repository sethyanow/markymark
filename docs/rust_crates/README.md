# Rust Crate Documentation Hub

<agent>
<goal>Find crate-specific patterns, APIs, and pitfalls quickly for Rust development.</goal>
<entrypoint>Start here, then jump to the crate doc you need.</entrypoint>
<docs_index>
|core:{core.md}
|lsp:{tower-lsp.md}
|parsing:{tree-sitter.md}
|graphs:{petgraph.md}
|memory:{bumpalo.md}
|errors:{error-handling.md}
|testing:{testing.md}
|navigation:{map.md}
</docs_index>
</agent>

This hub provides agent-friendly documentation for Rust crates used in markymark and other projects. Each document focuses on practical patterns, common pitfalls, and working examples.

## Crates by Domain

### Core Rust
- **core.md** - Rust language/std/Cargo workspace patterns for implementation work

### LSP Development
- **tower-lsp.md** - LSP server framework: async handlers, state management, capabilities

### Parsing
- **tree-sitter.md** - Incremental parsing: tree-sitter-markdown, queries, node traversal

### Data Structures
- **petgraph.md** - Graph library: directed graphs, algorithms, traversals
- **bumpalo.md** - Arena allocation: bump allocators, memory efficiency

### Error Handling
- **error-handling.md** - thiserror + anyhow patterns: when to use each, conversion

### Testing
- **testing.md** - insta snapshots + proptest property-based testing

## Quick Decision Guide

| Need | Crate | Doc |
|------|-------|-----|
| Rust workspace + fundamentals | (core) | core.md |
| Build LSP server | tower-lsp | tower-lsp.md |
| Parse markdown/code | tree-sitter | tree-sitter.md |
| Connection graph | petgraph | petgraph.md |
| Fast allocation | bumpalo | bumpalo.md |
| Library errors | thiserror | error-handling.md |
| App errors | anyhow | error-handling.md |
| Snapshot tests | insta | testing.md |
| Property tests | proptest | testing.md |

## Navigation

- Relationship map: `map.md`
- Agent instructions: `AGENTS.md`

## Related Documentation

- Rust guidelines: `../rust_guidelines/`
- markymark design: `../plans/2026-02-05-markymark-design.md`
