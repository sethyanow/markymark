# Rust Guidelines Map

<agent>
<goal>Pick the best next file to read when reviewing or changing Rust code.</goal>
<canonical>../rust_guidelines_full.md</canonical>
<editing_rules>See `AGENTS.md` for how to change these docs without breaking IDs/anchors.</editing_rules>
<routing>
<rule>unsafe / UB / soundness -> safety.md</rule>
<rule>FFI / DLL / dylib boundaries -> ffi.md</rule>
<rule>rustdoc / module docs / re-exports -> docs.md</rule>
<rule>features / publishing / `-sys` / `build.rs` -> libraries-build.md</rule>
<rule>API ergonomics / errors / builders / inputs -> libraries-ux.md</rule>
<rule>statics / mocking / test seams / re-exports -> libraries-resilience.md</rule>
<rule>external types / `Send` / escape hatches -> libraries-interop.md</rule>
<rule>hot paths / benchmarking / throughput / yield -> performance.md</rule>
<rule>naming / logging / lints / panic posture -> universal.md</rule>
<rule>agent friendliness / examples / testability -> ai.md</rule>
<rule>application defaults / app errors / mimalloc -> applications.md</rule>
</routing>
</agent>

Relationship map and common navigation paths for the progressive Rust guidelines. All guideline IDs/anchors match the original monolith in `../rust_guidelines_full.md`.

## Cross-guideline relationships
- AI (`ai.md`) → Docs (`docs.md`), UX (`libraries-ux.md`), Universal (`universal.md`).
- Applications (`applications.md`) → UX errors (`libraries-ux.md#M-ERRORS-CANONICAL-STRUCTS`), Performance (`performance.md`).
- Docs (`docs.md`) ↔ Resilience (`libraries-resilience.md#M-NO-GLOB-REEXPORTS`), AI (`ai.md`).
- FFI (`ffi.md`) ↔ Safety (`safety.md`), Interop (`libraries-interop.md`).
- Performance (`performance.md`) ↔ Safety (`safety.md`), Resilience (`libraries-resilience.md`).
- Safety (`safety.md`) ↔ FFI (`ffi.md`), Performance (`performance.md`), Universal panic/logging (`universal.md`).
- Universal (`universal.md`) ↔ Docs (`docs.md`), Safety (`safety.md`), UX (`libraries-ux.md`).
- Libraries/Build (`libraries-build.md`) ↔ Interop (`libraries-interop.md`), Resilience (`libraries-resilience.md`).
- Libraries/Interop (`libraries-interop.md`) ↔ UX (`libraries-ux.md`), Resilience (`libraries-resilience.md`).
- Libraries/Resilience (`libraries-resilience.md`) ↔ Docs (`docs.md`), Safety (`safety.md`), Performance (`performance.md`).
- Libraries/UX (`libraries-ux.md`) ↔ AI (`ai.md`), Applications (`applications.md`), Universal (`universal.md`).

## Navigation paths
- Agent onboarding path: `ai.md` → `docs.md` → `universal.md` → `libraries-ux.md` (if authoring libraries).
- Application path: `applications.md` → `performance.md` → `universal.md` → `docs.md` → `libraries-ux.md` (errors).
- Library path (build): `libraries-build.md` → `libraries-resilience.md` → `libraries-interop.md` → `libraries-ux.md`.
- Safety/FFI review path: `safety.md` → `ffi.md` → `libraries-interop.md` → `libraries-resilience.md`.

## Originals
- Full monolith: `../rust_guidelines_full.md`
