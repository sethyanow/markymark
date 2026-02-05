# find_references

Find all references to a symbol at the given position.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the markdown file |
| `line` | number | Yes | Line number (1-based) |
| `character` | number | Yes | Character position (1-based) |

## Returns

```typescript
{
  references: Array<{
    uri: string;      // File URI containing the reference
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  }>;
}
```

## Example

**Document (api.md):**
```markdown
# API Reference
^--- cursor here (line 1, char 3)

## Endpoints
...
```

**Call:**
```
find_references(file_path: "/docs/api.md", line: 1, character: 3)
```

**Result:**
```json
{
  "references": [
    {
      "uri": "file:///docs/index.md",
      "range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 20}}
    },
    {
      "uri": "file:///docs/tutorial.md",
      "range": {"start": {"line": 12, "character": 8}, "end": {"line": 12, "character": 24}}
    }
  ]
}
```

## Use Cases

### Find Backlinks

Position cursor on a heading to discover all documents linking to it. Essential for knowledge management.

### Impact Analysis

Before renaming or deleting content, find all references that would break.

### Understand Document Importance

Documents with many incoming references are likely important hubs.

### Audit Link Coverage

Find orphaned documents that have no incoming links.

## What Can Be Referenced

- **Headings**: Any `#` heading in the document
- **Documents**: The document itself (references to the file)
- **Heading anchors**: `#specific-heading` references

## Position Guidelines

For best results, position cursor:
- On the `#` of a heading to find references to that heading
- On the first character of heading text
- On the filename portion of a link

## Implementation Details

This tool uses the `textDocument/references` LSP method with `includeDeclaration: true`. Returns all locations that reference the symbol at the cursor position.

## Related

- [[goto_definition]] - Navigate FROM a reference to its target
- [[rename_symbol]] - Rename and update all references automatically
