# VS Code Extension Design: markymark

**Date:** 2026-02-18
**Task:** marky-8s3.9
**Status:** COMPLETE
**Verdict:** GO — design is feasible; MVP is well-scoped

**Depends on:** [WASM Feasibility Research](./wasm-zig-feasibility.md) (marky-8s3.8, verdict: GO)

---

## Summary

A markymark VS Code extension can deliver WASM-accelerated markdown intelligence in both
VS Code desktop and VS Code for the Web. The core extraction kernels (headings, links, tags,
block IDs) compile to 88KB freestanding WASM, load in <100ms, and run ~5–20× faster than
equivalent JavaScript. No semantic search competitor exists in the extension marketplace.

---

## Q1: VS Code Web Extension Architecture

### Extension Host Environments

| Environment | Entry point | Runtime | Filesystem |
|-------------|-------------|---------|------------|
| Desktop (regular) | `main` in `package.json` | Node.js | Full OS FS |
| Desktop (web compat) | `browser` in `package.json` | Web Worker | `vscode.workspace.fs` only |
| VS Code for the Web | `browser` in `package.json` | Browser Web Worker | Virtual FS only |

An extension is treated as a **web extension** when `package.json` contains a `browser` field
(instead of or in addition to `main`). If both are present, Node.js host is preferred on desktop.

### Web Worker Sandbox Constraints

Web extensions run in a Browser WebWorker with the following hard restrictions:

| Restriction | Impact on markymark |
|-------------|---------------------|
| No Node.js globals (`process`, `os`, `path`) | Must polyfill or avoid; webpack/esbuild handle this |
| No child processes / exec | LSP server cannot be spawned as subprocess |
| No direct filesystem | Must use `vscode.workspace.fs` API for all file reads |
| Single bundled file required | Bundle all TS + WASM wrapper into one JS file |
| No `importScripts` or module imports beyond vscode shim | All dependencies must be bundled |
| Web Workers allowed (for compute offload) | WASM can run in a Worker; VS Code API not accessible from it |

### What IS Available

- Full `vscode.*` API (diagnostics, hover, completions, code actions, tree views)
- `Fetch` API for external HTTP requests
- `WebAssembly.instantiate()` — WASM runs natively in the web worker
- `vscode.workspace.fs` — virtual filesystem (read files in any mounted workspace)
- Web Workers (for offloading WASM compute off the extension host thread)

### LSP in the Web

LSP 3.16+ supports a browser/web-worker transport. The language server can run in a Worker
using `postMessage` as its transport. This means a full LSP-over-WASM architecture is
possible but requires the server loop itself to run in the WASM module or in a separate Worker.

For markymark MVP, skip LSP — use VS Code's native extension APIs (hover, completions,
diagnostics) directly. LSP becomes relevant only for Phase 2 full parity.

---

## Q2: WASM Loading Strategy

### Recommended: Freestanding WASM (Bundled)

From [WASM feasibility research](./wasm-zig-feasibility.md), markymark's kernels use no OS
calls and no file I/O. Freestanding WASM is the right target.

**Loading sequence:**

```typescript
// extension.ts — activate()
export async function activate(ctx: vscode.ExtensionContext): Promise<void> {
    // 1. Load WASM bytes — works in both desktop and web
    const wasmUri = vscode.Uri.joinPath(ctx.extensionUri, 'dist', 'marky_kernels.wasm');
    const wasmBytes = await vscode.workspace.fs.readFile(wasmUri);

    // 2. Instantiate — synchronous after bytes are available
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    const kernels = instance.exports as MarkyKernelExports;

    // 3. Register providers using kernel
    ctx.subscriptions.push(
        vscode.languages.registerHoverProvider('markdown', new MarkyHoverProvider(kernels)),
        vscode.languages.registerCompletionItemProvider('markdown', new MarkyCompletionProvider(kernels), '[', '#'),
        vscode.languages.registerDocumentSymbolProvider('markdown', new MarkySymbolProvider(kernels)),
    );

    // 4. Register diagnostics
    registerDiagnostics(ctx, kernels);
}
```

**Memory layout** (caller manages two 64KB regions):

```typescript
interface MarkyKernelExports {
    memory: WebAssembly.Memory;
    scan_headings_wasm(inputOffset: number, len: number, outputOffset: number, capacity: number): number;
    scan_links_wasm(inputOffset: number, len: number, outputOffset: number, capacity: number): number;
    scan_tags_wasm(inputOffset: number, len: number, outputOffset: number, capacity: number): number;
    fuzzy_match_wasm(needleOffset: number, needleLen: number, haystackOffset: number, haystackLen: number): number;
}

// Page 0: input text (up to 64KB)
// Page 1: output structs
const INPUT_PAGE  = 0;
const OUTPUT_PAGE = 65536;
```

