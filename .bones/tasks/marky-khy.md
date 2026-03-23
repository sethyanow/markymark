---
id: marky-khy
title: 'Task 1: Wire frontmatter and Logseq properties into DocumentIndex'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-9mo
---




Wire existing parser frontmatter/properties extraction into DocumentIndex storage layer. Parser already extracts both (extract.rs:399, extract.rs:421) but DocumentIndex (document/mod.rs) doesn't store them. Add fields to DocumentDependent, extract in from_ast, expose via accessor methods, and surface in export-index MCP tool.

## Design

## Goal
Wire the parser's existing frontmatter and Logseq property extraction into the DocumentIndex storage layer so downstream MCP/LSP tools can query them.

**KEY CONTEXT**: The parser ALREADY extracts frontmatter and properties:
- `markymark-parser/src/extract.rs:399` — `extract_frontmatter()` parses YAML frontmatter
- `markymark-parser/src/extract.rs:421` — `extract_page_properties()` parses Logseq `key:: value`
- `markymark-parser/src/types/frontmatter.rs` — `Frontmatter`, `FrontmatterValue`, `Properties`, `PropertyValue` types

**THE GAP**: `DocumentDependent` (markymark-index/src/document/mod.rs:36-46) stores headings, blocks, wiki_links, tags, markdown_links, xml_tags — but NOT frontmatter or properties.

## Effort Estimate
4-6 hours

## Implementation

### 1. Study the existing arena pattern (MUST DO FIRST)
- Read `markymark-index/src/document/mod.rs:97-311` — the `from_ast()` method
- Pattern: Extract from AST into owned structs → move arena into DocumentOwner → re-allocate into arena-backed entries in DocumentDependent
- Every field follows: extract owned → arena_alloc_str → push to BumpVec → into_bump_slice
- Study an existing example: wiki_links_owned extraction (line 159-174) → arena allocation (line 249-258)

### 2. Write tests first (TDD)

**Test file**: `markymark-index/src/document/tests.rs` (or inline mod tests)

Tests targeting the actual implementation gap:

- `test_frontmatter_stored_in_document_index`: Parse markdown with frontmatter, build DocumentIndex via from_ast, verify frontmatter() accessor returns data
  - Bug this catches: frontmatter parsed but silently dropped during index construction
- `test_frontmatter_aliases_accessible`: Parse markdown with `aliases: [name1, name2]`, verify aliases() accessor returns Vec with both entries
  - Bug this catches: aliases not extracted from frontmatter or lost during owned→arena copy
- `test_properties_stored_in_document_index`: Parse Logseq file with `tags:: value`, verify properties() accessor returns data
  - Bug this catches: properties parsed but not wired into index
- `test_no_frontmatter_returns_empty`: Parse markdown without frontmatter block, verify frontmatter() returns empty/None
  - Bug this catches: missing frontmatter causes panic or wrong default
- `test_frontmatter_with_colon_in_value`: Parse `url: https://example.com`, verify URL preserved intact
  - Bug this catches: parse_simple_yaml splits on FIRST colon, breaking URLs (known bug in extract.rs:801)
- `test_frontmatter_and_properties_coexist`: Parse file with both YAML frontmatter AND Logseq properties, verify both stored
  - Bug this catches: one extraction overwrites the other
- `test_export_index_includes_frontmatter`: Verify export-index MCP tool output includes frontmatter fields
  - Bug this catches: frontmatter stored in index but not serialized in export output

### 3. Implementation checklist

- [ ] Add `FrontmatterOwned` struct to `from_ast()` — `key: String, value: FrontmatterValueOwned` (follows HeadingOwned pattern)
- [ ] Add `FrontmatterValueOwned` enum — `String(String), List(Vec<String>)` matching parser's `FrontmatterValue`
- [ ] Add `PropertyOwned` struct — `key: String, value: PropertyValueOwned` (follows same pattern)
- [ ] Add `PropertyValueOwned` enum — `String(String), List(Vec<String>), PageRef(String)` matching `PropertyValue`
- [ ] Extract frontmatter in `from_ast()` before moving arena: call `ast.frontmatter()`, convert to owned structs
- [ ] Extract properties in `from_ast()`: call `ast.page_properties()`, convert to owned structs
- [ ] Add `frontmatter: &'a [FrontmatterEntry<'a>]` field to `DocumentDependent` (line 36-46)
- [ ] Add `aliases: &'a [&'a str]` field to `DocumentDependent`
- [ ] Add `properties: &'a [PropertyEntry<'a>]` field to `DocumentDependent`
- [ ] Define `FrontmatterEntry<'a>` in `types.rs` — `key: &'a str, value: FrontmatterValueEntry<'a>`
- [ ] Define `FrontmatterValueEntry<'a>` — `String(&'a str), List(&'a [&'a str])`
- [ ] Define `PropertyEntry<'a>` — `key: &'a str, value: PropertyValueEntry<'a>`
- [ ] Define `PropertyValueEntry<'a>` — `String(&'a str), List(&'a [&'a str]), PageRef(&'a str)`
- [ ] In dependent construction closure: allocate frontmatter entries from owned data using arena_alloc_str + BumpVec
- [ ] In dependent construction closure: extract aliases from frontmatter `aliases` key, allocate as `&[&str]`
- [ ] In dependent construction closure: allocate property entries from owned data
- [ ] Add public accessor: `pub fn frontmatter(&self) -> &[FrontmatterEntry]`
- [ ] Add public accessor: `pub fn aliases(&self) -> &[&str]`
- [ ] Add public accessor: `pub fn properties(&self) -> &[PropertyEntry]`
- [ ] Update `Debug` impl for DocumentIndex to include frontmatter/properties counts
- [ ] Fix parse_simple_yaml colon-in-value bug: split on FIRST colon only via `splitn(2, ':')` (extract.rs:801)
- [ ] Update export-index serialization in markymark-mcp to include frontmatter and properties

