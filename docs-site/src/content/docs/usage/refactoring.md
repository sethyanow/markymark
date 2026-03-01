---
title: Refactoring
description: Rename headings and symbols across your entire workspace with confidence
---

markymark supports renaming headings with automatic cross-file reference updates.
When you rename a heading, every wiki link, markdown anchor, and reference pointing
to it gets updated across the workspace in a single operation.

## In your editor

Place your cursor on a heading and press **F2** (or right-click and select
**Rename Symbol**). Type the new name and press Enter.

Before applying the rename, your editor shows a preview of every file that will change
and exactly what each edit looks like. Review the preview, then confirm to apply all
changes at once.

**What gets updated:**

- The heading text itself
- Wiki links referencing the heading (e.g., `[[page#old-name]]` becomes `[[page#new-name]]`)
- Markdown link anchors (e.g., `[text](page.md#old-name)` becomes `[text](page.md#new-name)`)

**What stays unchanged:**

- The link display text (the human-readable part of the link)
- Links to other headings in the same or different files

You can also use **Prepare Rename** (hover over a heading to see if it's renameable)
to check whether a symbol supports renaming before starting.

## With MCP tools

```text
rename: {
  "uri": "file:///path/to/doc.md",
  "line": 0,
  "character": 2,
  "new_name": "Updated Heading"
}
```

The `line` and `character` parameters point to the heading you want to rename.
The response contains a list of document edits — one per affected file — with
the exact text ranges and replacement text for each change.

## Tips

- Rename is workspace-scoped. It only updates references within the current
  realm (all indexed folders).
- Rename a heading before deleting it to find and fix all references first.
- If a heading has no incoming references, hover over it to confirm — the
  backlink count will show zero.
