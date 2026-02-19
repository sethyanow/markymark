/// WASM Spike: heading_scan compiled to wasm32-freestanding
///
/// Research spike - NOT production code.
/// Answers Question 1: Does zig build produce valid WASM from our kernels?
/// Answers Question 3: Does @Vector SIMD map to WASM SIMD?
///
/// Build:  zig build wasm-spike
/// Output: zig-out/wasm/heading_scan.wasm
/// Test:   wasmtime zig-out/wasm/heading_scan.wasm --invoke has_simd_path
const heading_scan = @import("../../src/kernels/heading_scan.zig");
const HeadingScan = heading_scan.HeadingScan;

/// WASM-exported heading scan entry point.
/// text/out are pointers into WASM linear memory.
pub export fn scan_headings_wasm(
    text: [*]const u8,
    len: u32,
    out: [*]HeadingScan,
    cap: u32,
) u32 {
    return heading_scan.scan_headings(text, len, out, cap);
}

/// Benchmark entry: scans a fixed buffer N times. Returns total headings
/// found (keeps optimizer from eliminating the loop).
pub export fn bench_heading_scan(iterations: u32) u32 {
    const sample = "# Hello World\n## Section Two\n### Sub-section\n# Another H1\n## More\n";
    var buf: [64]HeadingScan = undefined;
    var total: u32 = 0;
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        total +%= heading_scan.scan_headings(sample.ptr, @intCast(sample.len), &buf, 64);
    }
    return total;
}

/// SIMD availability probe: returns 1 if @Vector(16, u8) compiles for this target.
/// Under WASM, Zig emits this as scalar loops unless --features=+simd128 is set.
pub export fn has_simd_path() u32 {
    const v: @Vector(16, u8) = @splat(0);
    const w: @Vector(16, u8) = @splat(1);
    const sum = v + w;
    return @reduce(.Add, sum);
}

/// Version sentinel so wasmtime can probe the module is working.
pub export fn marky_wasm_version() u32 {
    return 1;
}
