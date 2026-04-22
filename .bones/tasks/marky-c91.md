---
id: marky-c91
title: 'Bug: markymark-mcp compile errors under --features=semantic-search (test files out of sync with production types)'
status: open
type: bug
priority: 1
---




## Context

`cargo test -p markymark-mcp --tests --features=semantic-search` fails to compile with 9 errors across 2 test files. The errors reveal that test callsites retain an older API shape that production code evolved past:

1. `RuntimeEngine` gained an always-present `inference_provider: Option<Arc<dyn InferenceProvider>>` field (`markymark-mcp/src/engine/mod.rs:172`). Not cfg-gated — always required on struct literals.
2. `RealmData::new()` became feature-gated (`markymark-mcp/src/engine/mod.rs:50-66`): takes `Option<Arc<dyn EmbeddingProvider>>` under `semantic-search`, no args otherwise.

Production callers (`engine/mod.rs:178-181, 202-205, 237`; `engine/realm_ops.rs:15-64`) already use the correct `#[cfg]`-split pattern. Test files were not updated. CI runs `cargo test --workspace` (`.github/workflows/ci.yml:139`) without `--features=semantic-search`, so the gap is latent — it only surfaces when a contributor or Bazel test enables the flag.

### Exact failure surface

| File | Line | Error | Issue |
|------|------|-------|-------|
| `markymark-mcp/src/engine/tests/concurrency.rs` | 136 | E0063 | `RuntimeEngine {..}` missing `inference_provider` |
| `markymark-mcp/src/engine/tests/concurrency.rs` | 218 | E0063 | same |
| `markymark-mcp/src/engine/tests/concurrency.rs` | 316 | E0063 | same |
| `markymark-mcp/src/engine/tests/concurrency.rs` | 406 | E0063 | same |
| `markymark-mcp/src/engine/tests/mod.rs` | 818 | E0061 | `RealmData::new()` with 0 args (needs 1) |
| `markymark-mcp/src/engine/tests/mod.rs` | 839 | E0061 | same |
| `markymark-mcp/src/engine/tests/mod.rs` | 895 | E0061 | same |
| `markymark-mcp/src/engine/tests/mod.rs` | 918 | E0061 | same |
| `markymark-mcp/src/engine/tests/mod.rs` | 948 | E0061 | same |

`concurrency.rs` is already cfg-gated on `semantic-search` (`tests/mod.rs:27`), so only the feature-enabled struct shape needs fixing there. `tests/mod.rs` callsites are NOT cfg-gated — the `RealmData::new()` 0-arg form works under default features but breaks under `--features=semantic-search`. Fix must preserve dual-config compile.

## Requirements

1. `cargo test -p markymark-mcp --tests --features=semantic-search` compiles (currently fails with 9 errors).
2. `cargo test -p markymark-mcp --tests` (default features) continues to compile — no regression.
3. CI runs a feature-enabled test compile so this class of regression is caught by the merge gate.
4. Fix mirrors the existing production pattern (cfg-split constructors) rather than changing production API.
5. No new logic introduced — tests keep their current semantics.

## Design

### Mirror the existing production pattern

Production already demonstrates the correct shape. Apply the same shape in the tests:

- `engine/mod.rs:178-181` — cfg-split `RealmData::new(None)` / `RealmData::new()`
- `engine/mod.rs:186, 220, 251` — `inference_provider: None` in every `RuntimeEngine {..}`
- `engine/realm_ops.rs:15-64` — cfg-split constructor functions

### Test helper to keep 5 callsites readable

Instead of inlining cfg-split at every callsite in `tests/mod.rs`, introduce one helper adjacent to `make_temp_realm_dir` (`tests/mod.rs:5-7`):

```rust
// signature only — executing agent writes the body
fn new_test_realm() -> RealmData
```

Body returns `RealmData::new(None)` under `semantic-search`, `RealmData::new()` otherwise, using the same `#[cfg]` split pattern already in `engine/mod.rs`. Tests call `new_test_realm()` and stay readable under both feature configurations.

### CI regression gate

Add one step to `.github/workflows/ci.yml` alongside existing `cargo test --workspace`:

- **Scope decision:** narrow (`cargo test -p markymark-mcp --features=semantic-search`) vs broad (`cargo test --workspace --features=semantic-search,local-embeddings` — matches `release.yml:119-121` feature set).
- **Recommendation in Key Considerations below.** Surface as a user decision — do not pick silently.

