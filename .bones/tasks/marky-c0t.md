---
id: marky-c0t
title: 'Refactor bumpalo.md: split into submodules (649 lines, over 500 limit)'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal
Split bumpalo.md (636 lines) into focused submodules under 500 lines each.

## Current Structure
- Lines 1-22: Header + TL;DR + Checklist
- Lines 24-71: Setup (Cargo.toml, Basic Usage)  
- Lines 74-337: Patterns (Per-Document, Per-Realm, Collections, etc.)
- Lines 340-468: Real-World Patterns (Self-Referential, Arena Transfer, Hybrid, ArenaHashMap, Send)
- Lines 472-627: Pitfalls (6 pitfalls)
- Lines 630-637: Related

## Split Strategy
1. **bumpalo.md** (main, ~300-350 lines): Header, Setup, Basic Patterns, Related
   - Keep introductory content, basic usage, common patterns
   - Add links to advanced.md and pitfalls.md

2. **bumpalo/advanced.md** (~150 lines): Real-World Patterns
   - Self-Referential Arena Ownership
   - Arena Transfer (ptr::read + mem::forget)
   - Hybrid Ownership Model
   - hashbrown with Arena Allocator (ArenaHashMap)
   - Send Constraint

3. **bumpalo/pitfalls.md** (~150 lines): All Pitfalls
   - Lifetimes Must Match Arena
   - No Individual Deallocation
   - Drop Not Called
   - Thread Safety
   - Arena Capacity Growth

## Refactoring Steps (change → test → commit cycle)
1. Create docs/rust_crates/bumpalo/ directory
2. Extract pitfalls to bumpalo/pitfalls.md → commit
3. Extract advanced patterns to bumpalo/advanced.md → commit  
4. Update bumpalo.md with links to submodules → commit
5. Update AGENTS.md index to include submodules → commit

## Success Criteria
- All 3 files < 500 lines
- Links between files work
- AGENTS.md index updated
- No content lost
- Each commit passes validation
