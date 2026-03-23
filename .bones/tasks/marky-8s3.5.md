---
id: marky-8s3.5
title: Implement Zig slug generator kernel
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3
---



Create zig/src/kernels/slug.zig. SIMD-accelerated URL slug generation from heading text. Operations: ASCII lowercase, space/punctuation to hyphens, strip special characters, collapse consecutive hyphens, trim leading/trailing hyphens. Export as marky_slugify(text, len, output, output_cap) -> i32 (returns written length). Handle UTF-8: pass through non-ASCII bytes unchanged (GitHub-flavored slug behavior). Tests: basic heading, heading with special chars, all-punctuation input, empty input, unicode heading, very long heading (truncation at cap). Small kernel but called per-heading so SIMD matters at scale.

## Design

## Goal
Create zig/src/kernels/slug.zig implementing SIMD-accelerated URL slug generation from heading text. Operations: ASCII lowercase, space/punctuation to hyphens, strip special characters, collapse consecutive hyphens, trim leading/trailing hyphens. Passes through non-ASCII bytes unchanged (GitHub-flavored slug behavior). Small kernel but called per-heading so SIMD matters at scale.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] slug.zig compiles and exports marky_slugify via c_adapter.zig
- [ ] ASCII uppercase converted to lowercase
- [ ] Spaces and punctuation converted to single hyphens
- [ ] Consecutive hyphens collapsed to single hyphen
- [ ] Leading and trailing hyphens trimmed
- [ ] Non-ASCII UTF-8 bytes passed through unchanged (GitHub behavior)
- [ ] Returns written length as i32 (positive = bytes written, negative = error)
- [ ] cd zig && zig build test passes
- [ ] Output matches GitHub's heading slug algorithm for test cases

## Implementation Checklist
- [ ] Create zig/src/kernels/slug.zig
- [ ] SIMD lowercasing: vectorized comparison A-Z, add 32 to lowercase
- [ ] SIMD space/punctuation detection: vectorized character class check
- [ ] Replace space/punct with '-', strip other special chars
- [ ] Post-pass: collapse consecutive hyphens (can be done in-place)
- [ ] Trim leading/trailing hyphens from output
- [ ] Handle non-ASCII: bytes >= 128 pass through unchanged
- [ ] Add C ABI export to c_adapter.zig
- [ ] Write unit tests against GitHub slug reference
- [ ] Update build.zig

## Edge Cases
- Empty input: return 0 written
- All spaces: produces empty slug after trimming hyphens (return 0)
- All punctuation: produces empty slug after trimming (return 0)
- Leading/trailing spaces: "  heading  " -> "heading" (trimmed hyphens)
- Consecutive spaces: "a  b" -> "a-b" (collapsed)
- Non-ASCII: "cafe au lait" vs "cafe-au-lait" (accented chars preserved)
- Very long heading: output capped at output_cap, return -2 if truncated
- Heading with numbers: "Section 2.1" -> "section-21" (period stripped)
- Heading with code spans: "Using `fmt`" -> "using-fmt" (backticks stripped)
- Null bytes in input: treated as special chars, stripped

## Anti-patterns
- NO heap allocation (write directly to caller's output buffer)
- NO multi-pass when single-pass suffices (lowercase + replace in one SIMD pass)
- NO stripping non-ASCII (GitHub preserves them in slugs)
- NO assuming output is always shorter than input (it can be, but cap must be checked)
- NO tests that only check one slug without comparing to GitHub's known behavior

## Error Handling
- Null text pointer: return -1
- Zero length: return 0 (empty slug)
- Null output pointer: return -1
- Zero output_cap: return -2 (buffer too small for any output)
- Output truncated: return -2, write as much as fits

## Test Specifications (what bug does each test catch?)
- test_empty_input: catches null deref on zero-length text
- test_basic_heading: catches incorrect lowercase or hyphen conversion
- test_special_characters: catches failure to strip punctuation
- test_consecutive_hyphens: catches failure to collapse multiple hyphens
- test_leading_trailing_hyphens: catches failure to trim leading/trailing hyphens
- test_all_spaces: catches returning hyphens instead of empty string
- test_all_punctuation: catches returning hyphens instead of empty string
- test_non_ascii_preserved: catches stripping non-ASCII bytes (should be kept)
- test_numbers_preserved: catches stripping digits
- test_github_parity: catches slug algorithm divergence from GitHub's behavior
- test_output_cap_truncation: catches writing past output buffer capacity
- test_uppercase_to_lowercase: catches SIMD lowercasing off-by-one (e.g., '[' being lowered)
