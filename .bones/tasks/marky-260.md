---
id: marky-260
title: 'fix(dto): ExportedPropertyEntryDto.value loses list structure via string join'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

markymark-mcp/src/dto.rs:340-347 joins list property values with ', ' into a single String. This makes it impossible to distinguish a single value containing a comma from a two-element list. ExportedFrontmatterEntryDto correctly uses Vec<String>. Fix: change ExportedPropertyEntryDto.value from String to Vec<String> and update all consumers/serialization. Flagged by CodeRabbit in PR #36.
