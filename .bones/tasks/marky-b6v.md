---
id: marky-b6v
title: Restore DocumentIndex::from_scan() after self_cell merge
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---


During merge of PR #21 (self_cell refactor) into feature/mark-brza, DocumentIndex::from_scan() method was lost. This method provides the Zig SIMD scanning path (commits 2696947, 49b9f9f). All supporting code (ScanBackend, ZigScanBackend, FFI) is intact. Need to re-add: from_scan method, two helper functions, and test module - all adapted to use self_cell pattern like from_ast does.

## Design

# Plan: Restore `DocumentIndex::from_scan()` on self_cell Architecture

## Context

During the merge of origin/main (PR #21 - marky-5yt self_cell refactor) into feature/mark-brza, we accepted their version of `document.rs` which replaced the old `'static` lifetime pattern with `self_cell`. This was correct - self_cell is the superior architecture. But it dropped our `from_scan()` method (commits 2696947, 49b9f9f) which provides the Zig SIMD scanning path.

The old `from_scan` used `unsafe { &*(bump as *const Bump) }` to fake a `'static` lifetime. The new version must use the self_cell closure pattern that `from_ast` already demonstrates.

**All supporting code is intact**: `ScanBackend` trait, `ZigScanBackend`, `ScanLinkType`, all result types, FFI wrappers, feature flags. Only `from_scan`, two helper functions, and the test module need to be re-added.

## Files to Modify

**Single file**: `markymark-index/src/document.rs` (currently 857 lines)

## Changes

### 1. Add feature-gated import (line ~12)

After the existing imports, add:
\`\`\`rust
#[cfg(feature = "zig-kernels")]
use markymark_core::scanner::{ScanBackend, ScanLinkType};
\`\`\`

### 2. Add \`from_scan\` method (after \`from_ast\`, ~line 438)

The method follows the **identical self_cell construction pattern** as \`from_ast\` (line 224-437):

\`\`\`
Phase 1: Collect owned data from ScanBackend
Phase 2: Create DocumentOwner with DocumentArena::new()
Phase 3: Build DocumentDependent in DocumentIndexCell::new(owner, move |owner| { ... })
\`\`\`

Key differences from \`from_ast\`:

| Aspect | \`from_ast\` | \`from_scan\` |
|--------|-----------|-------------|
| Owned intermediates | Defines HeadingOwned, etc. inside fn | Uses ScanBackend result types directly (already owned Strings) |
| Arena source | \`ast.into_arena()\` | \`DocumentArena::new()\` |
| Position data | Direct from AST Range | \`byte_offset_to_position()\` conversion |
| Error handling | Infallible | \`unwrap_or_default()\` on scan results |
| XML tags | Extracted | Empty slice \`&[]\` |
| Feature gate | None | \`#[cfg(feature = "zig-kernels")]\` |

The scan result types (\`HeadingResult\`, \`LinkResult\`, etc.) already contain owned \`String\` fields, so **no intermediate owned types are needed** - they play the same role that \`HeadingOwned\`/\`BlockOwned\`/etc. play in \`from_ast\`.

**Offset formulas** (bug-fixed in commit 49b9f9f):
- Wiki \`[[target]]\`: end = offset + target.len() + 4
- Wiki \`[[target|alias]]\`: end = offset + target.len() + 1 + alias.len() + 4
- Markdown \`[text](url)\`: end = offset + text.len() + url.len() + 4
- Block \`^id\`: end = offset + 1 + id.len()
- Heading \`## Text\`: end = offset + level + 1 + text.len()

### 3. Add helper functions (after \`build_outline\`, before tests, ~line 620)

Both feature-gated with \`#[cfg(feature = "zig-kernels")]\`:

- \`byte_offset_line_starts(text: &str) -> Vec<u32>\` - builds line start offset table
- \`byte_offset_to_position(line_starts: &[u32], offset: u32) -> Position\` - binary search conversion

These are copied verbatim from the old implementation. They are pure functions with no lifetime concerns.

### 4. Add test module (end of file)

\`#[cfg(all(test, feature = "zig-kernels"))]\` module with:
- \`MockScanBackend\` or \`ZigScanBackend\` (tests use real backend since feature flag is already required)
- 16 tests covering: empty doc, headings, TOC, outline, links (wiki/markdown), tags, blocks, XML empty, parity with from_ast, and 4 bug-fix regression tests for range calculations

Tests are copied verbatim from commit 49b9f9f. They used \`ZigScanBackend\` which works since the test module is gated on \`zig-kernels\`.

## Validated Edge Cases

These were checked during plan validation:

1. **Empty xml_tags slice in self_cell closure**: Use \`BumpVec::<XmlTagEntry<'_>>::new_in(arena_ref).into_bump_slice()\` instead of bare \`&[]\`. Matches from_ast style and avoids type inference ambiguity.

2. **Wiki link heading fragments**: Old code used \`heading: None\` for all wiki links. Preserve this. Splitting target on \`#\` to extract heading fragments is an enhancement, not a restoration. Known gap documented in progress.json.

3. **from_ast filters empty wiki links** (lines 288-293 check target_page/heading/block_id). from_scan does NOT need this filter because the Zig scanner only emits links it actually detects - there are no empty-target links in scan output.

4. **Import must be feature-gated**: \`ScanBackend\` and \`ScanLinkType\` are always compiled in markymark-core, but the import in document.rs must be \`#[cfg(feature = "zig-kernels")]\` to avoid unused-import warnings in the default build.

5. **Closure captures**: \`line_starts: Vec<u32>\` and all \`Vec<*Result>\` from scan methods are moved into the closure via \`move |owner|\`. The \`text: &str\` and \`backend: &dyn ScanBackend\` borrows are NOT captured - they're used before the closure.

6. **HeadingResult.offset semantics**: Confirmed - offset points to the start of the heading line (the first \`#\`). Formula \`offset + level + 1 + text.len()\` is correct for \`## Text\` format.

7. **Tests use ZigScanBackend**: Requires Zig binaries built locally. Tests are gated on \`#[cfg(all(test, feature = "zig-kernels"))]\` and run via \`cargo test --features zig-kernels\`.

8. **CI path filter gap**: The zig-kernels CI job only triggers on \`zig/**\` and \`markymark-kernels/**\` changes. Changes to markymark-index alone won't trigger it. Add \`markymark-index/**\` to the dorny/paths-filter in ci.yml (1 line addition, trivial).

## Verification

1. \`cargo test -p markymark-index\` - existing tests still pass (no changes to from_ast or accessors)
2. \`cargo test --features zig-kernels\` - from_scan tests pass (requires Zig kernels built locally)
3. \`cargo clippy --features zig-kernels -- -D warnings\` - no lint warnings
4. \`cargo clippy --workspace --all-targets -- -D warnings\` - default build unaffected
5. Check: no conflict markers remain in any file (\`grep -r \"<<<<<<\" .\`)

**CI note**: Also add \`'markymark-index/**'\` to the zig-kernels path filter in ci.yml so future index-only changes trigger the zig-kernels test job.

## File Size Note

Adding ~120 lines (from_scan + helpers) + ~165 lines (tests) brings document.rs to ~1140 lines, past the 1000-line hard stop. File a follow-up bead to split document.rs into submodules (types.rs, build.rs, mod.rs). Not in scope for this task - we're restoring lost functionality, not refactoring.
