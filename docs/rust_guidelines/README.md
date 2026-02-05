<!-- Progressive disclosure hub for Rust guidelines. Original monolith left intact at ../rust_guidelines_full.md -->

# Progressive Rust Guidelines Hub

<agent>
<goal>Find the smallest relevant Rust guideline quickly, without breaking stable IDs/anchors.</goal>
<canonical>../rust_guidelines_full.md</canonical>
<entrypoint>Start here, then jump to a themed leaf; use the monolith only as a full reference.</entrypoint>
<maintenance>If you add/change a guideline, keep the `M-...` ID stable and update `map.md` + `checklists.md`.</maintenance>
</agent>

Welcome to the progressive-disclosure view of our Rust guidelines. Start with the smallest, highest-signal paths below, then dive into themed leaves as needed. The original, full document remains unchanged at `../rust_guidelines_full.md`.

## Start Here (fastest path)
- AI-friendly essentials: `ai.md`
- Universal guardrails: `universal.md`
- When building applications: `applications.md`
- When writing libraries: skim `libraries-build.md`, then `libraries-ux.md`

## How to use this hub
- Each themed file opens with a TL;DR and a short checklist.
- High-signal rules come first; niche/advanced notes are at the bottom of each leaf.
- Cross-links at the end of each file point you to related themes and back to the monolith for full detail.

## Theme navigation
- AI & agent usage: `ai.md` — Make APIs and docs agent-friendly.
- Applications: `applications.md` — Error handling and allocator choices.
- Documentation: `docs.md` — Canonical sections, doc inlining, first sentences, module docs.
- FFI: `ffi.md` — Isolation for DLL state.
- Performance: `performance.md` — Hot path, throughput, yielding.
- Safety: `safety.md` — Unsafe discipline, soundness, panic posture.
- Universal: `universal.md` — Naming, magic values, lint expectations, logging, panics, debug/display, crate sizing, static verification, upstream rules.
- Libraries / Build: `libraries-build.md` — Features, OOBE, `-sys` expectations.
- Libraries / Interop: `libraries-interop.md` — External types, escape hatches, `Send`, strong types.
- Libraries / Resilience: `libraries-resilience.md` — Statics, mockable syscalls, glob exports, test utilities.
- Libraries / UX: `libraries-ux.md` — Wrappers, DI hierarchy, canonical errors, inherent APIs, `impl AsRef`/IO/RangeBounds, builders, cascaded init, cloneable services, simple abstractions.

## Maps and checklists
- Relationship map and navigation paths: `map.md`
- One-line reminders by theme: `checklists.md`

## Editing/contributing
- Agent/contributor guidance for this folder: `AGENTS.md`

## Full reference
- Monolithic source (unchanged): `../rust_guidelines_full.md`
