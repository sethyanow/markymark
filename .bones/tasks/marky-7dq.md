---
id: marky-7dq
title: 'F: Debounce did_change with async cancellation (50-100ms)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-77i
---




## Problem
server.rs:140-166 fires apply_document_changes() + publish_diagnostics_for() synchronously on every did_change. No delay. Every keystroke = full reparse cycle. At 200wpm (~100ms between keystrokes), this wastes ~10 reparse cycles/sec.

## Implementation
- In Backend::did_change, spawn an async task with 50-100ms delay
- Cancel the previous task on each new did_change (debounce)
- Only the final pause triggers the actual reparse + diagnostics publish
- Use tokio::time::sleep + Arc<Notify> or watch channel for cancellation

## Key Files
- markymark-lsp/src/server.rs:140-166 (did_change handler)
- markymark-lsp/src/state/mod.rs:185-373 (apply_document_changes)

## Testing
- Existing correctness tests call apply_document_changes directly — no changes needed
- Add integration test: fire multiple rapid did_change events, assert only one diagnostics publish
- Verify debounce timer resets on each keystroke

## Expected Impact
During fast typing, eliminates ~10 redundant reparses/sec. User perceives same latency (parse on pause). Total CPU work drops dramatically.
