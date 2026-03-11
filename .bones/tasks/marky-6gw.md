---
id: marky-6gw
title: '[EPIC] tree-sitter migration: 0.19 → 0.26 + tree-sitter-md'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
---






## Design

## Requirements (IMMUTABLE)

1. Upgrade tree-sitter from =0.19.5 to 0.26
2. Replace dead tree-sitter-markdown 0.7.1 (ikatyang, archived 2021) with tree-sitter-md 0.5.x (tree-sitter-grammars, active)
3. Upgrade tree-sitter-json from =0.19.0 to 0.24.x
4. Remove unused tree-sitter-xml 0.6 dependency (XML-in-markdown uses custom tokenizer, not tree-sitter)
5. Adopt tree-sitter-md MarkdownParser wrapper (two-grammar: block + inline)
6. All existing tests pass — zero regression
7. XML tag extraction (custom stack tokenizer in extract.rs) fully preserved
8. Pre-commit hooks passing, cargo clippy clean

## Success Criteria (MUST ALL BE TRUE)

- [ ] Single tree-sitter version in dependency graph (no duplicate builds)
- [ ] tree-sitter-md MarkdownParser produces equivalent parse results for all existing test fixtures
- [ ] Node kind string changes verified and updated (tight_list/loose_list → list)
- [ ] JSON structured parser works with tree-sitter-json 0.24
- [ ] All 391+ existing tests pass
- [ ] XML extraction tests pass (unchanged)
- [ ] cargo clippy --workspace --all-targets clean
- [ ] Pre-commit hooks (lefthook) passing

## Anti-Patterns (FORBIDDEN)

- NO removing or degrading XML-in-markdown extraction (it's custom code, not tree-sitter-xml)
- NO changing the public Parser/Ast API surface (downstream crates depend on abstractions, not tree-sitter types)
- NO mixing block-only and wrapper approaches — fully commit to MarkdownParser wrapper
- NO keeping tree-sitter-xml dependency (it's dead code)

## Scope

### In scope
- Cargo.toml dependency version updates
- markymark-parser/src/lib.rs: Parser init migration to MarkdownParser
- markymark-parser/src/types.rs: Node kind string updates
- markymark-parser/src/ast.rs: Tree traversal updates for two-grammar architecture
- markymark-parser/src/structured/json.rs: tree-sitter-json 0.24 API (LANGUAGE constant, &Language ref)
- All parser test files (5 files)

### Out of scope
- extract.rs XML tokenizer (untouched — uses regex/custom parsing, not tree-sitter)
- markymark-index, markymark-lsp, markymark-mcp, markymark-cli (no direct tree-sitter imports)
- Multi-format epic (marky-lkj) remaining work — independent track
- Inline grammar exploitation for wiki-link/link extraction improvement (future follow-up)

## Architecture

### API Changes (tree-sitter 0.19 → 0.26)

**Language loading:**
- OLD: tree_sitter_markdown::language() → Language (Copy, by value)
- NEW: tree_sitter_md::LANGUAGE → LanguageFn, converted via .into() to Language (non-Copy, by &reference)

**Parser::set_language:**
- OLD: parser.set_language(lang) — by value
- NEW: parser.set_language(&lang) — by reference

**Node API:** Unchanged (kind, children, utf8_text, start_position, end_position, byte_range all stable)

**Query API (if used):**
- Query::new() takes &Language not Language
- capture_names() returns &[&str] not &[String]
- QueryCursor::matches/captures take TextProvider (pass source.as_bytes())

### tree-sitter-md Two-Grammar Architecture

tree-sitter-md 0.5 splits markdown into:
- Block grammar (LANGUAGE): headings, lists, code blocks, paragraphs, tables
- Inline grammar (INLINE_LANGUAGE): emphasis, links, code spans

High-level wrapper: MarkdownParser → MarkdownTree
- MarkdownTree.block_tree() for block-level CST
- MarkdownTree.inline_tree(node) for inline content within a block
- MarkdownTree.walk() for unified traversal via MarkdownCursor

### Node Kind Changes (block grammar)

| Old (tree-sitter-markdown 0.7) | New (tree-sitter-md 0.5) | Action |
|---|---|---|
| atx_heading | atx_heading | No change |
| setext_heading | setext_heading | No change |
| paragraph | paragraph | No change |
| list_item | list_item | No change |
| tight_list / loose_list | list | Update match arms |
| atx_h1_marker..atx_h6_marker | Same | Verify |
| html_block | html_block | Verify |

### File Impact Matrix

| File | Lines | Change scope |
|---|---|---|
| Cargo.toml (workspace) | 4 lines | Version bumps, remove tree-sitter-xml, add tree-sitter-md |
| markymark-parser/Cargo.toml | 3 lines | Same |
| markymark-parser/src/lib.rs | 62 lines | Parser init → MarkdownParser wrapper |
| markymark-parser/src/types.rs | 732 lines | Node kind string updates |
| markymark-parser/src/ast.rs | 199 lines | Tree traversal for MarkdownTree |
| markymark-parser/src/structured/json.rs | 318 lines | LANGUAGE constant + &ref |
| 5 test files | ~200 lines combined | Parser::new() → updated init |

### Dependency Resolution (after migration)

All grammar crates use tree-sitter-language 0.1 as bridge → single tree-sitter 0.26 build.
No more duplicate C runtime compilation.

## Design Rationale

### Why now
- tree-sitter-markdown 0.7.1 is from April 2021, published by an archived repo
- tree-sitter-md 0.5.2 is from January 2026, actively maintained by tree-sitter-grammars org
- Currently compiling two copies of tree-sitter C runtime (0.19 + 0.22 via tree-sitter-xml)
- tree-sitter-json pinned to =0.19.0, blocking access to grammar improvements
- Modern ecosystem uses tree-sitter-language bridge crate — eliminates version lockstep

### Why MarkdownParser wrapper (Option B)
- Richer parse output (block + inline trees) enables future improvements
- High-level API handles two-grammar coordination automatically
- Incremental edit support built-in via MarkdownTree.edit()
- Used by major projects (Biome, etc.)
