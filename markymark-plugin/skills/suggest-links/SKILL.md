---
name: suggest-links
description: >-
  Suggests concrete cross-links to improve documentation connectivity by analyzing
  orphan documents, low-connectivity pages, and graph topology. Use when improving
  cross-referencing, reducing orphans, or strengthening documentation structure.
  Do not use for auditing overall health — use doc-audit instead.
---

# suggest-links

Generate actionable link suggestions for a markymark realm. Produces concrete wiki-link syntax showing exactly which documents to link and where.

## When to Use

- Improving cross-referencing after writing new documentation
- Reducing orphan documents (no links in or out)
- Strengthening connectivity in a sparse documentation set
- Finding documents that should reference each other
- Following up on a doc-audit that flagged orphans or low connectivity

## When NOT to Use

- Running a full documentation health audit (use `doc-audit` skill)
- Searching documentation content (use `search-workspace` MCP tool)
- Checking single-file diagnostics (use `get-diagnostics` MCP tool)
- Generating a docs_index block (use `export-docs-index` skill)

## Prerequisites

- A markymark MCP server must be running with the target realm indexed
- The realm must have at least one root added via `add-root`

## Workflow

### Step 1: Gather curation data

Call the curation-diagnostics MCP tool with suggestions enabled:

```
curation-diagnostics { "realm": "<realm-name>", "include_suggestions": true, "max_suggestions": 20 }
```

### Step 2: Format suggestions

Pipe the result to the formatting script:

```bash
echo '<curation-diagnostics-result>' \
  | bash "${CLAUDE_PLUGIN_ROOT}/skills/suggest-links/scripts/format-suggestions.sh"
```

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--max-items` | 20 | Maximum items per section before truncation |

### Step 3: Present and augment

The formatted report shows concrete wiki-link syntax. Present it to the user, then optionally augment with topical analysis:

- Use `search-symbols` to find headings that share terminology with orphan documents
- Use `search-workspace` to identify topically related documents not yet cross-linked
- Suggest specific `[[wiki-link]]` additions based on shared topics

Focus augmentation on the highest-impact opportunities: orphan documents first, then low-connectivity pages.

## Output Sections

| Section | Content |
|---------|---------|
| Header | Realm name, document count, orphan count and percentage |
| Reduce Orphans | Concrete `[[wiki-link]]` suggestions to connect orphan documents to hubs |
| Improve Cross-Linking | Additional cross-link suggestions for better connectivity |
| Low Connectivity | Documents below median connectivity that need more links |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (suggestions found or none needed) |
| 1 | Invalid input (malformed JSON, empty stdin, tool error) |

## Behavior Notes

- Empty realms (0 documents) produce "nothing to suggest" message
- Single-document realms note that no cross-links are possible
- Well-connected realms (0 orphans, 0 low-connectivity) report "no suggestions needed"
- Orphans without available hub targets are listed with a manual linking tip
- Suggestions don't check existing links — users should verify before adding duplicates
- File URIs are shortened to paths (no `file://` prefix in output)

## Related

- `doc-audit` skill — comprehensive health report (broader scope)
- `curation-diagnostics` MCP tool — raw data source for this skill
- `graph-analysis` MCP tool — graph topology without suggestions
