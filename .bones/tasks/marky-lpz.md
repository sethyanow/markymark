---
id: marky-lpz
title: Wire integration tests into Bazel across all crates
status: open
type: task
priority: 2
depends_on: [marky-prs]
parent: marky-p88
---





## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

Every crate's `rust_test` target in its `BUILD.bazel` uses `crate = ":..."`, which only runs the library's internal `#[cfg(test)]` unit tests. The integration test files in each crate's `tests/` directory (`document_index.rs`, `connection_graph.rs`, `realm_index.rs`, `ast_self_cell.rs`, `tree_sitter_integration.rs`, `structured_json5.rs`, etc.) run under `cargo test` but **never under `bazel test //...`**.

This means our Bazel CI has been giving false confidence: a green Bazel run doesn't actually exercise any integration tests. Bug marky-prs (the panic fix) would have been caught pre-merge had the integration tests run in Bazel — or rather, the tests for it would have.

`markymark-index/BUILD.bazel` was updated this session to add one such target (`parse_robustness_test`) as a stopgap, but the pattern needs to be applied across all crates.

## Requirements

1. Inventory every `tests/*.rs` file across all crates.
2. For each, add a `rust_test` target in the corresponding `BUILD.bazel` with:
   - `srcs = ["tests/<name>.rs"]`
   - `edition = "2021"`
   - `deps` — the library target + any dev-deps the test imports
3. A Starlark macro or explicit list — either is fine, as long as the mapping is obvious and discoverable.
4. All new targets must pass `bazel test //...`.
5. No integration test currently passing under cargo may fail under Bazel (investigate and surface if so — don't paper over).

## Investigation notes

- Existing pattern to follow: the `parse_robustness_test` target added in marky-p88.1 (`markymark-index/BUILD.bazel`).
- Helper: `find . -path ./target -prune -o -path './*/tests/*.rs' -print` to enumerate.
- Each crate may need different deps (tokio for async tests, serde_json for fixture tests, etc.). Walk them case-by-case.

## Success Criteria

- [ ] Every `*/tests/*.rs` has a corresponding Bazel `rust_test` target
- [ ] `bazel test //...` runs them all (target count > 8)
- [ ] No test that passes under `cargo test --workspace` fails under Bazel
- [ ] Follow-up note: consider whether CI workflow needs updating to surface new targets
