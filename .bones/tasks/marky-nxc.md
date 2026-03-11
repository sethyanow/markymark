---
id: marky-nxc
title: 'Code quality review: decompose god files and methods'
status: open
type: epic
priority: 2
---

## Context

Full code quality review of the markymark Rust codebase (2026-03-11). Scanned ~100
production Rust files (~30k lines), excluding tests, benches, fixtures, and worktree
copies. Detection methods: LSP documentSymbol for method lengths and structure, Grep
for pattern scans, manual code reading to classify every hit.

**Methodology:** Anti-slop patterns (premature abstraction, enterprise cosplay, hollow
abstractions, verbose patterns), clean code violations (long methods, deep nesting,
magic numbers, god classes, poor naming), error handling (unwrap/expect in production,
swallowed errors, happy-path-only). Every detection hit was read and classified before
inclusion. False positives were filtered with reasoning.

**Overall health:** The codebase is in good shape — clean error handling via Result
types, no premature abstractions, good naming conventions, no magic numbers. The
findings are concentrated in two files that have grown organically as features were
added. Both are approaching or past the project's size limits.

## Requirements

### P0 — Must fix

1. **realm/mod.rs at 1011 lines breaches 1000-line HARD STOP.**
   `RealmIndex` has 14 fields and 30+ methods spanning cross-doc indexing, search,
   resolution, journal dates, and lifecycle. The retain-or-remove pattern
   (`entries.retain(..); if entries.is_empty() { map.remove(..); }`) is repeated 8
   times in `remove_from_cross_doc_indexes`.

2. **engine/mod.rs `execute()` is 559 lines (316-875).**
   Massive command dispatcher. The realm-lookup boilerplate (6 lines: resolve realm key,
   acquire read lock, check existence, return error) is copy-pasted ~15 times. The
   `AddRoot` arm alone is ~120 lines with 4-phase locking logic.

### P1 — Should fix

3. **server.rs at 978 lines — will breach 1000-line limit with any new LSP method.**
   Most mass is in the `LanguageServer` impl (lines 114-930). Three long methods:
   - `references()`: 153 lines (403-556), 5 match arms iterating all realm documents
   - `hover()`: 123 lines (558-681), match on 6 symbol types building markdown inline
   - `did_change()`: 117 lines (180-297), debounce with generation counters

### P2 — Nice to have

4. **Mutex::lock().unwrap() in server.rs** — 8 occurrences with bare `.unwrap()`.
   Replace with `.expect("debounce lock poisoned")` for debuggability.

5. **stack.pop().unwrap() in symbols.rs:203** — Safe due to prior `while let` check
   but invariant isn't documented. Replace with `.expect()`.

## Design

### Phase 1: realm/mod.rs decomposition (P0)

Split `RealmIndex` methods along cohesion seams:

- **realm/cross_doc.rs** — `remove_from_cross_doc_indexes`, `populate_cross_doc_indexes`,
  `patch_headings`, `patch_blocks`, `patch_code_spans`, `patch_stem`, `patch_journal_date`,
  `ensure_tags_clean`. Extract a `retain_or_remove()` helper to deduplicate the 8 repeated
  retain-if-empty blocks.
- **realm/search.rs** — `search_block_text`, `search_key_paths`, `lookup_heading`,
  `lookup_block`, `lookup_code_span`, `tag_counts`, `find_uri_by_stem`,
  `find_uri_by_relative_path`.
- **realm/journal.rs** — `lookup_journal_by_month`, `journal_date`, `detect_journal_date`
  helper.
- **realm/mod.rs** — Struct definition, `new`, `add_document*`, `update_document`,
  `remove_document`, `document_count`, `get_document`, `iter_*`, `semantic_*`, `Default`.

This uses Rust's `impl RealmIndex` split across files pattern (each file adds an `impl`
block on the same struct, re-exported from mod.rs).

### Phase 2: engine/mod.rs `execute()` extraction

- Extract `fn read_realm(&self, realm_name: Option<&str>) -> Result<(...), CoreOperationResult>`
  to eliminate the ~15 repeated realm-lookup blocks.
- Extract `AddRoot` arm into `fn handle_add_root(&self, realm: String, root: PathBuf) -> ...`
  (the 4-phase logic is complex enough to warrant its own function).