## Implementation

### Step 1 (RED — regression gate first): Add CI step that would have caught this

Edit `.github/workflows/ci.yml` after the existing `Run all tests` step (line 138-139). Add a new step:

```yaml
      - name: Run semantic-search tests
        run: cargo test -p markymark-mcp --features=semantic-search
```

(Final scope may widen to full workspace — see Key Considerations.)

**Verify locally** before writing fix code:
```bash
cargo test -p markymark-mcp --tests --features=semantic-search 2>&1 | tail -5
```
Expected: 9 compile errors (4× E0063, 5× E0061). This is the RED state.

### Step 2 (GREEN — concurrency.rs): Add `inference_provider: None` to 4 struct literals

Edit `markymark-mcp/src/engine/tests/concurrency.rs` at lines 136, 218, 316, 406. Each struct literal currently looks like:

```rust
let engine = Arc::new(RuntimeEngine {
    state: tokio::sync::RwLock::new({ /* ... */ }),
    provider: Some(provider),
});
```

Add the missing field after `provider`:

```rust
    provider: Some(provider),
    inference_provider: None,
});
```

`concurrency.rs` is cfg-gated on `semantic-search` (`tests/mod.rs:27`), so `inference_provider` resolves correctly — no cfg-split needed at the callsite.

### Step 3 (GREEN — tests/mod.rs helper): Add `new_test_realm()`

Edit `markymark-mcp/src/engine/tests/mod.rs`. Insert a new helper function adjacent to `make_temp_realm_dir` (after line 7, before the `make_engine_with_custom_realm` fn at line 9).

Signature:
```rust
fn new_test_realm() -> RealmData
```

Body uses the same `#[cfg]` pattern as `engine/mod.rs:178-181`:
- Under `#[cfg(feature = "semantic-search")]`: return `RealmData::new(None)`
- Under `#[cfg(not(feature = "semantic-search"))]`: return `RealmData::new()`

### Step 4 (GREEN — tests/mod.rs callsites): Replace 5 callsites

Edit `markymark-mcp/src/engine/tests/mod.rs` at lines 818, 839, 895, 918, 948. Each line currently reads:

```rust
    let mut realm = RealmData::new();
```

Replace with:

```rust
    let mut realm = new_test_realm();
```

No other changes at these callsites — the subsequent `index_root_into_realm(dir.path(), &mut realm).await` already works with both feature configurations.

### Step 5 (Verify): Compile + run under semantic-search

```bash
cargo test -p markymark-mcp --tests --features=semantic-search 2>&1 | tail -10
```
Expected: compile success, tests run. The pre-existing concurrency tests (`semantic_search_does_not_block_realm_writes`, etc.) should pass as they did under Bazel.

### Step 6 (Verify): No regression under default features

```bash
cargo test -p markymark-mcp --tests 2>&1 | tail -5
```
Expected: compile success, same test count as before (minus the `#[cfg(feature = "semantic-search")]`-gated concurrency and preview_profiling modules).

### Step 7 (Verify): Full workspace test under default features

```bash
cargo test --workspace 2>&1 | tail -5
```
Expected: no regression from Step 6.

### Step 8 (Commit): single commit for bug + CI gate

```bash
git add markymark-mcp/src/engine/tests/concurrency.rs \
        markymark-mcp/src/engine/tests/mod.rs \
        .github/workflows/ci.yml
git commit -m "fix(mcp): sync test files with RuntimeEngine.inference_provider and cfg-gated RealmData::new()

Tests retained an older API shape — RuntimeEngine struct literals missed the
inference_provider field and RealmData::new() was called without the provider
argument required under --features=semantic-search. Production callers already
use the correct cfg-split pattern (engine/mod.rs:178-181, realm_ops.rs:15-64);
tests now mirror it.

Added a CI step that compiles tests under --features=semantic-search so this
class of drift is caught at merge time. Previously CI only ran the default
feature set, making the break latent until a contributor enabled the flag.

Refs: marky-c91"
```

## Success Criteria

