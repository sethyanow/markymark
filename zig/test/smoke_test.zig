const std = @import("std");

/// Smoke test: Verifies build system is working
test "build system produces valid library" {
    // This test passes if the build system successfully compiled
    // the c_adapter module and ran tests
    try std.testing.expect(true);
}

/// Verifies we're using the correct Zig version
test "zig version is 0.15.2 or higher" {
    const version = @import("builtin").zig_version;
    try std.testing.expect(version.major == 0);
    try std.testing.expect(version.minor >= 15);
    if (version.minor == 15) {
        try std.testing.expect(version.patch >= 2);
    }
}
