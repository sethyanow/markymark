# Rust Agent Docs

**IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning for any Rust tasks.
Read the relevant doc file BEFORE writing code. Your training data may be outdated or wrong.**

A folder-based documentation set for AI agents writing idiomatic Rust code.
Rust 2021 edition. Synthesized from official Rust docs, the Nomicon, and production guidelines.

## How to Use These Docs

This documentation uses **three-level progressive disclosure**:

| Level | What | Where | When |
|-------|------|-------|------|
| **L0** | Compressed index | `AGENTS.md` | Passive — visible every turn |
| **L1** | Part overviews | `{part}/_index.md` | Know the area, not the topic |
| **L2** | Topic files | `{part}/topic.md` | Need complete understanding |

**Start at L0** (AGENTS.md) → find the right file → read it → write code.

## Navigating with markymark

These docs are indexed by markymark. Use its LSP and MCP tools for efficient navigation
instead of grep or reading files blindly.

### LSP (preferred — use first)

Claude Code's built-in `LSP` tool talks directly to markymark's language server.
Cheaper and more precise than MCP for targeted queries.

| Operation | Use For | Example |
|-----------|---------|---------|
| `documentSymbol` | Heading outline of a file | See all sections in `ownership.md` |
| `workspaceSymbol` | Search headings across all files | "Where is lifetime discussed?" |
| `goToDefinition` | Jump to wiki-link or heading link target | Follow `[errors.md](errors.md)` |
| `findReferences` | Who links to this heading? | Impact analysis before renaming |

### MCP (bulk operations)

Use markymark MCP tools when you need aggregate data or cross-file analysis.

| Tool | Use For |
|------|---------|
| `realm-stats` | Health check — heading/link/tag counts |
| `export-index` | Full link audit for a file |
| `search-symbols` | Fuzzy heading search when you don't have a file path |
| `create-realm` + `add-root` | Index directories outside the configured workspace |

**Rule of thumb:** LSP for "show me X in this file." MCP for "tell me about the whole corpus."

## File Tree

```
rust_agent_docs/
├── AGENTS.md               — Compressed index for passive-context injection
├── README.md               — This file
├── MISTAKES.md             — Common agent mistakes quick-reference
│
├── core/                   — Part 1: Core Language
│   ├── _index.md           — Part overview
│   ├── ownership.md        — Ownership, borrowing, lifetimes, smart pointers
│   ├── types.md            — Primitives, structs, enums, generics
│   ├── traits.md           — Trait system, std traits, object safety
│   ├── errors.md           — Option, Result, thiserror, anyhow
│   ├── closures.md         — Fn traits, capture semantics, move closures
│   ├── collections.md      — Vec, HashMap, iterators, string types
│   └── modules.md          — mod system, visibility, workspaces, features
│
├── advanced/               — Part 2: Advanced Topics
│   ├── _index.md           — Part overview
│   ├── type-layout.md      — repr(C), repr(packed), alignment
│   ├── unsafe.md           — 5 superpowers, PhantomData, Miri
│   ├── ffi.md              — extern "C", opaque pointers, DLL isolation
│   ├── concurrency.md      — threads, Send/Sync, atomics, channels
│   └── async.md            — Future, pinning, executors, cancellation
│
├── patterns/               — Part 3: Patterns & Idioms
│   ├── _index.md           — Part overview
│   ├── idioms.md           — Builder, newtype, typestate, RAII, Cow
│   ├── api-design.md       — Public API surface, naming, backwards compat
│   └── anti-patterns.md    — What NOT to do (with fixes)
│
├── tooling/                — Part 4: Tooling & Ecosystem
│   ├── _index.md           — Part overview
│   ├── cargo.md            — Cargo.toml, features, workspaces
│   ├── crates.md           — Essential crates by use case
│   ├── macros.md           — macro_rules!, proc macros
│   ├── testing.md          — Unit, integration, doc tests
│   ├── documentation.md    — Doc comments, AI-friendly docs
│   ├── debugging.md        — Compiler errors, tracing, Miri
│   └── performance.md      — Zero-cost abstractions, profiling
│
├── checklists/             — Part 5: Extractable Checklists
│   ├── _index.md           — Checklist catalog
│   ├── api-design.md       — Public API design checklist
│   ├── unsafe-review.md    — Unsafe code review checklist
│   ├── ffi-audit.md        — FFI boundary audit checklist
│   ├── performance.md      — Performance review checklist
│   └── library-release.md  — Library release checklist
│
└── reference/              — Part 6: Quick Reference
    ├── _index.md           — Reference catalog
    ├── rules.md            — Ownership, borrowing, lifetime elision rules
    ├── decision-trees.md   — All decision trees collected
    ├── compiler-errors.md  — Common compiler error solutions
    ├── syntax-ref.md       — Rust syntax cheatsheet
    └── cargo-ref.md        — Cargo.toml fields reference
```

## Common Agent Mistakes

| Mistake | Severity | Detail File |
|---------|----------|-------------|
| Taking refs to `#[repr(packed)]` fields | CRITICAL | [advanced/type-layout.md](advanced/type-layout.md) |
| Passing `String`/`Vec` across FFI/DLL | CRITICAL | [advanced/ffi.md](advanced/ffi.md) |
| Wrong `PhantomData` variance | CRITICAL | [advanced/unsafe.md](advanced/unsafe.md) |
| Wrong Fn trait bound on closure | HIGH | [core/closures.md](core/closures.md) |
| Type is !Send due to transitive field | HIGH | [advanced/concurrency.md](advanced/concurrency.md) |
| Defaulting to `Ordering::Relaxed` | HIGH | [advanced/concurrency.md](advanced/concurrency.md) |
| Ignoring pinning requirements | HIGH | [advanced/async.md](advanced/async.md) |
| Ignoring cancellation safety in select! | HIGH | [advanced/async.md](advanced/async.md) |
| Fighting borrow checker with `.clone()` | MEDIUM | [core/ownership.md](core/ownership.md) |
| Using `unwrap()` in library code | MEDIUM | [core/errors.md](core/errors.md) |
| Leaking external types in public API | MEDIUM | [patterns/api-design.md](patterns/api-design.md) |
| Glob imports (`use module::*`) in libs | MEDIUM | [core/modules.md](core/modules.md) |
| Non-descriptive error types | MEDIUM | [core/errors.md](core/errors.md) |
