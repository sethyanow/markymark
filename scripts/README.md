# Scripts

Test and development scripts for markymark.

## Embedding Provider Smoke Tests

These scripts test the semantic search feature by starting an MCP server, sending
JSON-RPC requests, and verifying responses. Each test creates a small ephemeral
workspace (2 markdown files), indexes it with the target provider, and runs a
semantic search query.

### Quick Start

```bash
# Run all available embedding tests
./scripts/smoke-embeddings.sh

# Run only the local ONNX test (no credentials needed)
./scripts/smoke-embeddings.sh local

# Run only the Voyage API test (requires VOYAGE_API_KEY)
./scripts/smoke-embeddings.sh voyage
```

### Individual Tests

| Script | Provider | Credentials | First-run overhead |
|--------|----------|-------------|--------------------|
| `smoke-local.sh` | Local ONNX (all-MiniLM-L6-v2) | None | ~23MB model download |
| `smoke-voyage.sh` | Voyage AI API (voyage-4) | `VOYAGE_API_KEY` | None |

### Configuration

**Voyage API key** can be provided via:
1. `VOYAGE_API_KEY` environment variable
2. `.env` file in the repo root (`VOYAGE_API_KEY=sk-...`)

**Local ONNX model** downloads automatically on first use to `~/.cache/markymark/models/`.

### What They Test

Each smoke test validates 4 assertions:
1. MCP server initializes successfully (returns `protocolVersion`)
2. `tools/list` returns the tool catalog
3. `semantic-search` tool is present in the catalog
4. `semantic-search` query returns at least one result

### Custom Workspace

Both individual tests accept an optional workspace path:

```bash
./scripts/smoke-local.sh /path/to/markdown/workspace
./scripts/smoke-voyage.sh /path/to/markdown/workspace
```

### Feature Flags

The tests build with these Cargo features:

| Test | Features |
|------|----------|
| Local | `semantic-search`, `local-embeddings` |
| Voyage | `semantic-search`, `voyage` |

### Troubleshooting

- **Local test hangs on first run**: The model (~23MB) is downloading. Subsequent runs are fast.
- **Voyage test skipped**: Set `VOYAGE_API_KEY` in env or `.env` file.
- **Build errors**: Ensure you have the semantic search code merged into your current branch.
