---
id: marky-hwc
title: '[EPIC] Knowledge Tool Plugins: Obsidian and Logseq'
status: open
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ix3]
---


Ship markymark to knowledge management tools. Obsidian: thin plugin + sidecar (can spawn child processes via Node/Electron). Logseq: cannot spawn subprocesses, needs HTTP transport or WASM lite fallback. HTTP transport in markymark unlocks Logseq plus web UIs and remote agents. Depends on ix3 for feature differentiation.
