---
id: marky-6n3
title: 'toml_scan.zig: SIMD TOML table/key extractor'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.7
---



## Design

## Goal
Create zig/src/kernels/formats/toml_scan.zig. SIMD scan for [table] and [[array]] headers, key = value assignments. Handle dotted keys (a.b.c = "value").

## Effort Estimate
4-5 hours

## Success Criteria
- [ ] toml_scan.zig compiles and exports marky_scan_toml
- [ ] Extracts [table] headers with byte offsets
- [ ] Extracts [[array]] headers with byte offsets
- [ ] Extracts key = value pairs with key and value offsets
- [ ] Handles dotted keys: a.b.c = "value"
- [ ] cd zig && zig build test passes

## Edge Cases
- Inline tables: { a = 1, b = 2 } — extract keys, not a full parser
- Multi-line strings: """ blocks — skip interior content
- Dotted keys with quoted segments: "a.b".c = "value"

## Test Specifications
- test_toml_tables: catches missing [table] detection
- test_toml_dotted_keys: catches incorrect a.b.c handling
- test_toml_array_tables: catches [[array]] not detected
- test_toml_inline_tables: catches crash on { } syntax