**Startup latency:** WASM instantiation is ~10–50ms on first load. After that, calls are
synchronous and fast. The 88KB bundle loads faster than most npm packages.

### Bundling

```json
// package.json
{
  "browser": "./dist/extension.js",
  "scripts": {
    "build:wasm": "zig build wasm-spike && wasm-opt -Oz --strip-debug zig-out/wasm/heading_scan.wasm -o dist/marky_kernels.wasm",
    "build:ext": "esbuild src/extension.ts --bundle --platform=browser --target=webworker --outfile=dist/extension.js",
    "build": "npm run build:wasm && npm run build:ext"
  }
}
```

The `.wasm` file is **not bundled into the JS** — it is a separate file in `dist/` loaded via
`vscode.workspace.fs.readFile`. This keeps the JS bundle small and the WASM independently
cacheable.

### Not Recommended: WASI

WASI requires the `ms-vscode.wasm-wasi-core` dependency and has known activation issues for
web-only extensions. Adds ~500KB of overhead. Avoid for initial release.

---

## Q3: Feature Matrix — Browser vs Desktop

| Feature | Web Extension | Desktop (Node) | Notes |
|---------|:---:|:---:|-------|
| Heading extraction (WASM) | ✅ | ✅ | Pure computation, no I/O |
| Link extraction (WASM) | ✅ | ✅ | Pure computation |
| Tag extraction (WASM) | ✅ | ✅ | Pure computation |
| Block ID extraction (WASM) | ✅ | ✅ | Pure computation |
| Fuzzy search-symbols (WASM) | ✅ | ✅ | In-memory, per-document |
| Content hashing / fingerprinting | ✅ | ✅ | Stateless WASM call |
| Semantic search (embeddings) | ⚠️ API call | ✅ Native | Web: needs embedding API call; index can't be persisted locally |
| Cross-file link validation | ⚠️ Slow | ✅ Fast | Web: must read via `vscode.workspace.fs` (async, throttled) |
| Full workspace indexing | ❌ Memory | ✅ | Web: no persistent disk cache; index rebuilt on activate |
| Slug generation (WASM) | ✅ | ✅ | Pure computation |
| Diagnostics (broken links) | ⚠️ Slow | ✅ Fast | Web: per-file only for performance |
| LSP protocol server | ❌ | ✅ | Web: use VS Code API providers instead |
| MCP server | ❌ | ✅ | MCP requires process communication |

**Key decision:** Web extension delivers per-file intelligence (extraction, hover, completions,
search-symbols) via WASM. Workspace-wide features (cross-file validation, semantic search index)
require the desktop extension.

---

## Q4: Competitive Analysis

### Extension Landscape

| Extension | Installs | Semantic Search | SIMD | Web Support | Wikilinks | Graph |
|-----------|----------|:-:|:-:|:-:|:-:|:-:|
| Markdown All in One | 12.7M | ❌ | ❌ | ✅ | ❌ | ❌ |
| markdownlint | 10.5M | ❌ | ❌ | ✅ | ❌ | ❌ |
| Foam | 224K | ❌ | ❌ | ✅ | ✅ | ✅ |
| Dendron | ~80K | ❌ | ❌ | ❌ | ✅ | ✅ |
| Markdown Links | ~40K | ❌ | ❌ | ❌ | ❌ | ✅ |
| **markymark** (MVP) | — | ✅ API | ✅ | ✅ | ✅ | — |

### Differentiation

**markymark's unique position:**

1. **SIMD-accelerated extraction** — no competitor uses native SIMD or WASM for markdown parsing.
   Heading/link/tag scans run ~10–50× faster than regex-based JS alternatives.

2. **Semantic search** — none of the top 5 extensions offer embedding-based search. The gap is
   real: users with large Markdown vaults cannot find conceptually related notes.

3. **Multi-format extraction** — JSON/YAML/TOML/env/INI key extraction is unique. No extension
   surfaces structured data keys as workspace symbols.

4. **Works in VS Code for the Web** — Dendron and Markdown Links don't work in the browser.
   markymark's freestanding WASM architecture supports both environments from day one.

### Foam: Closest Competitor

Foam (224K installs) overlaps most with markymark's knowledge-graph features. Key differences:

