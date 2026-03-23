---
id: marky-42e
title: Improve markdown document symbol detection in LSP
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

LSP documentSymbol operation returns incomplete heading hierarchy.

**Current behavior:**
When querying a markdown file with nested headings, only the first top-level heading is returned as a String symbol.

**Expected behavior:**
Should return full heading hierarchy as DocumentSymbol objects with:
- Proper nesting (h2 under h1, h3 under h2, etc.)
- Correct ranges and selectionRanges
- Symbol kind appropriate for headings

**Example input:**
```markdown
# Main Title
## Section A
### Subsection A1
### Subsection A2
## Section B
### Subsection B1
```

**Expected output:**
- Main Title (Heading, contains sections)
  - Section A (Heading, contains subsections)
    - Subsection A1 (Heading)
    - Subsection A2 (Heading)
  - Section B (Heading)
    - Subsection B1 (Heading)

**Benefits:**
- Token-efficient outline view for AI agents
- Better navigation in LSP-enabled editors
- Enables semantic queries on document structure
- Matches LSP behavior of other markdown servers
