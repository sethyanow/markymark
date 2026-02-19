// Provide ___chkstk_ms for Windows x86_64. Zig's compiler-rt does not bundle
// this symbol into static libraries (ziglang/zig#6817), but LLVM emits calls
// to it for any function whose stack frame exceeds 4KB. The implementation
// walks down the stack one page at a time, touching the guard page so Windows
// commits it. All registers are preserved (custom ABI).
//
// Reference: https://nullprogram.com/blog/2024/02/05/

const builtin = @import("builtin");

comptime {
    if (builtin.os.tag == .windows and builtin.cpu.arch == .x86_64) {
        asm (
            \\.globl ___chkstk_ms
            \\___chkstk_ms:
            \\    push %rax
            \\    push %rcx
            \\    neg  %rax
            \\    add  %rsp, %rax
            \\    mov  %gs:0x10, %rcx
            \\    jmp  1f
            \\0:
            \\    sub  $0x1000, %rcx
            \\    test %eax, (%rcx)
            \\1:
            \\    cmp  %rax, %rcx
            \\    ja   0b
            \\    pop  %rcx
            \\    pop  %rax
            \\    ret
        );
    }
}
