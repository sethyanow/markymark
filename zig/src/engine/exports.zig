// C ABI exports for DocumentEngine.
//
// Exposes the Zig DocumentEngine across the FFI boundary via opaque handles.
// Pattern follows exports_embed.zig: page_allocator, castHandle helpers,
// null-safe parameters, error codes (no panics).

const std = @import("std");
const DocumentEngine = @import("document.zig").DocumentEngine;
const result_ffi = @import("ffi_types.zig");
const get_result = @import("get_result.zig");

/// Allocator used for engine heap allocations.
/// page_allocator is the simplest choice for long-lived, FFI-owned memory.
const engine_allocator = std.heap.page_allocator;
pub const CEngineResult = result_ffi.CEngineResult;

// ── Export functions ──────────────────────────────────────────────────

/// Create a new DocumentEngine from markdown text.
///
/// Returns an opaque handle on success, or null on failure.
/// The caller (Rust) owns the handle and MUST call `marky_engine_destroy`
/// to free all memory.
///
/// Returns:
///   non-null  — success
///   null      — invalid input (null text with nonzero len) or allocation/parse failure
export fn marky_engine_create(text: ?[*]const u8, text_len: u32) ?*anyopaque {
    const slice = resolveTextSlice(text, text_len) orelse return null;
    const engine = DocumentEngine.create(slice, engine_allocator) catch return null;
    return @ptrCast(engine);
}

/// Update engine state with new markdown text.
///
/// On success, old state is freed and replaced. On failure, old state preserved.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle, null text with nonzero len)
///  -3  — allocation failure (out of memory)
///  -4  — parse failure (md4c error)
export fn marky_engine_update(handle: ?*anyopaque, text: ?[*]const u8, text_len: u32) i32 {
    const engine = castHandle(handle) orelse return -1;
    const slice = resolveTextSlice(text, text_len) orelse return -1;
    engine.update(slice) catch |e| return switch (e) {
        error.OutOfMemory => @as(i32, -3),
        error.ParseFailed => @as(i32, -4),
    };
    return 0;
}

/// Get a CEngineResult snapshot for the current engine state.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle or null output pointer)
///  -4  — allocation failure (out of memory)
///  -5  — overflow (text blob/counts exceed u32 max)
export fn marky_engine_get_result(handle: ?*anyopaque, out: ?*CEngineResult) i32 {
    const engine = castHandle(handle) orelse return -1;
    const out_result = out orelse return -1;

    get_result.getResult(engine, out_result) catch |e| return switch (e) {
        error.OutOfMemory => @as(i32, -4),
        error.Overflow => @as(i32, -5),
    };
    return 0;
}

/// Free all allocations attached to a CEngineResult.
///
/// Passing null is a no-op. Result is zeroed after free so double-free is safe.
export fn marky_engine_free_result(result: ?*CEngineResult) void {
    const r = result orelse return;
    get_result.freeResult(r);
}

/// Get the content hash for the current engine state.
///
/// The hash is computed during `create` / `update` over the parsed text.
/// Same text produces the same hash; different text produces a different hash.
/// Returns 0 if handle is null (also the hash value for empty input).
export fn marky_engine_get_content_hash(handle: ?*anyopaque) u64 {
    const engine = castHandle(handle) orelse return 0;
    return engine.content_hash;
}

/// Destroy a DocumentEngine, freeing all owned memory.
///
/// After this call the handle is invalid. Passing null is a no-op.
export fn marky_engine_destroy(handle: ?*anyopaque) void {
    const engine = castHandle(handle) orelse return;
    engine.destroy();
}

// ── Helpers ──────────────────────────────────────────────────────────

fn castHandle(handle: ?*anyopaque) ?*DocumentEngine {
    const ptr = handle orelse return null;
    return @ptrCast(@alignCast(ptr));
}

/// Resolve (text, len) pair to a slice.
/// Null text with len=0 is valid (empty document). Null text with len>0 is invalid.
fn resolveTextSlice(text: ?[*]const u8, len: u32) ?[]const u8 {
    if (text) |t| {
        return t[0..len];
    } else {
        // Null pointer: only valid if len == 0 (empty document)
        if (len == 0) return "";
        return null;
    }
}

// ── Tests ────────────────────────────────────────────────────────────

const testing = std.testing;

