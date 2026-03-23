---
id: marky-8pt
title: Overhaul plugin binary distribution to CI pre-packaging
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-peu]
---




## Goal
Replace 'binaries in git' model with CI pre-packaged per-platform plugin archives.

## Requirements
1. CI release workflow builds 5 platform targets: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-pc-windows-msvc
2. Each platform archive contains: complete plugin directory + correct platform binary in bin/
3. GitHub Releases gets 5 per-platform plugin archives + 5 standalone binaries
4. select-binary.sh updated to use bundled binary (no download, no git binary)
5. Remove all committed binaries from git history (or at minimum .gitignore bin/ except .gitkeep)
6. Plugin archive naming: markymark-plugin-{version}-{target}.tar.gz (or .zip for Windows)

## Design
### CI Matrix
```yaml
strategy:
  matrix:
    include:
      - target: aarch64-apple-darwin
        os: macos-latest
      - target: x86_64-apple-darwin
        os: macos-latest
      - target: x86_64-unknown-linux-gnu
        os: ubuntu-latest
      - target: aarch64-unknown-linux-gnu
        os: ubuntu-latest
      - target: x86_64-pc-windows-msvc
        os: windows-latest
```

### Archive Structure
```
markymark-plugin-0.1.0-aarch64-apple-darwin.tar.gz
└── markymark-plugin/
    ├── .claude-plugin/plugin.json
    ├── .lsp.json
    ├── .mcp.json
    ├── hooks/
    ├── skills/
    ├── scripts/select-binary.sh
    └── bin/markymark  (platform-specific)
```

### select-binary.sh Changes
- Detect platform (uname -s/-m)
- Expect binary at ${CLAUDE_PLUGIN_ROOT}/bin/markymark (already there from archive)
- Error with install instructions if binary missing (points to GitHub Releases)

## Success Criteria
- [ ] CI builds and packages all 5 platform archives on tag push
- [ ] Each archive contains working platform binary
- [ ] select-binary.sh works with bundled binary
- [ ] No binaries in git repo (bin/ gitignored except .gitkeep)
- [ ] GitHub Release has all 5 archives + 5 standalone binaries
