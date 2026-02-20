const std = @import("std");
const heading_scan = @import("kernels/heading_scan.zig");
const link_scan = @import("kernels/link_scan.zig");
const tag_scan = @import("kernels/tag_scan.zig");
const block_scan = @import("kernels/block_scan.zig");
const token_estimate = @import("kernels/token_estimate.zig");
const content_hash_mod = @import("kernels/content_hash.zig");
const fence_map = @import("kernels/fence_map.zig");
const multi_scan = @import("kernels/multi_scan.zig");
const slug_kernel = @import("kernels/slug.zig");
const env_scan = @import("kernels/formats/env_scan.zig");
const ini_scan = @import("kernels/formats/ini_scan.zig");
const toml_scan = @import("kernels/formats/toml_scan.zig");
const yaml_scan = @import("kernels/formats/yaml_scan.zig");
const json_keys = @import("kernels/formats/json_keys.zig");
const similarity = @import("shared/similarity.zig");
const normalize = @import("shared/normalize.zig");
const entities = @import("shared/entities.zig");
const quantize_mod = @import("shared/quantize.zig");
const embeddings_mod = @import("shared/embeddings.zig");

// Pull in C ABI exports from dedicated export files.
// The _ = @import forces Zig to include these export fn declarations in the library.
comptime {
    _ = @import("exports_embed.zig");
    _ = @import("exports_graph.zig");
    _ = @import("exports_serde.zig");
    _ = @import("md4c/exports.zig");
    _ = @import("engine/exports.zig");
    // Provide ___chkstk_ms on Windows x86_64 — Zig's compiler-rt does not
    // bundle it into static libraries (ziglang/zig#6817).
    _ = @import("chkstk.zig");
}

/// Re-export types for C consumers
pub const HeadingScan = heading_scan.HeadingScan;
pub const LinkScan = link_scan.LinkScan;
pub const TagScan = tag_scan.TagScan;
pub const BlockIdScan = block_scan.BlockIdScan;
pub const FenceRange = fence_map.FenceRange;
pub const ScanResult = multi_scan.ScanResult;
pub const EnvEntry = env_scan.EnvEntry;
pub const IniEntry = ini_scan.IniEntry;
pub const TomlEntry = toml_scan.TomlEntry;
pub const TomlKind = toml_scan.TomlKind;
pub const YamlEntry = yaml_scan.YamlEntry;
pub const JsonKeyEntry = json_keys.JsonKeyEntry;

/// Version constant for markymark kernels
/// Format: 0xMMmmpp (major, minor, patch)
const MARKY_VERSION: u32 = 0x000100; // 0.1.0

/// Returns the version of libmarky_kernels.
/// This is a no-op export to verify build system linkage.
/// `export fn` gets C calling convention by default in Zig 0.15.x.
export fn marky_version() u32 {
    return MARKY_VERSION;
}

/// SIMD-accelerated heading extraction.
///
/// Scans `text[0..len]` for ATX headings (# at line start followed by space).
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more headings than cap)
export fn marky_scan_headings(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]HeadingScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = heading_scan.scan_headings(t, len, o, cap);
    w.* = count;

    // If we filled the buffer exactly, there may be more headings
    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated link extraction.
///
/// Scans `text[0..len]` for markdown links [text](url) and wiki-links [[target]].
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more links than cap)
export fn marky_scan_links(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]LinkScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = link_scan.scan_links(t, len, o, cap);
    w.* = count;

    // If we filled the buffer exactly, there may be more links
    if (count >= cap) return -2;

    return 0;
}

/// Approximate BPE token count via SIMD word boundary detection.
///
/// Returns approximate token count for the given text.
/// Returns 0 for null text pointer or zero length.
export fn marky_estimate_tokens(
    text: ?[*]const u8,
    len: u32,
) u32 {
    const t = text orelse return 0;
    if (len == 0) return 0;
    return token_estimate.estimate_tokens(t, len);
}

/// FNV-1a 64-bit content fingerprint.
///
/// Returns a deterministic hash of the text content.
/// Returns 0 for null text pointer. Returns FNV offset basis for zero length.
export fn marky_content_hash(
    text: ?[*]const u8,
    len: u32,
) u64 {
    const t = text orelse return 0;
    return content_hash_mod.content_hash(t, len);
}

