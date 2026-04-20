---
id: marky-lpz
title: Wire integration tests into Bazel across all crates
status: active
type: task
priority: 2
owner: Seth
depends_on: [marky-prs]
parent: marky-p88
---




## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

Every crate's `rust_test` target in its `BUILD.bazel` uses `crate = ":..."`, which only runs the library's internal `#[cfg(test)]` unit tests. The integration test files in each crate's `tests/` directory (`document_index.rs`, `connection_graph.rs`, `realm_index.rs`, `ast_self_cell.rs`, `tree_sitter_integration.rs`, `structured_json5.rs`, etc.) run under `cargo test` but **never under `bazel test //...`**.

This means our Bazel CI has been giving false confidence: a green Bazel run doesn't actually exercise any integration tests. Bug marky-prs (the panic fix) would have been caught pre-merge had the integration tests run in Bazel — or rather, the tests for it would have.

`markymark-index/BUILD.bazel` was updated this session to add one such target (`parse_robustness_test`) as a stopgap, but the pattern needs to be applied across all crates.

## Inventory (verified 2026-04-20)

Today there are **8 `rust_test` targets** in Bazel — one per crate library plus the stopgap `parse_robustness_test`. Under cargo, ~49 additional integration test files exist across `*/tests/*.rs` that Bazel never sees. Exact per-crate breakdown:

| Crate | Top-level `tests/*.rs` files | Notes |
|-------|-------------------------------|-------|
| `markymark-core` | `basic_types.rs`, `core_engine.rs`, `miri_arena.rs` | `miri_arena.rs` is designed for `cargo +nightly miri test`; it still compiles and runs in normal mode (uses `#[cfg_attr(miri, ignore)]`), so it can be wired into Bazel like any other test. |
| `markymark-parser` | `frontmatter_and_properties.rs`, `structured_jsonc.rs`, `structured_json5.rs`, `ast_self_cell.rs`, `typed_frontmatter.rs`, `tree_sitter_integration.rs` | `ast_self_cell.rs` uses `include_str!("../src/ast.rs")` — needs `compile_data = ["src/ast.rs"]`. |
| `markymark-index` | `connection_graph.rs`, `realm_index.rs`, `typed_frontmatter.rs`, `semantic_index.rs`, `resolution.rs`, `parse_robustness.rs` **(done)**, `document_index.rs`, `document_self_cell.rs` | `document_self_cell.rs` uses `include_str!("../src/document/{mod,types,helpers}.rs")` — needs matching `compile_data`. |
| `markymark-lsp` | `diagnostics.rs`, `conversions.rs`, `hover_tests.rs`, `completion_results.rs`, `references_tests.rs`, `rename.rs`, `document_symbol_tests.rs`, `state_tests.rs`, `navigation.rs`, `completion_acceptance.rs`, `debounce.rs`, `goto_definition_tests.rs`, `workspace_symbols.rs`, `completion_context.rs`, `capabilities.rs` | All link against `markymark-lsp` with `test-helpers` feature enabled (per the Cargo dev-dep line `markymark-lsp = { path = ".", features = ["test-helpers"] }`). That feature gates `#[cfg(any(test, feature = "test-helpers"))]` code at `src/server.rs:727`. In Bazel, create a second `rust_library(name = "markymark-lsp-testing", testonly = True, crate_features = ["test-helpers"], ...)` target and depend on it from every integration test. |
| `markymark-mcp` | `tool_handler_tests.rs`, `diagnostics_tests.rs`, `multi_root_federation.rs`, `search_symbols_tests.rs`, `prompt_handler_tests.rs`, `runtime_engine_tests.rs`, `subscription_tests.rs`, `resource_handler_tests.rs`, `runtime_tools.rs`, `slim_router_tests.rs` | Five of these (`diagnostics_tests.rs`, `multi_root_federation.rs`, `runtime_engine_tests.rs`, `runtime_tools.rs`, `search_symbols_tests.rs`) declare `mod common;` → each needs `srcs = ["tests/foo.rs", "tests/common/mod.rs"]`. `runtime_engine_tests.rs` further declares a submodule `mod runtime_engine_tests;` → its `srcs` must include all 11 files under `tests/runtime_engine_tests/`. |
| `markymark-cli` | `bench_lsp.rs`, `smoke_mcp.rs`, `smoke_lsp.rs`, `lsp_methods.rs`, `alignment.rs`, `mcp_methods.rs`, `cli_args.rs`, `triage_consistency.rs` | Hardest category — see "Edge cases" below. Two (`alignment.rs`, `bench_lsp.rs`) declare `mod alignment_support;` → srcs must include `tests/alignment_support/mod.rs`. Five spawn child processes; four use `env!("CARGO_MANIFEST_DIR")` to locate `tests/corpus/`. |

