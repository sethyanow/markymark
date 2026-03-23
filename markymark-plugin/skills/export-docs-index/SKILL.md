---
name: export-docs-index
description: >-
  Generates a pipe-delimited docs_index block from a workspace directory tree.
  Use when setting up agent documentation indexes, creating CLAUDE.md docs_index
  blocks, or refreshing stale indexes after doc reorganization. Do not use for
  searching or querying docs — use recommend-docs or search-workspace instead.
---

# export-docs-index

Generate a pipe-delimited `<docs_index>` block from a directory of markdown files. The output format matches the Claude Code agent convention for documentation indexes in CLAUDE.md files.

## When to Use

- Setting up a new project's documentation index for Claude Code agents
- Refreshing a stale docs_index after files were added, renamed, or reorganized
- Generating indexes for multiple documentation directories (one section per directory)
- Bootstrapping CLAUDE.md with accurate file listings instead of hand-curating

## When NOT to Use

- Searching or querying documentation content (use `search-workspace` MCP tool)
- Getting recommendations for which docs to read (use `recommend-docs` skill)
- Checking documentation quality (use `doc-audit` or `markdown-check` skills)

## Usage

Run the bundled script with a directory path and section name:

```bash
bash "${CLAUDE_PLUGIN_ROOT}/skills/export-docs-index/scripts/generate-index.sh" \
  <directory-path> <section-name> [instruction-text]
```

**Parameters:**

| Parameter | Required | Description |
|-----------|----------|-------------|
| `directory-path` | Yes | Path to the documentation directory to index |
| `section-name` | Yes | Name for the index section (e.g., `my-docs`, `api-reference`) |
| `instruction-text` | No | Optional instruction text included after the root path |

**Example:**

```bash
bash "${CLAUDE_PLUGIN_ROOT}/skills/export-docs-index/scripts/generate-index.sh" \
  ./docs/guides project-guides \
  "Read the relevant guide before implementing features."
```

## Output Format

The script outputs a single pipe-delimited line to stdout:

```
[section-name]|root: ./path|instruction text|subdir:{file1.md,file2.md}|otherdir:{file3.md}
```

- Section name in square brackets: `[my-docs]`
- Root path as provided: `root: ./docs/my-docs`
- Optional instruction text (if third argument provided)
- Root-level files grouped as `.:{file1.md,file2.md}`
- Subdirectory files grouped as `dirname:{file1.md,nested/file2.md}`
- Files and directories sorted alphabetically
- Hidden files/directories excluded
- `node_modules` directories excluded

**No XML wrapper is included** — wrap the output in `<docs_index>` tags when inserting into CLAUDE.md.

## Integration into CLAUDE.md

After generating the index, wrap the output and insert it into the project's CLAUDE.md:

```xml
<docs_index>
[output from generate-index.sh]
</docs_index>
```

Multiple sections can be combined by running the script for each directory:

```xml
<docs_index>
[guides]|root: ./docs/guides|core:{setup.md,usage.md}|advanced:{deployment.md}
[api]|root: ./docs/api|.:{overview.md}|endpoints:{auth.md,users.md}
</docs_index>
```

## Behavior Notes

- Symlinks are not followed (prevents infinite directory loops)
- Maximum directory depth is 10 levels
- Files in nested subdirectories are listed with paths relative to their top-level subdirectory (e.g., `subdir:{nested/deep/file.md}`)
- Empty directories produce no output (exit 0 with a stderr warning)
- Missing directories cause exit 1 with an error message on stderr
