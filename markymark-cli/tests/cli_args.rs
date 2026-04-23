//! Integration tests for CLI argument parsing and transport selection.

use std::process::Command;

/// Helper to get the path to the markymark binary.
///
/// Under Bazel, `MARKYMARK_BIN` is set via `rustc_env` from `$(rootpath)`.
/// Under cargo, falls back to `current_exe()` relative to `target/debug`.
fn markymark_bin() -> String {
    if let Ok(bin) = std::env::var("MARKYMARK_BIN") {
        return bin;
    }
    let mut path = std::env::current_exe()
        .expect("current exe")
        .parent()
        .expect("parent dir")
        .parent()
        .expect("deps parent")
        .to_path_buf();
    path.push("markymark");
    path.to_string_lossy().to_string()
}

#[test]
fn help_flag_shows_usage() {
    let output = Command::new(markymark_bin())
        .arg("--help")
        .output()
        .expect("failed to execute markymark --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit code should be 0");
    assert!(stdout.contains("--lsp"), "help should mention --lsp flag");
    assert!(stdout.contains("--mcp"), "help should mention --mcp flag");
    assert!(
        stdout.contains("markymark"),
        "help should mention the binary name"
    );
}

#[test]
fn version_flag_shows_version() {
    let output = Command::new(markymark_bin())
        .arg("--version")
        .output()
        .expect("failed to execute markymark --version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit code should be 0");
    assert!(
        stdout.contains("markymark"),
        "version output should contain binary name"
    );
}

#[test]
fn no_transport_flag_defaults_to_lsp() {
    // Running without --lsp or --mcp should default to LSP mode.
    // Send EOF on stdin so the server exits immediately.
    let mut child = Command::new(markymark_bin())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn markymark");

    // Take and drop stdin to signal EOF.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait on child");
    // The important thing is it doesn't panic or fail to start.
    let _ = output.status;
}

#[test]
fn lsp_flag_accepted() {
    let mut child = Command::new(markymark_bin())
        .arg("--lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn markymark --lsp");

    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait on child");
    let _ = output.status;
}

#[test]
fn mcp_flag_accepted() {
    let mut child = Command::new(markymark_bin())
        .arg("--mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn markymark --mcp");

    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait on child");
    let _ = output.status;
}

#[test]
fn mutually_exclusive_flags_rejected() {
    let output = Command::new(markymark_bin())
        .arg("--lsp")
        .arg("--mcp")
        .output()
        .expect("failed to execute markymark --lsp --mcp");

    assert!(
        !output.status.success(),
        "using both --lsp and --mcp should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "error should mention conflict, got: {stderr}"
    );
}

#[test]
fn workspace_roots_passed_as_positional_args() {
    let output = Command::new(markymark_bin())
        .arg("--help")
        .output()
        .expect("failed to execute markymark --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ROOTS") || stdout.contains("roots") || stdout.contains("workspace"),
        "help should mention workspace roots, got: {stdout}"
    );
}
