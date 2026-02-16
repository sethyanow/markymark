const std = @import("std");

/// Verifies we're using the correct Zig version at test time
test "zig version is 0.15.2 or higher" {
    const version = @import("builtin").zig_version;
    try std.testing.expect(version.major == 0);
    try std.testing.expect(version.minor >= 15);
    if (version.minor == 15) {
        try std.testing.expect(version.patch >= 2);
    }
}
