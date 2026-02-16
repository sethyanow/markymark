const std = @import("std");

/// Verifies we're using the correct Zig version at test time
test "zig version is 0.15.2 or higher" {
    const version = @import("builtin").zig_version;
    const meets_minimum = version.major > 0 or
        (version.major == 0 and version.minor > 15) or
        (version.major == 0 and version.minor == 15 and version.patch >= 2);
    try std.testing.expect(meets_minimum);
}
