---
id: marky-0mr.8
title: 'PR#39 review: extract diagnostic-to-LSP helper (DRY)'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


**T3-4: Duplicate diagnostic-to-LSP conversion logic**
File: markymark-lsp/src/server.rs:232-244

The diagnostic-to-LSP conversion logic is duplicated between publish_diagnostics_for (lines 73-85) and the debounce task (lines 232-244). Both map crate::state::Diagnostic to lsp_types::Diagnostic using crate::convert::to_lsp_range and severity mapping.

Fix: extract shared fn to_lsp_diagnostics(diagnostics: impl IntoIterator<Item=crate::state::Diagnostic>) -> Vec<Diagnostic> and call from both sites. Ensure the helper sets range, severity, source ("markymark"), message and default fields consistently.

Source: PR #39 review — CodeRabbit

## Design

## Goal
DRY up the diagnostic-to-LSP conversion logic that is duplicated between two sites in
markymark-lsp/src/server.rs. Both sites do the same `crate::state::Diagnostic →
lsp_types::Diagnostic` mapping inline. Extract a single helper so future severity changes,
field additions, or source-string changes need only one edit.

## Placement Decision (SRE: must decide before writing code)

The helper belongs in **`markymark-lsp/src/convert.rs`** — every other `to_lsp_*`
conversion lives there. Do NOT add it to server.rs (that file is already 952 lines,
48 from the 1000-line hard stop). Placing it in convert.rs keeps the file-growth
pressure on the right file and keeps server.rs conversion-logic-free.

## Effort Estimate
2–3 hours.

## Current Duplicate Sites

**Site 1** — `publish_diagnostics_for` (server.rs:89–101):
```rust
let lsp_diagnostics: Vec<Diagnostic> = diagnostics
    .into_iter()
    .map(|d| Diagnostic {
        range: crate::convert::to_lsp_range(d.range),
        severity: Some(match d.severity {
            MarkyDiagSeverity::Error => DiagnosticSeverity::ERROR,
            MarkyDiagSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("markymark".to_string()),
        message: d.message,
        ..Default::default()
    })
    .collect();
```

**Site 2** — debounce task closure (server.rs:241–253):
```rust
let lsp_diagnostics: Vec<Diagnostic> = diagnostics
    .into_iter()
    .map(|d| Diagnostic {
        range: crate::convert::to_lsp_range(d.range),
        severity: Some(match d.severity {
            crate::state::DiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
            crate::state::DiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("markymark".to_string()),
        message: d.message,
        ..Default::default()
    })
    .collect();
```

Note: the two sites use **different import paths** for `DiagnosticSeverity`:
- Site 1: `MarkyDiagSeverity` (aliased at server.rs:12)
- Site 2: `crate::state::DiagnosticSeverity` (no alias)

Both refer to the same type. The helper in convert.rs should use
`markymark_core::engine::DiagnosticSeverity` or re-use the existing
`use crate::diagnostics::DiagnosticSeverity` re-export if convert.rs is in the same crate.

## Target Function Signature

In `markymark-lsp/src/convert.rs`:

```rust
/// Convert an iterator of markymark diagnostics to a vec of LSP diagnostics.
pub fn to_lsp_diagnostics(
    diagnostics: impl IntoIterator<Item = crate::diagnostics::MarkyDiagnostic>,
) -> Vec<ls_types::Diagnostic> {
    use crate::diagnostics::DiagnosticSeverity;
    diagnostics
        .into_iter()
        .map(|d| ls_types::Diagnostic {
            range: to_lsp_range(d.range),
            severity: Some(match d.severity {
                DiagnosticSeverity::Error => ls_types::DiagnosticSeverity::ERROR,
                DiagnosticSeverity::Warning => ls_types::DiagnosticSeverity::WARNING,
            }),
            source: Some("markymark".to_string()),
            message: d.message,
            ..Default::default()
        })
        .collect()
}
```

Both call sites in server.rs replace their inline collect with:
```rust
let lsp_diagnostics = crate::convert::to_lsp_diagnostics(diagnostics);
```

## Implementation Checklist

- [ ] Confirm `MarkyDiagnostic` type used by both sites — it's `crate::diagnostics::MarkyDiagnostic`
      (re-exported from `markymark_core::engine::CoreDiagnostic` in diagnostics.rs)
