---
id: marky-j7h
title: Migrate JSON structured parser to tree-sitter-json 0.24 API
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-36t]
parent: marky-6gw
---




Update markymark-parser/src/structured/json.rs: (1) Change tree_sitter_json::language() to tree_sitter_json::LANGUAGE.into(). (2) Change parser.set_language(lang) to parser.set_language(&lang). (3) Verify all Node method calls (kind, children, utf8_text, start_byte, end_byte) still compile — they should, these are stable. (4) Run all JSON parser tests to verify zero regression.
