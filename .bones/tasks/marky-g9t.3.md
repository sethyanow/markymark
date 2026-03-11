---
id: marky-g9t.3
title: Update parser extraction logic for arena allocation
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.2, marky-luy]
parent: marky-g9t
---





Update all extraction functions in markymark-parser/src/extract.rs (~638 lines) to accept &'arena Bump and allocate into it. Update ast.rs Ast struct to own a Bump and allocate root_elements into it. All extract_* functions switch from .to_string() to arena.alloc_str().

Success: cargo test -p markymark-parser passes. All parser tests green.

## Design

## Discovery
extract::extract_list_items was dead code — Ast::extract_list_items uses collect_top_level_list_items which already takes arena. Removed redundant function. Extraction is consistently arena-driven.
