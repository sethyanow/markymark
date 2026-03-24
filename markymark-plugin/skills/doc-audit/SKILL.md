---
name: doc-audit
description: >-
  Produces an actionable documentation quality report by composing graph-analysis,
  get-diagnostics, and curation-diagnostics MCP tools. Use when assessing documentation
  health, finding orphans, broken links, or connectivity gaps. Do not use for
  searching content — use search-workspace or recommend-docs instead.
---

# doc-audit

Run a comprehensive documentation quality audit on a markymark realm. The audit composes three MCP tools into a single formatted report with actionable findings.

## When to Use

- Assessing documentation health before a release or review
- Finding orphan documents with no cross-references
- Detecting broken links (wiki `[[…]]` and markdown `[…](…)`)
- Identifying connectivity gaps and poorly-linked documents
- Getting suggested cross-links to improve documentation structure
- Triaging documentation quality across a large corpus

## When NOT to Use

- Searching documentation content (use `search-workspace` MCP tool)
- Getting document recommendations for a query (use `recommend-docs` skill)
- Exporting a docs_index block (use `export-docs-index` skill)
- Checking a single file for diagnostics (use `get-diagnostics` MCP tool directly)

## Prerequisites

- A markymark MCP server must be running with the target realm indexed
- The realm must have at least one root added via `add-root`

## Workflow

### Step 1: Gather data from three MCP tools

Call each tool targeting the same realm. All three calls are independent and can run in parallel:

**graph-analysis** — returns orphans, hubs, broken links, and graph statistics:
```
graph-analysis { "realm": "<realm-name>" }
```

**get-diagnostics** — returns per-file diagnostics (broken links, duplicate headings, unclosed tags):
```
get-diagnostics { "realm": "<realm-name>" }
```

**curation-diagnostics** — returns orphan docs, low-connectivity docs, and cross-link suggestions:
```
curation-diagnostics { "realm": "<realm-name>", "include_suggestions": true }
```

### Step 2: Format the report

Combine results into a single JSON object and pipe to the formatting script:

```bash
echo '{"graph_analysis": <result1>, "diagnostics": <result2>, "curation": <result3>}' \
  | bash "${CLAUDE_PLUGIN_ROOT}/skills/doc-audit/scripts/format-report.sh"
```

If a tool call fails, include the error response as-is. The script detects `{"error": ...}` envelopes and produces a partial report with a warning banner.

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--max-items` | 20 | Maximum items per report section before truncation |

### Step 3: Present findings and suggest actions

The report includes actionable guidance per section. Prioritize findings by severity:

1. **Broken links** — fix or remove dead references
2. **File diagnostics** (errors) — resolve structural issues
3. **Orphan documents** — add cross-references or remove if obsolete
4. **Low connectivity** — improve linking to isolated documents
5. **Suggested links** — consider adding recommended cross-references

For large corpora, focus on the highest-impact items first. Use `--max-items` to control report verbosity.

## Report Sections

| Section | Source Tool | Content |
|---------|-----------|---------|
| Summary table | graph-analysis + curation | Doc count, links, orphans, broken links, connectivity |
| Orphan Documents | graph-analysis | Docs with zero incoming and outgoing links |
| Broken Links | graph-analysis | Unresolvable wiki and markdown link targets |
| Hub Documents | graph-analysis | Most-linked documents (incoming count) |
| Low Connectivity | curation-diagnostics | Docs below median connectivity threshold |
| Suggested Links | curation-diagnostics | Actionable cross-link recommendations |
| File Diagnostics | get-diagnostics | Per-file warnings and errors with line numbers |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tools succeeded |
| 1 | Invalid input (malformed JSON, empty stdin, all tools failed) |
| 2 | Partial success (some tools failed, report generated with warnings) |

## Example Output

```
# Documentation Audit Report

**Realm:** my-docs | **Documents:** 25 | **Internal links:** 48

## Summary

| Metric | Count |
|--------|-------|
| Documents | 25 |
| Orphan documents | 3 |
| Broken links | 2 |
| Avg connectivity | 3.8 |

## Orphan Documents

- /docs/stale-guide.md
- /docs/unused-reference.md
- /docs/draft-spec.md

**Action:** Add links to or from these documents, or remove them if obsolete.
```

## Behavior Notes

- Empty realms (0 documents) produce a short "nothing to audit" message, not an error
- Single-document corpora include a note that orphan status is expected
- File URIs (`file:///path`) are shortened to `/path` in the report
- Partial tool failures produce a report with a warning banner and exit code 2
- Each section is independently populated — a failed tool only omits its sections
