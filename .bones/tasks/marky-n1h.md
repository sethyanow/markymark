---
id: marky-n1h
title: Reorganize markymark-mcp/src/engine/tests/mod.rs into feature-topical submodules
status: open
type: task
priority: 2
---

## Context

`markymark-mcp/src/engine/tests/mod.rs` is a dumping ground. The directory
already uses the submodule pattern for some tests (`concurrency`, `curation`,
`enrich`, `export_docs_index`, `preview_profiling`, `recommend`), but many
inlined tests never got adopted into it. As tests accumulated, new additions
landed wherever they fit rather than where they belonged.

The 1000-line rule flagged this file (1420 lines as of commit `14735dce`),
but the line count is a smoke alarm — not the goal. The real cost is:
- Anyone adding a test for `outline` / `rename` / `search_symbols` scrolls
  past the full file looking for similar tests.
- Feature gating is per-test (`#[cfg(feature = "semantic-search")]` sprinkled
  over individual tests) instead of at the module boundary.
- Shared fixtures like `make_temp_realm_dir` and `make_engine_with_custom_realm`
  float at the top of the dumping ground; submodules have grown their own
  "Helpers" blocks in parallel.

## Why this is worth doing

1. **Test locality.** A contributor adding a new outline test should have one
   obvious file to open.
2. **Module-level feature gating.** `semantic-search`-only tests compile out
   cleanly when the feature is off, without per-test cfg noise. Compile-time
   win for `--no-default-features` builds.
3. **Honest pass on what's there.** Moving tests is the pretext for actually
   reading them — catching duplicates, tautological assertions, or stale tests
   pinned to APIs that have rotated.
4. **Shared fixtures get one home.** `mod.rs` already has them. Submodules
   need to either use them or explain why they need a local variant.

## Requirements

1. Every inlined engine test in `mod.rs` is homed in a feature-topical
   submodule matching the existing `curation/`, `enrich/` pattern.
2. Tests that belong to an existing submodule move there; new submodules get
   created for cohesive groups that don't yet have a home.
3. Feature-gated test sets (`semantic-search`) are `#[cfg]`'d at the module
   boundary, not per-test.
4. Shared fixtures live in a single location (`mod.rs`) with clear visibility.
   Submodule "Helpers" blocks that duplicate them are deleted; genuinely local
   helpers stay and get a comment explaining the local scope.
5. Audit during the move: deletions, merges, and flagged tautological tests
   are captured in the commit message + a `bn log` entry — don't silently
   absorb them.

---

## Diagnosis (captured 2026-04-22, do not re-derive)

Skeleton evidence from grepping `mod.rs` at commit `14735dce`. All line
numbers are pre-refactor.

### Current submodules and sizes

| Module | Lines | Status | Cfg gate |
|--------|-------|--------|----------|
| `concurrency.rs` | 478 | existing | `#[cfg(feature = "semantic-search")]` at `mod.rs:27` |
| `curation.rs` | 416 | existing | — |
| `enrich.rs` | 641 | existing | — |
| `export_docs_index.rs` | 371 | existing | — |
| `preview_profiling.rs` | 199 | existing | `#[cfg(feature = "semantic-search")]` at `mod.rs:35` |
| `recommend.rs` | 323 | existing | — |
| `mod.rs` | 1420 | dumping ground | per-test cfg sprinkled |

The existing submodules themselves sit at 200–640 lines. The 1000-line rule
is not consistently applied and is not the target; several submodules legitimately
need their size.

### Inlined tests in `mod.rs` (by cohesion group)

| Group | Line range | Count | New home |
|-------|------------|-------|----------|
| `get_outline_uses_named_realm` | 38–73 | 1 | `outline.rs` |
| `outline_*` / `outline_tree_*` | 79–366 | 9 | `outline.rs` |
| `export_index_uses_named_realm` | 366–399 | 1 | `export_docs_index.rs` if body affinity confirms, else standalone |
| `search_symbols_uses_named_realm` | 399–435 | 1 | decision at execution (standalone `search_symbols.rs` or absorb) |
| `find_references_*` + `find_references_uses_named_realm` | 435–596 | 4 | `find_references.rs` |
| `rename_*` + `rename_uses_named_realm` | 486–624 | 3 | `rename.rs` |
| `collect_documents_includes_json_alongside_markdown` | 625 | 1 | `workspace_scan.rs` |
| `fnv1a32_*` + `hash_embedding_*` (`semantic-search` gated per-test) | 655–736 | 5 | `hash_embedding.rs` with module-level cfg |
| `batch_indexed_*` | 736–799 | 2 | `engine_indexing.rs` |
| `collect_documents_markdown_unchanged` + v6c `collect_documents_*` suite | 800–1083 | 14 | `workspace_scan.rs` |
| `v6c_speedup_probe` (`#[ignore]`d) | 1083–1103 | 1 | `workspace_scan.rs` (default) |
| `engine_*` | 1103–1271 | 4 | `engine_indexing.rs` |
| `lto_eliminates_fault_injection` | 1170 | 1 | decision at execution (engine_indexing.rs OR standalone `lto.rs`) |
| `from_text_equivalence_*` | 1271–1420 | 2 | `from_text_equivalence.rs` |

