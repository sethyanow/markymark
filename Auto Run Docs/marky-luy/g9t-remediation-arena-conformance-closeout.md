# Auto Run: g9t remediation: arena conformance closeout (marky-luy)

## Preamble

<instructions>
For the first unchecked task below:

1. `/claude-harness:start`
2. `/hyperpowers:execute-plan` targeting the task's bead ID
3. `/coderabbit-review` — apply all recommendations
4. `/claude-harness:checkpoint`
5. Mark task `- [x]`, append any discovered tasks as `- [ ]`
</instructions>

<epic>
Epic: marky-luy
Run `bd show marky-luy` for full requirements, anti-patterns, and design rationale.
</epic>

## Tasks

- [x] marky-g9t.1: Add hashbrown dep and arena infrastructure to markymark-core
- [ ] marky-g9t.2: Migrate markymark-parser types to arena lifetimes
- [ ] marky-g9t.3: Update parser extraction logic for arena allocation
- [ ] marky-g9t.4: Migrate markymark-index document types to arena lifetimes
- [ ] marky-g9t.5: Update RealmIndex for hybrid arena model
- [ ] marky-g9t.6: Update LSP and MCP crates for arena lifetimes
- [ ] marky-g9t.7: Memory benchmark and cleanup
