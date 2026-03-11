---
id: marky-ccv
title: Scaffold Zig directory with build.zig and src structure
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---





















Create zig/ directory with build.zig producing libmarky_kernels.a. Set up src/kernels/, src/shared/, src/reference/, test/ directories. Initial build.zig should compile an empty C adapter (c_adapter.zig with no exports) and produce a valid static library. Verify with: cd zig && zig build lib. See docs/plans/brza-markymark.md Section 10 for directory structure.

## Design

## Goal
Create the zig/ directory scaffold with build.zig producing libmarky_kernels.a. Set up src/kernels/, src/shared/, src/reference/, test/ directories. Initial build.zig compiles an empty C adapter (c_adapter.zig with a single no-op export) and produces a valid static library. This is the foundation for all Zig kernel work.

## Effort Estimate
2-3 hours

## Success Criteria
- [ ] `cd zig && zig build lib` succeeds and produces zig/zig-out/lib/libmarky_kernels.a
- [ ] Static library exports at least one symbol (nm -g shows marky_noop or similar)
- [ ] Directory structure matches spec Section 10: src/kernels/, src/shared/, src/reference/, test/
- [ ] build.zig uses Zig 0.15.2 API (not 0.14.x patterns)
- [ ] `cd zig && zig build test` passes (at least one smoke test)
- [ ] .gitignore includes zig-cache/ and zig-out/

## Implementation Checklist
- [ ] Create zig/build.zig with lib artifact targeting c-shared static library
- [ ] Create zig/src/c_adapter.zig with single export fn marky_version() -> u32
- [ ] Create zig/src/kernels/ directory (empty, with .gitkeep or placeholder)
- [ ] Create zig/src/shared/ directory
- [ ] Create zig/src/reference/ directory
- [ ] Create zig/src/fixtures.zig (empty test fixture module)
- [ ] Create zig/src/harness.zig (empty test harness module)
- [ ] Create zig/test/ directory
- [ ] Add zig-cache/ and zig-out/ to .gitignore
- [ ] Verify with zig build lib && nm -g zig-out/lib/libmarky_kernels.a

## Edge Cases
- Empty input: N/A for scaffold task
- Unicode/UTF-8: N/A for scaffold task
- Large input: N/A for scaffold task
- Zig version mismatch: build.zig should check minimum Zig version and emit clear error if < 0.15.2
- Missing Zig compiler: Handled by downstream build.rs, not this task

## Anti-patterns
- NO Zig 0.14.x patterns (addStaticLibrary changed to addLibrary in 0.15.x — verify against zig langref)
- NO build.zig that silently succeeds without producing output (must verify artifact exists)
- NO deeply nested module structure before it's needed (keep flat, add subdirs when kernels arrive)
- NO placeholder files that won't compile (every .zig file must be parseable by zig build)

## Error Handling
- build.zig: Use @compileError for version checks
- c_adapter.zig: marky_version() is infallible, returns compile-time constant

## Test Specifications (what bug does each test catch?)
- test_build_produces_library: catches build.zig misconfiguration that silently produces no output
- test_version_export: catches c_adapter.zig export linkage failure (symbol not visible in .a file)
- test_zig_version_check: catches using deprecated 0.14.x APIs that compile but behave differently in 0.15.2