- [ ] `cargo test -p markymark-mcp --tests --features=semantic-search` compiles (was failing with 9 errors)
- [ ] `cargo test -p markymark-mcp --tests --features=semantic-search` runs — pre-existing `concurrency` and `preview_profiling` tests pass
- [ ] `cargo test -p markymark-mcp --tests` (default features) compiles — no regression
- [ ] `cargo test --workspace` passes — no regression
- [ ] `.github/workflows/ci.yml` has a new `Run semantic-search tests` step (or broader equivalent — see Key Considerations)
- [ ] New CI step would have caught the original bug — verified by reverting Steps 2–4 locally and confirming Step 5 fails
- [ ] No production code changes — fix is test-only + CI-only
- [ ] Commit message references `marky-c91`

## Anti-Patterns

- **Do NOT refactor `RealmData::new()` to unify its signature across features.** That changes production API and widens blast radius for a mechanical sync bug.
- **Do NOT remove `inference_provider` field from `RuntimeEngine`.** It's consumed at `engine/mod.rs:790` — tests are wrong, not production.
- **Do NOT inline cfg-split at all 5 `tests/mod.rs` callsites.** The helper is cleaner and matches the factoring-out pattern already used for `make_temp_realm_dir`.
- **Do NOT move the CI test step before the existing `cargo test --workspace`.** It's additive — the default path must remain the primary signal.
- **Do NOT skip the CI step (Requirement 3).** Without it, the same class of drift will re-occur. The fix is fragile without the regression gate.
- **Do NOT add new tests for `inference_provider` behavior in this task.** It's out of scope — the bug is that EXISTING tests don't compile. New coverage belongs in a follow-up.

## Key Considerations

### CI scope decision (surface to user)

Two valid shapes for the new CI step:

1. **Narrow:** `cargo test -p markymark-mcp --features=semantic-search` — exactly the failure surface; minimal CI time cost.
2. **Broad:** `cargo test --workspace --features=semantic-search,local-embeddings` — matches the feature set used by `release.yml:119-121`; catches drift across all feature-gated code in the workspace, not just markymark-mcp.

Recommendation: start narrow (option 1). Broadening is a separate CI improvement task — conflating it with the bug fix hides the bug-fix commit under a wider CI change.

### Edge case: concurrency.rs compiles but tests may fail at runtime under feature flag flux

The 4 concurrency tests assert on a specific timing invariant (write lock not held across slow embed). If `inference_provider` path in `engine/mod.rs:790` changes lock scoping, these tests could become flaky. Not this task's scope, but flag if encountered during execution.

### Edge case: Bazel vs cargo divergence

Bazel's `markymark-mcp_test` currently reports 122 passing tests despite `crate_features = ["semantic-search"]` — either (a) Bazel cache is stale and a clean rebuild would fail the same way, or (b) Bazel's test target compiles a different set of modules than cargo. Verify Step 5 also succeeds under `bazel test //markymark-mcp:markymark-mcp_test` after the fix. If Bazel-specific issues surface, file a separate task — they are not this bug.

### Why the helper lives in `tests/mod.rs` not `engine/mod.rs`

`new_test_realm()` is test-only scaffolding. Placing it in `engine/mod.rs` (production) under `#[cfg(test)]` would work but spreads test infrastructure across modules. Keeping it adjacent to the existing `make_temp_realm_dir` helper in `tests/mod.rs` co-locates test scaffolding.

### Session-boundary note

This plan is diagnosed and scoped — the fix executes in a NEW session via `executing-plans`. That session will SRE-refine this skeleton with fresh eyes, then TDD through Steps 1–8. The CI step in Step 1 serves as the RED test for TDD: verify it fails pre-fix, then passes post-fix.

### Adversarial failure catalog

**CI step — Temporal Betrayal: paper-tiger regression gate.**
- Assumption: adding the step catches future API drift.
- Betrayal: YAML block that doesn't actually compile affected modules (wrong scope / missing `--tests`) would be green but useless.
- Consequence: silent false-green; bug class re-lands.
- Mitigation: Criterion 6 already tests the gate (revert fix locally, confirm CI fails). Must execute, not just check.

**CI step — Resource Exhaustion: fastembed compile cost.**
- Assumption: adds ~2-5 min per PR.
- Betrayal: `--features=semantic-search` pulls fastembed + ort; Linux compile could add 5-10 min.
- Consequence: slower CI. Acceptable per Iron Law.
- Mitigation: narrow scope keeps it bounded. Revisit if complaints surface.

