---
id: marky-8s3.9
title: 'Research: VS Code extension with WASM-accelerated markymark'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3.8]
parent: marky-8s3
---



Design a VS Code extension that uses markymark's WASM-compiled Zig kernels for in-browser markdown intelligence. Questions: (1) VS Code web extension architecture (extension host worker), (2) WASM loading and initialization in extension context, (3) Which markymark features work in browser vs need filesystem (LSP needs files, MCP may not), (4) Competitive landscape (markdownlint, markdown-all-in-one, foam), (5) Distribution via VS Code marketplace. Deliverable: docs/research/vscode-extension-design.md. Depends on WASM research findings.

## Design

## Goal
Design a VS Code extension that uses markymark's WASM-compiled Zig kernels for in-browser markdown intelligence. Answer five questions about architecture, WASM loading, feature compatibility, competitive landscape, and distribution. Deliverable: design doc. No production code.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] Design doc at docs/research/vscode-extension-design.md
- [ ] VS Code web extension architecture documented (extension host worker, limitations)
- [ ] WASM loading strategy defined (bundled vs lazy-loaded, initialization sequence)
- [ ] Feature matrix: which markymark features work in browser vs need filesystem
- [ ] Competitive analysis: markdownlint, markdown-all-in-one, foam, dendron compared
- [ ] Distribution path: VS Code marketplace submission requirements
- [ ] MVP scope defined: minimal set of features for first release
- [ ] Architecture diagram: extension host -> WASM runtime -> Zig kernels

## Implementation Checklist
- [ ] Research VS Code web extension API (web worker, restricted APIs)
- [ ] Map markymark features to browser compatibility:
    - heading/link/tag extraction: YES (pure computation)
    - semantic search: MAYBE (needs embedding provider — API call?)
    - LSP: NO (needs file system for workspace indexing)
    - MCP: NO (needs process communication)
- [ ] Research WASM loading in VS Code extensions (wasm-wasi, direct instantiation)
- [ ] Analyze competitive extensions: features, downloads, gaps
- [ ] Define MVP: in-browser heading extraction, link validation, search-symbols
- [ ] Document marketplace requirements: publisher account, manifest, packaging
- [ ] Write architecture diagram (text-based: extension host -> WASM module)
- [ ] Write design doc with findings and recommendation

## Edge Cases
- VS Code desktop vs web: different capabilities (desktop has full Node.js)
- VS Code Server (Remote SSH): has filesystem but runs in a container
- Large workspaces in browser: memory limits for WASM
- Extension size limits: marketplace may have package size limits
- WASM startup latency: cold start time matters for UX

## Anti-patterns
- NO writing extension code (design doc only)
- NO assuming VS Code web extensions have full Node.js API (they don't)
- NO ignoring the competitive landscape (must differentiate)
- NO proposing features that require filesystem in a web extension
- NO designing for desktop-only (web extension is the differentiator)

## Error Handling
- N/A for research task

## Test Specifications (what bug does each test catch?)
- N/A for research task. Success is measured by completeness of design doc answering all 5 questions.
