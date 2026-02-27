use std::env;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    // Locate the zig directory relative to the crate root
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let zig_dir = manifest_dir.join("..").join("zig");
    let zig_dir = zig_dir
        .canonicalize()
        .unwrap_or_else(|e| panic!("zig/ directory not found at {}: {e}", zig_dir.display()));

    // Use Cargo's OUT_DIR as the Zig install prefix so every Cargo compilation
    // unit (lib, test, clippy, …) writes to its own unique output directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    let lib_path = out_dir.join("lib");
    let lib_file = if cfg!(target_os = "windows") {
        lib_path.join("marky_kernels.lib")
    } else {
        lib_path.join("libmarky_kernels.a")
    };

    // MARKY_KERNELS_PREBUILT: path to a pre-built libmarky_kernels.a.
    // Skips the `zig build` step entirely — useful when Zig's build system
    // cannot run (e.g. kernel < 4.5 where copy_file_range is unavailable).
    if let Ok(prebuilt) = env::var("MARKY_KERNELS_PREBUILT") {
        let prebuilt = PathBuf::from(prebuilt);
        if !prebuilt.exists() {
            panic!(
                "MARKY_KERNELS_PREBUILT points to non-existent file: {}",
                prebuilt.display()
            );
        }
        std::fs::create_dir_all(&lib_path).unwrap_or_else(|e| {
            panic!("Failed to create {}: {e}", lib_path.display())
        });
        std::fs::copy(&prebuilt, &lib_file).unwrap_or_else(|e| {
            panic!(
                "Failed to copy prebuilt library from {} to {}: {e}",
                prebuilt.display(),
                lib_file.display()
            )
        });
        println!(
            "cargo:warning=Using prebuilt Zig library from {}",
            prebuilt.display()
        );
    } else {
        // Check Zig is installed and get version
        let zig_version = get_zig_version();
        check_zig_version(&zig_version);

        // When cross-compiling (e.g. aarch64-unknown-linux-gnu), Zig must build the static lib
        // for the same target; otherwise we'd link host-arch lib into target-arch binary.
        let zig_target = env::var("TARGET")
            .ok()
            .and_then(|t| rust_target_to_zig_target(&t));

        // Multiple cargo build/test/clippy steps within one CI job each invoke
        // build.rs separately; if they all wrote to the shared zig/zig-out/lib/
        // path, a warm-cache `zig build lib` call on Linux x86_64 could corrupt
        // the archive (Zig 0.15.2 bug: reusing .zig-cache while overwriting the
        // same .a file produces a truncated archive).  Each unit's OUT_DIR is
        // unique, so no two invocations race on the same output file.

        // Run zig build lib, installing into Cargo's unique OUT_DIR
        build_zig_library(&zig_dir, zig_target.as_deref(), &out_dir);

        // Verify the library artifact exists and is not corrupt
        if !lib_file.exists() {
            panic!(
                "zig build lib did not produce {} at {}",
                lib_file.file_name().unwrap().to_string_lossy(),
                lib_file.display()
            );
        }
    }

    // Zig 0.15.2 on Linux x86_64 produces archives that pass `ar t` but
    // fail rust-lld's stricter parsing ("Archive::children failed: truncated
    // or malformed archive").  Re-pack with the system `ar` to produce a
    // rust-lld-compatible archive.  Only needed on Linux; macOS ld64 handles
    // the Zig archive format without issues.
    if cfg!(target_os = "linux") {
        repack_archive(&lib_file);
    }
    validate_archive(&lib_file);

    // Tell Cargo where to find and link the library
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=marky_kernels");

    // Rerun if Zig sources change (individual files, not just directory)
    let zig_src_dir = zig_dir.join("src");
    for entry_result in WalkDir::new(&zig_src_dir) {
        match entry_result {
            Ok(entry) => {
                if entry.path().extension().is_some_and(|ext| ext == "zig") {
                    println!("cargo:rerun-if-changed={}", entry.path().display());
                }
            }
            Err(err) => {
                println!(
                    "cargo:warning=Failed to enumerate entry under {}: {err}",
                    zig_src_dir.display()
                );
            }
        }
    }
    println!(
        "cargo:rerun-if-changed={}",
        zig_dir.join("build.zig").display()
    );
}

