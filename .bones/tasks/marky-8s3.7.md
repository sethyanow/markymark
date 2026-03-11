---
id: marky-8s3.7
title: Implement multi-format extractors for JSON/YAML/TOML/env/ini
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3
---








Create zig/src/kernels/formats/ directory with extractors for non-markdown formats. Ties into marky-lkj (multi-format document support) epic. Extractors: env_scan.zig (KEY=value pairs, SIMD newline+equals scanning), ini_scan.zig ([section] headers + key=value, SIMD bracket scanning), toml_scan.zig (TOML section headers [table] and key extraction), json_keys.zig (SIMD brace/colon scanning for key path extraction, simdjson-inspired), yaml_keys.zig (SIMD indent+colon scanning for key hierarchy). Each exports a C ABI function. Tests per format. These feed into markymark-index as alternative ScanBackend implementations for non-markdown files.

## Design

## Goal
Coordinator: Implement multi-format extractors. Broken into 5 subtasks (one per format, all parallelizable):
(a) marky-qpy: env_scan.zig — KEY=value pairs (2-3h)
(b) marky-g2g: ini_scan.zig — [section] + key=value (3-4h)
(c) marky-6n3: toml_scan.zig — [table] + key assignments (4-5h)
(d) marky-4g8: json_keys.zig — simdjson-inspired key paths (5-6h)
(e) marky-rbk: yaml_keys.zig — indent-based hierarchy (4-5h)

## Success Criteria
- [ ] All 5 child subtasks closed
- [ ] All format extractors export C ABI functions
- [ ] cd zig && zig build test passes for all formats
