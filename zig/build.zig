const std = @import("std");

// REQUIREMENT: Zig 0.15.2 or higher
// This build file uses Zig 0.15.x APIs (addLibrary, createModule).
// Zig 0.14.x patterns (addStaticLibrary, root_source_file in addLibrary) will not work.

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Root module for the static library
    const root_mod = b.createModule(.{
        .root_source_file = b.path("src/c_adapter.zig"),
        .target = target,
        .optimize = optimize,
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
}
