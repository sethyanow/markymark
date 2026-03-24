# Zig Patterns — markymark

Reusable patterns and hard-won lessons from Zig kernel and engine development.
Linked from [MEMORY.md](../MEMORY.md). Load when working on Zig code.

---

## FFI Boundary

- Generic `call_scan_ffi<T>` with buffer retry (start 64, double on -2, max 3 retries)
- `repr(C)` mirror structs at boundary, idiomatic Rust in public API
- `safe_slice()` rounds byte offsets to UTF-8 char boundaries
- `PhantomData<*mut ()>` for !Send/!Sync on stable Rust
- Drop impl sets handle to null for idempotent double-free protection
- FFI functions must initialize all output parameters before error returns
- For mmap-friendly binary formats, treat header counts and C pointers as untrusted input.
  Checked arithmetic avoids overflow panics; null-pointer guards prevent SIGSEGV. Zero
  padding bytes explicitly for deterministic output. Any `init()` accepting arbitrary
  `[]const u8` must also validate alignment before `@alignCast`

## Zig Kernel Conventions

- SIMD for sparse search: @Vector for candidates, scalar for validation
- Share parsing logic between SIMD and scalar via pub import from reference
- `exports_*.zig` + `comptime { _ = @import(...) }` for composable ABI
- `test { _ = @import(...); }` pulls sub-module tests into main test step
- Output-buffer capacity guard must come BEFORE the write, not after the increment (marky-wpl)
- Validate alignment before `@alignCast`: `if (@intFromPtr(p) % @alignOf(T) != 0) return null;`
  — panics in Debug/ReleaseSafe on misaligned arbitrary input (marky-5rq)

## errdefer + Explicit deinit = Double-Free (marky-gmny)

When a function has `errdefer obj.deinit()` at the top, **never** call `obj.deinit()`
explicitly on error paths — the errdefer fires on `return error.*` and double-frees.

For partially-transferred ownership (e.g. `headings.toOwnedSlice()` succeeded but
`links.toOwnedSlice()` failed), use a **scoped errdefer** immediately after the
successful transfer to clean up the transferred data:

```zig
const headings = ext.headings.toOwnedSlice(alloc) catch return error.OutOfMemory;
errdefer {
    for (headings) |h| alloc.free(h.text);
    alloc.free(headings);
}
const links = ext.links.toOwnedSlice(alloc) catch return error.OutOfMemory;
```

Also: `allocator.free(slice)` only frees the backing array, NOT owned strings inside
each element. Always iterate and free inner allocations first.

## OOM-Loop Testing Pattern

Iterate `FailingAllocator` `fail_index` from 0..N with GPA backing. GPA fills freed
memory with `0xaa` — double-free segfaults at `0xaaaaaaaaaaaaaaaa`. GPA `.deinit()`
returning `.leak` catches missing frees. Use 5 consecutive successes as termination
condition.

## ArrayListUnmanaged Scratch Buffer (marky-i3fl)

When a function builds a temporary string via `ArrayListUnmanaged(u8){}` and returns
`.items`, the backing allocation leaks because nobody calls `.deinit()`. Fix: add a
reusable scratch buffer field to the owning struct, `clearRetainingCapacity()` at
the start of each call, and have callers that persist the result `dupe()` it.
If a struct stores duped slices, its deinit must free them individually before
freeing the container list.

## md4c Error-Handling and Bounds (PR #39)

Patterns from marky-0mr.4/.6/.9 that recur in md4c Zig port:

- **Silent `catch {}`** for buffer appends hides allocation failures — use `try`
- **Pointer arithmetic on `BlockHeader`**: always compute alignment offset explicitly,
  never assume `+ @sizeOf(...)` lands on the right boundary; add bounds guard via `if`
- **Bounds before increment**: `pivot_end += 1` in binary search without checking
  `pivot_end + 1 < map.len` is latent OOB on degenerate fold tables
- **Dead code from dual-return**: when two consecutive branches both `return false`,
  the redundant one is unreachable — remove it
- **`>= N` vs `> N-1`**: use the form that most directly names the index being accessed
  (e.g. `beg > 1` for `content[beg - 2]`)

