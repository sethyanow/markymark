---
id: marky-0mr.2
title: 'PR#39 review: harden debounce tests'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0mr.1]
parent: marky-0mr
---



Improve debounce test coverage and robustness in markymark-lsp/tests/debounce.rs. Blocked by marky-0mr.1 (fix the race condition first, then test the fixed behavior).

**T2-3: Flaky timing — 200ms sleep is only 2.7x headroom over 75ms debounce**
Under load or slow CI this will intermittently fail. Fix: either export DEBOUNCE_MS and use DEBOUNCE_MS * 5, or increase fixed sleep to 500ms.

**T2-9: Missing test — close-during-debounce should cancel pending task**
New debounce feature has no test verifying that closing a document mid-debounce cancels the task and leaves the index unchanged. Fix: add debounce_close_during_debounce test: open doc → did_change → did_close before debounce fires → assert index not updated.

**T3-1: Missing test — empty change batch path**
No test covering the case where pending changes are empty when debounce fires.

Source: PR #39 review — Copilot + CodeRabbit