- Extract `GetContentBlocks` arm into its own function (inline filter/map logic).
- Leave simple delegating arms in the match (they're just 5-8 lines each).

### Phase 3: server.rs method extraction

- Extract symbol-specific hover builders: `hover_heading()`, `hover_wiki_link()`,
  `hover_xml_tag()`, `hover_code_span()`, `hover_markdown_link()`,
  `hover_structured_key()`.
- Extract references arms: `references_for_heading()`, `references_for_xml_tag()`,
  `references_for_structured_key()`, `references_for_wiki_link()`.
- Consider moving these into `server/hover.rs` and `server/references.rs` if server.rs
  still exceeds 500 lines after extraction.

### Phase 4: Low-severity cleanup (P2)

- Replace 8x `Mutex::lock().unwrap()` with `.expect("debounce lock poisoned")`.
- Replace `stack.pop().unwrap()` with `.expect("stack non-empty after last() check")`.

## Findings Detail

### HIGH Severity

| # | Pattern | Location | Lines | Evidence |
|---|---------|----------|-------|----------|
| 1 | God File | `markymark-index/src/realm/mod.rs` | 1011 | 14 fields, 30+ methods. `remove_from_cross_doc_indexes` is 100 lines with 8x retain-or-remove. `populate_cross_doc_indexes` is 72 lines mirroring it. |
| 2 | God Method | `markymark-mcp/src/engine/mod.rs:316-875` | 559 | `execute()` dispatcher. Realm-lookup boilerplate (6 lines) repeated ~15 times. `AddRoot` arm is ~120 lines with 4-phase lock protocol. |

### MEDIUM Severity

| # | Pattern | Location | Lines | Evidence |
|---|---------|----------|-------|----------|
| 3 | Long Method | `markymark-lsp/src/server.rs:403-556` | 153 | `references()` — 5 match arms, nested iteration over all realm documents with `resolve_wiki_link` calls. |
| 4 | Long Method | `markymark-lsp/src/server.rs:558-681` | 123 | `hover()` — match on 6 symbol types, XmlTag arm alone is 42 lines of string building. |
| 5 | Long Method | `markymark-lsp/src/server.rs:180-297` | 117 | `did_change()` — debounce with generation counters, spawned async task, multiple mutex acquisitions. |
| 6 | File approaching limit | `markymark-lsp/src/server.rs` | 978 | Will breach 1000-line HARD STOP with any new LSP method. |
| 7 | DRY violation | `markymark-index/src/realm/mod.rs:352-452` | 100 | Retain-or-remove pattern repeated 8 times across 4 index types (headings, blocks, tags, code spans). |
| 8 | DRY violation | `markymark-mcp/src/engine/mod.rs` | ~90 | Realm-lookup boilerplate (6 lines) copy-pasted ~15 times in `execute()`. |

### LOW Severity

| # | Pattern | Location | Occurrences | Evidence |
|---|---------|----------|-------------|----------|
| 9 | Bare unwrap on Mutex | `markymark-lsp/src/server.rs:171,207,232,265,293,305,941,961,975` | 8 | `self.debounce.lock().unwrap()` — bare unwrap gives no context in panic message. |
| 10 | Bare unwrap on pop | `markymark-lsp/src/symbols.rs:203` | 1 | `stack.pop().unwrap()` safe due to control flow but invariant undocumented. |

### Scanned But Clear

- **Premature Abstraction**: All 7 traits have multiple production impls or are API extension points (`ScanBackend`, `TypedFrontmatter`, `EdgeKind`, `GraphNode`, `CoreEngine`, `EmbeddingProvider`, `ArenaStringExt`).
- **Enterprise Cosplay**: No low-reference Factory/Builder/Strategy/Observer patterns.
- **Hollow Abstractions**: MCP `handle_*` functions add validation and mapping, not pure delegation.
- **Verbose Patterns**: No explicit boolean returns or unnecessary else-after-return.
- **Magic Numbers**: None in production code (all in tests or well-known constants).
- **Poor Naming**: Good — `handle_*` for handlers, descriptive method names throughout.
- **Deep Nesting**: All 4+ indent hits were data initialization or algorithm logic, not control flow.
- **Swallowed Errors**: Rust's type system enforces handling. No bare catches or ignored Results.

## Success Criteria

- [ ] `markymark-index/src/realm/mod.rs` under 500 lines (struct + core lifecycle methods only)
- [ ] Cross-doc index methods extracted to `realm/cross_doc.rs` with `retain_or_remove` helper
- [ ] Search/lookup methods extracted to `realm/search.rs`
- [ ] Journal methods extracted to `realm/journal.rs`
- [ ] `execute()` in engine/mod.rs under 200 lines via realm-lookup helper and arm extraction
- [ ] `references()` in server.rs under 40 lines (arms extracted to helpers)
- [ ] `hover()` in server.rs under 30 lines (builders extracted to helpers)
- [ ] server.rs total under 700 lines
- [ ] 8x `Mutex::lock().unwrap()` replaced with `.expect()`
- [ ] `stack.pop().unwrap()` replaced with `.expect()`
- [ ] All existing tests pass (no behavioral changes)
- [ ] Clippy clean across workspace

## Anti-Patterns

- Do NOT add new abstractions (traits, generics) — this is a decomposition, not a redesign.
- Do NOT change public API signatures — callers should not need updates.
- Do NOT move struct fields or change visibility — only move `impl` blocks.
- Do NOT combine decomposition with feature work — pure refactoring only.
- Do NOT target arbitrary line counts per output file — split along cohesion seams, not line budgets.

## Key Considerations (SRE Review)

- **Rust `impl` split pattern**: Multiple `impl SameStruct` blocks in different files within
  the same module is idiomatic Rust. Each file uses `use super::*` to access the struct.
  This preserves `pub(crate)` visibility without changes.
- **Test file stays put**: `realm/tests.rs` (`mod tests`) should remain as-is. Tests exercise
  the public API which doesn't change.
- **Phase ordering matters**: Phase 1 (realm) must complete before Phase 3 (server) because
  server.rs calls realm methods — if realm's module structure changes, imports in server.rs
  may need updating. Phase 2 (engine) is independent.
- **Line targets are outcomes, not goals**: The success criteria line counts are based on what
  remains after extracting the identified methods. If the actual split lands at 520 instead
  of 500, that's fine — the goal is cohesion, not a number. The only hard constraint is the
  1000-line HARD STOP.
- **`did_change()` is intentionally left as-is in Phase 3**: Its complexity is in interlocking
  state (debounce handles, generation counters, async spawns), not in switch-on-type dispatch.
  Extracting pieces would scatter related state logic across functions without improving
  readability. The method is well-commented with ticket references (T1-1, T2-2, T2-4, T2-8,
  marky-aemm).
