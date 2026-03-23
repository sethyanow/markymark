---
id: marky-qpy
title: 'env_scan.zig: SIMD .env file key-value extractor'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.7
---



## Design

## Goal
Create zig/src/kernels/formats/env_scan.zig. SIMD scan for KEY=value pairs. Newline scanning to find line boundaries, equals scanning to split key/value. Handle comments (#), empty values, export prefix, quoted values.

## Effort Estimate
2-3 hours

## Success Criteria
- [ ] env_scan.zig compiles and exports marky_scan_env
- [ ] Extracts KEY=value pairs with byte offsets for key and value
- [ ] Handles # comment lines (skips them)
- [ ] Handles empty values (KEY=)
- [ ] Handles export KEY=value prefix
- [ ] cd zig && zig build test passes

## Edge Cases
- Empty input: return 0 results
- KEY without = sign: skip line
- Quoted values: KEY="value with spaces"
- Multi-line values: not supported, document limitation

## Anti-patterns
- NO building a full dotenv parser (extraction only)
- NO heap allocation

## Test Specifications
- test_env_basic: catches incorrect KEY=value splitting
- test_env_empty_value: catches crash on KEY= with no value
- test_env_comments: catches including comment lines
- test_env_export_prefix: catches not stripping export keyword
