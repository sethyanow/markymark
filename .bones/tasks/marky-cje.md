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
- Tree-sitter-yaml `root.has_error()` is the gate. On error, the frontmatter is rejected (the document carries an empty frontmatter, key access returns `None` for everything) and a diagnostic is surfaced through the existing diagnostics channel — no fall-back to lenient parsing. Lenient was the source of this bug class; strict is the intended contract.

### Diagnostic plumbing — index-side, via the existing pull model

The diagnostics architecture in this codebase is **pull, not push**: parsers populate the `DocumentIndex`, and `markymark_index::compute_diagnostics(index, realm, uri) -> Vec<CoreDiagnostic>` derives diagnostics afterward by walking accessor methods (`wiki_links()`, `markdown_links()`, `headings()`, `xml_tags()`). Both LSP `textDocument/publishDiagnostics` and the MCP `get-diagnostics` tool consume the same function. See `markymark-index/src/diagnostics.rs:27`.

To fit this model, the malformed-YAML state must live on the index, not be emitted by the parser:

1. **New field on `DocumentDependent`** (`markymark-index/src/document/mod.rs:37-62`):
   `frontmatter_error: Option<FrontmatterParseError<'a>>`
   sitting next to the existing `frontmatter: &'a [FrontmatterEntry<'a>]`. Arena-backed; carries `range: Range` and `message: &'a str`.
2. **New accessor on `DocumentIndex`** mirroring `frontmatter()`:
   `pub fn frontmatter_error(&self) -> Option<&FrontmatterParseError<'_>>`.
3. **`compute_diagnostics` extension** — a 5th check after the existing four (broken wiki links, broken markdown anchors, duplicate heading slugs, unclosed XML tags). Reads `index.frontmatter_error()` and emits a `CoreDiagnostic { severity: Error, range, message }`. No transport-specific work — LSP and MCP both pick it up automatically.

**Diagnostic range = first ERROR node's byte range, not whole-frontmatter range.** Tree-sitter exposes the ERROR node directly; mapping its byte range to LSP coords gives an actionable pointer ("malformed flow mapping at column 14") instead of "something is wrong somewhere in your frontmatter."

### Return shapes

- **New parser function** in a new module `markymark-parser/src/extract/frontmatter_yaml.rs`:
  ```
  pub(crate) fn parse_yaml_to_frontmatter<'a>(
      content: &str,
      arena: &'a bumpalo::Bump,
  ) -> Result<Frontmatter<'a>, FrontmatterParseError<'a>>;
  ```
  Walks the tree-sitter-yaml CST. Reuses `scalar_to_value` (existing in `frontmatter.rs`) for leaf coercion. Recurses into `block_mapping` / `flow_mapping` / `block_sequence` / `flow_sequence` to populate `FrontmatterValue::Map` / `::List`. Returns `Err` with the first ERROR node's range when `root.has_error()` is true.

- **`extract_frontmatter` upgraded return type** to carry both:
  ```
  pub struct FrontmatterExtractResult<'a> {
      pub frontmatter: Frontmatter<'a>, // empty when error.is_some()
      pub error: Option<FrontmatterParseError<'a>>,
  }
  pub fn extract_frontmatter<'a>(...) -> Option<FrontmatterExtractResult<'a>>;
  ```
  The outer `Option` preserves the existing "no `---` delimiters at all" distinction. The inner `error: Option<...>` carries the strict-rejection state. Callers (the index builder) propagate `error` into the index's new `frontmatter_error` field.

- **`parse_simple_yaml` is removed in this task.** Strict means strict — no deprecation cycle, no dead-code retention. Verified only 3 references via LSP `findReferences` (def + 2 callsites in `extract_frontmatter`); both callsites become `parse_yaml_to_frontmatter` calls.

### Module placement

New module: `markymark-parser/src/extract/frontmatter_yaml.rs` (sibling to `frontmatter.rs`).

Not folded into `markymark-parser/src/structured/yaml.rs` despite reuse opportunities — that file is already 669 lines, has a different output shape (`StructuredAst` / `KeyEntry`, position-only), and a different consumer (outline / symbol features). Crowding it with frontmatter-specific value extraction conflates two contracts. If a small node-kind classification helper (e.g., flow vs block scalar) wants sharing, expose it `pub(crate)` from `structured/yaml.rs` — don't move logic into it.

### Out of scope for this fix

- Replicating tree-sitter-yaml grammar in Zig — different concern, different timeline, partially overlaps with `marky-0mr` (Zig md4c fast-path). YAML's grammar is multi-mode + indentation-sensitive; not the right vehicle for a P2 frontmatter fix.
- Multi-document YAML inside frontmatter — `extract_frontmatter` clips at the first `\n---\n`, so the content fed to the YAML parser is a single document by construction. Not a question.
- `extract_page_properties` (Logseq `key:: value` syntax) — separate parser, separate grammar, hand-rolled stays.
- `aliases()` accessor — currently derived from a flat-list `aliases` frontmatter key. Today it works. With nesting fixed, list-of-strings still works the same way; no migration needed. If a downstream consumer wants nested aliases later, that's a follow-up.
- The existing structured `parse_yaml` parser (used for outline of `.yaml` files) — untouched; only the frontmatter path migrates.

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
- [2026-04-24T23:24:51Z] [Seth] Resolved all open implementation questions ahead of SRE refinement (Seth's instruction).

Investigated existing diagnostics architecture: it's a pull model. Parsers populate DocumentIndex; markymark_index::compute_diagnostics(index, realm, uri) walks accessors after the fact and derives Vec<CoreDiagnostic>. Both LSP textDocument/publishDiagnostics and MCP get-diagnostics consume the same function (markymark-index/src/diagnostics.rs:27).

Decisions added to skeleton 'Diagnostic plumbing' / 'Return shapes' / 'Module placement' / 'Out of scope' sections:

1. Diagnostic plumbing: new field 'frontmatter_error: Option<FrontmatterParseError<'a>>' on DocumentDependent (markymark-index/src/document/mod.rs:37-62), arena-backed. New accessor DocumentIndex::frontmatter_error(). compute_diagnostics gets a 5th check after the four existing ones. No transport changes — LSP and MCP both pick it up automatically.

2. Diagnostic range: first ERROR node's byte range mapped to LSP coords, NOT the whole frontmatter block. Tree-sitter exposes the node directly; trivial cost; gives an actionable pointer.

3. Parser return: parse_yaml_to_frontmatter(content, arena) -> Result<Frontmatter<'a>, FrontmatterParseError<'a>>. Reuses scalar_to_value for leaf coercion. Recurses for block/flow mappings and sequences.

4. extract_frontmatter return: new struct FrontmatterExtractResult { frontmatter: Frontmatter<'a>, error: Option<FrontmatterParseError<'a>> }. extract_frontmatter returns Option<FrontmatterExtractResult>. Outer Option preserves 'no --- at all' distinction; inner error carries strict-rejection state. Callers propagate error into the index's new frontmatter_error field.

5. parse_simple_yaml: removed in this task. No deprecation. LSP findReferences confirmed 3 refs total — def + 2 callsites both in extract_frontmatter. Strict means strict.

6. Module placement: new markymark-parser/src/extract/frontmatter_yaml.rs sibling. Not folded into structured/yaml.rs (669 lines already, different output shape, different consumer). If small classification helpers want sharing, expose pub(crate) from structured/yaml.rs.

7. Multi-doc + Logseq properties + aliases accessor + structured parse_yaml — explicitly out of scope, recorded.

Skeleton is now SRE-refinement ready: scope is fixed, contracts are named, no further design questions blocking task creation.