Verify groupings by reading bodies during extraction — adjust if a body contradicts its filename.

### Shared fixtures in `mod.rs`

- `make_temp_realm_dir(_suffix: &str) -> TempDir` (line 5) — **`_suffix` is dead**; every caller passes a string that is discarded. Drop the parameter.
- `make_engine_with_custom_realm(realm_name, dir) -> RuntimeEngine` (line 9) — used across many tests.

### Smells (evidence-backed)

| Smell | Evidence | Severity | Risk | Suggested direction |
|-------|----------|----------|------|----------------------|
| Orphan inlined tests | 49 tests inlined despite submodule pattern | MED | LOW | Move Method (bulk) |
| Duplicate "Helpers" blocks | `curation.rs:5`, `export_docs_index.rs:5`, `recommend.rs:5` each begin `// ── Helpers ──` despite `use super::*;` importing `mod.rs:5-25` fixtures | MED | LOW | Audit each, delete duplicates, retain genuine locals |
| Per-test cfg noise | 5 sites at `mod.rs:655,682,697,707,719` each carry `#[cfg(feature = "semantic-search")]` individually | LOW | LOW | Extract Module with Cfg Gate at module boundary |
| Dead parameter | `make_temp_realm_dir(_suffix: &str)` | LOW | LOW | Remove Parameter |
| `collect_documents` scattered | Tests at lines 625, 800, then 815–1083 (v6c additions landed between older clusters) | LOW | LOW | Coalesces naturally in the move |
| Misplaced LTO test | `lto_eliminates_fault_injection` at `mod.rs:1170` sandwiched in engine suite with no topical link | LOW | LOW | Body read determines home |
| Large tail test bodies | `from_text_equivalence_with_fallback_scan_mixed_doc` ~120 lines (`mod.rs:1271`) | LOW | MED if parametrized | Move first, parametrize never (out of scope) |

### Test smells (flagged for audit, NOT deleted in this task)

| Test smell | Evidence | Action |
|------------|----------|--------|
| Possible `_uses_named_realm` duplication | 5 tests with same naming pattern at lines 39, 367, 400, 436, 487. Likely each asserts "op X respects realm param" | Read all 5 bodies during move. If structurally similar, flag to the user for a separate parametrization task. Don't rewrite here. |
| `v6c_speedup_probe` is not a test | `mod.rs:1085` — `#[ignore]`'d wall-clock, no assertions | Stays as probe (default). Promoting to Criterion bench = scope creep (no `benches/` dir in markymark-mcp today). |
| Possible tautologies | `hash_embedding_is_deterministic` (line 699), `outline_tree_root_node_no_heading` (line 209), `collect_documents_is_deterministic_across_runs` | Read during move; flag in audit log if tautological; don't delete. |

---

## Design (captured 2026-04-22)

### Pattern mapping

| Smell | Pattern | Rationale |
|-------|---------|-----------|
| 49 orphan tests | Move Method (bulk) | Complete an existing submodule pattern; zero semantic change |
| Duplicate Helpers blocks | Inline + Centralize | Single fixture surface prevents drift |
| 5 per-test cfgs | Extract Module with Cfg Gate | Matches `concurrency`/`preview_profiling` pattern |
| Dead `_suffix` param | Remove Parameter | No caller uses the value |
| `lto_*` orphan | Move (destination TBD at execution) | Decided from body read |

### Invariants to preserve (non-negotiable)

1. Every test function's name is unchanged (triage history references them).
2. Every test's assertion set is unchanged (this is code motion, not edits).
3. Every test's pass/fail state is unchanged across the refactor.
4. `#[cfg]` gating produces the same set of compiled tests under every
   feature flag combination. A mis-scoped module-level cfg could silently
   include or exclude tests — mitigated by the baseline-capture step.

### Dependency injection / new types / new invariants

N/A — test-file reorganization introduces no runtime dependencies,
interfaces, or types. Every test constructs its own `RuntimeEngine`; that
pattern is preserved.

---

## Sequencing (execution plan)

1. **Baseline capture.** Record the invariant we're preserving:
   ```bash
   cargo test -p markymark-mcp --lib --features semantic-search 2>&1 \
     | grep -E '^test .* \.\.\. (ok|FAILED|ignored)' | sort \
     > /tmp/v6c_results_pre_ss.txt
   cargo test -p markymark-mcp --lib --no-default-features 2>&1 \
     | grep -E '^test .* \.\.\. (ok|FAILED|ignored)' | sort \
     > /tmp/v6c_results_pre_default.txt
   ```
   These files must round-trip unchanged through every step below.

2. **Helper audit.** Read `curation.rs:5`, `export_docs_index.rs:5`,
   `recommend.rs:5` "Helpers" sections. For each helper: diff against
   `mod.rs:5-25`. Tag `delete-dup`, `keep-local`, or `promote-to-parent`.
   Record findings in a scratch note — they land in the final audit log.

3. **Drop `_suffix` parameter.** `make_temp_realm_dir(_suffix: &str)` →
   `make_temp_realm_dir()`. Update all callers via `rg 'make_temp_realm_dir\('`.
   Compile-check then baseline compare.