## Test Pointer Tricks for >4GB Fake Slices

To test early-return guards that fire before data is accessed (e.g. size checks
before `@intCast(text.len)`), construct a fake huge slice using a many-pointer:

```zig
var sentinel: u8 = 0;
const p: [*]const u8 = @ptrCast(&sentinel);  // [*] has no tracked length
const fake: []const u8 = p[0..huge_len];      // valid fat pointer; never dereference
```

`[*]const u8` slicing has no bounds check. The function must return before
touching slice data or the test will crash. Using `@as([*]const u8, ptr)` is NOT
valid in Zig 0.15 — use type-annotated variable form instead (marky-0mr.5).

## ExtractionRenderer Patterns

### Code Span Extraction (marky-pdyo)

- **Separate cursor**: `code_scan_cursor` is independent from `heading_scan_cursor` and
  `link_scan_cursor` (per marky-0rl6 — shared cursors corrupt offsets).
- **Dual accumulation**: When `in_code_span` and `in_heading` are both true,
  `text()` appends to BOTH buffers. Heading text includes code span content,
  code span is extracted independently.
- **Backtick run matching**: `findCodeSpanOffset()` scans for matching backtick runs
  (1, 2, or 3). Double-backtick spans work because the scan looks for a closing
  run of exactly the same length.
- **Fenced block exclusion**: `in_code_block` early return in `text()` fires before
  `in_code_span` check. Belt-and-suspenders guard.
- **Entity decoding**: md4c fires `TextType.code` (not `.entity`) for code span
  content, so entities are NOT decoded inside code spans (matches CommonMark spec).

### XML Tag Extraction (marky-fd74)

- **HTML callback parsing**: md4c fires `TextType.html` for block-level HTML.
  ExtractionRenderer parses `<tag>` / `</tag>` / `<tag />` patterns from raw
  HTML text, with case-insensitive tag name matching for close tags.
- **Void elements**: `<br>`, `<hr>`, `<img>` etc. are auto-closed.
- **md4c inline HTML pointers are NOT into source text** — md4c passes inline HTML
  fragments via `text()` callback with pointers to internal buffers, not the original
  source. Bounds check must validate both start AND end.
- **processHtmlFragments scans within fragments for multiple tags** — a single HTML
  block line like `<goal>win</goal>` contains multiple `<...>` sequences.
- **Test fixtures must use block-level HTML** — inline HTML on a single line is treated
  as inline by md4c. Tags only extracted from block-level (tag on its own line).

### ABI Changes

- Code spans: CMd4cResult grew from 40 to 48 bytes (added `code_spans` pointer +
  `code_spans_count`, removed `_padding`).
- XML tags: CMd4cResult grew from 136 to 144 bytes (added `xml_tags` pointer +
  `xml_tags_count`). Field order matters — SIGSEGVs result from layout mismatch.
- Blob header: `xml_tag_count` at offset 80 in v2 header. BlobXmlTag = 40 bytes.

## Build Quirks

- **Zig 0.15.2 archive format incompatibility with rust-lld** — `zig build lib` on
  Linux x86_64 produces archives that pass `ar t` but fail rust-lld. Fix (`f2a894f`):
  build.rs extracts .o files and re-packs with `ar rcs`. Only on Linux.
- **PIC required for Zig static libraries on Linux x86_64**
- **`zig build` ignores `ZIG_LOCAL_CACHE_DIR` env var** — must use `--cache-dir` CLI flag.
  build.rs purges both `zig/.zig-cache/` and `prefix/.zig-cache/` before each invocation.
- **build.rs invokes zig build lib via std::process::Command, zero build-dependencies**
- **rerun-if-changed enumerates individual .zig files** — directory-level watch only
  triggers on add/remove.

## Zig 0.15 API Breaks from 0.14

- `addStaticLibrary` → `addLibrary` with `.linkage = .static`
- `root_source_file` → `root_module` via `b.createModule()`
- `callconv(.C)` → just use `export fn`
- `ArrayList(T).init(allocator)` → `ArrayListUnmanaged(T){}` (allocator passed per-call)
- Always read Zig build system docs first
