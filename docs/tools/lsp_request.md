# lsp_request

Send a raw LSP request to markymark (escape hatch for advanced use).

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | Yes | LSP method name |
| `params` | object | Yes | LSP request parameters |

## Returns

The raw response from markymark LSP. Structure depends on the method called.

## Example

**Call:**
```
lsp_request(
  method: "textDocument/documentSymbol",
  params: {
    "textDocument": {
      "uri": "file:///path/to/doc.md"
    }
  }
)
```

**Result:**
```json
[
  {
    "name": "# Introduction",
    "kind": 15,
    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 2, "character": 0}},
    "selectionRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 14}},
    "children": [...]
  }
]
```

## Supported LSP Methods

### Document Methods

| Method | Description |
|--------|-------------|
| `textDocument/documentSymbol` | Get all symbols in a document |
| `textDocument/definition` | Go to definition |
| `textDocument/references` | Find all references |
| `textDocument/hover` | Get hover information |
| `textDocument/rename` | Rename symbol |
| `textDocument/prepareRename` | Check if rename is valid |
| `textDocument/completion` | Get completion suggestions |
| `textDocument/codeAction` | Get available code actions |
| `textDocument/formatting` | Format document |

### Workspace Methods

| Method | Description |
|--------|-------------|
| `workspace/symbol` | Search symbols across workspace |
| `workspace/executeCommand` | Execute a command |

### Lifecycle Methods

| Method | Description |
|--------|-------------|
| `initialize` | Initialize LSP connection |
| `shutdown` | Request shutdown |

## Use Cases

### Access Experimental Features

Use LSP features not yet exposed as high-level tools:

```
lsp_request(method: "textDocument/completion", params: {...})
```

### Debug LSP Behavior

Understand exactly what markymark returns:

```
lsp_request(method: "textDocument/documentSymbol", params: {...})
```

### Custom Integrations

Build workflows using raw LSP capabilities.

### Future Compatibility

As markymark adds features, access them before high-level tools are added.

## URI Format

markymark expects file URIs in standard format:

```
file:///absolute/path/to/file.md     # Unix
file:///C:/Users/name/doc.md         # Windows
```

## Position Format (LSP)

LSP uses 0-based positions:
- Line 0 is the first line
- Character 0 is the first character

This differs from the high-level tools which use 1-based positions.

## Common Patterns

### Get Document Symbols

```javascript
{
  method: "textDocument/documentSymbol",
  params: {
    textDocument: { uri: "file:///path/to/doc.md" }
  }
}
```

### Find References

```javascript
{
  method: "textDocument/references",
  params: {
    textDocument: { uri: "file:///path/to/doc.md" },
    position: { line: 0, character: 2 },  // 0-based!
    context: { includeDeclaration: true }
  }
}
```

### Workspace Symbol Search

```javascript
{
  method: "workspace/symbol",
  params: {
    query: "search term"
  }
}
```

## Error Handling

LSP errors are returned as:
```json
{
  "error": {
    "code": -32601,
    "message": "Method not found"
  }
}
```

Common error codes:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

## Related

- [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/specification-current/)
- [markymark GitHub](https://github.com/sethyanow/markymark)