/// SIMD-accelerated tag extraction.
///
/// Scans `text[0..len]` for #tag patterns (whitespace-bounded).
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more tags than cap)
export fn marky_scan_tags(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]TagScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = tag_scan.scan_tags(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated block ID extraction.
///
/// Scans `text[0..len]` for ^block-id patterns at end of line.
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more block IDs than cap)
export fn marky_scan_block_ids(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]BlockIdScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = block_scan.scan_block_ids(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated fence map builder.
///
/// Scans `text[0..len]` for fenced code blocks (triple+ backtick/tilde at
/// column 0). Writes byte ranges into `ranges_out[0..cap]`, sets `*written`
/// to the number of ranges found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more ranges than cap)
export fn marky_build_fence_map(
    text: ?[*]const u8,
    len: u32,
    ranges_out: ?[*]FenceRange,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = ranges_out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = fence_map.build_fence_map(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

fn in_fence_ranges_binary(ranges: []const FenceRange, pos: u32) bool {
    var lo: usize = 0;
    var hi: usize = ranges.len;
    while (lo < hi) {
        const mid = lo + (hi - lo) / 2;
        const r = ranges[mid];
        if (pos < r.start) {
            hi = mid;
        } else if (pos >= r.end) {
            lo = mid + 1;
        } else {
            return true;
        }
    }
    return false;
}

fn extract_multi_result(
    buf: []const u8,
    raw: ScanResult,
) ?ScanResult {
    const scan_ref = @import("reference/multi_scan_ref.zig");

    const ty: scan_ref.ScanType = @enumFromInt(raw.scan_type);

    switch (ty) {
        .heading => {
            const heading_ref = @import("reference/heading_scan_ref.zig");
            if (heading_ref.try_parse_heading(buf, raw.offset, @intCast(buf.len))) |h| {
                return ScanResult{
                    .offset = h.offset,
                    .length = h.length,
                    .scan_type = @intFromEnum(scan_ref.ScanType.heading),
                    .extra = h.level,
                };
            }
            return null;
        },
        .link_open => {
            const link_ref = @import("reference/link_scan_ref.zig");
            if (link_ref.try_parse_markdown_link(buf, raw.offset, @intCast(buf.len))) |l| {
                const clamped_target: u8 = if (l.target_length > std.math.maxInt(u8)) std.math.maxInt(u8) else @intCast(l.target_length);
                return ScanResult{
                    .offset = l.offset,
                    .length = l.text_length,
                    .scan_type = @intFromEnum(scan_ref.ScanType.link_open),
                    .extra = clamped_target,
                };
            }
            return null;
        },
        .wiki_link => {
            if (raw.offset == 0) return null;
            const link_ref = @import("reference/link_scan_ref.zig");
            const start = raw.offset - 1;
            if (link_ref.try_parse_wiki_link(buf, start, @intCast(buf.len))) |l| {
                const clamped_target: u8 = if (l.target_length > std.math.maxInt(u8)) std.math.maxInt(u8) else @intCast(l.target_length);
                return ScanResult{
                    .offset = l.offset,
                    .length = l.text_length,
                    .scan_type = @intFromEnum(scan_ref.ScanType.wiki_link),
                    .extra = clamped_target,
                };
            }
            return null;
        },
        .fence_backtick, .fence_tilde => {
            // Fence markers are used to build fence maps; not emitted as indexable content.
            return null;
        },
        .block_id => {
            const block_ref = @import("reference/block_scan_ref.zig");
            if (block_ref.try_parse_block_id(buf, raw.offset)) |b| {
                return ScanResult{
                    .offset = b.offset,
                    .length = b.length,
                    .scan_type = @intFromEnum(scan_ref.ScanType.block_id),
                    .extra = 0,
                };
            }
            return null;
        },
        .tag => {
            const tag_ref = @import("reference/tag_scan_ref.zig");
            if (tag_ref.try_parse_tag(buf, raw.offset, @intCast(buf.len))) |t| {
                return ScanResult{
                    .offset = t.offset,
                    .length = t.length,
                    .scan_type = @intFromEnum(scan_ref.ScanType.tag),
                    .extra = 0,
                };
            }
            return null;
        },
    }
}

/// Single-pass multi-pattern scan with fence filtering and typed extraction.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (partial results written)
export fn marky_multi_scan(
    text: ?[*]const u8,
    len: u32,
    fence_ranges: ?[*]const FenceRange,
    fence_count: u32,
    results_out: ?[*]ScanResult,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = results_out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    // Use threadlocal static buffers instead of stack-allocated arrays.
    // The raw_buf (16KB) and fence_buf (2KB) together exceed the 4KB
    // Windows stack frame threshold, causing LLVM to emit ___chkstk_ms
    // (an MSVC compiler-rt symbol not bundled into Zig static libraries).
    const S = struct {
        threadlocal var fence_buf: [256]FenceRange = undefined;
        threadlocal var raw_buf: [2048]ScanResult = undefined;
    };

    const fence_slice: []const FenceRange = if (fence_count == 0)
        &[_]FenceRange{}
    else blk: {
        const fr = fence_ranges orelse return -1;
        const src = fr[0..fence_count];

        if (fence_count > S.fence_buf.len) {
            w.* = 0;
            return -2; // Internal buffer too small
        }

        std.mem.copyForwards(FenceRange, S.fence_buf[0..fence_count], src);
        std.mem.sort(FenceRange, S.fence_buf[0..fence_count], {}, struct {
            fn lessThan(_: void, a: FenceRange, b: FenceRange) bool {
                return a.start < b.start;
            }
        }.lessThan);

        break :blk S.fence_buf[0..fence_count];
    };

    // Worst-case raw candidates can exceed cap due to rejected candidates.
    // threadlocal avoids heap allocation while keeping the stack frame small.
    const raw_buf = &S.raw_buf;
    const raw_cap: u32 = @intCast(raw_buf.len);
    const raw_count = multi_scan.scan_multi(t, len, raw_buf, raw_cap);

    // If raw candidates exceed internal buffer, results may be truncated.
    // Return -2 to signal partial results rather than silently dropping.
    if (raw_count >= raw_cap) {
        w.* = 0;
        return -2;
    }

    const text_slice = t[0..len];
    var out_written: u32 = 0;

    var i: u32 = 0;
    while (i < raw_count) : (i += 1) {
        const raw = raw_buf[i];

        if (extract_multi_result(text_slice, raw)) |extracted| {
            if (in_fence_ranges_binary(fence_slice, extracted.offset)) continue;

            if (out_written >= cap) {
                w.* = out_written;
                return -2;
            }

            o[out_written] = extracted;
            out_written += 1;
        }
    }

    w.* = out_written;
    return 0;
}

// ============================================================================
// Shared kernel exports: similarity, normalize, entities, quantize
// ============================================================================

/// SIMD-accelerated cosine similarity between two f32 vectors.
///
/// Returns cosine similarity in [-1.0, 1.0].
/// Returns -2.0 on error (null pointers, zero dims, zero-magnitude vector).
export fn zig_cosine_similarity(
    a: ?[*]const f32,
    b: ?[*]const f32,
    dims: u32,
) f32 {
    const va = a orelse return -2.0;
    const vb = b orelse return -2.0;
    if (dims == 0) return -2.0;
    return similarity.cosine_similarity(va, vb, dims);
}

/// Jaccard similarity between two sorted u32 hash sets.
///
/// Both sets MUST be sorted in ascending order.
/// Returns |intersection| / |union| in [0.0, 1.0].
/// Returns -1.0 on error (null pointers).
export fn zig_jaccard_similarity(
    set1: ?[*]const u32,
    set1_len: u32,
    set2: ?[*]const u32,
    set2_len: u32,
) f32 {
    const s1 = set1 orelse return -1.0;
    const s2 = set2 orelse return -1.0;
    return similarity.jaccard_similarity(s1, set1_len, s2, set2_len);
}

/// Fuzzy match score between query and candidate strings.
///
/// Returns:
///   >=0 — score (0 means no match)
///   -1  — invalid input (null pointer)
export fn marky_fuzzy_match(
    query: ?[*]const u8,
    query_len: u32,
    candidate: ?[*]const u8,
    candidate_len: u32,
) i32 {
    const q = query orelse return -1;
    const c = candidate orelse return -1;
    return similarity.fuzzy_match_score(q, query_len, c, candidate_len);
}

/// Batched fuzzy match top-k ranking.
///
/// Candidate ordering is deterministic:
/// - score descending
/// - candidate index ascending on ties
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointers)
///  -2  — invalid capacity (`top_k > output_cap` or `output_cap == 0` while `top_k > 0`)
export fn marky_fuzzy_match_batch(
    query: ?[*]const u8,
    query_len: u32,
    candidate_ptrs: ?[*]const ?[*]const u8,
    candidate_lens: ?[*]const u32,
    candidate_count: u32,
    scores_out: ?[*]i32,
    indices_out: ?[*]u32,
    output_cap: u32,
    top_k: u32,
    written: ?*u32,
) i32 {
    const q = query orelse return -1;
    const ptrs = candidate_ptrs orelse return -1;
    const lens = candidate_lens orelse return -1;
    const scores = scores_out orelse return -1;
    const indices = indices_out orelse return -1;
    const w = written orelse return -1;

    return similarity.fuzzy_match_top_k(
        q,
        query_len,
        ptrs,
        lens,
        candidate_count,
        scores,
        indices,
        output_cap,
        top_k,
        w,
    );
}

/// SIMD-accelerated slug generation from heading text.
///
/// Converts ASCII uppercase to lowercase, maps whitespace/punctuation to '-',
/// strips unsupported punctuation, collapses repeated hyphens, and trims leading/
/// trailing hyphens. Non-ASCII UTF-8 bytes are passed through unchanged.
///
/// Returns:
///  >=0 — bytes written
///  -1  — invalid input (null pointers)
///  -2  — output buffer too small (including output_cap == 0)
export fn marky_slugify(
    text: ?[*]const u8,
    len: u32,
    output: ?[*]u8,
    output_cap: u32,
) i32 {
    const t = text orelse {
        if (len == 0) return 0;
        return -1;
    };
    const out = output orelse return -1;

    if (len == 0) return 0;
    if (output_cap == 0) return -2;

    return slug_kernel.slugify(t, len, out, output_cap);
}

/// SIMD-accelerated .env file key-value extractor.
///
/// Scans `text[0..len]` for KEY=value pairs. Writes results into `out[0..cap]`,
/// sets `*written` to the number of entries found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more entries than cap)
export fn marky_scan_env(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]EnvEntry,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = env_scan.scan_env(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated INI file key-value extractor.
///
/// Scans `text[0..len]` for `[section]` headers and `key=value` pairs.
/// Each entry embeds the section it belongs to.  Keys before any section
/// have section_len=0 (global section).
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more entries than cap)
export fn marky_scan_ini(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]IniEntry,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = ini_scan.scan_ini(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated TOML file key-value extractor.
///
/// Scans `text[0..len]` for TOML structure: [table] headers, [[array_table]]
/// headers, and key = value assignments.  Each entry embeds its table context.
///
/// Entry kinds (entry.kind field):
///   0 — key-value pair (table_offset/len = current table; key_offset/len = key; val_offset/len = value)
///   1 — [table] header (table_offset/len = header name; key/val zero)
///   2 — [[array_table]] header (table_offset/len = header name; key/val zero)
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more entries than cap)
export fn marky_scan_toml(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]TomlEntry,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = toml_scan.scan_toml(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated YAML key extractor.
///
/// Scans `text[0..len]` for YAML mapping keys at all indentation levels.
/// Each entry records the key name's byte offset, length, and indentation
/// depth (in spaces; tabs normalised to 1 space each).  Callers reconstruct
/// key-path hierarchy by tracking indent level transitions.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more entries than cap)
export fn marky_scan_yaml_keys(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]YamlEntry,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = yaml_scan.scan_yaml_keys(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated JSON key extractor.
///
/// Scans `text[0..len]` for JSON object keys at all nesting levels.
/// Each entry records the key's byte offset (content only, excluding quotes),
/// byte length, and 0-indexed nesting depth.  Callers reconstruct dot-
/// separated key paths by tracking depth transitions.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more entries than cap), or nesting
///         depth exceeded MAX_DEPTH (100)
export fn marky_scan_json_keys(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]JsonKeyEntry,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    var depth_exceeded: bool = false;
    const count = json_keys.scan_json_keys(t, len, o, cap, &depth_exceeded);
    w.* = count;

    if (depth_exceeded) return -2;
    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated entity hash extraction.
///
/// Scans text for words, produces FNV-1a u32 hash for each.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (writes as many as fit)
export fn zig_extract_entity_hashes(
    text_ptr: ?[*]const u8,
    text_len: u32,
    output_ids: ?[*]u32,
    capacity: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;

    // Zero-length text is a no-op regardless of other params
    if (text_len == 0) {
        w.* = 0;
        return 0;
    }

    const t = text_ptr orelse return -1;

    if (capacity == 0) {
        w.* = 0;
        return -2;
    }

    const o = output_ids orelse return -1;

    return entities.extract_entity_hashes(t, text_len, o, capacity, w);
}

/// SIMD-accelerated L2 normalization of f32 vector.
///
/// Produces a unit vector (||output|| == 1.0).
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer, zero length, zero vector)
export fn zig_normalize_f32_l2(
    input: ?[*]const f32,
    output: ?[*]f32,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return normalize.normalize_f32_l2(i, o, n);
}

/// SIMD-accelerated Q4_0 quantization: f32 -> 4-bit packed format.
///
/// n must be divisible by 32 (Q4 block size).
///
/// Returns:
///   0  — success
///  -1  — invalid input (n not divisible by 32, zero, null pointer)
export fn zig_quantize_f32_to_q4_0(
    input: ?[*]const f32,
    output: ?[*]u8,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return quantize_mod.quantize_f32_to_q4_0(i, o, n);
}

/// SIMD-accelerated Q4_0 dequantization: 4-bit packed format -> f32.
///
/// n must be divisible by 32 (Q4 block size).
///
/// Returns:
///   0  — success
///  -1  — invalid input
export fn zig_dequantize_q4_0_to_f32(
    input: ?[*]const u8,
    output: ?[*]f32,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return quantize_mod.dequantize_q4_0_to_f32(i, o, n);
}

// ============================================================================
// Tests
// ============================================================================

// Pull in kernel tests so they run as part of `zig build test`
test {
    _ = @import("kernels/heading_scan.zig");
    _ = @import("reference/heading_scan_ref.zig");
    _ = @import("kernels/link_scan.zig");
    _ = @import("reference/link_scan_ref.zig");
    _ = @import("kernels/tag_scan.zig");
    _ = @import("reference/tag_scan_ref.zig");
    _ = @import("kernels/block_scan.zig");
    _ = @import("reference/block_scan_ref.zig");
    _ = @import("kernels/token_estimate.zig");
    _ = @import("kernels/content_hash.zig");
    _ = @import("kernels/fence_map.zig");
    _ = @import("reference/fence_map_ref.zig");
    // Multi-scan automaton (Aho-Corasick)
    _ = @import("reference/multi_scan_ref.zig");
    _ = @import("kernels/multi_scan.zig");
    _ = @import("kernels/slug.zig");
    // Shared kernels (forked from forge BRZA)
    _ = @import("shared/similarity.zig");
    _ = @import("reference/similarity_ref.zig");
    _ = @import("shared/normalize.zig");
    _ = @import("reference/normalize_ref.zig");
    _ = @import("shared/entities.zig");
    _ = @import("reference/entities_ref.zig");
    _ = @import("shared/quantize.zig");
    _ = @import("reference/quantize_ref.zig");
    // Embedding index (persistent data structure with lifecycle)
    _ = @import("shared/embeddings.zig");
    // Embedding C ABI exports + tests
    _ = @import("exports_embed.zig");
    // Link graph engine
    _ = @import("kernels/link_graph.zig");
    // Link graph C ABI exports + tests
    _ = @import("exports_graph.zig");
    // md4c FFI exports + tests
    _ = @import("md4c/exports.zig");
    // Format extractors
    _ = @import("kernels/formats/env_scan.zig");
    _ = @import("kernels/formats/ini_scan.zig");
    _ = @import("kernels/formats/toml_scan.zig");
    _ = @import("kernels/formats/yaml_scan.zig");
    _ = @import("kernels/formats/json_keys.zig");
    // Document engine (stateful composite scan with blob serialization)
    _ = @import("engine/document.zig");
    // Document engine C ABI exports + tests
    _ = @import("engine/exports.zig");
}

test "marky_version returns expected version" {
    const version = marky_version();
    try std.testing.expectEqual(@as(u32, 0x000100), version);
}

test "version format is correct" {
    const version = marky_version();
    const major: u8 = @truncate(version >> 16);
    const minor: u8 = @truncate(version >> 8);
    const patch: u8 = @truncate(version);

    try std.testing.expectEqual(@as(u8, 0), major);
    try std.testing.expectEqual(@as(u8, 1), minor);
    try std.testing.expectEqual(@as(u8, 0), patch);
}

test "marky_scan_headings basic" {
    const text = "# Hello\n## World\n";
    var out: [8]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "marky_scan_headings null text with zero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_headings null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings null written" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings zero cap" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- marky_scan_links tests --

test "marky_scan_links basic" {
    const text = "[hello](url) and [[wiki]]";
    var out: [8]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u8, 0), out[0].link_type); // markdown
    try std.testing.expectEqual(@as(u8, 1), out[1].link_type); // wiki
}

test "marky_scan_links null text with zero len" {
    var w: u32 = undefined;
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_links null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_links null written" {
    const text = "[hello](url)";
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_links zero cap" {
    const text = "[hello](url)";
    var out: [4]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_links buffer overflow returns -2" {
    const text = "[a](b) [c](d) [e](f)";
    var out: [1]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}

// -- marky_estimate_tokens tests --

test "marky_estimate_tokens basic" {
    const text = "hello world foo bar";
    const result = marky_estimate_tokens(text.ptr, text.len);
    // 4 words * 1.3 = 5.2 -> (4*13+5)/10 = 57/10 = 5
    try std.testing.expectEqual(@as(u32, 5), result);
}

test "marky_estimate_tokens null text" {
    const result = marky_estimate_tokens(null, 10);
    try std.testing.expectEqual(@as(u32, 0), result);
}

test "marky_estimate_tokens zero length" {
    const text = "hello";
    const result = marky_estimate_tokens(text.ptr, 0);
    try std.testing.expectEqual(@as(u32, 0), result);
}

// -- marky_content_hash tests --

test "marky_content_hash basic" {
    const text = "hello";
    const hash = marky_content_hash(text.ptr, text.len);
    try std.testing.expect(hash != 0);
    // Deterministic
    const hash2 = marky_content_hash(text.ptr, text.len);
    try std.testing.expectEqual(hash, hash2);
}

test "marky_content_hash null text" {
    const result = marky_content_hash(null, 10);
    try std.testing.expectEqual(@as(u64, 0), result);
}

test "marky_content_hash zero length" {
    const text = "hello";
    const result = marky_content_hash(text.ptr, 0);
    // FNV offset basis for empty
    try std.testing.expectEqual(@as(u64, 0xcbf29ce484222325), result);
}

test "marky_content_hash distinct" {
    const hash1 = marky_content_hash("abc".ptr, 3);
    const hash2 = marky_content_hash("def".ptr, 3);
    try std.testing.expect(hash1 != hash2);
}

// -- marky_scan_tags tests --

test "marky_scan_tags basic" {
    const text = "text #tag1 #tag2";
    var out: [8]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u32, 11), out[1].offset);
}

