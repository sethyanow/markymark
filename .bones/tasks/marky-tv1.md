---
id: marky-tv1
title: 'Task 6: VSCode extension — spawn markymark --lsp as stdio child'
status: closed
type: feature
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-9mo
---


## Design

## Goal
Ship a marketplace-ready VSCode extension that spawns markymark --lsp as a stdio language server, activating on .md and .mdx files.

## Context
All 5 data/MCP tasks are complete (Tasks 1-5 committed to feature/mark-ides).
This is the final requirement of epic marky-9mo.

## Scope
New directory: markymark-vscode/
- package.json (activation events, LanguageClient, publisher metadata)
- src/extension.ts (spawn markymark binary, configure LanguageClient)
- Binary selection: detect OS/arch at runtime, pick correct markymark binary
- .vscodeignore, README.md for marketplace listing

## Anti-Patterns (from epic)
- NO Obsidian vault config awareness
- NO write/editing tools — read-only LSP intelligence only

## Implementation Steps

### Step 1: Scaffold the extension project
mkdir markymark-vscode && cd markymark-vscode
Create: package.json with:
  - publisher, name, version fields
  - engines: {vscode: "^1.85.0"}
  - activationEvents: ["onLanguage:markdown", "onLanguage:mdx"]
  - contributes.languages for .mdx if not already registered
  - dependencies: vscode-languageclient ^9.0.0
  - devDependencies: @types/vscode, @vscode/vsce, typescript, esbuild

### Step 2: Write extension.ts (main entry point)
export function activate(context: vscode.ExtensionContext) {
  const binaryPath = getBinaryPath(context);
  const serverOptions: ServerOptions = {
    command: binaryPath,
    args: ["--lsp"],
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'markdown' },
      { scheme: 'file', language: 'mdx' },
    ],
  };
  const client = new LanguageClient('markymark', 'Markymark', serverOptions, clientOptions);
  context.subscriptions.push(client.start());
}

### Step 3: getBinaryPath — cross-platform binary selection
function getBinaryPath(context): string {
  const platform = process.platform;  // 'darwin' | 'linux' | 'win32'
  const arch = process.arch;          // 'x64' | 'arm64'
  const binaries = {
    'darwin-arm64': 'markymark-aarch64-apple-darwin',
    'darwin-x64':   'markymark-x86_64-apple-darwin',
    'linux-x64':    'markymark-x86_64-unknown-linux-gnu',
    'win32-x64':    'markymark-x86_64-pc-windows-msvc.exe',
  };
  const key = platform + '-' + arch;
  const name = binaries[key] ?? 'markymark';  // fallback for PATH
  return path.join(context.extensionPath, 'bin', name);
}

### Step 4: Build setup
tsconfig.json with outDir: dist/
esbuild.js or package.json scripts:
  compile: esbuild src/extension.ts --bundle --platform=node --external:vscode --outfile=dist/extension.js
  package: vsce package

### Step 5: .vscodeignore and README.md
.vscodeignore: exclude src/, node_modules/ from VSIX
README.md: usage instructions, what the extension provides

### Step 6: Tests
Since this is a TypeScript shell extension (no application logic beyond spawning):
- Unit test getBinaryPath for all platform/arch combinations (6 cases)
- Test that unknown platform falls back to 'markymark'
- Use vitest (no vscode dependency in unit tests)

### Step 7: Verify end-to-end (manual)
cargo build --release
cp target/release/markymark markymark-vscode/bin/markymark (for local platform)
vsce package
Install VSIX in VSCode
Open a .md file, verify LSP hover/completion working

## Success Criteria
- [ ] markymark-vscode/ directory created with package.json, src/extension.ts, tsconfig.json
- [ ] getBinaryPath covers darwin-arm64, darwin-x64, linux-x64, win32-x64 + fallback
- [ ] LanguageClient spawns markymark --lsp on markdown/mdx activation
- [ ] Unit tests for getBinaryPath: 5 cases passing
- [ ] vsce package succeeds (VSIX created)
- [ ] Local install test: LSP hover/completion functional on .md file
- [ ] cargo clippy --workspace --all-targets clean (no Rust changes, but verify)
- [ ] All existing tests pass (no regressions from any accidental changes)