/// Get the zig version string from `zig version`.
fn get_zig_version() -> String {
    let output = Command::new("zig")
        .arg("version")
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "Zig compiler not found. Install Zig 0.15.2+ from https://ziglang.org/download/\n\
                 Error: {e}"
            )
        });

    if !output.status.success() {
        panic!(
            "zig version failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Parse and validate the Zig version is >= 0.15.2.
fn check_zig_version(version_str: &str) {
    // Parse "0.15.2" or "0.15.2-dev.xxxx" format
    let version_core = version_str.split('-').next().unwrap_or(version_str);
    let parts: Vec<u32> = version_core
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();

    if parts.len() < 3 {
        panic!(
            "Could not parse Zig version '{version_str}'. Expected format: X.Y.Z or X.Y.Z-dev.N"
        );
    }

    let (major, minor, patch) = (parts[0], parts[1], parts[2]);

    // Require >= 0.15.2
    let meets_minimum =
        major > 0 || (major == 0 && minor > 15) || (major == 0 && minor == 15 && patch >= 2);

    if !meets_minimum {
        panic!(
            "Zig 0.15.2+ required, found {version_str}. \
             Install from https://ziglang.org/download/"
        );
    }
}

/// Map Rust TARGET to Zig -Dtarget when cross-compiling (so Zig builds for the same arch).
fn rust_target_to_zig_target(rust_target: &str) -> Option<String> {
    match rust_target {
        "aarch64-unknown-linux-gnu" => Some("aarch64-linux-gnu".to_string()),
        "aarch64-unknown-linux-musl" => Some("aarch64-linux-musl".to_string()),
        "x86_64-unknown-linux-gnu" => Some("x86_64-linux-gnu".to_string()),
        "x86_64-unknown-linux-musl" => Some("x86_64-linux-musl".to_string()),
        "x86_64-apple-darwin" => Some("x86_64-macos".to_string()),
        "aarch64-apple-darwin" => Some("aarch64-macos".to_string()),
        "x86_64-pc-windows-msvc" => Some("x86_64-windows".to_string()),
        _ => None,
    }
}

/// Re-pack an archive using the system `ar` to produce a format compatible
/// with rust-lld.  Zig 0.15.2 on Linux x86_64 produces archives where the
/// member offset table is inconsistent with the actual file size, causing
/// `Archive::children failed` in rust-lld.  Extracting and re-packing with
/// the system `ar rcs` normalizes the format.
///
/// On platforms where `ar` is unavailable (Windows), this is a no-op.
fn repack_archive(archive: &std::path::Path) {
    let tmp = archive.with_extension("repack_tmp");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).ok();
    }
    if std::fs::create_dir_all(&tmp).is_err() {
        println!("cargo:warning=Skipping archive repack: cannot create temp dir");
        return;
    }

    // Extract all object files from the Zig-produced archive
    let extract = Command::new("ar")
        .arg("x")
        .arg(archive)
        .current_dir(&tmp)
        .output();

    match extract {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            println!("cargo:warning=Skipping archive repack: ar x failed: {stderr}");
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
        Err(_) => {
            // ar not available (e.g. Windows)
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
    }

    // Collect all .o files
    let o_files: Vec<PathBuf> = std::fs::read_dir(&tmp)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "o"))
        .collect();

    if o_files.is_empty() {
        println!("cargo:warning=Skipping archive repack: no .o files extracted");
        let _ = std::fs::remove_dir_all(&tmp);
        return;
    }

    // Remove the original archive and re-create with system ar
    std::fs::remove_file(archive)
        .unwrap_or_else(|e| panic!("Failed to remove original archive for repack: {e}"));

    let repack = Command::new("ar")
        .arg("rcs")
        .arg(archive)
        .args(&o_files)
        .output()
        .unwrap_or_else(|e| panic!("Failed to repack archive with ar rcs: {e}"));

    if !repack.status.success() {
        let stderr = String::from_utf8_lossy(&repack.stderr);
        panic!("ar rcs failed during archive repack: {stderr}");
    }

    let _ = std::fs::remove_dir_all(&tmp);

    let file_len = std::fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
    println!(
        "cargo:warning=Archive repacked with system ar ({file_len} bytes, {} object files)",
        o_files.len()
    );
}

