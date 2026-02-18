/// Native benchmark for comparison with WASM spike.
/// Run: zig run spike/wasm/native_bench.zig -OReleaseFast
const std = @import("std");
const heading_scan = @import("../../src/kernels/heading_scan.zig");

pub fn main() !void {
    const sample = "# Hello World\n## Section Two\n### Sub-section\n# Another H1\n## More\n";
    var buf: [64]heading_scan.HeadingScan = undefined;
    const iterations: u32 = 1_000_000;
    var total: u32 = 0;
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        total +%= heading_scan.scan_headings(sample.ptr, @intCast(sample.len), &buf, 64);
    }
    std.debug.print("total={d}\n", .{total});
}
