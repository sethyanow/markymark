---
id: marky-i873
title: 'PR#41 hardening: Vendored autolinks.zig — boolean check, doc comment, debug assert'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Three minor hardening improvements in the vendored autolinks.zig file.

## Items

### D1: Boolean check instead of arithmetic (lines 31, 49)

Current: d.open_count + d.close_count > 0
Types are usize (inlines.zig:15-16) — overflow is practically impossible on 64-bit.
Fix: d.open_count != 0 or d.close_count != 0 (cleaner, zero overhead).
Risk: Vendored code divergence from Bun upstream. Document in commit message.

### D2: scanUrlComponent doc comment (lines 62-107)

Clarify that callers use min_components = 0 for optional URL components (query, fragment).
Pure documentation change.

### D3: postProcessAutolinkEnd debug assertion (lines 254-302)

Add std.debug.assert(end_in >= beg + 3) at function entry. The j = end - 2 subtraction
can underflow if precondition violated. Callers currently enforce it but no runtime guard exists.

## Effort Estimate

1 hour

## Success Criteria

- [ ] Boolean check on lines 31 and 49 uses != 0 or pattern
- [ ] Doc comment for scanUrlComponent explains min_components = 0 optionality
- [ ] Debug assertion at top of postProcessAutolinkEnd
- [ ] All md4c tests pass: zig build test
- [ ] No regression in autolink extraction tests

## Key Considerations (SRE Review)

**Vendored code divergence**: These changes diverge from Bun's md4c port. The vendoring
header (line 1-3) already notes "Modifications" — these should be added to that list.
Consider whether to track divergences formally. Current vendoring header:
"Vendored from https://github.com/oven-sh/bun (MIT License)"

**D1 risk assessment**: usize on 64-bit = 8 bytes. Max value ~1.8e19. Even if every
byte in a 100KB doc created a delimiter, counts would be ~100K. No real overflow risk.
This is purely a style improvement.

**D3 caller analysis**: postProcessAutolinkEnd is called from processAutolinks (internal).
The caller constructs end_in from validated parser state. The assertion is defense-in-depth.

## Anti-patterns

- Do NOT refactor the vendored autolinks logic (minimize divergence)
- Do NOT add error handling to the assertion (debug-only is appropriate here)
