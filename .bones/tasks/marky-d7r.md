---
id: marky-d7r
title: Documents indexed but not queryable in non-git directories
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Summary

When `add-root` indexes a directory that is **not inside a git repository** (e.g. `~/.gemini/antigravity/brain/`), the realm reports documents as indexed (`realm-stats` shows headings, key paths, etc.), but **no individual document can be queried** via `get-outline` or `export-index`. Every call returns `"document is not indexed"`.

This is distinct from marky-kvr (`find-references` on structured docs) — this bug affects **all file types** (markdown, JSON, TOML) in non-git directories.

## Reproduction Steps

### 1. Create a realm and index a non-git directory
```
create-realm(name: 'brain-test')
add-root(realm: 'brain-test', root: '/Users/seth/.gemini/antigravity/brain')
→ { document_count: 334, root_count: 1 }  ✅  (334 docs reported)
```

### 2. Verify realm-stats shows indexed content
```
realm-stats(realm: 'brain-test')
→ {
    document_count: 334,
    heading_count: 2066,
    key_path_count: 559,
    markdown_link_count: 443,
    structured_doc_count: 164,
    wiki_link_count: 8,
    xml_tag_count: 68
  }
```
Stats show 2,066 headings and 559 key paths exist — data IS being parsed and stored.

### 3. Attempt to query any individual document
```
get-outline(uri: 'file:///Users/seth/.gemini/antigravity/brain/a0fd96c5-729a-40cd-8052-3c783c4da3a9/implementation_plan.md')
→ Error: 'document is not indexed'  ❌

export-index(uri: 'file:///Users/seth/.gemini/antigravity/brain/a0fd96c5-729a-40cd-8052-3c783c4da3a9/implementation_plan.md')
→ Error: 'document is not indexed'  ❌

get-outline(uri: 'file:///Users/seth/.gemini/antigravity/brain/01731ea8-10bf-4c8f-b197-262c3c3be09b/implementation_plan.md')
→ Error: 'document is not indexed'  ❌

get-outline(uri: 'file:///Users/seth/.gemini/antigravity/brain/de9769b5-9283-4b03-a398-188c0772e9c1/walkthrough.md')
→ Error: 'document is not indexed'  ❌
```
All tested files exist on disk (confirmed via `find` command). Multiple .md files and .json files tested — all fail.

### 4. Contrast: same tools work on git-backed directories
```
# Same markymark binary, same session, different root
add-root(realm: 'repo-test', root: '/Volumes/code/markymark')
→ { document_count: 6543 }

get-outline(uri: 'file:///Volumes/code/markymark/Cargo.toml')
→ Returns full key-path hierarchy  ✅

get-outline(uri: 'file:///Volumes/code/markymark/lefthook.yml')
→ Returns nested YAML outline  ✅
```

## Expected Behavior

`get-outline` and `export-index` should work on any file that was indexed by `add-root`, regardless of whether the directory is inside a git repository.

## Root Cause Hypothesis

The document URI lookup (used by `get-outline` / `export-index`) may rely on a git-aware file discovery mechanism (e.g. `git ls-files` or similar) to build its document→URI mapping, while the bulk indexer (`add-root`) uses a filesystem walker that doesn't require git. This would explain why:
- `realm-stats` shows data (bulk indexer found and parsed files)
- Individual queries fail (URI lookup can't find the document without git)

Another possibility: the `.git/` directory presence triggers different code paths for URI normalization, and without it the URIs stored by the indexer don't match the URIs used for lookup.

## Environment
- macOS, markymark v0.3.0
- Brain directory: `/Users/seth/.gemini/antigravity/brain/` (103 subdirectories, no .git, no .gitignore)
- Files confirmed present via `find` command