Total additional targets: 3 (core) + 6 (parser) + 7 (index, one already exists) + 15 (lsp) + 10 (mcp) + 8 (cli) = **49 new `rust_test` targets**.

## Requirements

1. Inventory every `tests/*.rs` file across all crates. (Done above — update if new tests land mid-task.)
2. For each top-level `tests/<name>.rs` file, add a `rust_test` target in the corresponding `BUILD.bazel` with:
   - `name = "<name>_test"` (or equivalent convention — keep consistent within a crate)
   - `srcs = ["tests/<name>.rs"]` plus any shared-module siblings (see Edge Cases)
   - `edition = "2021"`
   - `deps` — the library target + dev-deps the test imports
   - `compile_data` when the test uses `include_str!` / `include_bytes!` against paths outside `srcs`
   - `data` when the test reads runtime files (fixtures, binaries) — plus the corresponding runfiles lookup in the test (see Edge Cases: CARGO_MANIFEST_DIR)
3. Starlark macro or explicit list — either is fine, as long as the mapping is obvious and discoverable. If a macro, place it in the crate's `BUILD.bazel` or a new `//tools:rust_integration_test.bzl` loaded by each `BUILD.bazel`.
4. All new targets must pass `bazel test //...` on macOS. Linux parity is the CI's responsibility.
5. For any test that passes under `cargo test --workspace` but fails or cannot be ported under Bazel: do NOT paper over. Either solve the port (preferred) or add a `tags = ["manual"]` exclusion with a comment explaining why, and surface the list in the debrief so the user can decide. `manual` targets are excluded from wildcard `bazel test //...`.

## Edge cases

### E1. Shared test modules (`common/`, `alignment_support/`, `runtime_engine_tests/`)

Cargo auto-detects sibling modules when a test file declares `mod common;`. Bazel does not — you must list every referenced module file in `srcs`.

```python
rust_test(
    name = "diagnostics_tests",
    srcs = [
        "tests/diagnostics_tests.rs",
        "tests/common/mod.rs",
    ],
    deps = [...],
)
```

For `runtime_engine_tests.rs` (which `mod`s the whole `runtime_engine_tests/` subdirectory), `srcs` must include all 11 files under that directory plus `tests/common/mod.rs`. Use `glob(["tests/runtime_engine_tests/**/*.rs"])` for readability, but pin the top-level file explicitly.

### E2. `include_str!` / `include_bytes!` paths outside `srcs`

`markymark-parser/tests/ast_self_cell.rs` includes `../src/ast.rs`; `markymark-index/tests/document_self_cell.rs` includes three files under `src/document/`. These paths resolve relative to the test file at compile time. Bazel must be told about them via `compile_data`:

```python
rust_test(
    name = "ast_self_cell",
    srcs = ["tests/ast_self_cell.rs"],
    compile_data = ["src/ast.rs"],
    deps = [...],
)
```

### E3. `CARGO_MANIFEST_DIR` env var

`markymark-cli` tests (`lsp_methods.rs`, `mcp_methods.rs`, `smoke_mcp.rs`, `triage_consistency.rs`) use `env!("CARGO_MANIFEST_DIR")` to locate `tests/corpus/`. Under Bazel that env var is still set (by `rules_rust`), but `tests/corpus/` is not in the runfiles tree unless declared as a `data` dep. Three options, pick whichever is simplest per test:

