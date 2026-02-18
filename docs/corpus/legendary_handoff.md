<handoff>
    <metadata>
      <project>markymark</project>
      <pr>36</pr>
      <branch>dev</branch>
      <created>2026-02-18</created>
      <handoff_from>Bender (claude-sonnet-4-6)</handoff_from>
      <handoff_to>Claude Opus + parallel subagents</handoff_to>
      <purpose>Address all actionable PR #36 review findings before merge</purpose>
    </metadata>


    <session_initialization>
      <instructions>
        You are a Claude Opus orchestrator. Your job is to dispatch parallel subagents
        to fix all PR #36 review findings tracked in beads, then validate everything
        is green before declaring the PR merge-ready. DO NOT merge the PR — human merges.
      </instructions>

      <steps>
        <step order="1">cd /Volumes/code/markymark and confirm you are on branch dev</step>
        <step order="2">Run: bd list --status=open to see current issue state</step>
        <step order="3">Read docs/MEMORY.md — mandatory, contains architectural decisions</step>
        <step order="4">Read markymark-lsp/src/convert.rs:44-62 — know the existing UTF-16 helper before dispatching Track A</step>
        <step order="5">Dispatch all parallel tracks in a SINGLE message (see orchestration)</step>
      </steps>

      <learned_rules>
        <rule>NEVER merge PRs — agent prepares, human merges (Project Rule #7)</rule>
        <rule>Use built-in LSP tool for Rust navigation, not Serena MCP (Project Rule #1)</rule>
        <rule>No file exceeds 1000 lines — hard stop (Project Rule #2)</rule>
        <rule>Never squash merge (Project Rule #5)</rule>
        <rule>Use cargo-mcp tools for build/test, not raw cargo commands</rule>
        <rule>Write tests first (TDD) for any behavior change, especially T1-1</rule>
        <rule>git push is mandatory before declaring work done</rule>
      </learned_rules>

      <past_failures_to_avoid>
        <failure>
          <approach>Claiming "the fix looks correct" without running tests</approach>
          <root_cause>Proof-of-work discipline skipped</root_cause>
          <lesson>Run cargo nextest after every change. Capture output. Report failures.</lesson>
        </failure>
        <failure>
          <approach>Using Serena MCP for Rust symbol navigation</approach>
          <root_cause>Serena has no Rust language server; returns garbage for .rs files</root_cause>
          <lesson>Use built-in LSP tool (documentSymbol, hover, goToDefinition, findReferences)</lesson>
        </failure>
        <failure>
          <approach>Amending commits after pre-commit hook failure</approach>
          <root_cause>--amend modifies previous commit, destroying work history</root_cause>
          <lesson>After hook failure, fix issue, re-stage, create NEW commit</lesson>
        </failure>
      </past_failures_to_avoid>
    </session_initialization>

    <orchestration>
      <strategy>
        Six parallel tracks covering disjoint file sets. Tracks A–E are subagents.
        Track F (nitpicks) can be a single subagent or the orchestrator itself.
        All tracks run simultaneously. Orchestrator waits for all to report back,
        then runs the integration validation phase.
      </strategy>

      <parallelization_map>

        <phase name="dispatch" parallel="false">
          <task>Orchestrator reads MEMORY.md and convert.rs:44-62</task>
          <task>Orchestrator dispatches Tracks A–F in a single message with 6 Task tool calls</task>
          <deliverable>All 6 subagents spawned simultaneously</deliverable>
        </phase>

        <phase name="implementation" parallel="true" depends_on="dispatch">

          <parallel_track name="A-critical-utf16" critical="true">
            <scope>markymark-lsp/src/state/completion.rs</scope>
            <beads>marky-9cw (P1 — MUST FIX)</beads>
            <task>
              1. LSP documentSymbol on completion.rs to find detect_completion_context
              2. LSP hover on pos.character usage at line 82 to confirm type
              3. Read convert.rs:44-62 to understand lsp_position_to_byte_offset signature
              4. RED: Write a failing test that panics with a multi-byte char at cursor position
              5. GREEN: Replace &amp;line[..col] with conversion via lsp_position_to_byte_offset
                 or utf16_offset_to_byte_offset. Update bounds check to use byte offset.
              6. Run: cargo nextest -p markymark-lsp
              7. bd update marky-9cw --status=in_progress, then bd close marky-9cw
            </task>
            <deliverable>completion.rs no longer panics on multi-byte chars. Tests green.</deliverable>
            <verification>cargo nextest -p markymark-lsp — zero failures</verification>
          </parallel_track>

          <parallel_track name="B-safety-comments">
            <scope>markymark-kernels/src/index_serde.rs, markymark-kernels/src/scan.rs</scope>
            <beads>marky-ktt (P1 — audit)</beads>
            <task>
              1. Read index_serde.rs — locate all unsafe blocks at lines 107, 138, 141, 147, 168
              2. Read scan.rs — locate unsafe blocks at lines 454, 513
              3. For each unsafe block, document:
                 - Buffer validity: why the pointer is non-null and in-bounds
                 - Alignment: why the alignment invariant holds
                 - Lifetime: why the pointed-to data outlives the unsafe block
                 - FFI: any ABI or ownership conventions upheld
              4. Add // SAFETY: {justification} directly above each unsafe { block
              5. Run: cargo clippy -p markymark-kernels
              6. bd comments add marky-ktt "SAFETY comments added to all kernels unsafe blocks: index_serde.rs (5 sites) and
  scan.rs (2 sites). See commit {hash}."
            </task>
            <deliverable>All 7 unsafe blocks documented with SAFETY invariants</deliverable>
            <verification>cargo clippy -p markymark-kernels — no new warnings</verification>
          </parallel_track>

          <parallel_track name="C-lsp-correctness">
            <scope>
              markymark-lsp/src/incremental/mod.rs,
              markymark-lsp/src/state/rename.rs
            </scope>
            <beads>marky-oiv (P4 — saturating math + doc mismatch)</beads>
            <task>
              === Part 1: incremental/mod.rs — saturating casts ===
              1. Read incremental/mod.rs:138-166 (adjust_range_after_edit, adjust_bytes_after_edit)
              2. For each i64+delta cast to u32: change (x as i64 + delta) as u32
                 to (x as i64 + delta).max(0) as u32
              3. For usize casts in adjust_bytes_after_edit:
                 change (x as isize + delta) as usize to (x as isize + delta).max(0) as usize
              4. Fix doc at lines 14-17: remove claim that MarkdownLink/XmlTag have "no byte
                 offsets" — they do have byte offsets and DO undergo neighbour-window checks.

              === Part 2: rename.rs — u32 underflow guard ===
              5. Read rename.rs:155-168
              6. Replace raw subtraction with checked_sub chain:
                 xml.range.end.character.checked_sub(1)
                   .and_then(|c| c.checked_sub(xml.tag_name.len() as u32))
                 Only push RenameEdit if Some(close_start).
              7. Run: cargo nextest -p markymark-lsp
              8. bd close marky-oiv
            </task>
            <deliverable>
              Saturating arithmetic everywhere in incremental path.
              No u32 underflow possible in rename.
              Module doc accurate.
            </deliverable>
            <verification>cargo nextest -p markymark-lsp — zero failures</verification>
          </parallel_track>

          <parallel_track name="D-mcp-dto">
            <scope>markymark-mcp/src/dto.rs and all consumers</scope>
            <beads>marky-260 (P3) + marky-r0g (P3)</beads>
            <task>
              === Part 1: ExportedPropertyEntryDto (marky-260) ===
              1. LSP documentSymbol dto.rs — find ExportedPropertyEntryDto
              2. LSP findReferences on ExportedPropertyEntryDto — map all consumers
              3. Change .value field from String to Vec&lt;String&gt;
              4. Update the builder/from impl that does .join(", ") — return the vec directly
              5. Update serde/JsonSchema derives if needed
              6. Fix all consumers (use .join(", ") at display sites if needed)
              7. Run: cargo nextest -p markymark-mcp
              8. bd close marky-260

              === Part 2: temp_dir → tempfile (marky-r0g) ===
              9. Read markymark-mcp/src/engine/tests.rs — find temp_dir helper at line 6
              10. Read markymark-mcp/src/pattern/tests.rs — find temp_dir at line 8
              11. Replace both helpers: return tempfile::TempDir instead of PathBuf
                  (tempfile = tempfile::tempdir().expect("create temp dir"))
              12. Update call sites: use dir.path() where &amp;Path is needed
              13. Remove manual fs::remove_dir_all and fs::create_dir_all calls
              14. Run: cargo nextest -p markymark-mcp
              15. bd close marky-r0g
            </task>
            <deliverable>DTO preserves list structure. Tests use safe temp dirs.</deliverable>
            <verification>cargo nextest -p markymark-mcp — zero failures</verification>
          </parallel_track>

          <parallel_track name="E-infra-docs">
            <scope>lefthook.yml, README.md, docs/rust_crates/bumpalo.md</scope>
            <beads>marky-fe7 (P3), marky-7cl (P2), marky-pyb (P3)</beads>
            <task>
              === lefthook.yml (marky-fe7) ===
              1. Read lefthook.yml — find 07-zig-build at line ~16
              2. Change run value to:
                 'command -v zig >/dev/null || { echo "zig is required: https://ziglang.org/download/"; exit 1; }; zig build
  -Doptimize=Debug'
              3. bd close marky-fe7

              === README.md (marky-7cl) ===
              4. Read README.md:130-155 — find crates table
              5. Check: does markymark-vscode/Cargo.toml exist? (it does not)
              6. Remove the markymark-vscode row OR add "(external — see markymark-plugin/)" note
              7. bd comments add marky-7cl "README crates table updated: markymark-vscode row removed/marked external per PR
  #36 review."

              === bumpalo.md (marky-pyb) ===
              8. Read docs/rust_crates/bumpalo.md:625-660
              9. Delete the second ## Pitfalls section (lines ~631-653) including the stale
                 reference to bumpalo/pitfalls.md
              10. Verify only one ## Pitfalls heading remains (markdownlint MD024)
              11. bd close marky-pyb
            </task>
            <deliverable>
              lefthook gives actionable error for missing zig.
              README crates table accurate.
              bumpalo.md has no duplicate headings or stale references.
            </deliverable>
            <verification>
              lefthook run --all-files (or inspect yaml)
              markdownlint docs/rust_crates/bumpalo.md
            </verification>
          </parallel_track>

          <parallel_track name="F-nitpicks">
            <scope>Multiple docs + small code sites (no cross-track file conflicts)</scope>
            <beads>T3 bundle (no dedicated beads — batch close as part of PR polish)</beads>
            <task>
              Process each T3 item. Verify against actual file content first (don't blindly apply).

              DOCS (markdown lint fixes):
              1. docs/agent-patterns-raw.md:33 — "markdown lists" → "Markdown lists"
              2. docs/plans/brza-markymark.md:259 — outer fence triple→quad backtick (MD040)
              3. docs/research/harness-memory-archive-2026-02-18.md:94 — pick one: "prerelease" or "pre-release", apply
  consistently
              4. docs/research/harness-memory-archive-2026-02-18.md:160 — "pre/post benchmark" → "pre/post-benchmark"
              5. docs/research/harness-memory-archive-2026-02-18.md:190 — add blank lines around Tech Stack table
              6. docs/research/wasm-zig-feasibility.md:40 — add ```text language specifier

              CODE (low-risk, no behavior change):
              7. markymark-mcp/src/tools/graph.rs:28-30:
                 orphan_count: u32 = orphans.len().try_into().unwrap_or(u32::MAX)
                 (same for broken_link_count and cluster_count)
              8. markymark-kernels/src/scan.rs:259-268 (starts_with_ascii_case_insensitive):
                 replace chars().count() with .len() — safe because fewer UTF-8 bytes
                 implies fewer or equal chars
              9. markymark-lsp/src/lib.rs:15-16 — add diagnostics + incremental to arch doc
              10. markymark-mcp/src/engine/search.rs:43-104 — extract sort_symbol_results helper,
                  replace both identical closures
              11. markymark-mcp/src/engine/references.rs:43-48 — extract sort_locations helper,
                  replace all 4 identical closures
              12. markymark-index/tests/extraction_parity.rs:193-206 — in contains_setext_heading,
                  add guard: if trimmed == "---" { continue; }

              After all edits:
              Run: cargo nextest --workspace
              Run: cargo clippy --workspace --all-targets
            </task>
            <deliverable>All T3 items addressed. Docs lint-clean. No new clippy warnings.</deliverable>
            <verification>
              cargo nextest --workspace
              cargo clippy --workspace --all-targets 2>&amp;1 | grep "^error" | wc -l → 0
            </verification>
          </parallel_track>

        </phase>

        <phase name="integration-validation" parallel="false" depends_on="implementation">
          <task>
            Orchestrator collects results from all tracks. Then:
            1. Run: cargo build --workspace (full build, all features)
            2. Run: cargo nextest --workspace (all tests)
            3. Run: cargo clippy --workspace --all-targets 2>&amp;1 | grep "^error"
            4. If anything fails — dispatch targeted fix subagent before proceeding
            5. git pull --rebase origin dev
            6. bd sync
            7. git add -p (selective — no .env, no generated artifacts)
            8. git commit -m "fix(pr36): address all review findings — T1 UTF-16 panic, T2 safety/correctness, T3 nitpicks"
            9. git push
            10. Verify git status shows "up to date with origin"
            11. Report final state to user
          </task>
          <deliverable>Green build. Green tests. All beads issues closed. Branch pushed.</deliverable>
        </phase>

      </parallelization_map>

      <subagent_spawn_pattern>
        Dispatch all 6 tracks in ONE message. Each Task tool call = one subagent.
        Use subagent_type="general-purpose" for all tracks (needs file read+write+bash).

        Each subagent prompt MUST include:
        - "You are working in /Volumes/code/markymark on branch dev"
        - Their specific track scope and beads IDs
        - The exact task list above for their track
        - "Use the built-in LSP tool (not Serena) for Rust navigation"
        - "Run cargo nextest after changes and report failures"
        - "Do NOT push — orchestrator handles git"
        - "Report back: DONE | BLOCKED | FAILED with details"
      </subagent_spawn_pattern>
    </orchestration>

    <tdd_workflow>
      <applies_to>Track A (T1-1 — completion.rs UTF-16 panic) is the only behavior change requiring TDD</applies_to>

      <phase name="red">
        <instruction>
          Write a test in markymark-lsp/tests/ or the existing state/completion tests
          that calls detect_completion_context with a document containing a multi-byte
          UTF-8 character (e.g., "café") and a cursor position inside or after the
          multi-byte sequence. Confirm it panics before the fix.
        </instruction>
      </phase>

      <phase name="green">
        <instruction>
          Apply the UTF-16→byte conversion using lsp_position_to_byte_offset or
          utf16_offset_to_byte_offset. The test must pass without panic.
        </instruction>
      </phase>

      <phase name="refactor">
        <instruction>
          Ensure the bounds check and prefix slice both use the converted byte offset.
          No direct pos.character-as-byte-index anywhere in the function.
        </instruction>
      </phase>

      <verification_commands>
        <command>cargo nextest -p markymark-lsp</command>
        <command>cargo clippy -p markymark-lsp</command>
      </verification_commands>
    </tdd_workflow>

    <checkpoint_protocol>
      <when_to_checkpoint>
        - Each subagent checkpoints after completing their track
        - Orchestrator checkpoints after integration validation
      </when_to_checkpoint>

      <subagent_reporting>
        Each subagent sends back:
        - Status: DONE | BLOCKED | FAILED
        - Beads IDs closed
        - Files modified
        - Test results (pass count / fail count)
        - Any blockers or decisions made
      </subagent_reporting>

      <orchestrator_steps>
        <step order="1">Wait for all subagent reports</step>
        <step order="2">Run full workspace build + test</step>
        <step order="3">Resolve any conflicts (same-file edits unlikely — tracked above)</step>
        <step order="4">Single commit with all changes</step>
        <step order="5">git push dev</step>
        <step order="6">Report to user: all issues addressed, PR #36 is merge-ready (human merges)</step>
      </orchestrator_steps>
    </checkpoint_protocol>

    <immediate_actions>
      <action priority="1">
        cd /Volumes/code/markymark — verify on branch dev, git status clean
      </action>
      <action priority="2">
        Read docs/MEMORY.md (mandatory project context)
      </action>
      <action priority="3">
        Read markymark-lsp/src/convert.rs:44-62 (know the UTF-16 helper before dispatching)
      </action>
      <action priority="4">
        Dispatch Tracks A–F simultaneously in ONE message with 6 Task tool calls
      </action>
      <action priority="5">
        Collect subagent reports, run integration validation, commit, push
      </action>
    </immediate_actions>

    <constraints>
      <constraint type="merge">NEVER run gh pr merge — human merges all PRs (Project Rule #7)</constraint>
      <constraint type="scope">Do not refactor code beyond what the triage items require</constraint>
      <constraint type="line-limit">No file may exceed 1000 lines (Project Rule #2)</constraint>
      <constraint type="history">Never squash merge, never amend after hook failure (Project Rule #5)</constraint>
      <constraint type="tests">No code change ships without tests green (CLAUDE.md rule)</constraint>
      <constraint type="tooling">Use cargo-mcp tools for build/test, built-in LSP for Rust navigation</constraint>
      <constraint type="git">Push is mandatory before declaring work done</constraint>
      <constraint type="artifacts">Do not commit generated report files or .env — stage selectively</constraint>
    </constraints>
  </handoff>