- [ ] Add `to_lsp_diagnostics` to `markymark-lsp/src/convert.rs` (after the existing `to_lsp_location` fn)
- [ ] Update Site 1 (server.rs:89–101): replace inline map+collect with `crate::convert::to_lsp_diagnostics(diagnostics)`
- [ ] Update Site 2 (server.rs:241–253): replace inline map+collect with `crate::convert::to_lsp_diagnostics(diagnostics)`
- [ ] Remove `use crate::state::DiagnosticSeverity as MarkyDiagSeverity` import from server.rs if no longer used elsewhere
- [ ] Add 5 tests in `markymark-lsp/src/convert.rs` (test module at bottom of file)
- [ ] Run `cargo nextest -p markymark-lsp` — all tests pass
- [ ] Run `cargo clippy -p markymark-lsp --all-targets -- -D warnings` — zero warnings

## Success Criteria

- [ ] `to_lsp_diagnostics` function exists in `markymark-lsp/src/convert.rs` with doc comment
- [ ] Both call sites in server.rs reduced to single-line `crate::convert::to_lsp_diagnostics(...)` calls
- [ ] `MarkyDiagSeverity` alias removed from server.rs (or confirmed still needed elsewhere — grep first)
- [ ] `cargo nextest -p markymark-lsp` exits 0
- [ ] `cargo clippy -p markymark-lsp --all-targets -- -D warnings` exits 0
- [ ] 5 named tests pass (see Test Specifications)
- [ ] server.rs line count does not increase (should decrease by ~12 lines)

## Test Specifications

Add to the `#[cfg(test)]` module at the bottom of `convert.rs`:

- `test_error_severity_maps_to_lsp_error` — single Error diagnostic in; assert output `severity == Some(DiagnosticSeverity::ERROR)`. Catches: severity mapping inverted or missing.
- `test_warning_severity_maps_to_lsp_warning` — single Warning diagnostic in; assert output `severity == Some(DiagnosticSeverity::WARNING)`. Catches: Warning treated as ERROR or None.
- `test_empty_iterator_returns_empty_vec` — call with empty Vec; assert `result.is_empty()`. Catches: panic on empty input or wrong default.
- `test_source_field_is_markymark` — single diagnostic in; assert `result[0].source == Some("markymark".to_string())`. Catches: source field hardcoded wrong string or set to None — this is a protocol requirement that editors use for filtering.
- `test_message_and_range_preserved` — diagnostic with specific message "test error" and a non-zero range; assert `result[0].message == "test error"` and `result[0].range == expected_lsp_range`. Catches: message or range field dropped or transformed incorrectly.

## Key Considerations

**Import path divergence (real risk):**
Site 1 uses `MarkyDiagSeverity` (alias). Site 2 uses `crate::state::DiagnosticSeverity` (no alias).
Both are the same underlying type. Before writing the helper, confirm the canonical path:
```
grep -n "DiagnosticSeverity" markymark-lsp/src/diagnostics.rs markymark-lsp/src/state/*.rs
```
Use the path from `crate::diagnostics::DiagnosticSeverity` — that module is the designed re-export point.

**Source field is protocol-observable:**
The string `"markymark"` in the `source` field is what LSP clients use to filter or display
diagnostics. If this string ever changes, clients that filter by source will silently drop
diagnostics. The test `test_source_field_is_markymark` pins this value explicitly.

**server.rs line count pressure:**
server.rs is at 952 lines. This refactor must NOT add the helper to server.rs. It must go to
convert.rs. Verify line count decreases after the change.

**Behavioral preservation (the whole point):**
After extraction, the output of both sites must be byte-for-byte identical to before.
Test this by verifying: range, severity, source field ("markymark"), message, and default fields.
Any field addition or change is a separate task — this task is pure DRY refactoring.

## Anti-patterns

- ❌ No unwrap/expect in production code — the mapping is infallible, no need for either
- ❌ No TODOs without issue numbers
- ❌ Do NOT place the helper in server.rs (counteracts the line-count goal)
- ❌ Do NOT change field values during this refactor — behavioral preservation only
- ❌ Do NOT make the helper `pub(crate)` in server.rs as a private fn — it belongs in convert.rs
      alongside all other `to_lsp_*` conversions