test "marky_scan_tags null text with zero len" {
    var w: u32 = undefined;
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_tags null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_tags null written" {
    const text = "#tag";
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_tags zero cap" {
    const text = "#tag";
    var out: [4]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_tags buffer overflow returns -2" {
    const text = "#a #b #c";
    var out: [1]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}

// -- marky_scan_block_ids tests --

test "marky_scan_block_ids basic" {
    const text = "text ^block-id\n";
    var out: [8]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}

test "marky_scan_block_ids null text with zero len" {
    var w: u32 = undefined;
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_block_ids null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_block_ids null written" {
    const text = "text ^id\n";
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_block_ids zero cap" {
    const text = "text ^id\n";
    var out: [4]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_block_ids not at EOL" {
    const text = "^id more text\n";
    var out: [4]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- zig_cosine_similarity tests --

test "zig_cosine_similarity basic" {
    const a = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const result = zig_cosine_similarity(&a, &b, 4);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-5);
}

test "zig_cosine_similarity null a" {
    const b = [_]f32{ 1.0, 2.0 };
    const result = zig_cosine_similarity(null, &b, 2);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

test "zig_cosine_similarity null b" {
    const a = [_]f32{ 1.0, 2.0 };
    const result = zig_cosine_similarity(&a, null, 2);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

test "zig_cosine_similarity zero dims" {
    const a = [_]f32{1.0};
    const result = zig_cosine_similarity(&a, &a, 0);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

// -- zig_jaccard_similarity tests --

test "zig_jaccard_similarity basic" {
    const s1 = [_]u32{ 1, 2, 3, 4, 5 };
    const s2 = [_]u32{ 3, 4, 5, 6, 7 };
    const result = zig_jaccard_similarity(&s1, 5, &s2, 5);
    // intersection = {3,4,5} = 3, union = 5+5-3 = 7
    try std.testing.expectApproxEqAbs(@as(f32, 3.0 / 7.0), result, 1e-6);
}

test "zig_jaccard_similarity null set1" {
    const s2 = [_]u32{1};
    const result = zig_jaccard_similarity(null, 1, &s2, 1);
    try std.testing.expectEqual(@as(f32, -1.0), result);
}

test "zig_jaccard_similarity null set2" {
    const s1 = [_]u32{1};
    const result = zig_jaccard_similarity(&s1, 1, null, 1);
    try std.testing.expectEqual(@as(f32, -1.0), result);
}

test "marky_fuzzy_match prefix scores higher than substring" {
    const prefix = marky_fuzzy_match("st".ptr, 2, "stage".ptr, 5);
    const substring = marky_fuzzy_match("st".ptr, 2, "setup".ptr, 5);

    try std.testing.expect(prefix > 0);
    try std.testing.expect(substring > 0);
    try std.testing.expect(prefix > substring);
}

test "marky_fuzzy_match is case-insensitive" {
    const score = marky_fuzzy_match("ST".ptr, 2, "Setup".ptr, 5);
    try std.testing.expect(score > 0);
}

test "marky_fuzzy_match supports subsequence" {
    const score = marky_fuzzy_match("stp".ptr, 3, "setup".ptr, 5);
    try std.testing.expect(score > 0);
}

test "marky_fuzzy_match no match returns zero" {
    const score = marky_fuzzy_match("zzz".ptr, 3, "setup".ptr, 5);
    try std.testing.expectEqual(@as(i32, 0), score);
}

test "marky_fuzzy_match null input returns -1" {
    const score1 = marky_fuzzy_match(null, 1, "setup".ptr, 5);
    const score2 = marky_fuzzy_match("st".ptr, 2, null, 5);
    try std.testing.expectEqual(@as(i32, -1), score1);
    try std.testing.expectEqual(@as(i32, -1), score2);
}

test "marky_fuzzy_match_batch stable top-k ordering" {
    const query = "ab";
    const candidates = [_]?[*]const u8{
        "acb".ptr,
        "adb".ptr,
        "aeb".ptr,
    };
    const lengths = [_]u32{ 3, 3, 3 };
    var scores: [3]i32 = undefined;
    var indices: [3]u32 = undefined;
    var written: u32 = 0;

    const rc = marky_fuzzy_match_batch(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        2,
        &written,
    );

    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), written);
    try std.testing.expectEqual(@as(u32, 0), indices[0]);
    try std.testing.expectEqual(@as(u32, 1), indices[1]);
    try std.testing.expect(scores[0] >= scores[1]);
}

test "marky_fuzzy_match_batch returns no matches for impossible query" {
    const query = "zzz";
    const candidates = [_]?[*]const u8{
        "stage".ptr,
        "setup".ptr,
    };
    const lengths = [_]u32{ 5, 5 };
    var scores: [2]i32 = undefined;
    var indices: [2]u32 = undefined;
    var written: u32 = 99;

    const rc = marky_fuzzy_match_batch(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        2,
        &written,
    );

    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), written);
}