**concurrency.rs patches — Temporal Betrayal: silent enrich skipping.**
- Assumption: `inference_provider: None` is a no-op — tests don't hit enrich path (mod.rs:790).
- Betrayal: if any of the 4 tests calls `CoreOperation::EnrichDocument`, `None` silently no-ops instead of failing loudly.
- Consequence: test appears green but doesn't exercise claimed path.
- Mitigation: grep each of the 4 test functions for `Enrich` before committing. Tests are named for AddRoot/CreateRealm/RemoveRoot/RealmStats — likely safe, but verify.

**new_test_realm() — Temporal Betrayal: feature-gated RealmIndex equivalence.**
- Assumption: `RealmData::new(None)` (semantic-search) and `RealmData::new()` (default) produce equivalent state.
- Betrayal: verified at mod.rs:70-83 — both paths call `RealmIndex::new()`. Safe. Only breaks if `RealmIndex::new()` itself becomes feature-gated asymmetrically.
- Consequence: would manifest as cross-config test divergence.
- Mitigation: mirrors production pattern — same equivalence production relies on.

**Commit scope — Temporal Betrayal: bundled revert.**
- Assumption: fix + CI gate ship together per Iron Law.
- Betrayal: if CI step is reverted later, bug fix goes with it.
- Consequence: bisect granularity loss.
- Mitigation: bundled is structurally correct — separating creates window where fix is in but gate isn't. Acceptable trade-off.

**Bazel vs cargo — State Corruption: stale cache masking divergence.**
- Assumption: post-fix, cargo and Bazel agree.
- Betrayal: Bazel currently reports 122 passing despite same feature flag cargo fails under — either stale cache or different module set.
- Consequence: build systems disagree on correctness; CI uses Bazel.
- Mitigation: after Step 5, run `bazel test //markymark-mcp:markymark-mcp_test`. If output diverges from cargo (different test count, different pass/fail), file separate task — not this bug.

## Log

- [2026-04-21T02:18:58Z] [Seth] Diagnosis: Test files in markymark-mcp/src/engine/tests/ use an older API shape. Production added RuntimeEngine.inference_provider (always present, not cfg-gated) and feature-gated RealmData::new() — test callsites weren't updated. CI gap: cargo test --workspace in ci.yml:139 omits --features=semantic-search, making this latent until a contributor enables the flag. Evidence: 9 compile errors (4x E0063 missing inference_provider in RuntimeEngine struct literals at tests/concurrency.rs:136/218/316/406; 5x E0061 RealmData::new() with 0 args at tests/mod.rs:818/839/895/918/948). Production pattern to mirror: engine/mod.rs:179-181,203-205 (cfg-gated RealmData::new(None)/RealmData::new()) and engine/mod.rs:186,220,251 (inference_provider: None in struct literals). Fix location: (1) add inference_provider: None to 4 concurrency.rs struct literals; (2) introduce new_test_realm() helper in tests/mod.rs for the 5 callsites; (3) add cargo test --features=semantic-search to ci.yml. Confidence: HIGH.
- [2026-04-21T02:21:48Z] [Seth] Plan written. Fix scoped: (1) concurrency.rs 4x struct literals add inference_provider:None; (2) tests/mod.rs new_test_realm() helper + 5 callsite swap; (3) CI step --features=semantic-search. Linked as regression test per Iron Law. Session boundary next — fix executes via executing-plans in a new session.
- [2026-04-21T02:29:13Z] [Seth] SRE fresh-eyes review: APPROVED as-is. Verified all line-number claims (mod.rs:49-67/172/178-181/186-220-251/790, realm_ops.rs:15-64, concurrency.rs:136/218/316/406, tests/mod.rs:818/839/895/918/948/5-7/27, ci.yml:139, release.yml:119-121). RED state confirmed: exactly 9 errors (4x E0063, 5x E0061) at claimed lines. Bijection Reqs 1-5 trace to Success Criteria. Criterion 6 (verify regression gate by reverting + confirming CI fails) is strong — tests the gate itself. One user decision surfaced in Key Considerations: narrow vs broad CI scope — executing agent must AskUserQuestion at Step 1.
- [2026-04-21T02:31:15Z] [Seth] Adversarial planning: 6 failure modes cataloged (paper-tiger CI, fastembed compile cost, silent enrich-skip, RealmIndex equivalence, bundled revert, Bazel-cargo divergence). No new success criteria needed — existing criterion 6 already tests the strongest risk (paper-tiger gate). Executing agent MUST grep the 4 concurrency test fns for Enrich ops before commit; post-fix MUST run bazel test for cross-build verification.
