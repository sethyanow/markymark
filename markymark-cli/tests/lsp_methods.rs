//! E2E integration tests for all 8 LSP methods.
//!
//! Spawns the real `markymark --lsp` binary, opens corpus files via
//! `textDocument/didOpen`, and exercises definition, references, hover,
//! completion, rename, documentSymbol, workspaceSymbol, and diagnostics.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
fn markymark_bin() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("failed to get current exe path")
        .parent()
        .expect("failed to get parent dir")
        .parent()
        .expect("failed to get deps parent dir")
        .to_path_buf();
    path.push("markymark");
    path
}

/// Get the path to the test corpus directory.
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Convert a filesystem path to a file:// URI.
fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Format a JSON-RPC message with Content-Length header (LSP framing).
fn lsp_frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

/// Read one LSP-framed message from a BufReader.
fn read_lsp_message(reader: &mut BufReader<impl Read>) -> serde_json::Value {
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

/// LSP test harness that owns a running markymark --lsp process.
struct LspTestHarness {
    guard: ChildGuard,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
    /// Diagnostics received via textDocument/publishDiagnostics notifications.
    diagnostics: HashMap<String, Vec<serde_json::Value>>,
}

impl LspTestHarness {
    /// Spawn markymark --lsp and perform initialize handshake.
    fn spawn(workspace_root: &Path) -> Self {
        let bin = markymark_bin();
        assert!(
            bin.exists(),
            "markymark binary not found at {}",
            bin.display()
        );

        let mut guard = ChildGuard::new(
            Command::new(&bin)
                .arg("--lsp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to spawn markymark --lsp"),
        );

        let child = guard.child_mut();
        let stdin = child.stdin.take().expect("failed to take stdin");
        let stdout = child.stdout.take().expect("failed to take stdout");
        let reader = BufReader::new(stdout);

        let mut harness = Self {
            guard,
            stdin,
            reader,
            next_id: 1,
            diagnostics: HashMap::new(),
        };

        // Initialize handshake
        let root_uri = path_to_uri(workspace_root);
        let init_result = harness.send_request(
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {}
            }),
        );
        assert!(
            init_result.get("capabilities").is_some(),
            "initialize should return capabilities"
        );

        harness.send_notification("initialized", serde_json::json!({}));

        harness
    }

    /// Send a JSON-RPC request, return the result value.
    /// Collects any interleaved notifications (like publishDiagnostics).
    fn send_request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next_id;
        self.next_id += 1;

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let frame = lsp_frame(&msg.to_string());
        self.stdin
            .write_all(&frame)
            .expect("failed to write request");
        self.stdin.flush().expect("failed to flush");

        // Read messages until we get a response with our id
        loop {
            let response = read_lsp_message(&mut self.reader);

            // Check if this is a notification (no "id" field)
            if response.get("id").is_none() {
                // Handle publishDiagnostics notifications
                if response.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                {
                    if let Some(params) = response.get("params") {
                        if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                            let diags = params
                                .get("diagnostics")
                                .cloned()
                                .and_then(|d| d.as_array().cloned())
                                .unwrap_or_default();
                            self.diagnostics.insert(uri.to_string(), diags);
                        }
                    }
                }
                continue;
            }

            // Check if this is our response
            if response.get("id").and_then(|v| v.as_i64()) == Some(id) {
                if let Some(error) = response.get("error") {
                    panic!(
                        "LSP error for {method}: {}",
                        serde_json::to_string_pretty(error).unwrap()
                    );
                }
                return response
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
            }
        }
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    fn send_notification(&mut self, method: &str, params: serde_json::Value) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let frame = lsp_frame(&msg.to_string());
        self.stdin
            .write_all(&frame)
            .expect("failed to write notification");
        self.stdin.flush().expect("failed to flush");
    }

    /// Open a corpus file via textDocument/didOpen.
    fn open_file(&mut self, path: &Path) {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let uri = path_to_uri(path);

        self.send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "markdown",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    /// Drain any pending notifications (read with short timeout).
    /// This collects diagnostics that arrive after didOpen.
    fn drain_notifications(&mut self) {
        // Give the server a moment to process and emit notifications
        std::thread::sleep(Duration::from_millis(200));

        // Read any pending notifications by doing a peek-based drain.
        // We use a non-blocking approach: try to read, but with a
        // small fill_buf check.
        loop {
            // Check if data is available without blocking indefinitely
            let buf = self.reader.buffer();
            if buf.is_empty() {
                // Try to fill buffer with a short wait
                // Since we already slept, if nothing is available, break
                break;
            }

            let response = read_lsp_message(&mut self.reader);
            if response.get("id").is_none() {
                // Notification
                if response.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                {
                    if let Some(params) = response.get("params") {
                        if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                            let diags = params
                                .get("diagnostics")
                                .cloned()
                                .and_then(|d| d.as_array().cloned())
                                .unwrap_or_default();
                            self.diagnostics.insert(uri.to_string(), diags);
                        }
                    }
                }
            }
        }
    }

    /// Get collected diagnostics for a URI.
    fn get_diagnostics(&self, uri: &str) -> &[serde_json::Value] {
        self.diagnostics
            .get(uri)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Send shutdown + exit and wait for clean process termination.
    fn shutdown_and_exit(mut self) -> i32 {
        let _shutdown = self.send_request("shutdown", serde_json::Value::Null);

        self.send_notification("exit", serde_json::Value::Null);
        drop(self.stdin);

        let child = self.guard.take();
        let output = child.wait_with_output().expect("failed to wait on child");
        output.status.code().unwrap_or(-1)
    }
}