test "marky_fuzzy_match_batch capacity guard returns -2" {
    const query = "st";
    const candidates = [_]?[*]const u8{"stage".ptr};
    const lengths = [_]u32{5};
    var scores: [1]i32 = undefined;
    var indices: [1]u32 = undefined;
    var written: u32 = 0;

    const rc = marky_fuzzy_match_batch(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        2,
        &written,
    );

    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), written);
}

test "marky_fuzzy_match_batch matches scalar reference ranking" {
    const fuzzy_ref = @import("reference/fuzzy_match_ref.zig");

    const query = "st";
    const candidate_text = [_][]const u8{
        "setup",
        "stage",
        "toast",
        "street",
        "rust",
    };
    const candidates = [_]?[*]const u8{
        candidate_text[0].ptr,
        candidate_text[1].ptr,
        candidate_text[2].ptr,
        candidate_text[3].ptr,
        candidate_text[4].ptr,
    };
    const lengths = [_]u32{ 5, 5, 5, 6, 4 };
    var scores: [5]i32 = undefined;
    var indices: [5]u32 = undefined;
    var written: u32 = 0;

    const rc = marky_fuzzy_match_batch(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        5,
        &written,
    );

    try std.testing.expectEqual(@as(i32, 0), rc);
    const expected = try fuzzy_ref.rank_candidates(std.testing.allocator, query, &candidate_text, 5);
    defer std.testing.allocator.free(expected);

    try std.testing.expectEqual(expected.len, @as(usize, @intCast(written)));
    for (expected, 0..) |exp, i| {
        try std.testing.expectEqual(exp.index, indices[i]);
        try std.testing.expectEqual(exp.score, scores[i]);
    }
}

