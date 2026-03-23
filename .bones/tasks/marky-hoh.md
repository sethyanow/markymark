---
id: marky-hoh
title: Tag and verify first alpha release (v0.1.0-alpha.1)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-1vb, marky-7u7]
parent: marky-peu
---




Tag the first alpha release and verify the CI/CD pipeline produces correct artifacts.

## Deliverables
1. git tag v0.1.0-alpha.1 pushed to origin
2. GitHub Actions release.yml builds all 5 platforms successfully
3. GitHub Release contains: plugin archive + 5 platform binaries
4. Verify plugin archive structure is correct

## Design

## Goal
Tag the first alpha release and verify CI/CD produces correct artifacts.

## Blocked By
- marky-1vb (hooks integrated)
- marky-7u7 (crates.io metadata)

## Implementation Steps

### Step 1: Pre-release verification
\`\`\`bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash markymark-plugin/tests/test_select_binary.sh
bash markymark-plugin/tests/test_hooks.sh
\`\`\`

### Step 2: Update version to alpha
In root Cargo.toml, change:
\`\`\`toml
version = "0.1.0-alpha.1"
\`\`\`
Commit:
\`\`\`bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.1.0-alpha.1"
\`\`\`

### Step 3: Tag and push
\`\`\`bash
git tag v0.1.0-alpha.1
git push origin main
git push origin v0.1.0-alpha.1
\`\`\`

### Step 4: Monitor GitHub Actions
Watch release.yml workflow. Verify:
- All 5 platform builds succeed
- Plugin archive is created
- GitHub Release is created with all artifacts

### Step 5: Verify release artifacts
\`\`\`bash
gh release view v0.1.0-alpha.1
gh release download v0.1.0-alpha.1 --pattern 'markymark-plugin-*.tar.gz' --dir /tmp/verify
tar -tzf /tmp/verify/markymark-plugin-*.tar.gz | head -20
\`\`\`
Verify archive contains: plugin.json, scripts/, bin/ (5 binaries), hooks/, skills/, README.md

## Success Criteria
- [ ] All quality gates pass before tagging
- [ ] git tag v0.1.0-alpha.1 pushed to origin
- [ ] GitHub Actions release.yml completes for all 5 platforms
- [ ] GitHub Release contains plugin archive + 5 platform binaries
- [ ] Plugin archive has correct directory structure