4. **Extract `hash_embedding.rs`.** 5 tests + module-level cfg. Smallest
   extraction; proves the pattern. Baseline compare.

5. **Extract `workspace_scan.rs`.** 15 `collect_documents_*` + `v6c_speedup_probe`.
   Biggest single group. Include the do-NOT-env-mutate comment in the module
   header (v6c-specific hazard). Baseline compare.

6. **Extract `outline.rs`.** 9 outline tests + `get_outline_uses_named_realm`.
   Baseline compare.

7. **Extract `engine_indexing.rs`.** 6 tests. Read `lto_eliminates_fault_injection`
   body — decide between `engine_indexing.rs` home or standalone `lto.rs`.
   Baseline compare.

8. **Extract small remaining groups.** `find_references.rs`, `rename.rs`,
   `from_text_equivalence.rs`. Decide homes for `search_symbols_uses_named_realm`
   and `export_index_uses_named_realm` (body affinity determines). Baseline
   compare after each.

9. **Helper consolidation completion.** Execute the deletions tagged in step 2.
   Baseline compare.

10. **Audit log + final gate.** Commit message + `bn log marky-n1h` capture:
    fixture changes, execution-time home decisions, tautology flags,
    deleted tests (with reason each). Run `bazel test //...` as the close.

### Per-step validation command

```bash
cargo test -p markymark-mcp --lib --features semantic-search 2>&1 \
  | grep -E '^test .* \.\.\. (ok|FAILED|ignored)' | sort \
  > /tmp/v6c_results_post_ss.txt
diff /tmp/v6c_results_pre_ss.txt /tmp/v6c_results_post_ss.txt
```
Empty diff = continue. Non-empty diff = stop, investigate.

---

## Open-at-execution decisions

These are judgement calls the executor makes by reading bodies during the
move. Document the chosen option + reason in the final audit log.

1. **`lto_eliminates_fault_injection` home** — `engine_indexing.rs` or
   standalone `lto.rs`? (Step 7)
2. **`search_symbols_uses_named_realm` home** — standalone `search_symbols.rs`
   or absorb into nearest existing submodule? (Step 8)
3. **`export_index_uses_named_realm` home** — standalone or into
   `export_docs_index.rs`? (Step 8)
4. **`v6c_speedup_probe` placement** — default is "stays as `#[ignore]`d in
   `workspace_scan.rs`." Only migrate if execution reveals a real need
   (e.g., it depends on data that belongs elsewhere). (Step 5)
5. **Tautology flags** — enumerated during read, surfaced in audit log.
   Don't delete here; that's a separate task if the user wants to pursue it.

## User-decision questions (ask before starting or flag in audit log)

- If the `_uses_named_realm` tests turn out to be structurally near-identical,
  should we create a follow-up parametrization task, or leave as-is and accept
  the duplication? (Don't decide unilaterally — surface.)
- If a body read reveals a test that asserts nothing meaningful (pure
  tautology), should the executor delete it with reason logged, or flag it
  in the audit log only? Default: flag-only; deletion is a user call.

---

## Anti-Patterns

- **Do NOT** target a line count. Line count is a side effect, not a criterion.
- **Do NOT** rename test functions. Search/triage history references them.
- **Do NOT** rewrite tests. Code motion + deletions + flags — not rewrites.
- **Do NOT** parametrize the `_uses_named_realm` cluster. That's a rewrite,
  out of scope; surface as follow-up.
- **Do NOT** skip the audit. The move is the pretext for the read; skipping
  the read wastes the pretext.
- **Do NOT** silently delete a test. If a test goes, the audit log names it
  and says why.
- **Do NOT** add new test scenarios. Adversarial / edge-case additions belong
  to their own task.
- **Do NOT** use `std::env::set_var` inside tests (v6c-specific hazard;
  env is process-global and `cargo test` runs in parallel — data race).
  Carry this note into `workspace_scan.rs`'s module header.
- **Do NOT** batch-commit across all 10 steps. One commit per extracted
  submodule so bisect stays useful and the audit log has paragraph breaks.

---

## Success Criteria

- [ ] Baseline test-result files captured before step 1 and round-trip unchanged after each step (both `--features semantic-search` and `--no-default-features` profiles)
- [ ] Every inlined engine test in `mod.rs` has been homed in a feature-topical submodule
- [ ] No test function renamed during the move (diff shows moves + import cleanup only)
- [ ] `semantic-search` gating lives at module boundaries, not per-test
- [ ] `make_temp_realm_dir` signature no longer carries the dead `_suffix` parameter
- [ ] Duplicate "Helpers" blocks in existing submodules have been audited: duplicates deleted, genuine locals annotated with a scope comment
- [ ] Audit log in commit messages + `bn log marky-n1h` captures: helper audit findings, open-at-execution decisions (5 above) and what was chosen, tautology flags, tests deleted with per-test reason
- [ ] `cargo test -p markymark-mcp --lib` green under both `--features semantic-search` and `--no-default-features`
- [ ] `bazel test //markymark-mcp:markymark-mcp_test` green
- [ ] `bazel test //...` green
