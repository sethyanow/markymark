---
title: Diagnostics
description: Detect broken links, duplicate headings, and other issues in your markdown files
---

markymark checks your documents for structural problems as you type. Issues appear
inline in your editor and update automatically, so you catch broken references before
they reach readers.

## What gets detected

**Broken wiki links.** A link like `[[nonexistent]]` that points to a page or heading
that doesn't exist in the workspace. Shown as a warning underline on the link text.

**Broken heading anchors.** A Markdown link like `[text](other.md#missing)` where the
target heading doesn't exist in the referenced document.

**Duplicate headings.** Two or more headings in the same file with identical text.
This causes ambiguous anchors — links can't reliably target the right one.

## In your editor

Diagnostics appear as underlines on the problematic text. The **Problems panel**
(Ctrl+Shift+M / Cmd+Shift+M on macOS) lists all issues across open files with
severity, message, and location.

Diagnostics update as you type with a short debounce delay. When you fix a broken
link or rename a duplicate heading, the warning clears automatically.

## With MCP tools

Check diagnostics for a single document:

```text
get-diagnostics: { "uri": "file:///path/to/doc.md" }
```

Check all documents in a realm:

```text
get-diagnostics: { "realm": "my-docs" }
```

The response lists each file with issues, including the diagnostic range, severity,
and message. This is useful for automated quality checks — run `get-diagnostics` on
a realm to find all broken links in a documentation set at once.

## Common fixes

| Problem | Fix |
|---------|-----|
| Broken wiki link | Create the target page, or correct the link text to match an existing page |
| Broken heading anchor | Update the anchor to match the target heading's actual text |
| Duplicate heading | Rename one of the duplicates to make it unique within the file |

## Tips

- Diagnostics are workspace-aware. A link to `[[setup]]` resolves if any file in the
  workspace has a matching heading or filename.
- Single-file mode (no folder open) still detects duplicate headings and internal
  broken anchors, but can't verify cross-file wiki links.