test "marky_slugify basic heading" {
    const text = "Hello World";
    var out: [32]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 11), rc);
    try std.testing.expectEqualStrings("hello-world", out[0..@as(usize, @intCast(rc))]);
}

test "marky_slugify strips punctuation and collapses hyphens" {
    const text = "Using `fmt`!!!";
    var out: [32]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 9), rc);
    try std.testing.expectEqualStrings("using-fmt", out[0..@as(usize, @intCast(rc))]);
}

test "marky_slugify all punctuation returns empty" {
    const text = "!!!...---";
    var out: [32]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "marky_slugify preserves non ascii bytes" {
    const text = "Café au lait";
    var out: [32]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 13), rc);
    try std.testing.expectEqualStrings("café-au-lait", out[0..@as(usize, @intCast(rc))]);
}

test "marky_slugify null text with zero len" {
    var out: [8]u8 = undefined;
    const rc = marky_slugify(null, 0, &out, out.len);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "marky_slugify null text with nonzero len returns -1" {
    var out: [8]u8 = undefined;
    const rc = marky_slugify(null, 1, &out, out.len);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_slugify null output returns -1" {
    const text = "hello";
    const rc = marky_slugify(text.ptr, text.len, null, 8);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_slugify zero output cap returns -2" {
    const text = "hello";
    var out: [8]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, 0);
    try std.testing.expectEqual(@as(i32, -2), rc);
}

