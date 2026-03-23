---
id: marky-g9t
title: '[EPIC] Arena allocation: bumpalo migration for parser + index types'
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-peu]
---









Migrate markymark-parser and markymark-index types from owned String/Vec/HashMap to arena-allocated equivalents using bumpalo. Hybrid ownership model: per-document Bump for parsed content, realm-level owned storage for cross-doc index lookups. Uses hashbrown with bumpalo allocator for HashMap fields.

## Scope
- 19 parser types + 8 index types → all get 'arena lifetimes
- ~60+ String allocations per document eliminated
- ~2000+ lines across markymark-core, markymark-parser, markymark-index
- LSP/MCP crates need adapter updates for lifetime propagation

## Design Decisions
- Hybrid arena: per-document Bump (re-parse = swap arena), realm-level owned for cross-doc
- hashbrown with bumpalo allocator for HashMap fields (Frontmatter, Properties, XmlTag attrs)
- Full depth: parser AND index types in one epic
- Post-alpha: depends on marky-peu completion

## Success Criteria
- [ ] All parser types use &'arena str instead of String
- [ ] All index types borrow from document arena where possible
- [ ] RealmIndex uses owned storage for cross-doc lookups
- [ ] HashMap fields use hashbrown with bumpalo allocator
- [ ] All existing tests pass
- [ ] Memory benchmark shows measurable improvement