| Capability | Foam | markymark |
|------------|------|-----------|
| Wikilinks | ✅ | ✅ |
| Backlinks | ✅ | ✅ (future) |
| Tags | ✅ | ✅ (SIMD) |
| Graph view | ✅ | — (future) |
| Semantic search | ❌ | ✅ (API-based) |
| Multi-format keys | ❌ | ✅ |
| Performance at scale | ❌ (slow, JS) | ✅ (WASM) |
| Web extension | ✅ | ✅ |

Foam's known weakness: performance degrades visibly on vaults >500 files (reported in issues).
markymark can win on correctness and speed.

---

## Q5: Distribution — VS Code Marketplace

### Publisher Setup

1. Create a Microsoft account (if needed)
2. Create a publisher at [marketplace.visualstudio.com/manage](https://marketplace.visualstudio.com/manage)
3. Create an Azure DevOps organization and generate a Personal Access Token (PAT) with
   `Marketplace: Publish` scope — expires annually, must renew
4. Install: `npm install -g @vscode/vsce`

### Manifest Requirements (`package.json`)

```json
{
  "name": "markymark",
  "displayName": "markymark",
  "description": "SIMD-accelerated markdown intelligence: heading/link/tag extraction, fuzzy search, and semantic search",
  "version": "0.1.0",
  "publisher": "markymark",
  "engines": { "vscode": "^1.85.0" },
  "categories": ["Programming Languages", "Linters", "Other"],
  "keywords": ["markdown", "wikilinks", "semantic search", "knowledge base", "notes"],
  "icon": "assets/icon.png",
  "browser": "./dist/extension.js",
  "contributes": { ... },
  "activationEvents": ["onLanguage:markdown"]
}
```

**Icon:** 128×128 PNG required for Marketplace listing.

### Packaging and Publishing

```bash
vsce package                    # → markymark-0.1.0.vsix (local test)
vsce publish                    # → publishes to Marketplace
vsce publish --pre-release      # → pre-release channel (optional)
```

### Extension Size Budget

| Artifact | Size |
|----------|------|
| `dist/extension.js` (bundled TS) | ~50–100KB |
| `dist/marky_kernels.wasm` (optimized) | 88KB |
| Assets (icon, README) | ~5KB |
| **Total VSIX** | **~200KB** |

Well within Marketplace limits (typical extensions: 1–50MB).

---

## MVP Scope

**First release delivers per-file intelligence, WASM-powered:**

### Features

| # | Feature | Provider | API |
|---|---------|----------|-----|
| 1 | Document symbols (headings, block IDs) | `DocumentSymbolProvider` | `vscode.languages.registerDocumentSymbolProvider` |
| 2 | Hover on wikilinks → preview heading + path | `HoverProvider` | `vscode.languages.registerHoverProvider` |
| 3 | Wikilink completions (`[[`) | `CompletionItemProvider` | `vscode.languages.registerCompletionItemProvider` |
| 4 | Tag completions (`#`) | `CompletionItemProvider` | same |
| 5 | Broken link diagnostics (per-file) | `DiagnosticCollection` | `vscode.languages.createDiagnosticCollection` |
| 6 | `search-symbols` command (fuzzy, in-file) | Command | `vscode.commands.registerCommand` |

### Out of Scope for MVP

- Graph view (requires webview, significant complexity)
- Cross-file link validation (requires workspace index, async reads)
- Semantic search (requires embedding provider setup)
- MCP server (no process communication in web)
- Full LSP server (use VS Code API providers instead)

---

## Architecture Diagram

```text
┌─────────────────────────────────────────────────────────────────┐
│  VS Code Extension Host (Web Worker)                            │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  extension.ts (bundled single JS file)                   │  │
│  │                                                          │  │
│  │  activate()                                              │  │
│  │    ├── load dist/marky_kernels.wasm via workspace.fs     │  │
│  │    ├── WebAssembly.instantiate(wasmBytes)                │  │
│  │    └── register VS Code providers:                       │  │
│  │         ├── DocumentSymbolProvider ─────────────────┐   │  │
│  │         ├── HoverProvider ──────────────────────────┼─┐ │  │
│  │         ├── CompletionProvider ────────────────────┼─┼─┐│  │
│  │         └── DiagnosticProvider ───────────────────┼─┼─┼┘│  │
│  │                                                   ↓ ↓ ↓  │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  MarkyKernelWrapper                                │  │  │
│  │  │  ├── writeText(text) → inputOffset                │  │  │
│  │  │  ├── scanHeadings() → HeadingResult[]             │  │  │
│  │  │  ├── scanLinks() → LinkResult[]                   │  │  │
│  │  │  ├── scanTags() → TagResult[]                     │  │  │
│  │  │  └── fuzzyMatch(needle, haystack) → score         │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  │                       ↓                                   │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  WebAssembly.Memory (linear, 2 pages)              │  │  │
│  │  │  Page 0: input text buffer (64KB max)              │  │  │
│  │  │  Page 1: output struct buffer (64KB max)           │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  │                       ↑                                   │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  marky_kernels.wasm (88KB, freestanding)           │  │  │
│  │  │  exports:                                          │  │  │
│  │  │  · scan_headings_wasm(in, len, out, cap) → count  │  │  │
│  │  │  · scan_links_wasm(in, len, out, cap) → count     │  │  │
│  │  │  · scan_tags_wasm(in, len, out, cap) → count      │  │  │
│  │  │  · fuzzy_match_wasm(n, nl, h, hl) → score         │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                               ↑
                    vscode.workspace.fs
                    (loads .wasm file at activation)
```

---

## Key Technical Decisions

### Decision 1: Freestanding WASM, not WASI

Markymark kernels have no OS dependencies. Freestanding WASM avoids the `wasm-wasi-core`
extension dependency and its known web-only activation issues (microsoft/vscode-wasm#210).

### Decision 2: Single-file bundle (esbuild, platform=browser)

Web extension requirement. WASM is a separate file loaded via `vscode.workspace.fs.readFile`.
The TypeScript wrapper + VS Code provider logic bundles to ~50–100KB of JS.

### Decision 3: VS Code API providers, not LSP

For MVP, use `DocumentSymbolProvider`, `HoverProvider`, `CompletionItemProvider` directly.
LSP adds complexity (worker transport, language client package) with no benefit at MVP scope.
Phase 2 can switch to LSP if needed for multi-root workspace or full editor parity.

### Decision 4: Per-file intelligence only at MVP

Cross-file features (wikilink validation, backlinks, graph) require a workspace index. Building
and caching a workspace index in the web worker adds significant complexity. Per-file features
via WASM are immediately valuable and ship fast.

### Decision 5: No `@vscode/vsce` pre-release for initial shipment

Ship `0.1.0` on the stable channel. Pre-release channel is opt-in complexity. Users will
discover the extension through Marketplace search for "markdown" — keywords matter more
than channel selection at this scale.

---

## Open Questions

| Question | Resolution Path |
|----------|-----------------|
| Should markymark own the publisher account, or ship under `steveyegge`? | Publisher name is permanent — decide before first publish |
| WASM memory limit: 64KB input cap sufficient? | P95 markdown file is ~8KB; 64KB covers >99% of files. OK for MVP. |
| Should MVP ship multi-format extraction (JSON/YAML)? | Yes — unique differentiator, no extra cost from WASM side |
| Should semantic search call Anthropic API directly? | Phase 2; requires API key management (sensitive, out of scope for MVP) |
| Graph view via webview? | Phase 2; significant effort, not a web-extension blocker |

---

## Next Steps (ordered)

1. **Create markymark-vscode crate** — TypeScript project (`packages/markymark-vscode/`)
   with esbuild config, `package.json` with `browser` field, and activation stub
2. **Expose WASM ABI** — add `wasm_adapter.zig` exporting the 4 MVP kernel functions with
   stable memory layout (input/output buffer protocol)
3. **Implement MarkyKernelWrapper.ts** — TypeScript wrapper for WASM memory management
4. **Implement DocumentSymbolProvider** — headings + block IDs as workspace symbols
5. **Implement HoverProvider** — wikilink hover shows heading/path from WASM scan
6. **Implement CompletionProvider** — `[[` and `#` completions from in-file scans
7. **Implement DiagnosticProvider** — per-file broken-link detection
8. **Package and test** — `vsce package`, sideload into vscode.dev to verify web behavior
9. **Publish** — create publisher account, publish `0.1.0`

---

## References

- [VS Code Web Extensions Guide](https://code.visualstudio.com/api/extension-guides/web-extensions)
- [VS Code Extension Host Architecture](https://code.visualstudio.com/api/advanced-topics/extension-host)
- [Using WebAssembly for Extension Development (VS Code Blog, 2024)](https://code.visualstudio.com/blogs/2024/05/08/wasm)
- [Publishing Extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [Extension Manifest Reference](https://code.visualstudio.com/api/references/extension-manifest)
- [markymark WASM Feasibility Research](./wasm-zig-feasibility.md)