test "engine_create_and_destroy" {
    const text = "# Hello\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    marky_engine_destroy(handle);
}

test "engine_create_null_text_zero_len" {
    // Empty document via null pointer + len 0
    const handle = marky_engine_create(null, 0);
    try testing.expect(handle != null);
    marky_engine_destroy(handle);
}

test "engine_create_null_text_nonzero_len" {
    const handle = marky_engine_create(null, 10);
    try testing.expect(handle == null);
}

test "engine_destroy_null" {
    // Should be a no-op, not crash
    marky_engine_destroy(null);
}

test "engine_update_basic" {
    const old_text = "# Old\n";
    const handle = marky_engine_create(old_text.ptr, @intCast(old_text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    const new_text = "# New\n## Sub\n";
    const rc = marky_engine_update(handle, new_text.ptr, @intCast(new_text.len));
    try testing.expectEqual(@as(i32, 0), rc);
}

test "engine_update_null_handle" {
    const text = "# Hello\n";
    const rc = marky_engine_update(null, text.ptr, @intCast(text.len));
    try testing.expectEqual(@as(i32, -1), rc);
}

test "engine_update_null_text_nonzero_len" {
    const init_text = "# Init\n";
    const handle = marky_engine_create(init_text.ptr, @intCast(init_text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    const rc = marky_engine_update(handle, null, 10);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "engine_get_result_basic" {
    const text = "# Heading\n\n[[Page|Alias]]\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var result: CEngineResult = std.mem.zeroes(CEngineResult);
    defer marky_engine_free_result(&result);

    const rc = marky_engine_get_result(handle, &result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expect(result.text_blob_len > 0);
    try testing.expect(result.generation >= 1);
}

test "engine_get_result_null_checks" {
    const text = "# Hello\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var result: CEngineResult = std.mem.zeroes(CEngineResult);
    try testing.expectEqual(@as(i32, -1), marky_engine_get_result(null, &result));
    try testing.expectEqual(@as(i32, -1), marky_engine_get_result(handle, null));
}

test "engine_get_result_generation_increments_on_update" {
    const text = "# One\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var result1: CEngineResult = std.mem.zeroes(CEngineResult);
    defer marky_engine_free_result(&result1);
    try testing.expectEqual(@as(i32, 0), marky_engine_get_result(handle, &result1));
    const gen1 = result1.generation;
    try testing.expect(gen1 >= 1);

    const updated = "# Two\n## Sub\n";
    try testing.expectEqual(@as(i32, 0), marky_engine_update(handle, updated.ptr, @intCast(updated.len)));

    var result2: CEngineResult = std.mem.zeroes(CEngineResult);
    defer marky_engine_free_result(&result2);
    try testing.expectEqual(@as(i32, 0), marky_engine_get_result(handle, &result2));
    try testing.expect(result2.generation > gen1);
}

test "engine_free_result_null_and_double_free_safe" {
    marky_engine_free_result(null);

    const text = "# Test\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var result: CEngineResult = std.mem.zeroes(CEngineResult);
    try testing.expectEqual(@as(i32, 0), marky_engine_get_result(handle, &result));
    marky_engine_free_result(&result);
    marky_engine_free_result(&result);
}

test "engine_lifecycle" {
    const start_text = "# Start\n";
    const handle = marky_engine_create(start_text.ptr, @intCast(start_text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    // 10 updates with varied content
    const docs = [_][]const u8{
        "# One\n",
        "## Two\n### Three\n",
        "[link](url) and [[wiki]]\n",
        "#tag1 #tag2\n",
        "text ^block-id\n",
        "# Mixed\n[a](b) #c ^d\n",
        "",
        "# Back\n",
        "## Deep\n### Deeper\n#### Deepest\n",
        "# Final\n",
    };

    for (docs) |doc| {
        const rc = marky_engine_update(handle, doc.ptr, @intCast(doc.len));
        try testing.expectEqual(@as(i32, 0), rc);
    }

    // Get result after all updates — should be valid
    var result: CEngineResult = undefined;
    const rc = marky_engine_get_result(handle, &result);
    try testing.expectEqual(@as(i32, 0), rc);
    defer marky_engine_free_result(&result);

    // Should reflect the last update's content ("# Final\n")
    try testing.expectEqual(@as(u32, 1), result.headings_count);
}
