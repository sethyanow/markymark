---
id: marky-cje
title: Frontmatter parser drops nesting — nested lists and flow maps mangled on parse
status: open
type: bug
priority: 2
---



## Context

I'm using markymark as a frontmatter parser for a downstream project that stores arbitrary user-declared keys in frontmatter. When I put nested YAML values in the frontmatter block, markymark either flattens them into broken strings or treats them as a single scalar. Primitives and flat lists work fine — it's nesting and flow maps that break.

## Repro

Given a markdown file like:

```markdown
---
id: example
title: Nesting probe
reviewer_pairs: [[alice, bob], [carol, dave]]
config: {env: prod, region: us-east}
nested_list: [[1, 2], [3, 4]]
---
```

Parse via `DocumentIndex::from_text(source)` → `frontmatter_map_from_entries(doc_index.frontmatter())`.

## What I expected

- `reviewer_pairs` → `List` of two inner `List`s, each containing two `String`s
- `config` → `Map` with two `String → String` entries
- `nested_list` → `List` of two inner `List`s of `Integer`s

## What actually happens

- `reviewer_pairs` → flat `List` with four mangled `String` items: `"[alice"`, `"bob]"`, `"[carol"`, `"dave]"` (the bracket boundaries are treated as part of the string because the parser just strips the outer `[...]` and splits on `,`)
- `config` → single `String`: `"{env: prod, region: us-east}"` (flow maps aren't recognized at all — treated as a scalar)
- `nested_list` → same flattening pattern as `reviewer_pairs`

The outer `[...]` is the only bracket structure the parser recognizes. Anything inside that also uses brackets or braces gets corrupted.

## Where

Looks like `parse_simple_yaml` in `markymark-parser/src/extract/frontmatter.rs` — the branch that handles `[...]` does a flat `split(',')` without bracket balancing, and there's no branch for `{...}` flow maps at all. They fall through to `scalar_to_value` and become a `String`.

## Impact

Any downstream consumer that stores typed values in frontmatter loses structure on nested data. In my case I'm round-tripping a `BTreeMap<String, serde_yaml::Value>` — the write side produces valid YAML, but markymark's read side flattens it, so the next rewrite corrupts the on-disk file. Silent data loss on the second write after a hand-edit.

Primitives, flat lists of primitives, and flat scalar maps (no nesting, no `{}`) are unaffected and continue to work.

## Direction (decided 2026-04-24)

Replace `parse_simple_yaml` with a tree-sitter-yaml-driven parser that walks the CST and emits owned `FrontmatterValue` trees.

**Why tree-sitter-yaml:**
- Already a workspace dep (`Cargo.toml:41`, `markymark-parser/Cargo.toml:17`, `markymark-parser/BUILD.bazel:12`) — paid for in build/binary today.
- Already used in `markymark-parser/src/structured/yaml.rs` (669 lines of CST walker covering block_mapping, flow_mapping, block_sequence, flow_sequence, multi-line strings, anchors, aliases, merge keys). That walker classifies into `KeyEntry` / `ValueKind`; the bug fix needs a sibling visitor that emits `FrontmatterValue` instead.
- Avoids adding `serde_yaml` (in maintenance) or `saphyr` on top of a YAML grammar we already ship.
- Hand-rolling a bracket-balanced parser was ruled out — every YAML feature beyond the reporter's repro becomes a future bug (escaped quotes in flow scalars, multi-line plain scalars, block scalars `|`/`>`, anchors/aliases, complex keys).

**Behavior on malformed YAML: strict + diagnostic (decided 2026-04-24).**
- Tree-sitter-yaml `root.has_error()` is the gate. On error, the frontmatter is rejected (treated as if absent) and a diagnostic is surfaced through the existing diagnostics channel — no fall-back to lenient parsing. Lenient was the source of this bug class; strict is the intended contract.
- Concrete plumbing to settle in SRE refinement: where the diagnostic is emitted (parser-side error type vs. index-side warning vs. LSP `PublishDiagnostics`), and the structure for `extract_frontmatter`'s return when YAML is malformed (`Result<Frontmatter>` vs. carrying diagnostics on the AST). The reporter's caller path is `DocumentIndex::from_text` → `frontmatter()` → `frontmatter_map_from_entries`; the diagnostic must reach the index/LSP layer, not be swallowed at the parser boundary.

**Out of scope for this fix:**
- Replicating tree-sitter-yaml grammar in Zig — different concern, different timeline, partially overlaps with `marky-0mr` (Zig md4c fast-path). YAML's grammar is multi-mode + indentation-sensitive; not the right vehicle for a P2 frontmatter fix.
- Multi-document YAML support (the existing `parse_yaml` has a TODO for this; same TODO inherits here).

## Workaround

None that preserves the original structure. Downstream consumers can detect-and-reject nested values at the boundary, but that changes the pass-through contract.

## Log

- [2026-04-24T22:06:35Z] [Seth] Validation pass — bug confirmed verbatim against current code on dev (commit d744b063).

Code site: markymark-parser/src/extract/frontmatter.rs:225-237 (parse_simple_yaml).

The [...] branch does:
  let inner = &value_str[1..value_str.len() - 1];
  for item in inner.split(',') { ... }
No bracket balancing, no recursion. Flow maps {...} have no branch — fall through to scalar_to_value and become String.

Note: parse_simple_yaml uses splitn(2, ':') per line, so 'config: {env: prod, region: us-east}' splits cleanly at the first colon and the entire brace expression survives as raw_value. It then fails the [...] guard and is stored as a single String (not split further). Matches reporter's expectation.

Validation method: appended a temp test to markymark-index/tests/typed_frontmatter.rs using the public DocumentIndex::from_text -> frontmatter_map_from_entries pipeline with the reporter's exact repro markdown. Ran with --nocapture, observed:
  reviewer_pairs -> [String("[alice"), String("bob]"), String("[carol"), String("dave]")]
  config         -> String("{env: prod, region: us-east}") (get_list returns None)
  nested_list    -> [String("[1"), String("2]"), String("[3"), String("4]")]
Output matches reporter's claim verbatim. Temp test reverted.

Type system already supports nesting: FrontmatterValue at markymark-parser/src/types/frontmatter.rs:51 has List(&'arena [FrontmatterValue]) and Map(&'arena [(&'arena str, FrontmatterValue)]) variants. FrontmatterValueEntry / FrontmatterValueRef / FrontmatterValueOwned all carry Map+List too. The fix is parser-only — no type plumbing required.

Diagnosis:
  Root cause: parse_simple_yaml is not a recursive YAML parser. The [...] branch flat-splits on ',' without bracket/brace tracking, and there is no {...} branch.
  Confidence: HIGH (direct repro via public API).
  Recommended fix location: parse_simple_yaml in markymark-parser/src/extract/frontmatter.rs.

Direction question still open in the skeleton (hand-rolled bracket-balanced recursive parser vs. swap to a real YAML lib for the frontmatter block). Worth resolving before SRE refinement — they imply different scope and edge-case surface.
- [2026-04-24T22:51:18Z] [Seth] Direction decided (Seth, 2026-04-24): Option C — reuse tree-sitter-yaml. Strict-with-diagnostic on malformed YAML. Skeleton 'Proposed direction' rewritten as 'Direction (decided 2026-04-24)' with rationale and out-of-scope items recorded.

Open implementation questions for SRE refinement:
- Diagnostic plumbing: where the malformed-YAML diagnostic surfaces (parser-side error variant, index-side warning channel, or LSP PublishDiagnostics).
- Return shape for extract_frontmatter when YAML is malformed (currently Option<Frontmatter>; may want Result or carry diagnostics on the value).
- Whether parse_simple_yaml is removed in this task or kept as dead code with a deprecation note for one cycle (favoring removal — strict means strict).
- Confirm the existing structured/yaml.rs first-document-only behavior is acceptable for frontmatter (it should be; frontmatter is by definition a single doc bounded by ---).

Optimize-branch impact validated: zero overlap. Only commit on optimize touching frontmatter.rs is c9f6860c (cosmetic let-else in extract_frontmatter, different function from parse_simple_yaml). marky-p88 epic is panic/normalization work, not value extraction. Fix on dev will merge cleanly.
