---
id: marky-ayb
title: 'CI integration: Zig build step in GitHub Actions'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv, marky-een]
---



Add Zig compiler installation to ci.yml. When zig-kernels feature is enabled, CI should: install zig, build libmarky_kernels.a, run zig tests, run cargo test with zig-kernels feature. Add as optional CI job (not blocking default build). Platform matrix: macOS arm64, Linux x86_64 minimum.

## Design

## Goal
Add Zig compiler installation and build steps to ci.yml for GitHub Actions. When zig-kernels feature is enabled, CI should: install Zig 0.15.2, build libmarky_kernels.a, run Zig tests, run cargo test with zig-kernels feature. Added as optional CI job (not blocking default build). Platform matrix: macOS arm64, Linux x86_64 minimum.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] New CI job "zig-kernels" in .github/workflows/ci.yml
- [ ] Zig 0.15.2 installed via official release or mlugg/setup-zig action
- [ ] cd zig && zig build test runs and passes in CI
- [ ] cargo test --features zig-kernels runs and passes in CI
- [ ] cargo clippy --features zig-kernels -- -D warnings runs and passes
- [ ] Platform matrix includes at least: ubuntu-latest (x86_64), macos-14 (arm64)
- [ ] Job is NOT a required check (optional, doesn't block PRs without Zig changes)
- [ ] Default build job (without zig-kernels) remains unchanged and unblocked
- [ ] Zig and libmarky_kernels.a cached between runs for speed

## Implementation Checklist
- [ ] Add zig-kernels job to .github/workflows/ci.yml
- [ ] Use mlugg/setup-zig@v1 or equivalent to install Zig 0.15.2
- [ ] Cache zig-cache/ and zig-out/ directories for faster builds
- [ ] Step 1: cd zig && zig build lib (build the static library)
- [ ] Step 2: cd zig && zig build test (run Zig unit tests)
- [ ] Step 3: cargo test --features zig-kernels (run Rust FFI tests)
- [ ] Step 4: cargo clippy --features zig-kernels -- -D warnings
- [ ] Configure matrix: os: [ubuntu-latest, macos-14]
- [ ] Make job optional: continue-on-error: true or separate workflow
- [ ] Add path filter: only run when zig/ or markymark-kernels/ files change
- [ ] Verify existing CI jobs are unaffected

## Edge Cases
- Zig download failure: job should fail with clear error, not hang
- Zig version mismatch in setup action: pin exact version 0.15.2
- Cache invalidation: cache key should include zig/build.zig hash
- macOS arm64 runner availability: macos-14 is arm64, verify Zig supports it
- Windows: not in initial matrix, but note as future addition
- Concurrent CI runs: Zig cache must be per-run to avoid conflicts
- Zig build succeeds but cargo build fails: missing link directives

## Anti-patterns
- NO making zig-kernels a required CI check (would block all PRs)
- NO installing Zig from source (use pre-built binaries for speed)
- NO caching without version-keyed cache key (stale cache after Zig upgrade)
- NO running zig-kernels tests in the default cargo test job (feature isolation)
- NO hardcoding Zig download URLs (use setup action or version variable)

## Error Handling
- Zig install fails: job fails immediately with error in logs
- Zig build fails: job fails, cargo steps skipped (fail-fast within job)
- Cargo test fails: job fails with test output in logs
- Cache restore fails: build from scratch (slower but still works)
- Path filter excludes changes: job skips entirely (saves CI minutes)

## Test Specifications (what bug does each test catch?)
- test_ci_zig_install: verifies Zig 0.15.2 is available in CI environment
- test_ci_zig_build_lib: catches build.zig incompatibility with CI's Zig version
- test_ci_zig_tests: catches platform-specific SIMD test failures (x86 vs arm)
- test_ci_cargo_test_with_feature: catches link-time failures in CI environment
- test_ci_clippy_with_feature: catches lint issues not caught locally
- test_ci_cache_correctness: catches stale cache providing wrong library version
- test_ci_path_filter: catches job running unnecessarily on non-Zig changes
- test_ci_default_unaffected: catches zig-kernels job interfering with default build