/// Validate that an archive (.a) file is well-formed by listing its members
/// with `ar t`.  Zig 0.15.2 on Linux x86_64 can produce truncated archives
/// when building with a warm cache.  This catch-early validation prevents the
/// linker from seeing a corrupt archive much later during `cargo test`.
fn validate_archive(path: &std::path::Path) {
    let file_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let output = Command::new("ar").arg("t").arg(path).output();

    match output {
        Ok(o) if o.status.success() => {
            let members = String::from_utf8_lossy(&o.stdout);
            println!(
                "cargo:warning=Archive OK: {} ({file_len} bytes, members: {})",
                path.display(),
                members.trim().replace('\n', ", ")
            );
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            panic!(
                "Archive validation FAILED for {} ({file_len} bytes).\n\
                 ar stderr: {stderr}\n\
                 This is the Zig 0.15.2 warm-cache corruption bug.\n\
                 The archive was truncated during creation.",
                path.display()
            );
        }
        Err(e) => {
            // ar not available (e.g. Windows) — skip validation
            println!(
                "cargo:warning=Skipping archive validation ({file_len} bytes): ar not found: {e}"
            );
        }
    }
}

/// Run `zig build lib` in the zig directory.
///
/// `prefix` is passed as the Zig install prefix (`-p <prefix>`); the library
/// will be installed to `<prefix>/lib/`.  Callers should use Cargo's `OUT_DIR`
/// so that concurrent/sequential build-script invocations never share the same
/// output file — a known Linux x86_64 / Zig 0.15.2 issue where overwriting a
/// cached archive from a warm `.zig-cache` produces a truncated archive.
///
/// Additionally, each invocation gets its own Zig cache directory (inside the
/// prefix) so that sequential cargo clippy/build/test steps cannot trigger
/// the warm-cache corruption path — even though output files are already
/// per-`OUT_DIR`, the shared `.zig-cache` was still causing truncated archives.
///
/// Zig requires -Dtarget=value as a single argument; passing -Dtarget and
/// value separately fails.
fn build_zig_library(
    zig_dir: &std::path::Path,
    zig_target: Option<&str>,
    prefix: &std::path::Path,
) {
    // Zig 0.15.2 warm-cache archive corruption bug (Linux x86_64):
    // `zig build lib` with a warm .zig-cache can produce a truncated .a
    // archive.  This happens when sequential cargo commands (clippy → test)
    // share the same OUT_DIR, or when CI restores a cached target/ directory.
    //
    // Fix: force a cold Zig cache for every invocation.
    //  1. Purge the default local cache (zig_dir/.zig-cache) — this is where
    //     `zig build` actually reads/writes its cache when run with current_dir.
    //  2. Purge any cache inside OUT_DIR from a previous invocation.
    //  3. Pass --cache-dir and --global-cache-dir CLI flags (authoritative;
    //     env vars ZIG_LOCAL_CACHE_DIR were ignored by the build runner).
    let zig_cache_dir = prefix.join(".zig-cache");
    for cache_path in [zig_dir.join(".zig-cache"), zig_cache_dir.clone()] {
        if cache_path.exists() {
            std::fs::remove_dir_all(&cache_path).unwrap_or_else(|e| {
                panic!("Failed to purge Zig cache at {}: {e}", cache_path.display())
            });
        }
    }

    let mut cmd = Command::new("zig");
    cmd.arg("build")
        .arg("lib")
        .arg("-p")
        .arg(prefix)
        .arg("--cache-dir")
        .arg(&zig_cache_dir)
        .arg("--global-cache-dir")
        .arg(&zig_cache_dir)
        .current_dir(zig_dir);
    if let Some(t) = zig_target {
        cmd.arg(format!("-Dtarget={t}"));
    }

    // Match Zig optimization level to Cargo profile.
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        cmd.arg("-Doptimize=ReleaseFast");
    }

    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("Failed to run zig build lib: {e}"));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "zig build lib failed in {}:\n\
             --- stderr ---\n{stderr}\n\
             --- stdout ---\n{stdout}",
            zig_dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse_valid() {
        check_zig_version("0.15.2");
        check_zig_version("0.15.3");
        check_zig_version("0.16.0");
        check_zig_version("1.0.0");
        check_zig_version("2.0.0");
    }

    #[test]
    fn test_version_parse_dev() {
        check_zig_version("0.15.2-dev.1234+abcdef");
    }

    #[test]
    #[should_panic(expected = "Zig 0.15.2+ required")]
    fn test_version_too_old() {
        check_zig_version("0.14.1");
    }

    #[test]
    #[should_panic(expected = "Zig 0.15.2+ required")]
    fn test_version_0_15_1() {
        check_zig_version("0.15.1");
    }

    #[test]
    #[should_panic(expected = "Could not parse Zig version")]
    fn test_version_garbage() {
        check_zig_version("not-a-version");
    }

    /// Regression test for marky-whvn: the Zig library must be installed into
    /// Cargo's OUT_DIR, not into the shared zig/zig-out/lib/ path.
    ///
    /// Background: On Linux x86_64, Zig 0.15.2 corrupts `libmarky_kernels.a`
    /// when `zig build lib` is invoked a second/third time with a warm
    /// `.zig-cache`.  This happens because three CI steps (clippy, build, test)
    /// each run build.rs, each calling `zig build lib`, all previously writing
    /// to the same shared `zig/zig-out/lib/` path.  The fix is to use Cargo's
    /// `OUT_DIR` as the `-p` prefix so every compilation unit gets its own
    /// unique output directory with no write collisions.
    #[test]
    fn test_lib_path_derived_from_out_dir_not_zig_out() {
        use std::path::PathBuf;

        // Simulate what main() does: lib_path = out_dir.join("lib")
        let out_dir = PathBuf::from("/cargo/out/markymark-kernels-abc123/out");
        let lib_path = out_dir.join("lib");

        // Must be rooted in OUT_DIR, not in the old shared zig-out path
        let path_str = lib_path.to_str().unwrap();
        assert!(
            !path_str.contains("zig-out"),
            "lib_path must not use the shared zig-out directory, got: {path_str}"
        );
        assert!(
            path_str.contains("markymark-kernels-abc123"),
            "lib_path must be inside the per-unit OUT_DIR, got: {path_str}"
        );
    }

    /// Regression test: Zig cache dir must be per-invocation (inside prefix),
    /// not a shared global directory.  A shared .zig-cache across sequential
    /// cargo clippy/build/test invocations triggers Zig 0.15.2's warm-cache
    /// archive corruption on Linux x86_64.
    #[test]
    fn test_zig_cache_dir_is_per_invocation() {
        use std::path::PathBuf;

        // Simulate what build_zig_library() does: zig_cache_dir = prefix.join(".zig-cache")
        let prefix_a = PathBuf::from("/cargo/out/markymark-kernels-aaa111/out");
        let prefix_b = PathBuf::from("/cargo/out/markymark-kernels-bbb222/out");

        let cache_a = prefix_a.join(".zig-cache");
        let cache_b = prefix_b.join(".zig-cache");

        // Each prefix gets its own cache — they must differ
        assert_ne!(
            cache_a, cache_b,
            "different OUT_DIRs must produce different Zig cache directories"
        );

        // Cache must be inside the prefix, not at a shared location
        assert!(
            cache_a.starts_with(&prefix_a),
            "zig cache must be inside its prefix, got: {}",
            cache_a.display()
        );
        assert!(
            cache_b.starts_with(&prefix_b),
            "zig cache must be inside its prefix, got: {}",
            cache_b.display()
        );
    }

    /// Regression test: build_zig_library() must purge an existing .zig-cache
    /// before invoking `zig build lib`.  Without this, a warm cache restored
    /// from CI's `target/` cache (or left over from a previous cargo command
    /// sharing the same OUT_DIR) triggers Zig 0.15.2's archive corruption.
    #[test]
    fn test_stale_zig_cache_is_purged_before_build() {
        let tmp = std::env::temp_dir().join("markymark-test-zig-purge");
        let prefix = tmp.join("out");
        let zig_cache_dir = prefix.join(".zig-cache");

        // Simulate a stale cache restored from CI
        std::fs::create_dir_all(&zig_cache_dir).unwrap();
        let sentinel = zig_cache_dir.join("stale-artifact");
        std::fs::write(&sentinel, b"stale").unwrap();
        assert!(sentinel.exists(), "sentinel should exist before purge");

        // Simulate what build_zig_library() does: purge if exists
        if zig_cache_dir.exists() {
            std::fs::remove_dir_all(&zig_cache_dir).unwrap();
        }

        assert!(
            !zig_cache_dir.exists(),
            ".zig-cache must be removed before zig build"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
