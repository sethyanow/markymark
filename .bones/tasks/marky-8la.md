---
id: marky-8la
title: 'markymark: XML tag false positives in fenced code blocks'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

markymark's diagnostics engine parses Rust generics like <T>, <Mutex>, <dyn Trait> inside fenced code blocks as unclosed XML tags. Code block content (between triple backticks) should be excluded from XML tag parsing. Affects 198+ false positives across the rust_agent_docs corpus.
