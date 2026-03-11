---
id: marky-uux
title: search-symbols returns results across realm boundaries instead of scoping to caller's realm
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

## Summary

`search-symbols` returns results from **all indexed realms globally**, not scoped to the realm that was queried. Additionally, results from **destroyed realms** persist in symbol search, suggesting realm destruction doesn't clean up the global symbol index.

## Reproduction Steps

### 1. Set up two separate realms
```
# Realm A: the markymark repo
create-realm(name: 'repo-test')
add-root(realm: 'repo-test', root: '/Volumes/code/markymark')
→ { document_count: 6543 }

# Realm B: a completely separate directory (Antigravity brain)
create-realm(name: 'brain')
add-root(realm: 'brain', root: '/Users/seth/.gemini/antigravity/brain')
→ { document_count: 334 }
```

### 2. Search for symbols — results come from both realms
```
search-symbols(query: 'Verification')
```
All returned symbols have URIs under `file:///Volumes/code/markymark/` (realm A), **none** from `/Users/seth/.gemini/antigravity/brain/` (realm B). But this illustrates the issue from the other direction — if I'm working in the brain realm, I'd expect only brain results.

### 3. Destroy realm A, search again — realm A results still appear
```
destroy-realm(name: 'repo-test')
→ { success: true }

# Now only 'brain' realm exists
search-symbols(query: 'Verification')
→ Still returns results from /Volumes/code/markymark/  ❌
```
Results from the destroyed `repo-test` realm continue to appear in symbol search.

### Detailed Evidence

The 'Verification' search returned 80+ results, all from `/Volumes/code/markymark/`:
- `file:///Volumes/code/markymark/.claude/commands/flow.md` → heading 'Phase 4.3: Verification and Team Cleanup'
- `file:///Volumes/code/markymark/RELEASING.md` → heading 'Post-Release Verification'
- `file:///Volumes/code/markymark/.claude-harness/config.json` → key paths 'verification.build', 'verification.test', etc.
- All .worktrees copies of the above
- **Zero** results from the brain directory, despite 2,066 headings being indexed there

### Same behavior observed with 'Proposed Changes' query
```
search-symbols(query: 'Proposed Changes')
→ { symbols: [] }  (empty, even though implementation_plan.md files in brain contain this heading)
```
This further confirms brain realm documents aren't entering the searchable symbol index.

## Expected Behavior

1. `search-symbols` should accept a `realm` parameter, **or** it should be scoped to a default/active realm
2. `destroy-realm` should remove all symbols from the global index that belonged to that realm
3. Alternatively, if cross-realm search is intentional, it should be documented and realm origin should be indicated in results

## Impact

- **Realm isolation is defeated**: Users who create realms for separate projects will get cross-contaminated results
- **Memory leak on destroy**: Destroying realms doesn't reclaim symbol index entries
- **Silent data inconsistency**: No error is raised — results just silently include stale/foreign data

## Related Issues
- marky-kvr: `find-references` fails on structured docs
- marky-d7r: Documents indexed but not queryable in non-git directories (may share root cause — documents aren't registering in the per-document index, only in bulk stats)

## Root Cause Hypothesis

The symbol index appears to be a single global data structure shared across all realms. `add-root` inserts symbols into this global index, but:
1. There's no realm tag on individual symbol entries, so they can't be filtered by realm
2. `destroy-realm` removes the realm metadata but doesn't cascade-delete symbols from the global index

The marky-d7r bug (brain files not appearing in search either) suggests the brain directory's files may not be entering the symbol index at all — possibly due to the same git-dependency issue noted in that bug.
