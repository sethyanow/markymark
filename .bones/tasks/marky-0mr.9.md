---
id: marky-0mr.9
title: 'PR#39 review: Zig parser edge-case fixes'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---



Five small edge-case fixes across the Zig md4c parser. All independent, low risk:

**T2-7: Off-by-one in matchHtmlTag (line_analysis.zig:207)**
>= should be > — rejects valid tags ending exactly at buffer boundary. One-character fix.

**T2-13: Self-closing tag bounds check (html_renderer.zig:441-457)**
updateTagFilterRawDepth checks content[content.len - 2] without guaranteed len >= 3. Add explicit guard before the "/> check.

**T3-6: Defensive bounds check in fold table scan (unicode.zig:45-46)**
pivot_end += 1 without ensuring pivot_end < map.len. Add compound condition that checks bounds first.

**T3-10: Unreachable return false (containers.zig:93-102)**
Final return false in isContainerCompatible is unreachable. Remove it or collapse with the bullet-list check.

**T3-13: Confusing condition beg >= 2 vs beg > 1 (autolinks.zig:200-207)**
Functionally equivalent but beg > 1 is clearer for reasoning about content[beg - 2] access.

Source: PR #39 review — CodeRabbit