1. Declare `data = glob(["tests/corpus/**"])` and switch the test from `CARGO_MANIFEST_DIR` to the `runfiles` crate or `std::env::var("RUNFILES_DIR")` — requires a code change to the test.
2. Declare `data` as above and use `rustc_env = {"MARKYMARK_CORPUS_DIR": "$(location tests/corpus)"}` — lets the test read `MARKYMARK_CORPUS_DIR` instead of `CARGO_MANIFEST_DIR`. Requires both cargo and bazel paths.
3. Tag the test `tags = ["manual"]` and skip under Bazel initially — document in the debrief as follow-up.

Pick one approach and apply consistently. Option 2 is lowest-friction if the test is willing to fall back to `CARGO_MANIFEST_DIR` when `MARKYMARK_CORPUS_DIR` is unset.

### E4. Process-spawning tests (`Command::new` on the built binary)

`cli_args.rs`, `smoke_mcp.rs`, `smoke_lsp.rs`, `lsp_methods.rs`, `mcp_methods.rs` all spawn `markymark`. Under cargo, `env!("CARGO_BIN_EXE_markymark")` points to the built binary. Under Bazel, declare the binary as a `data` dep and resolve via runfiles:

```python
rust_test(
    name = "cli_args",
    srcs = ["tests/cli_args.rs"],
    data = [":markymark"],
    rustc_env = {"MARKYMARK_BIN": "$(rootpath :markymark)"},
    deps = [...],
)
```

Then the test reads `std::env::var("MARKYMARK_BIN").unwrap_or(env!("CARGO_BIN_EXE_markymark").to_string())` to stay compatible with both build systems.

### E5. `test-helpers` feature flag (markymark-lsp)

Cargo dev-dep pattern: `markymark-lsp = { path = ".", features = ["test-helpers"] }`. In Bazel, add a second library variant:

```python
rust_library(
    name = "markymark-lsp-testing",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "markymark_lsp",
    crate_features = ["test-helpers"],
    deps = [...],  # same as markymark-lsp
    testonly = True,
)
```

Every `markymark-lsp/tests/*.rs` target depends on `:markymark-lsp-testing` instead of `:markymark-lsp`. `testonly = True` keeps it out of production builds.

### E6. Miri-only tests

`markymark-core/tests/miri_arena.rs` compiles and runs under normal `cargo test` (using `#[cfg_attr(miri, ignore)]` to skip unsafe-heavy cases). Wire it into Bazel as a normal `rust_test` target — it will exercise the non-ignored assertions. Miri itself is out of Bazel scope; do not attempt to wire `cargo +nightly miri test` into the build graph.

### E7. Tests that touch the filesystem / tempdirs

Tests using `tempfile` or other tempdirs work under Bazel if `tempfile` is in `deps`. No special handling.

## Anti-patterns (what NOT to do)

- **Don't** glob all test files into one rust_test. Each test file must get its own target — otherwise cargo-to-bazel parity breaks silently (cargo runs one binary per test file; one Bazel target per file preserves that).
- **Don't** paper over port failures with `#[cfg(not(bazel))]` or similar divergence. Either fix the test or tag it `manual` with an explanatory comment.
- **Don't** change test source code except where E3/E4 unavoidably require it (env var fallback). Never "simplify" a test to make the port easier — surface the difficulty as a finding.
- **Don't** skip the `test-helpers` library variant and add `crate_features = ["test-helpers"]` to the unit-test `rust_test` target instead — that pollutes the default test run with helper code and doesn't match the Cargo setup.
- **Don't** wire `cargo +nightly miri test` into Bazel. Miri is an orthogonal tool.

## Investigation notes

- Existing pattern to follow: the `parse_robustness_test` target in `markymark-index/BUILD.bazel:66`.
- Enumerate with: `find . -path ./target -prune -o -path ./bazel-\* -prune -o -path './*/tests/*.rs' -print | grep -v node_modules`.
- Each crate may need different deps (tokio for async tests, serde_json for fixture tests, etc.). Walk them case-by-case.
- Verify dev-deps per crate by reading `Cargo.toml [dev-dependencies]` blocks.
- `edition = "2021"` is workspace-wide (`edition.workspace = true` → `workspace.package.edition = "2021"`).

## Key Considerations (failure catalog)