test "marky_slugify truncation returns -2" {
    const text = "hello-world";
    var out: [5]u8 = undefined;
    const rc = marky_slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqualStrings("hello", out[0..]);
}

// -- zig_extract_entity_hashes tests --

test "zig_extract_entity_hashes basic" {
    const text = "hello world";
    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "zig_extract_entity_hashes null text with zero len" {
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "zig_extract_entity_hashes null text with nonzero len" {
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_extract_entity_hashes null written" {
    const text = "hello";
    var out: [4]u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_extract_entity_hashes buffer overflow" {
    const text = "one two three four five";
    var out: [2]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 2, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "zig_extract_entity_hashes capacity zero sets written" {
    const text = "hello world";
    var w: u32 = 99;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, null, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "zig_extract_entity_hashes text_len zero ignores null output_ids" {
    var w: u32 = 99;
    const rc = zig_extract_entity_hashes(null, 0, null, 0, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- zig_normalize_f32_l2 tests --

test "zig_normalize_f32_l2 basic" {
    const input = [_]f32{ 3.0, 4.0, 0.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = zig_normalize_f32_l2(&input, &output, 4);
    try std.testing.expectEqual(@as(i32, 0), rc);
    var norm_sq: f32 = 0.0;
    for (output) |v| norm_sq += v * v;
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), @sqrt(norm_sq), 1e-5);
}

test "zig_normalize_f32_l2 null input" {
    var output: [4]f32 = undefined;
    const rc = zig_normalize_f32_l2(null, &output, 4);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_normalize_f32_l2 null output" {
    const input = [_]f32{ 1.0, 0.0 };
    const rc = zig_normalize_f32_l2(&input, null, 2);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_normalize_f32_l2 zero n" {
    const input = [_]f32{1.0};
    var output: [1]f32 = undefined;
    const rc = zig_normalize_f32_l2(&input, &output, 0);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

// -- asm_quantize/dequantize tests --

test "zig_quantize_f32_to_q4_0 basic" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    const rc = zig_quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "zig_quantize_f32_to_q4_0 null input" {
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    const rc = zig_quantize_f32_to_q4_0(null, &q4_buf, 32);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_dequantize_q4_0_to_f32 round trip" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    _ = zig_quantize_f32_to_q4_0(&input, &q4_buf, 32);

    var output: [32]f32 = undefined;
    const rc = zig_dequantize_q4_0_to_f32(&q4_buf, &output, 32);
    try std.testing.expectEqual(@as(i32, 0), rc);

    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        try std.testing.expect(err < 0.15);
    }
}

test "zig_dequantize_q4_0_to_f32 null input" {
    var output: [32]f32 = undefined;
    const rc = zig_dequantize_q4_0_to_f32(null, &output, 32);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

// -- marky_build_fence_map tests --

test "marky_build_fence_map basic" {
    const text = "```\ncode here\n```\n";
    var out: [8]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].start);
    try std.testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "marky_build_fence_map null text with zero len" {
    var w: u32 = undefined;
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_build_fence_map null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_build_fence_map null written" {
    const text = "```\ncode\n```\n";
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_build_fence_map zero cap" {
    const text = "```\ncode\n```\n";
    var out: [4]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_build_fence_map buffer overflow returns -2" {
    const text = "```\na\n```\n```\nb\n```\n```\nc\n```\n";
    var out: [1]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}

fn test_in_fence_linear(ranges: []const FenceRange, pos: u32) bool {
    for (ranges) |r| {
        if (pos >= r.start and pos < r.end) return true;
    }
    return false;
}

fn test_contains_scan_result(haystack: []const ScanResult, needle: ScanResult) bool {
    for (haystack) |item| {
        if (item.offset == needle.offset and
            item.length == needle.length and
            item.scan_type == needle.scan_type and
            item.extra == needle.extra)
        {
            return true;
        }
    }
    return false;
}

test "marky_multi_scan fence filtering basic" {
    const scan_ref = @import("reference/multi_scan_ref.zig");

    const text =
        "# Outside\n" ++
        "```\n" ++
        "# Inside\n" ++
        "[[inside]] and [inside](url) #inside ^inside\n" ++
        "```\n" ++
        "[outside](url) #tag\n";

    var ranges: [8]FenceRange = undefined;
    var range_written: u32 = 0;
    const fence_rc = marky_build_fence_map(text.ptr, text.len, &ranges, 8, &range_written);
    try std.testing.expectEqual(@as(i32, 0), fence_rc);
    try std.testing.expectEqual(@as(u32, 1), range_written);

    var out: [64]ScanResult = undefined;
    var written: u32 = 0;
    const rc = marky_multi_scan(text.ptr, text.len, &ranges, range_written, &out, 64, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    var heading_count: u32 = 0;
    var markdown_count: u32 = 0;
    var tag_count: u32 = 0;

    for (out[0..written]) |r| {
        const ty: scan_ref.ScanType = @enumFromInt(r.scan_type);
        switch (ty) {
            .heading => heading_count += 1,
            .link_open => markdown_count += 1,
            .tag => tag_count += 1,
            else => {},
        }
    }

    try std.testing.expectEqual(@as(u32, 1), heading_count);
    try std.testing.expectEqual(@as(u32, 1), markdown_count);
    try std.testing.expectEqual(@as(u32, 1), tag_count);
}

test "marky_multi_scan all filtered inside fences" {
    const text =
        "```\n" ++
        "# Inside\n" ++
        "[[inside]] [inside](url) #tag ^id\n" ++
        "```\n";

    var ranges: [4]FenceRange = undefined;
    var range_written: u32 = 0;
    const fence_rc = marky_build_fence_map(text.ptr, text.len, &ranges, 4, &range_written);
    try std.testing.expectEqual(@as(i32, 0), fence_rc);
    try std.testing.expectEqual(@as(u32, 1), range_written);

    var out: [16]ScanResult = undefined;
    var written: u32 = 0;
    const rc = marky_multi_scan(text.ptr, text.len, &ranges, range_written, &out, 16, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), written);
}

test "marky_multi_scan handles unsorted fence ranges" {
    const scan_ref = @import("reference/multi_scan_ref.zig");

    const text =
        "```\n# hidden-one\n```\n" ++
        "# outside\n" ++
        "```\n# hidden-two\n```\n";

    var ranges: [4]FenceRange = undefined;
    var range_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_build_fence_map(text.ptr, text.len, &ranges, 4, &range_written));
    try std.testing.expectEqual(@as(u32, 2), range_written);

    // Deliberately unsort the fence ranges.
    const tmp = ranges[0];
    ranges[0] = ranges[1];
    ranges[1] = tmp;

    var out: [16]ScanResult = undefined;
    var written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_multi_scan(text.ptr, text.len, &ranges, range_written, &out, 16, &written));

    var heading_count: u32 = 0;
    for (out[0..written]) |r| {
        if (r.scan_type == @intFromEnum(scan_ref.ScanType.heading)) heading_count += 1;
    }

    // Only the outside heading should remain.
    try std.testing.expectEqual(@as(u32, 1), heading_count);
}

test "marky_multi_scan parity with individual scans" {
    const scan_ref = @import("reference/multi_scan_ref.zig");

    const text =
        "# Heading\n" ++
        "[[wiki]] and [md](url) #tag\n" ++
        "line ^block-id\n" ++
        "```\n" ++
        "# hidden\n" ++
        "[hidden](url) #hidden ^hidden\n" ++
        "```\n";

    var ranges: [8]FenceRange = undefined;
    var range_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_build_fence_map(text.ptr, text.len, &ranges, 8, &range_written));

    var multi_out: [64]ScanResult = undefined;
    var multi_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_multi_scan(text.ptr, text.len, &ranges, range_written, &multi_out, 64, &multi_written));

    var headings: [16]HeadingScan = undefined;
    var heading_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_scan_headings(text.ptr, text.len, &headings, 16, &heading_written));

    var links: [16]LinkScan = undefined;
    var link_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_scan_links(text.ptr, text.len, &links, 16, &link_written));

    var tags: [16]TagScan = undefined;
    var tag_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_scan_tags(text.ptr, text.len, &tags, 16, &tag_written));

    var blocks: [16]BlockIdScan = undefined;
    var block_written: u32 = 0;
    try std.testing.expectEqual(@as(i32, 0), marky_scan_block_ids(text.ptr, text.len, &blocks, 16, &block_written));

    var expected: [64]ScanResult = undefined;
    var expected_written: u32 = 0;

    for (headings[0..heading_written]) |h| {
        if (!test_in_fence_linear(ranges[0..range_written], h.offset)) {
            expected[expected_written] = .{
                .offset = h.offset,
                .length = h.length,
                .scan_type = @intFromEnum(scan_ref.ScanType.heading),
                .extra = h.level,
            };
            expected_written += 1;
        }
    }

    for (links[0..link_written]) |l| {
        if (!test_in_fence_linear(ranges[0..range_written], l.offset)) {
            const ty: scan_ref.ScanType = if (l.link_type == 0) .link_open else .wiki_link;
            expected[expected_written] = .{
                .offset = l.offset,
                .length = l.text_length,
                .scan_type = @intFromEnum(ty),
                .extra = if (l.target_length > std.math.maxInt(u8)) std.math.maxInt(u8) else @intCast(l.target_length),
            };
            expected_written += 1;
        }
    }

    for (tags[0..tag_written]) |t| {
        if (!test_in_fence_linear(ranges[0..range_written], t.offset)) {
            expected[expected_written] = .{
                .offset = t.offset,
                .length = t.length,
                .scan_type = @intFromEnum(scan_ref.ScanType.tag),
                .extra = 0,
            };
            expected_written += 1;
        }
    }

    for (blocks[0..block_written]) |b| {
        if (!test_in_fence_linear(ranges[0..range_written], b.offset)) {
            expected[expected_written] = .{
                .offset = b.offset,
                .length = b.length,
                .scan_type = @intFromEnum(scan_ref.ScanType.block_id),
                .extra = 0,
            };
            expected_written += 1;
        }
    }

    try std.testing.expectEqual(expected_written, multi_written);
    for (expected[0..expected_written]) |e| {
        try std.testing.expect(test_contains_scan_result(multi_out[0..multi_written], e));
    }
}

