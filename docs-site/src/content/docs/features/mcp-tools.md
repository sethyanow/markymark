---
title: MCP Tools Reference
description: Complete reference for all Model Context Protocol tools with parameters and examples
---

markymark exposes 15 tools through the Model Context Protocol (MCP), giving AI agents
and scripts programmatic access to workspace intelligence that complements the
real-time [LSP features](/features/lsp/). Agents with both protocols available get
the best results by combining them — see [Using with AI Agents](/guides/agents/)
for workflow examples. Every tool accepts a `realm` parameter where noted, which
defaults to `"default"` if omitted.

## Document inspection

### get-outline

Returns the heading hierarchy for a single document.

| Param | Required | Description |
|-------|----------|-------------|
| `uri` | yes | Document URI (`file://...`) |
| `realm` | no | Realm to query |

### export-index

Returns all indexed symbols for a document: headings, XML tags, wiki links, and
Markdown links.

| Param | Required | Description |
|-------|----------|-------------|
| `uri` | yes | Document URI |
| `realm` | no | Realm to query |

## Search

### search-symbols

Fuzzy-match headings, tags, and code references by name across the workspace.

| Param | Required | Description |
|-------|----------|-------------|
| `query` | yes | Text to match against symbol names |
| `realm` | no | Realm to query |

### semantic-search

Rank document sections by relevance to a natural-language query using vector
embeddings. Requires the `semantic-search` feature flag (Voyage API) or
`local-embeddings` (offline ONNX via fastembed). See
[Installation](/getting-started/installation/) for build flags.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `query` | yes | — | Natural-language query |
| `realm` | no | `"default"` | Realm to query |
| `top_k` | no | `10` | Max results to return |
| `min_score` | no | `0.5` | Minimum relevance threshold |

### search-workspace

Search documents by free text, frontmatter, Logseq properties, or tags.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `query` | no | — | Free-text substring search |
| `frontmatter_filter_key` | no | — | Frontmatter key to filter on |
| `frontmatter_filter_value` | no | — | Frontmatter value (case-insensitive) |
| `property_filter_key` | no | — | Logseq property key |
| `property_filter_value` | no | — | Logseq property value |
| `tag_filter` | no | — | Include only documents with this tag |
| `realm` | no | `"default"` | Realm to search |
| `limit` | no | `20` | Max results (0-100) |

### search-for-pattern

Regex search across workspace file content with context lines.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `pattern` | yes | — | Regex pattern |
| `include_glob` | no | — | Glob filter (e.g., `"*.md"`) |
| `context_lines` | no | `2` | Lines of context (0-20) |
| `limit` | no | `100` | Max matches (1-500) |
| `case_insensitive` | no | `false` | Case-insensitive matching |
| `realm` | no | `"default"` | Realm to search |

## Navigation

### find-references

Find all references to a heading or XML tag at a given position.

| Param | Required | Description |
|-------|----------|-------------|
| `uri` | yes | Document URI containing the symbol |
| `line` | yes | 0-based line number |
| `character` | yes | 0-based character offset |
| `realm` | no | Realm to query |

### rename

Rename a heading or XML tag and update all references across the workspace.

| Param | Required | Description |
|-------|----------|-------------|
| `uri` | yes | Document URI containing the symbol |
| `line` | yes | 0-based line number |
| `character` | yes | 0-based character offset |
| `new_name` | yes | New name for the symbol |
| `realm` | no | Realm to query |

## Diagnostics

### get-diagnostics

Returns diagnostics (broken links, duplicate headings, unclosed XML tags) for a
file or all files in a realm.

| Param | Required | Description |
|-------|----------|-------------|
| `uri` | no | Document URI (omit to check all files) |
| `realm` | no | Realm to query |

### graph-analysis

Analyze the link graph of a workspace. Returns orphan documents, hub documents,
broken links, and summary statistics.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `realm` | no | `"default"` | Realm to analyze |
| `top_n_hubs` | no | `10` | Number of top hub documents |
| `include_clusters` | no | `false` | Compute connected clusters |

## Workspace management

### create-realm

Create a named realm for isolated workspace indexing.

| Param | Required | Description |
|-------|----------|-------------|
| `name` | yes | Unique realm name |

### destroy-realm

Delete a realm and unindex all its documents.

| Param | Required | Description |
|-------|----------|-------------|
| `name` | yes | Realm to destroy |

### add-root

Add a workspace root directory to a realm, indexing all files within it.

| Param | Required | Description |
|-------|----------|-------------|
| `realm` | yes | Realm to add the root to |
| `root` | yes | Filesystem path of the directory |

### remove-root

Remove a workspace root from a realm and unindex its documents.

| Param | Required | Description |
|-------|----------|-------------|
| `realm` | yes | Realm to remove the root from |
| `root` | yes | Filesystem path of the directory |

### realm-stats

Get aggregate statistics for a realm: document, heading, tag, and link counts.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `realm` | yes | — | Realm name |
| `check_duplicates` | no | `false` | Include duplicate document pair count |
| `include_token_counts` | no | `false` | Include aggregate token estimation |
