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

    // Check Zig is installed and get version
    let zig_version = get_zig_version();
    check_zig_version(&zig_version);

    // When cross-compiling (e.g. aarch64-unknown-linux-gnu), Zig must build the static lib
    // for the same target; otherwise we'd link host-arch lib into target-arch binary.
    let zig_target = env::var("TARGET")
        .ok()
        .and_then(|t| rust_target_to_zig_target(&t));

    // Run zig build lib
    build_zig_library(&zig_dir, zig_target.as_deref());

    // Verify the library artifact exists
    // Zig produces libmarky_kernels.a on Unix, marky_kernels.lib on Windows
    let lib_path = zig_dir.join("zig-out").join("lib");
    let lib_file = if cfg!(target_os = "windows") {
        lib_path.join("marky_kernels.lib")
    } else {
        lib_path.join("libmarky_kernels.a")
    };
    if !lib_file.exists() {
        panic!(
            "zig build lib did not produce {} at {}",
            lib_file.file_name().unwrap().to_string_lossy(),
            lib_file.display()
        );
    }

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

/// Run `zig build lib` in the zig directory, optionally with -Dtarget for cross-compilation.
/// Zig requires -Dtarget=value as a single argument; passing -Dtarget and value separately fails.
fn build_zig_library(zig_dir: &std::path::Path, zig_target: Option<&str>) {
    let mut cmd = Command::new("zig");
    cmd.arg("build").arg("lib").current_dir(zig_dir);
    if let Some(t) = zig_target {
        cmd.arg(format!("-Dtarget={t}"));
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
}
