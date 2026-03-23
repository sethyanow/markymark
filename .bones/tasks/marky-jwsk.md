---
id: marky-jwsk
title: 'LSP: clean up stale document_generations entries on close'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Copilot PR #40 finding C4: document_generations HashMap in DebounceState never removes entries. Long-running server grows without bound (~50-100 bytes per unique URI).

## Design

## Goal
Prevent unbounded growth of the document_generations HashMap in DebounceState for long-running LSP servers that touch many unique URIs.

## Effort Estimate
1-2 hours

## Problem (Copilot C4)
document_generations (server.rs:31) is a HashMap<DocumentUri, u64> that gets entries added in did_open and incremented in did_close but never has entries removed. The comment at line 297 says "Do NOT remove the entry — the debounce task needs to see the bump." This is correct for the debounce window, but after the debounce task completes (or is aborted), the entry is stale and can be cleaned up.

For a long-running server editing thousands of unique files, this grows without bound (~50-100 bytes per entry).

## Fix Options

**Option A (recommended): Clean up in debounce task completion**
After the debounce task successfully applies changes (or is aborted), remove the generation entry if no pending changes or handles remain for that URI. This happens in try_apply_drained or when abort is called.

**Option B: Periodic sweep**
Add a sweep that runs every N close events, removing entries where no debounce_handle or pending_changes exist for that URI. Simpler but less precise.

**Option C: Move generations into ServerState**
Move document_generations into ServerState and update under the state write lock. This eliminates the debounce lock ordering concern entirely but changes the architecture more.

## Success Criteria
- [ ] document_generations entries are cleaned up when no longer needed
- [ ] Existing debounce tests pass (5/5 including regression test)
- [ ] Generation counter still prevents stale close/reopen race (marky-aemm regression test)
- [ ] No new lock ordering issues introduced
- [ ] cargo test -p markymark-lsp passes

## Key Considerations (SRE Review)

**Edge Case: Race between cleanup and new open**
If we remove a generation entry and the document is immediately reopened, the generation starts at 0 (or 1 on first insert). This is safe because the debounce task captures the generation at drain time — a fresh open would create a new debounce task with the new generation.

**Edge Case: Abort without cleanup**
When did_close aborts a debounce handle, the aborted task may never run its cleanup. Ensure the abort path also cleans up the generation entry (after the abort, no task can reference it).

**Test Meaningfulness**
Add a test that opens/closes many unique URIs and verifies document_generations doesn't grow without bound. This catches regressions where cleanup is accidentally removed.

## Anti-patterns
- Do NOT remove generation entries in did_close — the debounce task may still be in flight
- Do NOT use a global lock to synchronize cleanup — use existing lock scoping
- Do NOT change the generation counter semantics — it must still be monotonically increasing per URI
