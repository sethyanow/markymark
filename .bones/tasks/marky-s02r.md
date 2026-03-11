---
id: marky-s02r
title: Copy Bun md4c Zig port and strip Bun-specific dependencies
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


## Design

## Goal
Copy Bun's Zig md4c parser (~8K lines, 15 files from src/md/) into our zig/src/md4c/ directory and strip all Bun-specific dependencies so it compiles standalone with our existing Zig build infrastructure (Zig 0.15.2+).

## Context
- Bun repo: https://github.com/oven-sh/bun (MIT license)
- Source files in bun/src/md/ (~8,274 lines, 15 files)
- Key files: parser.zig, blocks.zig, inlines.zig, line_analysis.zig, links.zig, types.zig, html_renderer.zig, ref_defs.zig, containers.zig, entity.zig, helpers.zig, autolinks.zig, unicode.zig, render_blocks.zig, root.zig
- Our Zig build infrastructure: build.zig requires Zig 0.15.2+, produces libmarky_kernels.a
- Bun's md4c uses standard std.mem.Allocator interface — allocator-agnostic
- Primary Bun dependency: bun.JSError in Renderer callback signatures — replace with standard Zig error set

## Implementation Steps

### Step 1: Pin Bun source version
- Clone Bun repo at a specific recent commit (pick latest release tag or HEAD of main)
- Record the exact commit hash in a VENDORED_FROM comment at top of root.zig
- Verify src/md/ directory contains the 15 expected files

### Step 2: Copy files
- Create zig/src/md4c/ directory
- Copy all 15 files from bun/src/md/ into zig/src/md4c/
- Add MIT license header to each file referencing Bun origin:
  // Vendored from https://github.com/oven-sh/bun (MIT License)
  // Original: src/md/<filename> at commit <hash>

### Step 3: Strip Bun-specific dependencies
Audit every file for non-standard imports. Known replacements:

| Bun Import | Replacement |
|------------|-------------|
| @import("bun") for bun.JSError | Define local error set: const Md4cError = error{OutOfMemory, ParseError, RenderError}; |
| bun.JSError!void callback returns | Replace with Md4cError!void |
| @import("root") | Replace with relative @import or remove |
| Any bun.Output | Replace with std.io.AnyWriter or std.ArrayList(u8) |
| bun.StackCheck | Remove entirely (our build.zig already disables stack checking) |

After replacement, verify: grep -r '@import.*bun' zig/src/md4c/ returns zero matches.
Also verify: grep -r '@import.*root' zig/src/md4c/ returns zero matches (unless root.zig self-ref).

### Step 4: Fix inter-file imports
All @import paths within md4c/ files should use relative imports:
- @import("./types.zig"), @import("./helpers.zig"), etc.
- If Bun used package-relative imports, convert to file-relative

### Step 5: Update zig/build.zig
- Do NOT add md4c files to libmarky_kernels static library yet
- Add a test step that compiles the md4c module for verification:
  const md4c_test = b.addTest(.{ .root_source_file = b.path("src/md4c/root.zig"), ... });
  const run_md4c_test = b.addRunArtifact(md4c_test);
  test_step.dependOn(&run_md4c_test.step);
- Ensure existing kernel tests still pass: zig build test

### Step 6: Write smoke test
- Location: zig/src/md4c/root.zig (add test block at bottom) OR zig/test/md4c_smoke_test.zig
- Test using HtmlRenderer to parse and render, verify output:
  test "md4c smoke: heading and paragraph" {
      const allocator = std.testing.allocator;
      const input = "# Hello\n\nworld\n";
      const html = try renderToHtml(allocator, input, .{});
      defer allocator.free(html);
      try std.testing.expectEqualStrings("<h1>Hello</h1>\n<p>world</p>\n", html);
  }
- Additional smoke tests:
  test "md4c smoke: empty input" — verify empty string doesn't crash, returns empty
  test "md4c smoke: code fence" — verify fenced code block renders correctly
  test "md4c smoke: wiki link passthrough" — verify [[link]] passes through as text (not md4c's job to handle)

## Success Criteria
- [ ] All 15 md4c files present in zig/src/md4c/
- [ ] Bun commit hash recorded in root.zig header comment
- [ ] Zero Bun imports: grep -r '@import.*bun' zig/src/md4c/ returns 0 matches
- [ ] Zero non-local root imports: grep -r '@import.*root' zig/src/md4c/ shows only root.zig self-reference
- [ ] Code compiles: zig build (zero errors, zero warnings)
- [ ] Existing kernel tests pass: zig build test (all green)
- [ ] Smoke test verifies actual parse output (heading renders to <h1>, paragraph to <p>)
- [ ] Empty input smoke test passes (no crash, empty or minimal output)
- [ ] MIT license attribution on each vendored file

## Key Considerations (SRE Review)

**Zig Version Mismatch Risk**:
Bun may target a different Zig version than our 0.15.2+. If Bun uses 0.14 APIs:
- Check for deprecated std library functions
- Watch for ArrayList API changes between Zig versions
- If significant migration needed, document changes in commit message
- Read docs/modules/zig/01-langref/README.md for 0.14→0.15 migration guide BEFORE fixing

**entity.zig is 2164 lines (exceeds 1000-line hard stop)**:
- This file is a lookup table (HTML entity mappings), not logic
- Lookup tables are exempt from the 1000-line rule — they are data, not code
- Do NOT split entity.zig. Leave as-is.
- If any OTHER vendored file exceeds 1000 lines of logic, create a follow-up bead

**@import("root") resolution**:
Bun's build system resolves @import("root") to Bun's root module.
Our build won't have this. Must trace what "root" provides and either:
- Replace with direct imports from within md4c/
- Create a thin shim root that re-exports needed types

**Circular dependency between md4c files**:
Files like blocks.zig, inlines.zig, links.zig may have mutual imports.
Zig handles circular @imports at comptime — this should work if all files are in same package.
If not, use forward declarations or restructure imports.

**Allocator compatibility**:
Bun's md4c uses std.mem.Allocator interface — fully compatible with our patterns.
No changes needed for allocator plumbing.

## Anti-Patterns
- ❌ Do NOT modify md4c parsing logic — only strip Bun dependencies
- ❌ Do NOT wire into c_adapter.zig or FFI yet (later task)
- ❌ Do NOT remove html_renderer.zig — needed for smoke test verification
- ❌ Do NOT split entity.zig (it's a data table, not logic)
- ❌ Do NOT add md4c to libmarky_kernels.a yet — only add test step
- ❌ Do NOT use @panic or unreachable in any replacement code — use proper error returns
