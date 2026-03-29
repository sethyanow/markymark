---
id: marky-hgz
title: 'PR#58 triage fixes: release runner, test key, cargo features, install wrapper, LTO canary'
status: open
type: task
priority: 1
---


## Requirements

PR#58 review triage (CodeRabbit + Copilot, 2026-03-29) surfaced 7 valid findings.

### P1 — Broken release builds
1. `release.yml:27` — `macos-13` is deprecated Intel runner. Replace with `macos-15` (ARM). Drop `x86_64-apple-darwin` from matrix.
2. `test_select_binary.sh:434` — reads `['markdown']` but .lsp.json key is `['markymark']`. Test fails.
3. `release.yml:122-125` — Cargo builds (Windows, Linux ARM64) missing `--features semantic-search,local-embeddings`. Produces broken binaries.

### P2 — Build ergonomics
4. `install.sh:16` — adds `--config=macos-lto` on Darwin but doesn't copy `tools/clang-lto-wrapper.sh` to `/opt/homebrew/opt/llvm/bin/clang-lto-wrapper`.
5. `engine/tests/mod.rs:879` — LTO canary gates on `cfg!(debug_assertions)`. Breaks under `cargo test --release` (opt without LTO). Needs explicit LTO signal.

### P4 — Hygiene
6. `.gitignore` — add `__temp_coderabbit.out*` pattern.
7. `test_select_binary.sh:730` — hard-coded `/usr/bin:/bin` should use `CLEAN_PATH` helper.

## Context

Source: PR#58 automated review triage. 36 raw findings → 22 unique → 7 valid after code verification. 15 dismissed (wrong assumptions about Intel macOS, stale code reads, untracked files, intentional design decisions).

## Success Criteria

- [ ] `release.yml` uses `macos-15` runner, no `x86_64-apple-darwin` target
- [ ] `test_select_binary.sh` reads `['markymark']` key
- [ ] Cargo release builds pass `--features semantic-search,local-embeddings`
- [ ] `install.sh` copies clang-lto-wrapper into place on Darwin
- [ ] LTO canary test uses explicit LTO signal instead of `debug_assertions`
- [ ] `.gitignore` includes `__temp_coderabbit.out*`
- [ ] `test_select_binary.sh` uses `CLEAN_PATH` instead of hard-coded path
