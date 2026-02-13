# list_workspace_symbols

Search for symbols across the workspace.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `workspace_path` | string | No | Workspace root path |

## Returns

```typescript
{
  symbols: Array<{
    name: string;           // Symbol name (e.g., "# Installation")
    kind: string;           // Symbol kind (typically "String" for headings)
    location: {
      uri: string;          // File URI
      range: {...};         // Position in file
    };
  }>;
}
```

## Example

**Call:**
```
list_workspace_symbols(query: "install")
```

**Result:**
```json
{
  "symbols": [
    {
      "name": "# Installation",
      "kind": "String",
      "location": {
        "uri": "file:///docs/getting-started.md",
        "range": {"start": {"line": 10, "character": 0}, ...}
      }
    },
    {
      "name": "## Installing Dependencies",
      "kind": "String",
      "location": {
        "uri": "file:///docs/development.md",
        "range": {"start": {"line": 5, "character": 0}, ...}
      }
    },
    {
      "name": "### Install on macOS",
      "kind": "String",
      "location": {
        "uri": "file:///docs/getting-started.md",
        "range": {"start": {"line": 15, "character": 0}, ...}
      }
    }
  ]
}
```

## Use Cases

### Search Documentation

Find all headings matching a topic across all markdown files:

```
list_workspace_symbols(query: "authentication")
```

### Jump to Specific Section

When you know roughly what you're looking for:

```
list_workspace_symbols(query: "API rate limit")
```

### Audit Documentation Coverage

Search for expected topics to verify documentation completeness:

```
list_workspace_symbols(query: "error handling")
```

### Navigate Large Vaults

In Obsidian-style vaults with hundreds of files, quickly locate content.

## Search Behavior

- **Fuzzy matching**: Partial matches are included
- **Case insensitive**: "API" matches "api" and "Api"
- **Heading text**: Searches within heading content
- **Ranked results**: More relevant matches appear first

## Workspace Detection

If `workspace_path` is not provided, markymark uses:
1. The Git repository root
2. The directory of the opened file

## Implementation Details

This tool uses the `workspace/symbol` LSP method. The query is passed directly to markymark's fuzzy matcher.

## Related

- [[get_document_outline]] - Get headings for a single document
- [[get_diagnostics]] - Validate links across workspace
