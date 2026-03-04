---
title: Search
description: Search symbols, headings, tags, and code references across your workspace
---

markymark provides several ways to search across your workspace — from quick symbol
lookup in your editor to structured queries via MCP tools. Each mode serves a
different use case.

## Symbol search

Find headings, tags, and inline code references by name across all files.

**In your editor:** Press Ctrl+T (Cmd+T on macOS) to open workspace symbol search.
Start typing and markymark fuzzy-matches against every heading, tag, and backtick
code reference in the index. Results update as you type.

**With MCP:**

```text
search-symbols: { "query": "authentication" }
```

Returns matching symbols with file URIs and position ranges. The match is fuzzy —
partial names and abbreviations work.

## Semantic search

Rank document sections by relevance to a natural-language query using vector
embeddings. Sections are embedded when a workspace root is added and
re-embedded incrementally when documents change.

**With MCP:**

```text
semantic-search: { "query": "how to configure SSL", "top_k": 5 }
```

Returns the top matching sections with a relevance score, heading text, and a
section preview. Use `top_k` to control how many results come back (default 10)
and `min_score` to filter low-confidence matches (default 0.5).

Semantic search is MCP-only — designed for agent workflows where ranking by
meaning matters more than exact text matching. Two embedding providers are
available:

- **Voyage API** — cloud embeddings, enabled with the `semantic-search` feature flag
  (requires `VOYAGE_API_KEY` environment variable)
- **Local ONNX** — fully offline embeddings via fastembed, enabled with the
  `local-embeddings` feature flag (downloads a ~23 MB model on first use)

## Full-text search

Search document content with metadata filters.

**With MCP:**

```text
search-workspace: { "query": "deployment", "limit": 20 }
```

This searches across document titles and content. You can narrow results with
filters:

```text
search-workspace: {
  "query": "setup",
  "tag_filter": "guide",
  "frontmatter_filter_key": "status",
  "frontmatter_filter_value": "published"
}
```

Filters match against frontmatter fields, Logseq-style properties, and tags.

## Regex pattern search

Search file content with regular expressions, including context lines.

**With MCP:**

```text
search-for-pattern: {
  "pattern": "TODO|FIXME",
  "context_lines": 2,
  "case_insensitive": true
}
```

Returns each match with surrounding context lines, file URI, line number, and
column. Use `include_glob` to restrict matches to specific file patterns
(e.g., `"*.md"`).

## Tips

- Symbol search is the fastest way to find a heading — use Ctrl+T in your editor.
- Use `search-for-pattern` for text that isn't in headings or tags (body content,
  code blocks, TODO markers).
- Combine `search-workspace` filters to find documents by metadata without
  needing to know their file paths.
