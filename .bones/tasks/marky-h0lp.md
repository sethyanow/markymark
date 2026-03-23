---
id: marky-h0lp
title: 'Bug: scan_all couples heading/link fallback - regression from marky-0mr.7'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

scan_all(...).unwrap_or_default() was introduced in marky-0mr.7 to eliminate the double-parse. But unwrap_or_default on the whole Result drops BOTH headings and links if scan_all returns Err. The old code called scan_headings and scan_links independently so each could fall back to empty independently. Fix: on scan_all error, fall back to independent scan_headings and scan_links calls so diagnostics and link indexing still work when only one side fails.
