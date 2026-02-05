# get_diagnostics

Get diagnostics (broken links, issues) for a file or workspace.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | No | Specific file path to check |
| `workspace_path` | string | No | Workspace root path to check all files |

At least one parameter should be provided.

## Returns

```typescript
{
  diagnostics: Array<{
    uri: string;          // File with the issue
    diagnostics: Array<{
      range: {...};       // Location of the issue
      message: string;    // Description of the problem
      severity: number;   // 1=Error, 2=Warning, 3=Info, 4=Hint
      source: string;     // "marksman"
    }>;
  }>;
}
```

## Example

**Call:**
```
get_diagnostics(workspace_path: "/path/to/docs")
```

**Result:**
```json
{
  "diagnostics": [
    {
      "uri": "file:///docs/index.md",
      "diagnostics": [
        {
          "range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 25}},
          "message": "Link target not found: deleted-page.md",
          "severity": 1,
          "source": "marksman"
        }
      ]
    },
    {
      "uri": "file:///docs/api.md",
      "diagnostics": [
        {
          "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 12}},
          "message": "Duplicate heading: # Overview",
          "severity": 2,
          "source": "marksman"
        }
      ]
    }
  ]
}
```

## Diagnostic Types

### Broken Links (Severity: Error)

- Link target file doesn't exist
- Heading anchor doesn't exist in target file
- Invalid link syntax

### Duplicate Headings (Severity: Warning)

- Same heading text appears multiple times
- Can cause ambiguous anchor links

### Ambiguous References (Severity: Info)

- Link could resolve to multiple targets
- May need to add file path for clarity

## Use Cases

### Pre-Commit Validation

Check for broken links before committing documentation changes:

```
get_diagnostics(workspace_path: "/path/to/docs")
```

### Single File Check

Validate a specific file during editing:

```
get_diagnostics(file_path: "/path/to/docs/api.md")
```

### Documentation Health Dashboard

Periodically check entire documentation for issues.

### CI/CD Integration

Fail builds when documentation has broken links.

## Severity Levels

| Level | Value | Description |
|-------|-------|-------------|
| Error | 1 | Broken link, invalid reference |
| Warning | 2 | Duplicate heading, ambiguous link |
| Information | 3 | Style suggestions |
| Hint | 4 | Minor recommendations |

## Implementation Details

This tool uses the `textDocument/diagnostic` LSP method when given a file path. For workspace-wide diagnostics, it triggers a refresh and collects diagnostics from all files.

## Related

- [[find_references]] - Check what links TO a document
- [[goto_definition]] - Test individual link resolution
