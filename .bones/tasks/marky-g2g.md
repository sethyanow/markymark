---
id: marky-g2g
title: 'ini_scan.zig: SIMD INI file section/key extractor'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.7
---



## Design

## Goal
Create zig/src/kernels/formats/ini_scan.zig. SIMD scan for [section] headers and key=value pairs. Bracket scanning for sections, equals for keys, semicolon and hash for comments.

## Effort Estimate
3-4 hours

## Success Criteria
- [ ] ini_scan.zig compiles and exports marky_scan_ini
- [ ] Extracts [section] headers with byte offsets
- [ ] Extracts key=value pairs associated with their section
- [ ] Handles ; and # comment lines
- [ ] cd zig && zig build test passes

## Edge Cases
- Keys before first section: belong to implicit global section
- Duplicate sections: keep both occurrences
- Comments after values: key=value ; comment

## Test Specifications
- test_ini_sections: catches missing section detection
- test_ini_keys_under_section: catches key not associated with section
- test_ini_comments: catches including comments as keys
- test_ini_global_keys: catches keys before first section being dropped
