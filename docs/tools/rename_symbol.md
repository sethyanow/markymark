# rename_symbol

Rename a heading and update all references.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the markdown file |
| `line` | number | Yes | Line number (1-based) |
| `character` | number | Yes | Character position (1-based) |
| `new_name` | string | Yes | New name for the symbol |

## Returns

```typescript
{
  changes: {
    [uri: string]: Array<{
      range: {
        start: { line: number; character: number };
        end: { line: number; character: number };
      };
      newText: string;
    }>;
  };
}
```

## Example

**Before rename:**

`api.md`:
```markdown
# API Reference
^--- cursor here (line 1, char 3)

## Endpoints
```

`index.md`:
```markdown
See [[api#API Reference]] for details.
```

**Call:**
```
rename_symbol(
  file_path: "/docs/api.md",
  line: 1,
  character: 3,
  new_name: "API Documentation"
)
```

**Result:**
```json
{
  "changes": {
    "file:///docs/api.md": [
      {
        "range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 15}},
        "newText": "API Documentation"
      }
    ],
    "file:///docs/index.md": [
      {
        "range": {"start": {"line": 0, "character": 10}, "end": {"line": 0, "character": 23}},
        "newText": "API Documentation"
      }
    ]
  }
}
```

**After rename:**

`api.md`:
```markdown
# API Documentation

## Endpoints
```

`index.md`:
```markdown
See [[api#API Documentation]] for details.
```

## Use Cases

### Refactor Documentation

Rename headings while keeping all links working:

```
rename_symbol(file_path: "/docs/guide.md", line: 10, character: 3, new_name: "Quick Start Guide")
```

### Fix Typos

Correct heading typos and automatically fix references.

### Standardize Terminology

Change terminology across documentation (e.g., "Setup" → "Installation").

### Restructure Documents

When reorganizing, rename headings to reflect new structure.

## What Can Be Renamed

- **Headings**: Any `#` through `######` heading
- **Documents**: Position on filename in a link (renames file)

## Safety Features

- **Preview mode**: The tool returns changes but doesn't apply them
- **All references updated**: Every link pointing to the old name is updated
- **Scope awareness**: Only references in the workspace are affected

## Position Guidelines

Position cursor on the heading to rename:

```markdown
# My Heading Title
^--- here (the # character)
```

Or on the heading text:
```markdown
# My Heading Title
  ^--- anywhere in the text
```

## Important Notes

1. **Changes are not automatically applied** - you receive the list of changes to review
2. **Backup recommended** - for large renames across many files
3. **Git integration** - commit before renaming for easy rollback

## Implementation Details

This tool uses the `textDocument/rename` LSP method. The `prepareRename` request validates the rename is possible before returning changes.

## Related

- [[find_references]] - Preview what will be affected
- [[get_diagnostics]] - Verify no broken links after rename