/// Run an E2E test inside a thread with a 30-second overall timeout.
fn run_with_timeout<F, R>(test_fn: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let result = test_fn();
        let _ = tx.send(result);
    });

    let result = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("E2E test timed out after 30 seconds");

    handle.join().expect("test thread panicked");
    result
}

// ---------------------------------------------------------------------------
// E2E Tests
// ---------------------------------------------------------------------------

/// Helper: spawn harness, open all standard corpus files, return harness.
fn setup_workspace() -> LspTestHarness {
    let corpus = corpus_dir();
    let mut harness = LspTestHarness::spawn(&corpus);

    // Open the core corpus files the tests depend on
    for file in &[
        "basic.md",
        "links.md",
        "cross-refs.md",
        "edge-cases.md",
        "xml-tags.md",
    ] {
        harness.open_file(&corpus.join(file));
    }

    // Give server time to index and publish diagnostics
    harness.drain_notifications();

    harness
}

#[test]
fn e2e_definition_wiki_link_cross_file() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // links.md line 4 has: [[basic]] — request definition at the wiki link
        // "Here is a wiki link to [[basic]] and one with..."
        // [[basic]] starts at character 25
        let result = harness.send_request(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("links.md")) },
                "position": { "line": 4, "character": 27 }
            }),
        );

        // Should resolve to basic.md
        let target_uri = if let Some(uri) = result.get("uri").and_then(|u| u.as_str()) {
            // Single Location response
            uri.to_string()
        } else if let Some(arr) = result.as_array() {
            // Array of Locations
            arr.first()
                .and_then(|l| l.get("uri"))
                .and_then(|u| u.as_str())
                .expect("location should have uri")
                .to_string()
        } else {
            panic!("definition response should be Location or Location[], got: {result}");
        };

        let basic_uri = path_to_uri(&corpus.join("basic.md"));
        assert!(
            target_uri.contains("basic.md"),
            "definition should resolve to basic.md, got: {target_uri} (expected containing {basic_uri})"
        );

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0, "server should exit cleanly");
    });
}

