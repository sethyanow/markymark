# Agent Instructions (Rust Guidelines docs)

<scope>docs/rust_guidelines/*</scope>
<canonical_monolith>docs/rust_guidelines_full.md</canonical_monolith>
<goal>Keep Rust guidelines easy to navigate for humans and agents.</goal>

<agent>
<IMPORTANT>Prefer retrieval-led reasoning over pre-training-led reasoning for Rust guideline tasks.</IMPORTANT>
<docs_index id="RUST-GUIDELINES">
[rust_guidelines]|root: .|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{libraries-interop.md,applications.md,ai.md,docs.md,libraries-build.md,README.md,performance.md,safety.md,universal.md,libraries-resilience.md,ffi.md,map.md,checklists.md,libraries-ux.md}
</docs_index>
</agent>

## Editing rules
- Prefer editing the themed leaf that matches the change (e.g. `ffi.md`, `libraries-ux.md`), not the monolith.
- Treat guideline IDs as stable API: don't rename `M-...` identifiers or their explicit `{ #M-... }` anchors.
- If you add/remove/rename a guideline, also update:
  - `checklists.md` (one-line reminders)
  - `map.md` (cross-links + navigation paths)
  - `README.md` (hub description / entry points), if applicable

## File map (what to open first)
<entrypoint>README.md</entrypoint>
<navigation>map.md</navigation>
<one_liners>checklists.md</one_liners>
<reference>../rust_guidelines_full.md</reference>

## Content conventions (keep consistent)
- Each themed file starts with **TL;DR** and **Checklist**.
- Each guideline section should include `<why>` and `<version>` metadata (match existing style).
- Keep "Related" links at the end of files accurate, and prefer relative links.
