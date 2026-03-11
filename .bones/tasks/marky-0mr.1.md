---
id: marky-0mr.1
title: 'PR#39 review: fix debounce race condition and close cleanup'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---



Fix four related issues in the debounce logic in markymark-lsp/src/server.rs:

**T1-1: Race condition between debounce_handles and pending_changes mutexes**
The two mutexes are acquired and released independently. Between dropping pending_changes lock and acquiring debounce_handles lock, another thread can insert a new handle or modify pending changes — wrong task could be aborted or changes lost. Fix: protect both maps with a single Mutex or restructure lock scopes so both are held simultaneously.

**T2-2: Lock ordering concern in debounce task**
Spawned debounce task acquires state.write() then state.read(). Future changes could introduce overlapping lock order. Fix: collapse into a single write lock scope covering both apply_document_changes and compute_diagnostics.

**T2-4: unwrap_or_default() masks invariant violation**
pending.remove(&doc_uri_clone) should always find an entry at this point. unwrap_or_default() turns a logic error into silent empty-vec return. Fix: replace with .expect("pending_changes entry missing for document URI").

**T2-8: did_close doesn't cancel pending debounce or clear buffered changes**
If a document is closed while a debounce is pending, the debounce fires post-close — redundant work. Fix: in did_close, abort the debounce handle for the document, remove pending changes entry, then close document in state.

Source: PR #39 review — Copilot + CodeRabbit
