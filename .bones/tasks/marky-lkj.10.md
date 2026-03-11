---
id: marky-lkj.10
title: Implement JSON5 parser (json5 crate)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


## Context
The marky-lkj epic requires .json5 as a first-class format (requirement #1). The approach section specifies using the json5 crate. Currently parse_structured() returns CoreError::NotImplemented for DocumentKind::Json5, and there's a test explicitly asserting this error.

## Requirements
- Add markymark-parser/src/structured/json5.rs using the json5 crate
- json5 crate handles .jsonc and .json5 formats natively (comments, trailing commas, unquoted keys)
- Produce StructuredAst with Vec<KeyEntry> matching the uniform interface
- Byte-accurate source ranges for all keys/values (json5 crate preserves structure)
- Update parse_structured() dispatch to route Json5 to the new parser
- Update the test_parse_structured_dispatch_json5_unimplemented test to assert success

## Acceptance Criteria
- parse_structured("...", DocumentKind::Json5) returns Ok(StructuredAst)
- Tests: flat, nested, arrays, unquoted keys, trailing commas, comments, position accuracy
- Existing tests pass (zero regression)
- cargo clippy clean
