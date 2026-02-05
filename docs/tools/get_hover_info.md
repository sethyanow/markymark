# get_hover_info

Get hover information (preview) for a link.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the markdown file |
| `line` | number | Yes | Line number (1-based) |
| `character` | number | Yes | Character position (1-based) |

## Returns

```typescript
{
  contents: {
    kind: string;     // "markdown" or "plaintext"
    value: string;    // Preview content
  };
  range?: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
}
```

## Example

**Source document:**
```markdown
For setup instructions, see [[getting-started]].
                            ^--- cursor here
```

**Target document (getting-started.md):**
```markdown
# Getting Started

This guide walks you through initial setup.

## Prerequisites
...
```

**Call:**
```
get_hover_info(file_path: "/docs/index.md", line: 1, character: 30)
```

**Result:**
```json
{
  "contents": {
    "kind": "markdown",
    "value": "# Getting Started\n\nThis guide walks you through initial setup.\n\n## Prerequisites\n..."
  },
  "range": {
    "start": {"line": 0, "character": 28},
    "end": {"line": 0, "character": 46}}
  }
}
```

## Use Cases

### Preview Without Navigating

Quickly see what a link points to without leaving the current document.

### Verify Link Content

Check that a link goes to the expected content before clicking through.

### Documentation Review

When reviewing markdown, hover to understand context without disrupting flow.

### Heading Previews

Hover over `#heading` references to see the heading and surrounding content.

## What Gets Previewed

- **Document links**: First few paragraphs of target document
- **Heading links**: The heading and content immediately following
- **Image links**: May show image metadata
- **Broken links**: Returns null contents

## Position Guidelines

Position cursor anywhere within the link:

```markdown
See [[document-name#heading]]
     ^------ anywhere here ------^
```

For markdown links:
```markdown
See [link text](./path/to/doc.md)
     ^---- on text or path ----^
```

## Implementation Details

This tool uses the `textDocument/hover` LSP method. Marksman returns a preview of the linked content, typically the first few hundred characters.

## Limitations

- Preview length is determined by Marksman (typically first ~500 chars)
- Binary files (images) may show metadata only
- Broken links return empty hover

## Related

- [[goto_definition]] - Actually navigate to the link target
- [[find_references]] - Find documents linking to content