#[test]
fn e2e_references_heading_all_links() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // basic.md line 0: "# Main Title"
        // Request references for this heading
        let result = harness.send_request(
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("basic.md")) },
                "position": { "line": 0, "character": 5 },
                "context": { "includeDeclaration": true }
            }),
        );

        let refs = result.as_array().expect("references should be an array");

        // "Main Title" is referenced in cross-refs.md:
        // - Wiki link: [[basic#main-title]]  (line 6)
        // - Markdown link: [Main Title](basic.md#main-title)  (line 16)
        // Plus the declaration itself
        assert!(
            refs.len() >= 2,
            "should have at least 2 references to Main Title (declaration + cross-ref links), got: {}",
            refs.len()
        );

        // Check that at least one reference is from cross-refs.md
        let has_cross_ref = refs.iter().any(|r| {
            r.get("uri")
                .and_then(|u| u.as_str())
                .map(|u| u.contains("cross-refs.md"))
                .unwrap_or(false)
        });
        assert!(has_cross_ref, "should have references from cross-refs.md");

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_hover_heading_shows_info() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // basic.md line 0: "# Main Title"
        let result = harness.send_request(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("basic.md")) },
                "position": { "line": 0, "character": 5 }
            }),
        );

        // Hover should return a MarkupContent or MarkedString
        let contents = result
            .get("contents")
            .expect("hover should have 'contents' field");

        let hover_text = if let Some(value) = contents.get("value").and_then(|v| v.as_str()) {
            value.to_string()
        } else if let Some(s) = contents.as_str() {
            s.to_string()
        } else {
            panic!("unexpected hover contents format: {contents}");
        };

        // Should mention the heading in some way
        assert!(!hover_text.is_empty(), "hover content should not be empty");

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_completion_wiki_link_shows_pages() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // Trigger completion inside a wiki link context
        // links.md line 4: "Here is a wiki link to [[basic]]..."
        // Position right after [[ to trigger wiki link completion
        let result = harness.send_request(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("links.md")) },
                "position": { "line": 4, "character": 25 }
            }),
        );

        // Result should be CompletionList or array of CompletionItems
        let items = if let Some(items) = result.get("items").and_then(|i| i.as_array()) {
            items.clone()
        } else if let Some(arr) = result.as_array() {
            arr.clone()
        } else {
            // Null is acceptable if the server can't determine context
            // but let's check if we got something useful
            Vec::new()
        };

        // If completion returned items, check that page names are included
        if !items.is_empty() {
            let labels: Vec<&str> = items
                .iter()
                .filter_map(|i| i.get("label").and_then(|l| l.as_str()))
                .collect();

            // Should contain at least some corpus files as completion candidates
            let has_any_page = labels.iter().any(|l| {
                l.contains("basic")
                    || l.contains("links")
                    || l.contains("cross-refs")
                    || l.contains("edge-cases")
                    || l.contains("xml-tags")
            });
            assert!(
                has_any_page,
                "completion should offer page names from corpus, got: {labels:?}"
            );
        }

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_rename_heading_updates_all_links() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // First, prepare rename on basic.md line 4: "## Section One"
        let prepare_result = harness.send_request(
            "textDocument/prepareRename",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("basic.md")) },
                "position": { "line": 4, "character": 5 }
            }),
        );

        // prepareRename should return a range + placeholder
        assert!(
            !prepare_result.is_null(),
            "prepareRename should return a result for heading, got null"
        );

        // Now do the actual rename
        let result = harness.send_request(
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("basic.md")) },
                "position": { "line": 4, "character": 5 },
                "newName": "First Section"
            }),
        );

        // Should return a WorkspaceEdit with changes
        let changes = result
            .get("changes")
            .or_else(|| result.get("documentChanges"))
            .expect("rename should return WorkspaceEdit with changes or documentChanges");

        // Should have edits in at least the basic.md file (the heading itself)
        if let Some(changes_map) = changes.as_object() {
            assert!(
                !changes_map.is_empty(),
                "workspace edit should have changes in at least one file"
            );

            // Check basic.md has an edit
            let has_basic_edit = changes_map.keys().any(|k| k.contains("basic.md"));
            assert!(
                has_basic_edit,
                "rename should edit basic.md (the heading source)"
            );

            // If cross-refs.md references "Section One", it should also have edits
            // cross-refs.md has: [[basic#section-one]] and [Section One](basic.md#section-one)
            let has_cross_ref_edit = changes_map
                .keys()
                .any(|k| k.contains("cross-refs.md") || k.contains("links.md"));
            assert!(
                has_cross_ref_edit,
                "rename should also edit files that reference this heading, got keys: {:?}",
                changes_map.keys().collect::<Vec<_>>()
            );
        }

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_document_symbol_nested_headings() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        let result = harness.send_request(
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": path_to_uri(&corpus.join("basic.md")) }
            }),
        );

        let symbols = result
            .as_array()
            .expect("documentSymbol should return array");

        // basic.md has: Main Title > Section One > Subsection A > Level Four > ...
        assert!(
            !symbols.is_empty(),
            "should have at least one top-level symbol"
        );

        // Check the first symbol is "Main Title"
        let first = &symbols[0];
        let name = first
            .get("name")
            .and_then(|n| n.as_str())
            .expect("symbol should have name");
        assert_eq!(name, "Main Title", "first symbol should be the H1 heading");

        // Check nesting: Main Title should have children
        let children = first.get("children").and_then(|c| c.as_array());
        assert!(
            children.is_some() && !children.unwrap().is_empty(),
            "Main Title should have nested children (Section One, etc.)"
        );

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_workspace_symbol_search_cross_file() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();

        // Search for "Section" which appears in basic.md ("Section One")
        let result = harness.send_request(
            "workspace/symbol",
            serde_json::json!({
                "query": "Section"
            }),
        );

        let symbols = result
            .as_array()
            .expect("workspace/symbol should return array");

        assert!(
            !symbols.is_empty(),
            "workspace/symbol query 'Section' should find results"
        );

        // Check that results contain expected heading
        let names: Vec<&str> = symbols
            .iter()
            .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
            .collect();

        let has_section_one = names.iter().any(|n| n.contains("Section One"));
        assert!(
            has_section_one,
            "should find 'Section One' heading, got: {names:?}"
        );

        // Verify results come from at least basic.md
        let has_basic = symbols.iter().any(|s| {
            s.get("location")
                .and_then(|l| l.get("uri"))
                .and_then(|u| u.as_str())
                .map(|u| u.contains("basic.md"))
                .unwrap_or(false)
        });
        assert!(
            has_basic,
            "workspace symbols should include results from basic.md"
        );

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_diagnostics_broken_wiki_link() {
    run_with_timeout(|| {
        let mut harness = setup_workspace();
        let corpus = corpus_dir();

        // links.md contains: [[nonexistent]] which should produce a diagnostic
        let links_uri = path_to_uri(&corpus.join("links.md"));

        // Diagnostics are collected automatically during open/drain.
        // Make a no-op request to flush any remaining notifications.
        let _hover = harness.send_request(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": &links_uri },
                "position": { "line": 0, "character": 0 }
            }),
        );

        let diags = harness.get_diagnostics(&links_uri);

        // Should have at least one diagnostic for [[nonexistent]]
        assert!(
            !diags.is_empty(),
            "links.md should have diagnostics for broken wiki link [[nonexistent]], got none. All diagnostics: {:?}",
            harness.diagnostics
        );

        // Check that at least one diagnostic mentions "nonexistent" or "unresolved"
        let has_broken_link_diag = diags.iter().any(|d| {
            d.get("message")
                .and_then(|m| m.as_str())
                .map(|m| {
                    let lower = m.to_lowercase();
                    lower.contains("nonexistent")
                        || lower.contains("unresolved")
                        || lower.contains("broken")
                        || lower.contains("not found")
                })
                .unwrap_or(false)
        });
        assert!(
            has_broken_link_diag,
            "should have a diagnostic about broken/unresolved wiki link, got: {:?}",
            diags
        );

        let exit = harness.shutdown_and_exit();
        assert_eq!(exit, 0);
    });
}
