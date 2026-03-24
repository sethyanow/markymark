---
name: recommend-docs
description: >-
  Recommends the most relevant documents for a topic by combining text search
  scoring with graph hub analysis. Use when selecting which documentation to
  read or retrieve for a given question. Do not use for raw keyword search —
  use search-workspace instead.
---

# recommend-docs

Find the most relevant documents in a markymark realm for a given topic. Returns ranked results combining text search relevance (70%) with graph hub importance (30%), optionally enriched with section summaries.

## When to Use

- Deciding which documentation files to read for a given topic or question
- Populating agent context with the most relevant background material
- Finding authoritative documents that are both topically relevant and well-connected
- Selecting reference material when answering questions about a documentation corpus
- Narrowing a large corpus to the most useful subset for a specific task

## When NOT to Use

- Searching for specific keywords or text patterns (use `search-workspace` MCP tool)
- Searching by regex pattern across files (use `search-for-pattern` MCP tool)
- Improving cross-referencing or reducing orphan documents (use `suggest-links` skill)
- Auditing documentation health and quality (use `doc-audit` skill)
- Generating a docs_index block for CLAUDE.md (use `export-docs-index` skill)

## Prerequisites

- A markymark MCP server must be running with the target realm indexed
- The realm must have at least one root added via `add-root`

## Workflow

### Step 1: Call recommend-docs

Call the recommend-docs MCP tool with a descriptive query:

```
recommend-docs { "query": "<topic or question>", "realm": "<realm-name>" }
```

**Parameters:**

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `query` | Yes | — | Topic, question, or keywords to find relevant docs for |
| `realm` | No | `"default"` | Realm to search |
| `top_k` | No | 5 | Number of results to return (1–20) |
| `include_sections` | No | false | Include per-section summaries from enrichment sidecars |

### Step 2: Interpret results

The tool returns a ranked array of document recommendations. Each result contains:

| Field | Type | Description |
|-------|------|-------------|
| `uri` | string | Document URI (`file://...`) |
| `title` | string | First H1 heading or derived from filename |
| `relevance_score` | float | Combined score: 0.7 × search + 0.3 × hub (0.0–1.0) |
| `search_score` | float | Text search relevance (0.0–1.0) |
| `hub_score` | float | Normalized graph hub importance (0.0–1.0) |
| `matched_fields` | [string] | Which fields matched (e.g., `["title", "heading"]`) |
| `tags` | [string] | Document tags |
| `document_summary` | string? | From enrichment sidecar, if available |
| `sections` | [object]? | Per-section summaries, if `include_sections=true` and sidecars exist |

### Step 3: Use the results

- Read the top-ranked documents using the Read tool or LSP
- Higher `relevance_score` means both topically relevant and well-connected in the documentation graph
- Documents with high `hub_score` but low `search_score` are important reference documents that may be tangentially relevant
- Documents with high `search_score` but low `hub_score` are topically on-point but may be isolated pages

## Behavior Notes

- `query` is required — the tool returns an error if omitted or empty
- No matching documents returns an empty results array, not an error
- Non-existent realm returns an error
- Realm with no roots (no indexed documents) returns an error
- `top_k` values outside 1–20 are clamped to the valid range
- `include_sections` requires enrichment sidecars (`.markymark/` directory); sections are omitted for documents without sidecars
- Single-document realms return at most one recommendation
- Scoring formula: `relevance_score = 0.7 × search_score + 0.3 × hub_score`

## Related

- `search-workspace` MCP tool — raw text search without ranking by graph importance
- `suggest-links` skill — improves documentation connectivity (link suggestions, not retrieval)
- `doc-audit` skill — comprehensive health report (quality assessment, not retrieval)
- `export-docs-index` skill — generates docs_index blocks for agent instructions