## Key Considerations (SRE Review)

**Arena Allocation Safety**:
- The from_ast() method uses a self_cell pattern with unsafe raw pointer dereference (line 84-94)
- ALL new allocations MUST follow the existing pattern: owned data extracted BEFORE arena move, then re-allocated in the dependent closure
- Accessing the arena outside the dependent closure is UNSAFE and will cause UB
- Study existing xml_tags allocation (line 193-295) as the closest pattern to frontmatter (both have nested structures)

**The Colon-in-Value Bug**:
- `parse_simple_yaml` at extract.rs:801 does `line.find(':')` which finds the FIRST colon
- This breaks on `url: https://example.com` → key=`url`, value=`https` (truncated at second colon)
- Fix: use `splitn(2, ':')` to split on first colon only
- This is a pre-existing bug that MUST be fixed in this task since frontmatter URLs are common in Obsidian

**Edge Case: Frontmatter at EOF**:
- File with `---\n...\n` but no closing `---\n` — existing extract_frontmatter returns None, which is correct
- File ending with `---\n...\n---` (no trailing newline) — the `find("\n---\n")` check will MISS this. May need `find("\n---")` with boundary check.

**Edge Case: Unicode Keys/Values**:
- Frontmatter keys and values may contain Unicode (e.g., Japanese tags, emoji in aliases)
- arena_alloc_str handles UTF-8 strings, so this should work — but add a test

**Edge Case: Frontmatter + Properties Coexistence**:
- A file can have YAML frontmatter AND Logseq properties (frontmatter first, then properties after)
- extract_page_properties starts scanning from line 0, which would match inside frontmatter
- Need to verify that Logseq properties extraction skips the frontmatter block

**Edge Case: Empty Frontmatter**:
- `---\n---\n` (frontmatter block with no content) → should return empty HashMap, not None

**Performance**:
- Frontmatter extraction adds two HashMap allocations per document
- For most documents, frontmatter is small (<20 keys) — no performance concern
- For the arena pattern, owned structs are temporary — memory freed after arena re-allocation

## Anti-Patterns (FORBIDDEN)

- ❌ NO re-implementing extract_frontmatter() — use existing parser extraction at extract.rs:399
- ❌ NO storing arena-borrowed references directly — MUST follow owned→arena-alloc pattern (data race safety)
- ❌ NO unwrap/expect in new code (except the existing arena_ref mutex expect which is documented)
- ❌ NO adding new FrontmatterValue variants (Number, Bool, Date, Map) — current String/List is sufficient for aliases and basic queries. Extend in follow-up if needed.
- ❌ NO modifying existing DocumentDependent fields — only ADD new fields
- ❌ NO using serde_yaml or external YAML parser — existing parse_simple_yaml is sufficient (fix the colon bug)

## Success Criteria
- [ ] `ast.frontmatter()` data accessible via `DocumentIndex::frontmatter()` after `from_ast()`
- [ ] `ast.page_properties()` data accessible via `DocumentIndex::properties()` after `from_ast()`
- [ ] Aliases extracted from frontmatter `aliases` key into dedicated `aliases()` accessor
- [ ] Documents without frontmatter return empty slice from `frontmatter()`
- [ ] Frontmatter with colon-in-values (URLs) parsed correctly (colon bug fixed)
- [ ] export-index MCP tool output includes frontmatter and properties fields
- [ ] 7+ new tests passing covering: storage, aliases, properties, no-frontmatter, colon-in-value, coexistence, export
- [ ] All existing tests still passing (no regressions)
- [ ] Pre-commit hooks passing (fmt, clippy, audit, gitleaks)
- [ ] cargo clippy --workspace --all-targets clean
