// C ABI exports for DocumentEngine.
//
// Exposes the Zig DocumentEngine across the FFI boundary via opaque handles.
// Pattern follows exports_embed.zig: page_allocator, castHandle helpers,
// null-safe parameters, error codes (no panics).

const std = @import("std");
const DocumentEngine = @import("document.zig").DocumentEngine;
const blob = @import("blob.zig");

/// Allocator used for engine heap allocations.
/// page_allocator is the simplest choice for long-lived, FFI-owned memory.
const engine_allocator = std.heap.page_allocator;

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

/// Get the serialized blob for the current engine state.
///
/// On success, writes blob pointer and length to the output parameters.
/// The blob memory is owned by the engine — valid until next update() or destroy().
/// Caller must NOT free the returned pointer.
///
/// Returns:
///   0  — success (blob_ptr and blob_len set)
///  -1  — invalid input (null handle or null output pointers)
///  -3  — allocation failure (out of memory during serialization)
///  -4  — parse failure (md4c error during serialization)
///  -5  — blob size overflow (exceeds u32 max)
export fn marky_engine_get_blob(
    handle: ?*anyopaque,
    blob_ptr: ?*[*]const u8,
    blob_len: ?*u32,
) i32 {
    const engine = castHandle(handle) orelse return -1;
    const out_ptr = blob_ptr orelse return -1;
    const out_len = blob_len orelse return -1;

    const data = engine.getBlob() catch |e| return switch (e) {
        error.OutOfMemory => @as(i32, -3),
        error.ParseFailed => @as(i32, -4),
    };

    // Defense-in-depth: blobs are bounded by u32 throughout (computeBlobSize returns ?u32),
    // so this can only trigger on 64-bit if somehow >4 GB of blob data is produced.
    if (data.len > std.math.maxInt(u32)) return @as(i32, -5);
    out_ptr.* = data.ptr;
    out_len.* = @intCast(data.len);
    return 0;
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

test "engine_get_blob_basic" {
    const text = "# Hello\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var blob_ptr: [*]const u8 = undefined;
    var blob_len: u32 = undefined;
    const rc = marky_engine_get_blob(handle, &blob_ptr, &blob_len);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expect(blob_len >= @sizeOf(blob.ScanBlobHeader));

    // Validate header magic and version
    const header = blob.readHeader(blob_ptr[0..blob_len]);
    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
    try testing.expect(header.heading_count >= 1);
}

test "engine_get_blob_null_handle" {
    var blob_ptr: [*]const u8 = undefined;
    var blob_len: u32 = undefined;
    const rc = marky_engine_get_blob(null, &blob_ptr, &blob_len);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "engine_get_blob_null_output_ptrs" {
    const text = "# Test\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    // Null blob_ptr
    var blob_len: u32 = undefined;
    try testing.expectEqual(@as(i32, -1), marky_engine_get_blob(handle, null, &blob_len));

    // Null blob_len
    var blob_ptr: [*]const u8 = undefined;
    try testing.expectEqual(@as(i32, -1), marky_engine_get_blob(handle, &blob_ptr, null));

    // Both null
    try testing.expectEqual(@as(i32, -1), marky_engine_get_blob(handle, null, null));
}

test "engine_get_blob_caching" {
    const text = "# Cached\n";
    const handle = marky_engine_create(text.ptr, @intCast(text.len));
    try testing.expect(handle != null);
    defer marky_engine_destroy(handle);

    var ptr1: [*]const u8 = undefined;
    var len1: u32 = undefined;
    try testing.expectEqual(@as(i32, 0), marky_engine_get_blob(handle, &ptr1, &len1));

    var ptr2: [*]const u8 = undefined;
    var len2: u32 = undefined;
    try testing.expectEqual(@as(i32, 0), marky_engine_get_blob(handle, &ptr2, &len2));

    // Same cached blob — pointer and length should match
    try testing.expectEqual(ptr1, ptr2);
    try testing.expectEqual(len1, len2);
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

    // Get blob after all updates — should be valid
    var blob_ptr: [*]const u8 = undefined;
    var blob_len: u32 = undefined;
    const rc = marky_engine_get_blob(handle, &blob_ptr, &blob_len);
    try testing.expectEqual(@as(i32, 0), rc);

    // Validate header
    const header = blob.readHeader(blob_ptr[0..blob_len]);
    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
}
