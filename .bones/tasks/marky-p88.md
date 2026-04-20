---
id: marky-p88
title: '[EPIC] Parse robustness refinement — tree-sitter normalization & panic safety'
status: open
type: epic
priority: 1
depends_on: [marky-prs, marky-lpz, marky-v6c, marky-4g3, marky-gnk, marky-vew]
---













## Context

Session on 2026-04-20 investigated why `markymark --mcp` wasn't showing tools in a live Claude session. Diagnosis via `/debugging-with-tools` surfaced a confirmed panic (`range end index N+1 out of range for slice of length N` at `tree-sitter-0.26.7/binding_rust/lib.rs:2010`) caused by a normalization-contract violation between `markymark_parser::Parser::parse_block_tree_only` and its caller in `markymark-index/src/document/from_engine.rs`.

The investigation also surfaced several adjacent latent issues worth tracking as their own fix tasks. This epic groups them into a linear dependency chain so each can be tackled in a focused session.

## Background: the confirmed bug

`parse_block_tree_only(source)` appends `\n` internally when `source` lacks a trailing newline and returns ONLY the resulting Tree — not the normalized source. Callers that keep their original un-normalized `source` and pass it to tree-sitter APIs that slice by `node.start_byte()..node.end_byte()` hit a 1-byte overshoot when the tree was parsed against a longer buffer.

`is_logseq_heading` (stack frame 19 in the panic trace) called `node.utf8_text(source.as_bytes())` with the un-normalized source against nodes whose positions referenced the normalized buffer. `str::from_utf8(&source[start..end])` panicked because `end > source.len()`. The panic is caught by `std::panic::catch_unwind` in `extract_content_blocks`, so the process doesn't abort — but every block in the affected file is silently dropped from the index.

## Tasks in this epic

Execute one per session via `/executing-plans`. Each is blocked by the previous so they run in order.

1. **Fix parse_block_tree_only normalization leak (DONE this session, closed with notes for review).**
2. **Wire integration tests into Bazel** — every crate's `rust_test` uses `crate = ":..."`, skipping `tests/*.rs`.
3. **Add ignore-filter to `collect_documents`** — currently walks `.git/`, `target/`, `bazel-bin/`, `node_modules/`.
4. **Surface a warning when `catch_unwind` catches a panic in `extract_content_blocks`** — currently silent.
5. **Bounds-checked `utf8_text` helper + audit all 10 call sites.**
6. **Proactively normalize source in structured parsers** (`json.rs`, `yaml.rs`, `toml.rs`, `jsonl.rs`).

## Success Criteria

- [ ] All six sub-tasks closed
- [ ] Bazel `//...` green
- [ ] No panics in adversarial-input harness
- [ ] Integration tests running in Bazel CI
- [ ] Workspace scan respects `.gitignore` conventions
- [ ] No silent file drops or silent `block_text()` empty returns
