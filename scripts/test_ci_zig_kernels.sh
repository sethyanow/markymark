#!/usr/bin/env bash
# Test: validates ci.yml has a properly structured zig-kernels job
# Run: bash scripts/test_ci_zig_kernels.sh
set -euo pipefail

CI_FILE=".github/workflows/ci.yml"
PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

echo "=== CI zig-kernels job validation ==="
echo ""

# 1. ci.yml exists and is valid YAML
if [ ! -f "$CI_FILE" ]; then
  fail "ci.yml does not exist"
  echo "Results: $PASS passed, $FAIL failed"
  exit 1
fi
pass "ci.yml exists"

# Use python3 to parse YAML (available on macOS + Ubuntu CI runners)
PARSE_RESULT=$(python3 -c "
import yaml, sys, json
with open('$CI_FILE') as f:
    data = yaml.safe_load(f)
json.dump(data, sys.stdout)
" 2>&1) || { fail "ci.yml is not valid YAML"; echo "Results: $PASS passed, $FAIL failed"; exit 1; }
pass "ci.yml is valid YAML"

# Helper: query parsed YAML via python
query() {
  python3 -c "
import yaml, sys
with open('$CI_FILE') as f:
    data = yaml.safe_load(f)
$1
"
}

# 2. zig-kernels job exists
query "
jobs = data.get('jobs', {})
if 'zig-kernels' not in jobs:
    sys.exit(1)
" && pass "zig-kernels job exists" || fail "zig-kernels job does not exist"

# 3. Platform matrix includes ubuntu-latest and macos-14
query "
job = data.get('jobs', {}).get('zig-kernels', {})
strategy = job.get('strategy', {})
matrix = strategy.get('matrix', {})
os_list = matrix.get('os', [])
if 'ubuntu-latest' not in os_list:
    print('missing ubuntu-latest in matrix.os')
    sys.exit(1)
if 'macos-14' not in os_list:
    print('missing macos-14 in matrix.os')
    sys.exit(1)
" && pass "platform matrix has ubuntu-latest and macos-14" || fail "platform matrix missing required platforms"

# 4. Zig 0.15.2 installation step exists
query "
job = data.get('jobs', {}).get('zig-kernels', {})
steps = job.get('steps', [])
zig_steps = [s for s in steps if s.get('uses', '').startswith('mlugg/setup-zig')]
if not zig_steps:
    sys.exit(1)
version = zig_steps[0].get('with', {}).get('version', '')
if version != '0.15.2':
    print(f'expected version 0.15.2, got {version}')
    sys.exit(1)
" && pass "Zig 0.15.2 installation step present" || fail "Zig installation step missing or wrong version"

# 5. zig build test step exists
query "
job = data.get('jobs', {}).get('zig-kernels', {})
steps = job.get('steps', [])
has_zig_test = any('zig build test' in str(s.get('run', '')) for s in steps)
if not has_zig_test:
    sys.exit(1)
" && pass "'zig build test' step present" || fail "'zig build test' step missing"

# 6. cargo test --features zig-kernels step exists
query "
job = data.get('jobs', {}).get('zig-kernels', {})
steps = job.get('steps', [])
has_cargo_test = any('cargo test' in str(s.get('run', '')) and 'zig-kernels' in str(s.get('run', '')) for s in steps)
if not has_cargo_test:
    sys.exit(1)
" && pass "'cargo test --features zig-kernels' step present" || fail "'cargo test --features zig-kernels' step missing"

# 7. cargo clippy --features zig-kernels step exists
query "
job = data.get('jobs', {}).get('zig-kernels', {})
steps = job.get('steps', [])
has_clippy = any('cargo clippy' in str(s.get('run', '')) and 'zig-kernels' in str(s.get('run', '')) for s in steps)
if not has_clippy:
    sys.exit(1)
" && pass "'cargo clippy --features zig-kernels' step present" || fail "'cargo clippy --features zig-kernels' step missing"

# 8. Job is optional (continue-on-error: true)
query "
job = data.get('jobs', {}).get('zig-kernels', {})
if not job.get('continue-on-error', False):
    sys.exit(1)
" && pass "job is optional (continue-on-error: true)" || fail "job is not optional"

# 9. Path filter present (zig/ or markymark-kernels/)
query "
import json
# Check for paths filter in the on section or job-level if condition
raw = json.dumps(data)
if 'zig/' not in raw and 'markymark-kernels/' not in raw:
    sys.exit(1)
" && pass "path filter references zig/ or markymark-kernels/" || fail "no path filter for zig/markymark-kernels changes"

# 10. Default build-and-test job still exists and unchanged structure
query "
jobs = data.get('jobs', {})
bat = jobs.get('build-and-test', {})
if not bat:
    sys.exit(1)
steps = bat.get('steps', [])
# Must have: checkout, rust, zig, cache, fmt, clippy, build, test
step_count = len(steps)
if step_count < 8:
    print(f'build-and-test has only {step_count} steps, expected >= 8')
    sys.exit(1)
" && pass "build-and-test job exists with >= 8 steps" || fail "build-and-test job missing or degraded"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