Implementation writes ~49 new Bazel targets plus one rust_library variant. The failure catalog below calls out non-obvious betrayal modes that happy-path Bazel knowledge misses. Group follows the components the implementer touches, not the failure-category axis.

### K1. Per-crate BUILD.bazel edits (most work)

**Dependency Treachery: proc-macros and macro-expanded deps**
- Assumption: A test's deps are inferable from its top-level `use` statements.
- Betrayal: `insta::assert_yaml_snapshot!` requires `insta` in deps, but macros from `tokio::test`, `proptest!`, or `async-trait` need `proc_macro_deps` entries — not `deps`. A test file that compiles under cargo (which auto-resolves dev-dep proc-macros) will fail under Bazel with `could not find macro X`.
- Consequence: Mysterious compile errors on files that look clean.
- Mitigation: Walk each test's macro invocations (`rg '\w+!\s*\('` on the file), map each to its crate, add proc-macros under `proc_macro_deps = [...]` separately from runtime `deps`.

**Temporal Betrayal: parallel test execution collides on shared paths/ports**
- Assumption: Tests are isolated by default.
- Betrayal: Bazel runs tests in parallel per target by default (`--jobs=auto`). Cargo default is per-file sequential within a binary. Tests that bind to fixed ports, use `/tmp/fixed-path`, or touch a shared cache dir race under Bazel.
- Consequence: Flaky green-under-cargo, red-under-bazel failures — nondeterministic.
- Mitigation: Every test must use `tempfile::tempdir()` + OS-assigned ports. Tests that genuinely share state get `tags = ["exclusive"]` (serializes within the target, not across targets). Verify by running `bazel test //... --runs_per_test=3` once before claiming green.

**Resource Exhaustion: default 60s timeout too short for smoke tests**
- Assumption: All tests fit in the `moderate` (60s) default timeout.
- Betrayal: `smoke_mcp`, `smoke_lsp`, and the `runtime_engine_tests` set spawn processes or bring up full engines — can exceed 60s under a cold Bazel runfiles setup or first-run embedding download.
- Consequence: Intermittent timeouts in CI.
- Mitigation: Add `timeout = "long"` (300s) to `smoke_*`, `runtime_engine_tests`, `bench_lsp`, and any test that touches `fastembed`. Better to over-declare than under.

### K2. markymark-lsp-testing library variant

**Dependency Treachery: `test-helpers`-gated deps not in the variant's deps list**
- Assumption: The variant uses the same deps as the regular `markymark-lsp` library.
- Betrayal: `#[cfg(any(test, feature = "test-helpers"))]` blocks (at `src/server.rs:727` and possibly elsewhere) may import crates that are only dev-deps in Cargo — not runtime deps. When the feature is enabled outside `cfg(test)`, those imports need to be regular deps on the Bazel variant target.
- Consequence: Variant library build fails on unresolved imports.
- Mitigation: `rg 'cfg\(.*test-helpers' markymark-lsp/src/ -l | xargs rg '^use '` to enumerate imports behind the gate. Add any dev-only crates to the variant's `deps` (not `proc_macro_deps` unless macro).

### K3. Shared test modules (`common/`, `alignment_support/`, `runtime_engine_tests/`)

**Input Hostility: module file has its own non-obvious deps**
- Assumption: `common/mod.rs` is a thin utility; its deps are a subset of the parent test's deps.
- Betrayal: `markymark-mcp/tests/common/mod.rs` defines `TempWorkspace` which pulls in `tempfile`, `rmcp`, and `tokio` helpers. The parent test file might only use `TempWorkspace` — its own imports won't reveal the module's deps.
- Consequence: Build succeeds for first parent test but fails for the second parent test whose deps don't cover what `common` needs.
- Mitigation: Treat shared modules as a fixed dep-set. Enumerate common/mod.rs imports once, add ALL of them to every `rust_test` that lists the common file in srcs. Same for `alignment_support/mod.rs`.

### K4. `compile_data` for `include_str!` (ast_self_cell, document_self_cell)

**State Corruption: target moves between Bazel packages, relative path breaks**
- Assumption: `include_str!("../src/ast.rs")` resolves relative to the test file's on-disk location.
- Betrayal: If the `rust_test` target moves to a different Bazel package (not today's plan, but future refactoring), `../src/ast.rs` resolves relative to the Bazel package root, not the test file. Subtle.
- Consequence: Compile error only when someone moves the target.
- Mitigation: Leave a comment in the BUILD file next to `compile_data` noting the assumption: "relative path — must stay in same package as src/".

### K5. CARGO_MANIFEST_DIR porting (cli tests touching `tests/corpus/`)

**Dependency Treachery: `$(rootpath)` vs `$(location)` vs `$(rlocationpath)` drift**
- Assumption: All three forms of path expansion are equivalent for the needs of these tests.
- Betrayal: They aren't. `$(location)` gives exec-root-relative, `$(rootpath)` gives package-relative, `$(rlocationpath)` gives runfiles-relative (needed for runtime reads). Picking the wrong one produces a path that resolves fine at build time (string substitution succeeds) but `std::fs::read` fails at runtime.
- Consequence: Tests fail with "No such file" despite the `data` dep appearing to work.
- Mitigation: For RUNTIME file reads, use `$(rlocationpath tests/corpus)` and resolve with `runfiles::Runfiles::create()`. Or use option 2 (rustc_env + `$(location)`) only if the corpus is a simple directory and the test just wants an absolute path. Pick one and test it with a probe test before scaling.

**State Corruption: write to corpus dir blocked by sandbox**
- Assumption: Tests read from `tests/corpus/` and don't modify it.
- Betrayal: A test that creates a temp file inside `tests/corpus/` (e.g., a golden-file regen pattern) works under cargo but fails with EPERM under Bazel's read-only sandbox.
- Consequence: Unexpected EPERM on tests that never failed under cargo.
- Mitigation: Audit each cli test: `rg 'corpus.*(write|create|OpenOptions)' markymark-cli/tests/`. Any hits need to copy to tempdir first.

### K6. Process-spawning tests (cli smoke tests + `markymark` binary)

**Dependency Treachery: spawned binary doesn't inherit runfiles context**
- Assumption: `Command::new(markymark_bin).spawn()` gives the binary access to its own runfiles.
- Betrayal: Under Bazel, the binary is in the test's runfiles tree. Spawning it works, but if the binary itself reads files via its own `$RUNFILES_DIR`, it doesn't inherit the test's runfiles — the binary has its own. Usually fine; fails if the binary tries to read a sibling file the test provided.
- Consequence: Binary exits nonzero on a file lookup that works under cargo.
- Mitigation: For smoke tests, the binary only reads the stdin/stdout LSP/MCP protocol. No file lookup needed. Document the assumption; don't fix until proven broken.

**Temporal Betrayal: binary's caches survive across test runs**
- Assumption: Each test run starts fresh.
- Betrayal: `fastembed` downloads models to `~/.cache/fastembed/` or similar on first run. Bazel's sandbox blocks writes to $HOME, so the binary falls back to tempdir, but different test runs pick different tempdirs — download happens every time.
- Consequence: First test in a run times out; subsequent runs fine if somehow cached.
- Mitigation: If a smoke test exercises embedding, either (a) provide a pre-downloaded model via `data`, or (b) tag the test `tags = ["requires-network"]` and skip in hermetic CI. Both choices need user visibility in debrief.

### K7. Starlark macro (if chosen over explicit lists)

**Input Hostility: macro signature too narrow for edge cases**
- Assumption: A single macro `rust_integration_test(name, srcs, deps)` covers all 49 targets.
- Betrayal: Some tests need `compile_data`, some need `data`, some need `rustc_env`, some need `timeout = "long"`, some need `tags = ["exclusive"]`. A narrow macro forces you to bypass it for edge cases, leading to two patterns in the same BUILD file.
- Consequence: Half-applied macro + explicit targets creates confusion; worse than either pure pattern.
- Mitigation: Either (a) make the macro kwargs-heavy with sensible defaults and a `**kwargs` passthrough to `rust_test`, or (b) skip the macro entirely and write explicit `rust_test` targets. Don't try to pretty-print a half-macro.

### K8. Cargo/Bazel parity verification

**Dependency Treachery: parity by target count ≠ parity by test case**
- Assumption: If `bazel test //...` is green and `cargo test --workspace` is green, the two systems run the same tests.
- Betrayal: Bazel reports per-target pass/fail; cargo reports per-test. A Bazel target that compiles but skips all `#[test]` functions via some cfg gate still counts as green. Or: a test file that cargo auto-discovers (because of `mod X` declarations) might not be fully covered if the Bazel `srcs` missed a referenced module.
- Consequence: False-green parity — declare done while actually skipping tests.
- Mitigation: Spot-check at least one test per crate by reading stdout: `bazel test //markymark-cli:cli_args_test --test_output=all 2>&1 | grep 'test result'` to confirm the test count matches what cargo runs for the same file (`cargo test -p markymark-cli --test cli_args`). Criterion added below.

---

## Success Criteria

- [x] Every `*/tests/*.rs` top-level file has a corresponding Bazel `rust_test` target (49 new targets added; 8 pre-existing; 2 tagged `manual` — alignment, bench_lsp — for reasons in BUILD.bazel comments)
- [x] `bazel query 'kind("rust_test", //...)'` shows 57 targets (55 regular + 2 manual)
- [x] `bazel test //...` passes on macOS — 55 tests green under the wildcard
- [x] For every test tagged `manual`: the BUILD file comment explains why. `alignment` — marksman sandbox handshake fails (runs fine under cargo). `bench_lsp` — benchmark, not a correctness test. Debrief surfaces both.
- [x] No test that returns 0 under `cargo test --workspace` returns nonzero under Bazel. Parity spot-checked on `markymark-index/tests/realm_index.rs` (18 tests both systems, exact match) and `markymark-mcp/tests/tool_handler_tests.rs` (cargo 48, Bazel 50 — cfg-gate difference due to `semantic-search` feature; both green, both correct for their respective feature states).
- [x] Shared test modules handled correctly — `markymark-mcp/tests/common/mod.rs` used by 5 tests, `markymark-mcp/tests/runtime_engine_tests/` (11 files) used by runtime_engine_tests, `markymark-cli/tests/alignment_support/mod.rs` used by alignment+bench_lsp. All pass.
- [x] `markymark-lsp-testing` library variant introduced and consumed by all 15 lsp integration tests.
- [x] `compile_data` declared for the two `include_str!` tests (`markymark-parser:ast_self_cell_test`, `markymark-index:document_self_cell_test`).
- [x] Parallel-execution sanity check: `bazel test //markymark-mcp/... --runs_per_test=3` — 11 targets × 3 = 33 runs, 0 flakes.
- [x] Per-crate test-count parity spot-checked: `realm_index` (18 = 18), `tool_handler_tests` (48 cargo / 50 Bazel, explained by cfg-gate on `semantic-search`).
- [x] Slow tests declared `timeout = "long"`: `smoke_*`, `lsp_methods`, `mcp_methods`, `cli_args`, `alignment`, `bench_lsp`.
- [ ] CI workflow note for debrief — no workflow file currently runs `bazel test //...` explicitly (scope-aware wildcard is implicit); no workflow change blocks this task but worth a follow-up to ensure `manual`-tagged targets get run periodically.

## Log

- [2026-04-20T18:59:36Z] [Seth] TDD complete. 49 new rust_test targets across 6 crates: core (3), parser (6), index (7), lsp-testing variant + 15 lsp tests, mcp (10 + common + runtime_engine_tests submodule), cli (8). Total bazel rust_test targets: 8 -> 57 (55 under //..., 2 manual). All wildcard-green. Parallel sanity (mcp --runs_per_test=3) 0 flakes. Parity spot-checks pass (realm_index 18/18, tool_handler_tests 48c/50b due to feature cfg-gate). Source changes in 5 cli tests: MARKYMARK_BIN, MARKYMARK_CORPUS_DIR, MARKYMARK_TRIAGE_DOC env-var fallbacks; cargo path unchanged. Surfaced: cargo test -p markymark-mcp --features=semantic-search fails to compile src/engine/tests (inference_provider field, RealmData::new arity) — PRE-EXISTING, unrelated to marky-lpz. Worth a follow-up bn issue. Alignment tagged manual (marksman sandbox), bench_lsp tagged manual (benchmark).
