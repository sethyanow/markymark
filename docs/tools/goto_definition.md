# goto_definition

Jump to the target of a link at the given position.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the markdown file |
| `line` | number | Yes | Line number (1-based) |
| `character` | number | Yes | Character position (1-based) |

## Returns

```typescript
{
  locations: Array<{
    uri: string;      // File URI (file:///path/to/file.md)
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  }>;
}
```

## Supported Link Types

### Wiki-Links (Obsidian/Logseq style)

```markdown
See [[other-document]]
See [[folder/nested-doc]]
See [[document#heading]]
See [[document#heading|display text]]
```

### Standard Markdown Links

```markdown
See [link text](./other.md)
See [link text](./other.md#heading)
See [link text](#local-heading)
```

### Heading References

```markdown
See [Installation](#installation)
```

## Example

**Document (index.md):**
```markdown
# Index

See [[getting-started]] for setup instructions.
     ^--- cursor here (line 3, char 6)
```

**Call:**
```
goto_definition(file_path: "/docs/index.md", line: 3, character: 6)
```

**Result:**
```json
{
  "locations": [
    {
      "uri": "file:///docs/getting-started.md",
      "range": {
        "start": {"line": 0, "character": 0},
        "end": {"line": 0, "character": 0}
      }
    }
  ]
}
```

## Use Cases

### Navigate Knowledge Bases

Follow links through an Obsidian vault or documentation site.

### Verify Link Targets

Check that a link points to the expected document/heading.

### Understand Document Relationships

Trace the path through interconnected documents.

## Edge Cases

- **Broken link**: Returns empty `locations` array
- **Multiple definitions**: Returns all matching locations
- **Ambiguous reference**: Returns best match based on Marksman's resolution

## Implementation Details

This tool uses the `textDocument/definition` LSP method. Position is converted from 1-based (user-friendly) to 0-based (LSP protocol).

## Related

- [[find_references]] - Find documents linking TO a location
- [[get_hover_info]] - Preview link target without jumping
