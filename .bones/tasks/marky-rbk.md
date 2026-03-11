---
id: marky-rbk
title: 'yaml_keys.zig: SIMD YAML key hierarchy extractor'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.7
---



## Design

## Goal
Create zig/src/kernels/formats/yaml_keys.zig. SIMD colon detection and indentation-level tracking for YAML key hierarchy extraction. Build dot-separated key paths from indentation structure.

## Effort Estimate
4-5 hours

## Success Criteria
- [ ] yaml_keys.zig compiles and exports marky_scan_yaml_keys
- [ ] Extracts key paths from indentation hierarchy (e.g., "root.child.key")
- [ ] Handles both spaces and tabs for indentation (spaces preferred)
- [ ] Skips comment lines (# comments)
- [ ] cd zig && zig build test passes

## Edge Cases
- Mixed tabs/spaces: handle gracefully (YAML spec says spaces)
- Multi-line values (| and > blocks): skip interior content
- Anchors and aliases (&name, *name): skip, not extracted
- Flow style (inline JSON-like): treat as opaque value

## Test Specifications
- test_yaml_basic_keys: catches incorrect colon detection
- test_yaml_indentation: catches wrong hierarchy from indentation
- test_yaml_mixed_indent: catches crash on tab/space mixing
- test_yaml_comments: catches including comments as keys
- test_yaml_multiline: catches scanning inside block scalar content
