//! Smoke tests for markymark MCP mode.
//!
//! Spawns the `markymark --mcp` binary, sends MCP JSON-RPC initialize/tools-list
//! messages over stdio, and validates the server responds correctly.
//!
//! NOTE: rmcp's stdio transport uses **line-delimited JSON** (newline-terminated),
//! NOT Content-Length framing. Each message is a single JSON line terminated by `\n`.

use std::io::{BufRead, BufReader, Write};
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

/// Get the path to the test corpus directory.
fn corpus_dir() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/corpus")
}

/// Send a line-delimited JSON message (rmcp stdio transport format).
fn send_json_line(stdin: &mut impl Write, value: &serde_json::Value) {
    let json = serde_json::to_string(value).expect("failed to serialize JSON");
    writeln!(stdin, "{json}").expect("failed to write JSON line");
    stdin.flush().expect("failed to flush");
}

/// Read one line-delimited JSON message from a BufReader.
fn read_json_line(reader: &mut BufReader<impl std::io::Read>) -> serde_json::Value {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("failed to read JSON line");
    serde_json::from_str(line.trim()).unwrap_or_else(|e| {
        panic!("failed to parse JSON-RPC message: {e}\nraw line: {line:?}");
    })
}

/// Spawn markymark in MCP mode with piped stdio, pointing at the test corpus.
fn spawn_mcp() -> ChildGuard {
    let child = Command::new(markymark_bin())
        .arg("--mcp")
        .arg(corpus_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn markymark --mcp");
    ChildGuard::new(child)
}

#[test]
fn mcp_initialize_tools_list_and_shutdown() {
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let mut guard = spawn_mcp();
        let child = guard.child_mut();

        let mut stdin = child.stdin.take().expect("failed to take stdin");
        let stdout = child.stdout.take().expect("failed to take stdout");
        let mut reader = BufReader::new(stdout);

        // 1. Send MCP initialize request (line-delimited JSON)
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "smoke-test",
                    "version": "0.1.0"
                }
            }
        });
        send_json_line(&mut stdin, &init_request);

        // 2. Read initialize response
        let response = read_json_line(&mut reader);

        // 3. Validate response structure
        let result = response
            .get("result")
            .expect("initialize response missing 'result'");

        // Protocol version should be present
        assert!(
            result.get("protocolVersion").is_some(),
            "missing protocolVersion in initialize result"
        );

        // Server info should be present
        let server_info = result
            .get("serverInfo")
            .expect("missing serverInfo in initialize result");
        assert!(server_info.get("name").is_some(), "missing serverInfo.name");

        // Capabilities should include tools
        let caps = result
            .get("capabilities")
            .expect("missing capabilities in initialize result");
        assert!(
            caps.get("tools").is_some(),
            "missing tools capability in server response"
        );

        // 4. Send initialized notification
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        send_json_line(&mut stdin, &initialized);

        // 5. Send tools/list request
        let tools_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        send_json_line(&mut stdin, &tools_request);

        // 6. Read tools/list response
        let tools_response = read_json_line(&mut reader);
        let tools_result = tools_response
            .get("result")
            .expect("tools/list response missing 'result'");
        let tools = tools_result
            .get("tools")
            .and_then(|t| t.as_array())
            .expect("tools/list result missing 'tools' array");

        // Collect tool names
        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        // Validate expected tools are present
        assert!(
            tool_names.contains(&"get-outline"),
            "tools/list should include 'get-outline', got: {tool_names:?}"
        );
        assert!(
            tool_names.contains(&"search-symbols"),
            "tools/list should include 'search-symbols', got: {tool_names:?}"
        );
        assert!(
            tool_names.contains(&"find-references"),
            "tools/list should include 'find-references', got: {tool_names:?}"
        );
        assert!(
            tool_names.contains(&"rename"),
            "tools/list should include 'rename', got: {tool_names:?}"
        );

        // 7. Close stdin to signal shutdown
        drop(stdin);

        // 8. Wait for process to exit
        let child = guard.take();
        let output = child.wait_with_output().expect("failed to wait on child");
        let exit_code = output.status.code().unwrap_or(-1);

        tx.send(exit_code).expect("failed to send result");
    });

    // Overall timeout: 10 seconds
    let exit_code = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("MCP smoke test timed out after 10 seconds");

    handle.join().expect("test thread panicked");

    assert_eq!(exit_code, 0, "markymark --mcp should exit with code 0");
}

#[test]
fn mcp_invalid_json_does_not_crash() {
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let mut guard = spawn_mcp();
        let child = guard.child_mut();

        let mut stdin = child.stdin.take().expect("failed to take stdin");

        // Send invalid JSON as a line (rmcp line-delimited transport)
        writeln!(stdin, "{{invalid json").expect("failed to write invalid JSON");
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
        .expect("MCP invalid JSON test timed out after 10 seconds");

    handle.join().expect("test thread panicked");
}
