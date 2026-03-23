---
id: marky-6qri
title: String interning for RealmIndex cross-doc indexes
status: closed
type: feature
priority: 4
owner: sethyanow@users.noreply.github.com
depends_on: [marky-io3h]
---


## Problem

`RealmIndex::add_document` at `markymark-index/src/realm/mod.rs:78-116` calls `.to_string()` on every heading text, slug, tag name, and block ID when populating cross-document indexes. For large vaults (10k+ files, 200k+ headings), this produces hundreds of thousands of fragmented heap allocations.

```rust
let resolved = ResolvedHeading {
    text: entry.text.to_string(),   // heap alloc
    slug: entry.slug.to_string(),   // heap alloc
    ...
};
self.slug_to_headings
    .entry(entry.slug.to_string())  // another heap alloc for the same slug
    ...
```

## Impact

- Memory fragmentation at workspace scale
- Not a hot-path latency issue (runs at workspace-scan frequency, not keystroke frequency)
- Correctness is fine — this is purely a memory/allocation optimization

## Fix Direction

Options (evaluate during implementation):
1. `lasso` or `ustr` string interner — global dedup, O(1) lookup
2. `Arc<str>` shared ownership — dedup within same document's entries
3. Realm-level `Bump` arena — bulk allocation, freed on realm rebuild

The Document Engine blob format (marky-io3h) creates a contiguous text pool per document. A future optimization could have `from_blob()` produce interned or `Arc<str>` slices directly, avoiding double-allocation entirely.

## Parent

Follow-up from epic marky-io3h (Document Engine). The engine addresses per-document pipeline costs; this addresses workspace-level aggregation costs.

## Discovery

Identified via external code review (Gemini 3.1 Pro). Valid observation, misprioritized as critical.

## Files

- `markymark-index/src/realm/mod.rs:72-135`