test "marky_multi_scan buffer overflow returns -2 partial" {
    const text = "^a\n^b\n^c\n^d\n";
    var out: [2]ScanResult = undefined;
    var written: u32 = 0;

    const rc = marky_multi_scan(text.ptr, text.len, null, 0, &out, 2, &written);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 2), written);
}

test "marky_multi_scan fence_count exceeds internal buffer returns -2" {
    // 257 fence ranges exceeds the 256-entry internal stack buffer.
    // Should return -2 (buffer too small), not -1 (invalid input).
    const text = "hello";
    var dummy_fences: [257]FenceRange = undefined;
    for (&dummy_fences, 0..) |*f, i| {
        f.* = FenceRange{ .start = @intCast(i * 2), .end = @intCast(i * 2 + 1) };
    }
    var out: [1]ScanResult = undefined;
    var written: u32 = 0;

    const rc = marky_multi_scan(text.ptr, text.len, &dummy_fences, 257, &out, 1, &written);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), written);
}

test "marky_multi_scan internal raw_buf overflow returns -2" {
    // The internal raw_buf is 2048 elements. Generating >= 2048 raw scan
    // candidates should return -2 rather than silently truncating results.
    // Each "#x " pattern (4 bytes) produces one tag candidate.
    // 2100 patterns = 8400 bytes, should exceed 2048 raw candidate limit.
    const pattern_count = 2100;
    var text_buf: [pattern_count * 4]u8 = undefined;
    for (0..pattern_count) |i| {
        const base = i * 4;
        text_buf[base] = '#';
        text_buf[base + 1] = 'a' + @as(u8, @intCast(i % 26));
        text_buf[base + 2] = ' ';
        text_buf[base + 3] = '\n';
    }

    var out: [4096]ScanResult = undefined;
    var written: u32 = 0;

    const rc = marky_multi_scan(&text_buf, text_buf.len, null, 0, &out, 4096, &written);
    // Should return -2 because internal raw_buf (2048) was exceeded.
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), written);
}
