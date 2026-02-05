# get_document_outline

Get the heading hierarchy of a markdown document.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the markdown file |

## Returns

```typescript
{
  headings: Array<{
    level: number;    // Heading level (1-6)
    text: string;     // Heading text (without # prefix)
    line: number;     // 0-based line number
    children: [];     // Nested headings
  }>;
}
```

## Example

**Input:**
```markdown
# Getting Started

## Installation

### macOS

### Linux

## Usage
```

**Call:**
```
get_document_outline(file_path: "/path/to/README.md")
```

**Result:**
```json
{
  "headings": [
    {
      "level": 1,
      "text": "Getting Started",
      "line": 0,
      "children": [
        {
          "level": 2,
          "text": "Installation",
          "line": 2,
          "children": [
            {"level": 3, "text": "macOS", "line": 4, "children": []},
            {"level": 3, "text": "Linux", "line": 6, "children": []}
          ]
        },
        {
          "level": 2,
          "text": "Usage",
          "line": 8,
          "children": []
        }
      ]
    }
  ]
}
```

## Use Cases

### Navigate Large Documents

When working with a large markdown file, get the outline first to understand structure:

```
get_document_outline(file_path: "/path/to/long-doc.md")
```

### Verify Document Structure

After editing, verify the heading hierarchy is correct.

### Generate Table of Contents

Use the outline to programmatically generate a TOC.

## Implementation Details

This tool uses the `textDocument/documentSymbol` LSP method. Marksman returns `DocumentSymbol` objects which are converted to a simplified heading hierarchy.

## Related

- [[README]] - Tools overview
- [[lsp_request]] - Direct LSP access for more symbol details
