---
id: marky-0mr.5
title: 'PR#39 review: FFI boundary safety'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Fix four issues at the FFI boundary between Zig and Rust (exports.zig + md4c.rs + parser.zig):

**T1-3: blob_size computed as usize but packed as u32 — overflow for large inputs (exports.zig:187)**
For documents with many headings/links, summed text length can exceed u32::MAX, silently wrapping blob_offset and corrupting writes. Fix: add guard after computing blob_size: if (blob_size > std.math.maxInt(u32)) return error code.

**T2-11: Silent UTF-8 fallback .unwrap_or("") hides data corruption (md4c.rs:151-155)**
Invalid UTF-8 in the FFI blob becomes an empty string silently. This could mask corruption bugs in the Zig packing logic. Fix: at minimum add debug_assert! or tracing::warn! on decode failure; better to propagate as KernelError::InternalError(-100).

**T3-2: Missing defensive bounds check on FFI blob pointer (exports.zig:211)**
Add defensive check that blob pointer arithmetic stays within allocated bounds.

**T3-3: Parser.init truncates >4GB docs via @intCast (parser.zig:103)**
Parser.init casts text.len to OFF (u32) via @intCast — truncation for >4GB input with no guard. Fix: validate that text.len fits into OFF before casting, return error if not.

All four are the same class of bug: u32 overflow at the FFI boundary. Fix them consistently.

Source: PR #39 review — CodeRabbit (Major) + Copilot
