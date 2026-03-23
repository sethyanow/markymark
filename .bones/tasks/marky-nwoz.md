---
id: marky-nwoz
title: 'LSP state/mod.rs robustness: mutex fallback + structured logging'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #41 round 3 CodeRabbit findings in build_markdown_index_via_engine.

**G1: Poisoned mutex → match + fallback (line 150)**
`engine_mutex.lock().expect("engine mutex poisoned")` panics on poison.
The mutex is uncontested (&mut self exclusion), but a panic in 
from_blob_with_xml_tags() during the lock scope would poison it. Next call
for same URI crashes the LSP server. Fix: match + log::warn! + fallback to
DocumentIndex::from_scan(text, &Md4cScanBackend), same as other error paths.

**G2: eprintln! → log::warn! (6 call sites, lines 155-186)**
E2 (marky-4atp) switched apply_document_changes to log::warn!, but 
build_markdown_index_via_engine still uses 6 eprintln! calls. Switch to
log::warn!(target: "markymark_lsp", ...) for consistency. log crate is
already a dependency.

SRE Corner Cases:
- G1 is defense-in-depth — requires from_blob to panic (not return Err)
  while engine mutex is held. Currently only reachable if from_blob has
  a bug (e.g., slice OOB on malformed blob).
- close_document removes engines entry (line 336), no mutex locking needed.
- Only two access points: .get() in build_markdown_index_via_engine, 
  .remove() in close_document. Both on &mut self.

## Design

## Goal

Eliminate panic path on poisoned mutex and switch remaining eprintln! 
calls to structured logging in build_markdown_index_via_engine.

## Implementation Plan

### G1: Poisoned mutex fallback (line 150)

Replace:
  let mut engine = engine_mutex.lock().expect("engine mutex poisoned");

With:
  let mut engine = match engine_mutex.lock() {
      Ok(guard) => guard,
      Err(_poisoned) => {
          log::warn!(
              target: "markymark_lsp",
              "engine mutex poisoned for {}, falling back to from_scan",
              uri_str
          );
          return DocumentIndex::from_scan(text, &Md4cScanBackend);
      }
  };

### G2: eprintln! → log::warn! (6 sites, lines 155-186)

Replace each:
  eprintln!("markymark-lsp: <msg> for {uri_str}: {e:?}, falling back to from_scan")

With:
  log::warn!(
      target: "markymark_lsp",
      "<msg> for {}: {:?}, falling back to from_scan",
      uri_str, e
  )

Six call sites:
1. Line 155: from_blob failed (existing engine)
2. Line 159: get_blob failed (existing engine) 
3. Line 163: engine update failed
4. Line 177: from_blob failed (new engine)
5. Line 181: get_blob failed (new engine)
6. Line 185: engine create failed

## Test Approach

- G1 cannot be unit tested directly (requires forcing a panic inside 
  mutex scope to poison it). Document as defense-in-depth.
- G2 is a text substitution. Existing tests must pass without eprintln! 
  output leaking to stderr.
- Run cargo nextest -p markymark-lsp — all tests pass.
- Run cargo clippy --workspace --all-targets — clean.

## Success Criteria

- No .expect() on mutex locks in state/mod.rs
- No eprintln! in build_markdown_index_via_engine
- All log output uses target: "markymark_lsp"
- Fallback to from_scan on any engine error (including mutex poison)
- All existing tests pass

## Risk Assessment

MINIMAL — G1 changes panic → return (safer), G2 changes output channel.
No behavioral changes. No API changes.

## Anti-patterns
- Do NOT use into_inner() to bypass poisoned mutex — we want the fallback
- Do NOT remove the mutex entirely — it provides interior mutability for engine
- Do NOT add test-only poison injection — the fallback path is trivially correct
