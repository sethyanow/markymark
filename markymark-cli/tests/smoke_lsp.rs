//! Smoke tests for markymark LSP mode.
//!
//! Spawns the `markymark --lsp` binary, sends JSON-RPC initialize/shutdown
//! messages over stdio, and validates the server responds correctly.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// RAII guard that kills the child process on drop, preventing zombie processes.
struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child already taken")
    }

    fn take(mut self) -> Child {
        self.child.take().expect("child already taken")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Get the path to the markymark binary built by cargo.
fn markymark_bin() -> String {
    let mut path = std::env::current_exe()
        .expect("failed to get current exe path")
        .parent()
        .expect("failed to get parent dir")
        .parent()
        .expect("failed to get deps parent dir")
        .to_path_buf();
    path.push("markymark");
    path.to_string_lossy().to_string()
}

/// Format a JSON-RPC message with Content-Length header (LSP framing).
fn lsp_frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

/// Read one LSP-framed message from a BufReader.
/// Returns the parsed JSON value.
fn read_lsp_message(reader: &mut BufReader<impl Read>) -> serde_json::Value {
    // Read headers until empty line
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("failed to read header line");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = Some(
                len_str
                    .trim()
                    .parse()
                    .expect("failed to parse Content-Length"),
            );
        }
    }

    let len = content_length.expect("no Content-Length header found");
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .expect("failed to read message body");

    serde_json::from_slice(&body).expect("failed to parse JSON-RPC message")
}

/// Spawn markymark in LSP mode with piped stdio.
fn spawn_lsp() -> ChildGuard {
    let child = Command::new(markymark_bin())
        .arg("--lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn markymark --lsp");
    ChildGuard::new(child)
}

#[test]
fn lsp_initialize_capabilities_and_shutdown() {
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let mut guard = spawn_lsp();
        let child = guard.child_mut();

        let mut stdin = child.stdin.take().expect("failed to take stdin");
        let stdout = child.stdout.take().expect("failed to take stdout");
        let mut reader = BufReader::new(stdout);

        // 1. Send initialize request
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": "file:///tmp/markymark-smoke-test",
                "capabilities": {}
            }
        });
        let msg = lsp_frame(&init_request.to_string());
        stdin.write_all(&msg).expect("failed to write initialize");
        stdin.flush().expect("failed to flush");

        // 2. Read initialize response
        let response = read_lsp_message(&mut reader);

        // 3. Validate capabilities
        let result = response
            .get("result")
            .expect("initialize response missing 'result'");
        let caps = result
            .get("capabilities")
            .expect("result missing 'capabilities'");

        assert!(
            caps.get("textDocumentSync").is_some(),
            "missing textDocumentSync capability"
        );
        assert!(
            caps.get("completionProvider").is_some(),
            "missing completionProvider capability"
        );
        assert!(
            caps.get("hoverProvider").is_some(),
            "missing hoverProvider capability"
        );
        assert!(
            caps.get("definitionProvider").is_some(),
            "missing definitionProvider capability"
        );
        assert!(
            caps.get("referencesProvider").is_some(),
            "missing referencesProvider capability"
        );
        assert!(
            caps.get("renameProvider").is_some(),
            "missing renameProvider capability"
        );

        // 4. Send initialized notification
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        let msg = lsp_frame(&initialized.to_string());
        stdin
            .write_all(&msg)
            .expect("failed to write initialized notification");
        stdin.flush().expect("failed to flush");

        // 5. Send hover request on nonexistent file (tests pipeline responds)
        let hover_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///tmp/markymark-smoke-test/nonexistent.md" },
                "position": { "line": 0, "character": 0 }
            }
        });
        let msg = lsp_frame(&hover_request.to_string());
        stdin
            .write_all(&msg)
            .expect("failed to write hover request");
        stdin.flush().expect("failed to flush");

        // 6. Read hover response (may be null result or error, both are valid)
        let hover_response = read_lsp_message(&mut reader);
        assert_eq!(
            hover_response.get("id").and_then(|v| v.as_i64()),
            Some(2),
            "hover response should have id 2"
        );
        // Valid JSON-RPC: has either "result" or "error"
        assert!(
            hover_response.get("result").is_some() || hover_response.get("error").is_some(),
            "hover response must have 'result' or 'error', got: {}",
            hover_response
        );

        // 7. Send shutdown request
        let shutdown = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "shutdown",
            "params": null
        });
        let msg = lsp_frame(&shutdown.to_string());
        stdin
            .write_all(&msg)
            .expect("failed to write shutdown request");
        stdin.flush().expect("failed to flush");

        // 8. Read shutdown response
        let shutdown_response = read_lsp_message(&mut reader);
        assert_eq!(
            shutdown_response.get("id").and_then(|v| v.as_i64()),
            Some(3),
            "shutdown response should have id 3"
        );

        // 9. Send exit notification
        let exit = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        });
        let msg = lsp_frame(&exit.to_string());
        stdin.write_all(&msg).expect("failed to write exit");
        stdin.flush().expect("failed to flush");
        drop(stdin);

        // 10. Wait for process to exit
        let child = guard.take();
        let output = child.wait_with_output().expect("failed to wait on child");
        let exit_code = output.status.code().unwrap_or(-1);

        tx.send(exit_code).expect("failed to send result");
    });

    // Overall timeout: 10 seconds
    let exit_code = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("LSP smoke test timed out after 10 seconds");

    handle.join().expect("test thread panicked");

    assert_eq!(exit_code, 0, "markymark --lsp should exit with code 0");
}

#[test]
fn lsp_invalid_json_does_not_crash() {
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let mut guard = spawn_lsp();
        let child = guard.child_mut();

        let mut stdin = child.stdin.take().expect("failed to take stdin");

        // Send invalid JSON with Content-Length framing
        let invalid = "{invalid json";
        let msg = lsp_frame(invalid);
        stdin.write_all(&msg).expect("failed to write invalid JSON");
        stdin.flush().expect("failed to flush");

        // Give server a moment to process, then close stdin
        std::thread::sleep(Duration::from_millis(500));
        drop(stdin);

        // Wait for process to exit
        let child = guard.take();
        let output = child.wait_with_output().expect("failed to wait on child");

        let stderr = String::from_utf8_lossy(&output.stderr);
        // Must NOT panic
        assert!(
            !stderr.contains("panicked at"),
            "server panicked on invalid JSON: {stderr}"
        );

        // Must NOT segfault (exit code 139)
        let code = output.status.code().unwrap_or(-1);
        assert_ne!(code, 139, "server segfaulted on invalid JSON");

        tx.send(true).expect("failed to send result");
    });

    let _passed = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("LSP invalid JSON test timed out after 10 seconds");

    handle.join().expect("test thread panicked");
}
