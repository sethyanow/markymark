---
name: markdown-check
description: Validate markdown quality in the current workspace using markymark diagnostics
---

# markdown-check

Validate markdown files in your workspace for common issues using markymark's built-in diagnostics engine.

## When to Use

Use this skill when:
- You want to check for broken wiki links or markdown links
- You need to find duplicate heading slugs that would cause anchor conflicts
- You want to validate XML tag syntax (unclosed tags, malformed attributes)
- You're preparing documentation for commit and want to ensure quality
- You need a quick health check of all markdown files in a workspace

## What It Does

This skill runs markymark's diagnostic engine across your workspace markdown files and reports:

1. **Broken Links**
   - Wiki links to non-existent pages: `[[MissingPage]]`
   - Markdown links to non-existent headings: `[text](#missing-anchor)`
   - Cross-file wiki links with invalid targets: `[[File#BadHeading]]`

2. **Duplicate Headings**
   - Multiple headings that would generate the same slug
   - Example: `## Details` and `## details!` both create `#details`

3. **XML Tag Issues**
   - Unclosed XML tags: `<agent>` without `</agent>`
   - Malformed tag syntax
   - Attribute parsing errors

## How to Use

Simply invoke the skill from Claude Code:

```
/markdown-check
```

The skill will:
1. Scan all `**/*.md` and `**/*.mdx` files in the workspace
2. Run markymark diagnostics on each file
3. Report issues grouped by file with line numbers
4. Provide a summary of total issues found

## Output Format

```
📝 Markdown Quality Report

issues.md:
  Line 15: Broken wiki link [[MissingPage]]
  Line 23: Duplicate heading slug: #overview

README.md:
  Line 42: Broken markdown link [text](#missing-section)
  Line 67: Unclosed XML tag <example>

✅ Summary: 4 issues found in 2 files
```

## Implementation

This skill is a thin wrapper around markymark's LSP diagnostics:

1. Use markymark MCP tools to get diagnostics for each file
2. Filter for error-level diagnostics (not warnings)
3. Format and present results

## Configuration

No configuration required - uses workspace root detection from markymark.

## Related

- markymark LSP server (provides real-time diagnostics in editors)
- markymark MCP server (provides tools for markdown intelligence)
