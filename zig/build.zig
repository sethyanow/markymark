const std = @import("std");

// Compile-time version gate: reject Zig < 0.15.2
comptime {
    const v = @import("builtin").zig_version;
    const meets_minimum = v.major > 0 or
        (v.major == 0 and v.minor > 15) or
        (v.major == 0 and v.minor == 15 and v.patch >= 2);
    if (!meets_minimum) {
        @compileError("Zig 0.15.2+ required. This build file uses 0.15.x APIs (addLibrary, createModule).");
    }
}

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Root module for the static library
    // PIC required: Rust links with -fPIC on Linux x86_64; without it,
    // rust-lld rejects R_X86_64_32 relocations from Zig's debug.zig.
    const root_mod = b.createModule(.{
        .root_source_file = b.path("src/c_adapter.zig"),
        .target = target,
        .optimize = optimize,
        .pic = true,
        // Disable stack checking so the static library doesn't reference
        // ___zig_probe_stack / ___chkstk_ms (compiler-rt symbols not
        // bundled into static libs). See ziglang/zig#6817.
        .stack_check = false,
        // Strip debug info so Zig's panic/debug infrastructure (panicExtra
        // etc.) is removed. On Windows/MSVC those functions have large stack
        // frames that emit ___chkstk_ms even with stack_check=false.
        .strip = true,
        // Disable unwind tables — not needed for a static FFI library and
        // avoids MSVC exception-handling references to compiler-rt.
        .unwind_tables = .none,
    });

    // Static library artifact (libmarky_kernels.a)
    const lib = b.addLibrary(.{
        .name = "marky_kernels",
        .linkage = .static,
        .root_module = root_mod,
    });

    // Install the library artifact to zig-out/lib/
    const install = b.addInstallArtifact(lib, .{});

    // Convenience step: zig build lib
    const lib_step = b.step("lib", "Build libmarky_kernels.a static library");
    lib_step.dependOn(&install.step);

    // ── WASM Spike (research only, not production) ──────────────────────────
    // Build: zig build wasm-spike
    // Output: zig-out/wasm/heading_scan.wasm
    const wasm_target = b.resolveTargetQuery(.{
        .cpu_arch = .wasm32,
        .os_tag = .freestanding,
        // Enable WASM SIMD128 extension to test @Vector -> wasm SIMD mapping
        .cpu_features_add = std.Target.wasm.featureSet(&.{.simd128}),
    });
    // heading_scan_ref is a dependency of heading_scan
    const wasm_ref_mod = b.createModule(.{
        .root_source_file = b.path("src/reference/heading_scan_ref.zig"),
        .target = wasm_target,
        .optimize = .ReleaseFast,
    });
    const wasm_hs_mod = b.createModule(.{
        .root_source_file = b.path("src/kernels/heading_scan.zig"),
        .target = wasm_target,
        .optimize = .ReleaseFast,
    });
    wasm_hs_mod.addImport("../reference/heading_scan_ref.zig", wasm_ref_mod);
    const wasm_spike_mod = b.createModule(.{
        .root_source_file = b.path("spike/wasm/wasm_spike.zig"),
        .target = wasm_target,
        .optimize = .ReleaseFast,
    });
    wasm_spike_mod.addImport("../../src/kernels/heading_scan.zig", wasm_hs_mod);
    const wasm_exe = b.addExecutable(.{
        .name = "heading_scan",
        .root_module = wasm_spike_mod,
    });
    wasm_exe.rdynamic = true; // Export symbols via rdynamic
    wasm_exe.entry = .disabled; // Library-style WASM: no _start entry point
    const wasm_install = b.addInstallArtifact(wasm_exe, .{
        .dest_dir = .{ .override = .{ .custom = "wasm" } },
    });
    const wasm_step = b.step("wasm-spike", "Build WASM spike (research) at zig-out/wasm/heading_scan.wasm");
    wasm_step.dependOn(&wasm_install.step);
    // ────────────────────────────────────────────────────────────────────────

    // ── Native Bench Spike (for comparison vs WASM) ─────────────────────────
    // Run: zig build native-bench
    const native_bench_mod = b.createModule(.{
        .root_source_file = b.path("spike/wasm/native_bench.zig"),
        .target = target,
        .optimize = .ReleaseFast,
    });
    const native_hs_mod = b.createModule(.{
        .root_source_file = b.path("src/kernels/heading_scan.zig"),
        .target = target,
        .optimize = .ReleaseFast,
    });
    const native_ref_mod = b.createModule(.{
        .root_source_file = b.path("src/reference/heading_scan_ref.zig"),
        .target = target,
        .optimize = .ReleaseFast,
    });
    native_hs_mod.addImport("../reference/heading_scan_ref.zig", native_ref_mod);
    native_bench_mod.addImport("../../src/kernels/heading_scan.zig", native_hs_mod);
    const native_bench_exe = b.addExecutable(.{
        .name = "native_bench",
        .root_module = native_bench_mod,
    });
    const native_bench_install = b.addInstallArtifact(native_bench_exe, .{});
    const run_native_bench = b.addRunArtifact(native_bench_exe);
    run_native_bench.step.dependOn(&native_bench_install.step);
    const native_bench_step = b.step("native-bench", "Run native heading_scan benchmark (1M iterations)");
    native_bench_step.dependOn(&run_native_bench.step);
    // ────────────────────────────────────────────────────────────────────────

    // Unit tests (c_adapter tests)
    const test_mod = b.createModule(.{
        .root_source_file = b.path("src/c_adapter.zig"),
        .target = target,
        .optimize = optimize,
    });

    const tests = b.addTest(.{
        .root_module = test_mod,
    });

    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);

    // ── md4c parser tests ────────────────────────────────────────────────
    // Vendored Zig md4c parser (from Bun). Linked into libmarky_kernels.a
    // via c_adapter.zig comptime import of md4c/exports.zig.
    const md4c_test_mod = b.createModule(.{
        .root_source_file = b.path("src/md4c/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const md4c_tests = b.addTest(.{
        .root_module = md4c_test_mod,
    });
    const run_md4c_tests = b.addRunArtifact(md4c_tests);
    test_step.dependOn(&run_md4c_tests.step);
    // ────────────────────────────────────────────────────────────────────
}
