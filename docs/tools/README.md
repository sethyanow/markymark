# MCP Tools

markymark provides 8 MCP tools for working with markdown files. These tools leverage the markymark LSP to provide intelligent navigation and refactoring.

## Quick Reference

| Tool | Purpose | Key Params |
|------|---------|------------|
| [[get_document_outline]] | Get heading hierarchy | `file_path` |
| [[goto_definition]] | Follow links | `file_path`, `line`, `character` |
| [[find_references]] | Find backlinks | `file_path`, `line`, `character` |
| [[list_workspace_symbols]] | Search symbols | `query` |
| [[get_diagnostics]] | Find broken links | `file_path` or `workspace_path` |
| [[get_hover_info]] | Preview linked content | `file_path`, `line`, `character` |
| [[rename_symbol]] | Rename headings | `file_path`, `line`, `character`, `new_name` |
| [[lsp_request]] | Raw LSP access | `method`, `params` |

## Common Use Cases

### Understanding Document Structure

```
get_document_outline(file_path: "/path/to/doc.md")
```

Returns the heading hierarchy for quick navigation.

### Following Links

```
goto_definition(file_path: "/path/to/doc.md", line: 10, character: 15)
```

Jump to the target of a wiki-link or markdown link.

### Finding Backlinks

```
find_references(file_path: "/path/to/doc.md", line: 1, character: 3)
```

Position cursor on a heading to find all documents linking to it.

### Searching Across Documents

```
list_workspace_symbols(query: "API")
```

Find all headings matching a query across the workspace.

### Validating Links

```
get_diagnostics(workspace_path: "/path/to/vault")
```

Find broken links and duplicate headings.

### Safe Refactoring

```
rename_symbol(file_path: "/path/to/doc.md", line: 1, character: 3, new_name: "New Title")
```

Rename a heading and automatically update all references.

## Position Parameters

Several tools require position parameters (`line` and `character`). These are:

- **1-based**: Line 1 is the first line, character 1 is the first character
- **Cursor position**: Place cursor on the symbol of interest

For links, position the cursor anywhere within the link text:
```markdown
See [[other-doc#heading]]
        ^--- position here
```

## Return Values

All tools return JSON results. Common patterns:

```json
// get_document_outline
{
  "headings": [
    {"level": 1, "text": "Title", "line": 0, "children": [...]}
  ]
}

// goto_definition
{
  "locations": [
    {"uri": "file:///path/to/target.md", "range": {...}}
  ]
}

// find_references
{
  "references": [
    {"uri": "file:///path/to/source.md", "range": {...}}
  ]
}
```

## Tool Documentation

- [[get_document_outline]] - Document structure and heading hierarchy
- [[goto_definition]] - Link navigation and definition lookup
- [[find_references]] - Backlink discovery and reference finding
- [[list_workspace_symbols]] - Workspace-wide symbol search
- [[get_diagnostics]] - Link validation and problem detection
- [[get_hover_info]] - Content preview for links
- [[rename_symbol]] - Safe heading refactoring
- [[lsp_request]] - Direct LSP access for advanced use
