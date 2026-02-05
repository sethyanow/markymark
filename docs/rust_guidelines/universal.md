# Universal Guidelines (progressive)

<agent>
<goal>Default Rust “house rules” for naming, lints, logs, panics, and API shape.</goal>
<when_to_use>When you need baseline conventions and consistency checks across crates.</when_to_use>
<contains>M-CONCISE-NAMES, M-DOCUMENTED-MAGIC, M-LINT-OVERRIDE-EXPECT, M-LOG-STRUCTURED, M-PANIC-IS-STOP, M-PANIC-ON-BUG, M-PUBLIC-DEBUG, M-PUBLIC-DISPLAY, M-REGULAR-FN, M-SMALLER-CRATES, M-STATIC-VERIFICATION, M-UPSTREAM-GUIDELINES</contains>
<see_also>docs.md, safety.md, libraries-ux.md</see_also>
<canonical>../rust_guidelines_full.md</canonical>
</agent>

**TL;DR:** Use clear names, document magic, prefer `#[expect]` for lint overrides, structured logging, panic only for stop/bugs, derive `Debug`/`Display` where appropriate, keep crates small, use static verification, and follow upstream guidance.

**Checklist:**
- Avoid weasel words in names; be precise.
- Document magic values; use types where possible.
- Use `#[expect]` (with reason) for lint overrides; keep them rare.
- Emit structured logs with templates; avoid ad-hoc string formatting.
- Panic means stop; bugs panic, not errors.
- Public types derive `Debug`; user-facing types implement `Display`.
- Prefer free functions over associated when feasible; split crates if in doubt.
- Enable/keep strong lints; honor upstream guidelines.

## Names are Free of Weasel Words (M-CONCISE-NAMES) { #M-CONCISE-NAMES }

Avoid vague suffixes like `Service`, `Manager`, `Factory`. Name by responsibility (`Bookings`, `BookingDispatcher`). Prefer builders over “factory”; use closures for repeatable instantiation.

## Magic Values are Documented (M-DOCUMENTED-MAGIC) { #M-DOCUMENTED-MAGIC }

Document any non-obvious constants; prefer newtypes/strong types instead of raw literals to encode semantics.

## Lint Overrides Should Use `#[expect]` (M-LINT-OVERRIDE-EXPECT) { #M-LINT-OVERRIDE-EXPECT }

Use `#[expect(lint, reason = \"...\")]` rather than `allow`. Keep overrides narrow and justified.

## Use Structured Logging with Message Templates (M-LOG-STRUCTURED) { #M-LOG-STRUCTURED }

- Prefer structured logs with templates over interpolated strings.
- Avoid string formatting that hides fields; name events; follow OpenTelemetry semantic conventions; redact sensitive data.

## Panic Means 'Stop the Program' (M-PANIC-IS-STOP) { #M-PANIC-IS-STOP }

Panic is for unrecoverable stop conditions; prefer errors for recoverable cases.

## Detected Programming Bugs are Panics, Not Errors (M-PANIC-ON-BUG) { #M-PANIC-ON-BUG }

Programmer bugs (logic violations, invariant breaks) should panic rather than be modeled as recoverable errors.

## Public Types are Debug (M-PUBLIC-DEBUG) { #M-PUBLIC-DEBUG }

Public types should derive `Debug` to aid diagnostics.

## Public Types Meant to be Read are Display (M-PUBLIC-DISPLAY) { #M-PUBLIC-DISPLAY }

If a public type is meant for user-visible output, implement `Display`.

## Prefer Regular over Associated Functions (M-REGULAR-FN) { #M-REGULAR-FN }

Prefer free/regular functions when methods are not tied to a specific receiver state; this keeps APIs flexible and testable.

## If in Doubt, Split the Crate (M-SMALLER-CRATES) { #M-SMALLER-CRATES }

Favor smaller, focused crates over monoliths to reduce compile times and clarify ownership boundaries.

## Use Static Verification (M-STATIC-VERIFICATION) { #M-STATIC-VERIFICATION }

Enable strong compiler/clippy lints; prefer `warn`/`deny` where signal outweighs noise. Keep overrides minimal and documented. Example baseline:

```toml
[workspace.metadata.cargo-clippy]
warn = ["clippy::pedantic", "clippy::nursery"]
```

Adjust as needed; document opt-outs with `#[expect(..., reason = \"...\")]`.

## Follow the Upstream Guidelines (M-UPSTREAM-GUIDELINES) { #M-UPSTREAM-GUIDELINES }

Follow upstream Rust API guidelines and ecosystem conventions unless you have a documented reason to deviate.

### Related
- Logging details: `libraries-ux.md` (API ergonomics)
- Panic/error posture: `safety.md`
- Original: `../rust_guidelines_full.md`
