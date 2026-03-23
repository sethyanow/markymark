---
id: marky-0rf
title: Rename asm_* exports to zig_* in c_adapter.zig for consistent FFI naming
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #18 Copilot review flagged that c_adapter.zig mixes zig_* prefix (similarity, entities, embeddings) with asm_* prefix (normalize, quantize, dequantize). Before the ABI becomes relied upon, rename:
- asm_normalize_f32_l2 → zig_normalize_f32_l2
- asm_quantize_f32_to_q4_0 → zig_quantize_f32_to_q4_0
- asm_dequantize_q4_0_to_f32 → zig_dequantize_q4_0_to_f32

Must also update any Rust FFI extern declarations in markymark-kernels/src/ that reference these symbols.
