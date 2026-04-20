//! E2E integration tests for all 10 MCP tools.
//!
//! Spawns the real `markymark --mcp` binary, performs MCP initialization, then
//! exercises every tool via `tools/call` over stdio using line-delimited JSON.
//!
//! NOTE: rmcp's stdio transport uses **line-delimited JSON** (newline-terminated),
//! NOT Content-Length framing. Each message is a single JSON line terminated by `\n`.

use std::io::{BufRead, BufReader, Write};
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

/// Get the path to the markymark binary. Honours `MARKYMARK_BIN` (Bazel)
/// then falls back to `current_exe()` under cargo.
fn markymark_bin() -> PathBuf {
    if let Ok(bin) = std::env::var("MARKYMARK_BIN") {
        return PathBuf::from(bin);
    }
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

/// Get the path to the test corpus directory. Honours `MARKYMARK_CORPUS_DIR`
/// (Bazel — anchored on a specific file, test strips filename) then falls
/// back to `CARGO_MANIFEST_DIR/tests/corpus` under cargo.
fn corpus_dir() -> PathBuf {
    if let Ok(path) = std::env::var("MARKYMARK_CORPUS_DIR") {
        let p = PathBuf::from(&path);
        return if p.is_file() {
            p.parent()
                .expect("corpus file has parent dir")
                .to_path_buf()
        } else {
            p
        };
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Convert a filesystem path to a file:// URI.
fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
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

/// MCP test harness that owns a running markymark --mcp process.
struct McpTestHarness {
    guard: ChildGuard,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

impl McpTestHarness {
    /// Spawn markymark --mcp and perform MCP initialize handshake.
    fn spawn(corpus_root: &Path) -> Self {
        let bin = markymark_bin();
        assert!(
            bin.exists(),
            "markymark binary not found at {}",
            bin.display()
        );

        let mut guard = ChildGuard::new(
            Command::new(&bin)
                .arg("--mcp")
                .arg(corpus_root.to_str().unwrap())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to spawn markymark --mcp"),
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
        };

        // Send initialize
        let init_result = harness.send_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "e2e-mcp-test",
                    "version": "0.1.0"
                }
            }),
        );
        assert!(
            init_result.get("protocolVersion").is_some(),
            "initialize should return protocolVersion"
        );

        // Send initialized notification
        harness.send_notification("notifications/initialized");

        harness
    }

    /// Send a JSON-RPC request, return the result value.
    fn send_request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next_id;
        self.next_id += 1;

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        send_json_line(&mut self.stdin, &msg);

        // Read messages until we get a response with our id.
        // MCP servers may send notifications between requests.
        loop {
            let response = read_json_line(&mut self.reader);

            // Skip notifications (no "id" field)
            if response.get("id").is_none() {
                continue;
            }

            // Check if this is our response
            if response.get("id").and_then(|v| v.as_i64()) == Some(id) {
                if let Some(error) = response.get("error") {
                    panic!(
                        "MCP error for {method}: {}",
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
    fn send_notification(&mut self, method: &str) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method
        });
        send_json_line(&mut self.stdin, &msg);
    }

    /// Call an MCP tool and return the structured content.
    /// Returns the parsed JSON from the first text content block.
    fn call_tool(&mut self, tool_name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let result = self.send_request(
            "tools/call",
            serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            }),
        );

        // MCP tool results have a "content" array with content blocks.
        // Extract the first text block and parse as JSON.
        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .expect("tool result should have 'content' array");

        assert!(
            !content.is_empty(),
            "tool {tool_name} returned empty content"
        );

        let first = &content[0];
        let text = first
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| {
                panic!("tool {tool_name} content[0] should have 'text' field, got: {first}")
            });

        serde_json::from_str(text).unwrap_or_else(|e| {
            panic!("failed to parse tool {tool_name} response as JSON: {e}\nraw: {text}")
        })
    }

    /// Shut down the MCP process by closing stdin and waiting for exit.
    fn shutdown(self) -> i32 {
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
        .expect("E2E MCP test timed out after 30 seconds");

    handle.join().expect("test thread panicked");
    result
}

// ---------------------------------------------------------------------------
// E2E Tests: Document Tools (use "default" realm, corpus auto-indexed)
// ---------------------------------------------------------------------------

#[test]
fn e2e_mcp_get_outline_basic() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let basic_uri = path_to_uri(&corpus_dir().join("basic.md"));
        let result = harness.call_tool("get-outline", serde_json::json!({ "uri": basic_uri }));

        // Should return headings array (Vec<String> of heading text)
        let headings = result
            .get("headings")
            .and_then(|h| h.as_array())
            .expect("get-outline should return 'headings' array");

        // basic.md has: Main Title (H1), Section One (H2), Subsection A (H3),
        // Level Four (H4), Level Five (H5), Level Six (H6), Section One (H2 dup)
        assert!(
            headings.len() >= 7,
            "basic.md should have at least 7 heading entries, got: {}",
            headings.len()
        );

        // First heading should be "Main Title" (headings are plain strings)
        let first_text = headings[0].as_str().expect("heading should be a string");
        assert_eq!(
            first_text, "Main Title",
            "first heading should be Main Title"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0, "server should exit cleanly");
    });
}

#[test]
fn e2e_mcp_search_symbols_headings() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let result = harness.call_tool("search-symbols", serde_json::json!({ "query": "Section" }));

        let symbols = result
            .get("symbols")
            .and_then(|s| s.as_array())
            .expect("search-symbols should return 'symbols' array");

        assert!(
            !symbols.is_empty(),
            "search-symbols for 'Section' should find results"
        );

        // Should find "Section One" from basic.md
        let names: Vec<&str> = symbols
            .iter()
            .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
            .collect();
        let has_section_one = names.iter().any(|n| n.contains("Section One"));
        assert!(
            has_section_one,
            "should find 'Section One' heading, got: {names:?}"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_search_symbols_cross_file() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        // Search for "Tags" which matches "XML Tags Test" and sub-headings in xml-tags.md
        let result = harness.call_tool("search-symbols", serde_json::json!({ "query": "Tags" }));

        let symbols = result
            .get("symbols")
            .and_then(|s| s.as_array())
            .expect("search-symbols should return 'symbols' array");

        assert!(
            !symbols.is_empty(),
            "search-symbols for 'Tags' should find results across corpus"
        );

        // Should find headings from xml-tags.md (e.g., "XML Tags Test", "Paired Tags", etc.)
        let has_xml_tags_file = symbols.iter().any(|s| {
            s.get("uri")
                .and_then(|u| u.as_str())
                .map(|u| u.contains("xml-tags.md"))
                .unwrap_or(false)
        });
        assert!(
            has_xml_tags_file,
            "search for 'Tags' should include results from xml-tags.md"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_find_references_heading() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        // basic.md line 0: "# Main Title"
        let basic_uri = path_to_uri(&corpus_dir().join("basic.md"));
        let result = harness.call_tool(
            "find-references",
            serde_json::json!({
                "uri": basic_uri,
                "line": 0,
                "character": 5
            }),
        );

        let locations = result
            .get("locations")
            .and_then(|l| l.as_array())
            .expect("find-references should return 'locations' array");

        // "Main Title" is referenced in cross-refs.md
        assert!(
            !locations.is_empty(),
            "should have references to 'Main Title'"
        );

        // Check for cross-file references
        let has_cross_ref = locations.iter().any(|l| {
            l.get("uri")
                .and_then(|u| u.as_str())
                .map(|u| u.contains("cross-refs.md"))
                .unwrap_or(false)
        });
        assert!(
            has_cross_ref,
            "should have cross-file references from cross-refs.md"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_rename_heading() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        // basic.md line 4: "## Section One"
        let basic_uri = path_to_uri(&corpus_dir().join("basic.md"));
        let result = harness.call_tool(
            "rename",
            serde_json::json!({
                "uri": basic_uri,
                "line": 4,
                "character": 5,
                "new_name": "First Section"
            }),
        );

        let changes = result
            .get("changes")
            .and_then(|c| c.as_array())
            .expect("rename should return 'changes' array");

        // Should have at least one document with edits
        assert!(
            !changes.is_empty(),
            "rename should produce at least one document edit"
        );

        // basic.md should have an edit for the heading itself
        let has_basic_edit = changes.iter().any(|c| {
            c.get("uri")
                .and_then(|u| u.as_str())
                .map(|u| u.contains("basic.md"))
                .unwrap_or(false)
        });
        assert!(
            has_basic_edit,
            "rename should edit basic.md (the heading source)"
        );

        // cross-refs.md references Section One, so it should also have edits
        let has_cross_ref_edit = changes.iter().any(|c| {
            c.get("uri")
                .and_then(|u| u.as_str())
                .map(|u| u.contains("cross-refs.md"))
                .unwrap_or(false)
        });
        assert!(
            has_cross_ref_edit,
            "rename should edit cross-refs.md (references to this heading)"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_export_index_basic() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let basic_uri = path_to_uri(&corpus_dir().join("basic.md"));
        let result = harness.call_tool("export-index", serde_json::json!({ "uri": basic_uri }));

        // Should have all four index categories
        let headings = result
            .get("headings")
            .and_then(|h| h.as_array())
            .expect("export-index should return 'headings'");
        assert!(
            !headings.is_empty(),
            "basic.md should have headings in export"
        );

        // basic.md has no XML tags
        let xml_tags = result
            .get("xml_tags")
            .and_then(|t| t.as_array())
            .expect("export-index should return 'xml_tags'");
        assert!(xml_tags.is_empty(), "basic.md should have no XML tags");

        // Check wiki_links and markdown_links arrays exist
        assert!(
            result.get("wiki_links").is_some(),
            "export-index should include 'wiki_links'"
        );
        assert!(
            result.get("markdown_links").is_some(),
            "export-index should include 'markdown_links'"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_export_index_xml_tags() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let xml_uri = path_to_uri(&corpus_dir().join("xml-tags.md"));
        let result = harness.call_tool("export-index", serde_json::json!({ "uri": xml_uri }));

        let xml_tags = result
            .get("xml_tags")
            .and_then(|t| t.as_array())
            .expect("export-index should return 'xml_tags'");

        // xml-tags.md has agent, task, step, checkpoint, broken tags
        assert!(
            xml_tags.len() >= 5,
            "xml-tags.md should have at least 5 XML tag entries, got: {}",
            xml_tags.len()
        );

        let tag_names: Vec<&str> = xml_tags
            .iter()
            .filter_map(|t| t.get("tag_name").and_then(|n| n.as_str()))
            .collect();
        assert!(
            tag_names.contains(&"agent"),
            "should include 'agent' tag, got: {tag_names:?}"
        );
        assert!(
            tag_names.contains(&"task"),
            "should include 'task' tag, got: {tag_names:?}"
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

// ---------------------------------------------------------------------------
// E2E Tests: Realm Management Tools
// ---------------------------------------------------------------------------

#[test]
fn e2e_mcp_realm_stats_default() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let result = harness.call_tool("realm-stats", serde_json::json!({ "realm": "default" }));

        let doc_count = result
            .get("document_count")
            .and_then(|d| d.as_u64())
            .expect("realm-stats should return 'document_count'");

        // Corpus has 7 files: basic.md, links.md, cross-refs.md, edge-cases.md,
        // xml-tags.md, empty.md, large.md
        assert!(
            doc_count >= 5,
            "default realm should have at least 5 documents, got: {doc_count}"
        );

        let heading_count = result
            .get("heading_count")
            .and_then(|h| h.as_u64())
            .expect("realm-stats should return 'heading_count'");
        assert!(heading_count > 0, "default realm should have headings");

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_create_and_destroy_realm() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        // Create a new realm
        let create_result =
            harness.call_tool("create-realm", serde_json::json!({ "name": "test-realm" }));
        let realm_name = create_result
            .get("name")
            .and_then(|n| n.as_str())
            .expect("create-realm should return 'name'");
        assert_eq!(realm_name, "test-realm");

        let doc_count = create_result
            .get("document_count")
            .and_then(|d| d.as_u64())
            .expect("create-realm should return 'document_count'");
        assert_eq!(doc_count, 0, "new realm should have 0 documents");

        // Destroy the realm
        let destroy_result =
            harness.call_tool("destroy-realm", serde_json::json!({ "name": "test-realm" }));
        let success = destroy_result
            .get("success")
            .and_then(|s| s.as_bool())
            .expect("destroy-realm should return 'success'");
        assert!(success, "destroy-realm should succeed");

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_add_root_and_remove_root() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        // Create a new realm first
        harness.call_tool(
            "create-realm",
            serde_json::json!({ "name": "add-root-test" }),
        );

        // Add the corpus as a root
        let add_result = harness.call_tool(
            "add-root",
            serde_json::json!({
                "realm": "add-root-test",
                "root": corpus_dir().to_str().unwrap()
            }),
        );

        let doc_count = add_result
            .get("document_count")
            .and_then(|d| d.as_u64())
            .expect("add-root should return 'document_count'");
        assert!(
            doc_count >= 5,
            "add-root should index corpus documents, got: {doc_count}"
        );

        let root_count = add_result
            .get("root_count")
            .and_then(|r| r.as_u64())
            .expect("add-root should return 'root_count'");
        assert_eq!(root_count, 1, "should have 1 root after add-root");

        // Now check realm-stats on the new realm
        let stats = harness.call_tool(
            "realm-stats",
            serde_json::json!({ "realm": "add-root-test" }),
        );
        let stats_doc = stats
            .get("document_count")
            .and_then(|d| d.as_u64())
            .unwrap_or(0);
        assert!(
            stats_doc >= 5,
            "realm-stats should show indexed documents after add-root"
        );

        // Remove the root
        let remove_result = harness.call_tool(
            "remove-root",
            serde_json::json!({
                "realm": "add-root-test",
                "root": corpus_dir().to_str().unwrap()
            }),
        );

        let post_remove_docs = remove_result
            .get("document_count")
            .and_then(|d| d.as_u64())
            .expect("remove-root should return 'document_count'");
        assert_eq!(
            post_remove_docs, 0,
            "remove-root should leave 0 documents in the realm"
        );

        // Clean up
        harness.call_tool(
            "destroy-realm",
            serde_json::json!({ "name": "add-root-test" }),
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

// ---------------------------------------------------------------------------
// E2E Tests: Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn e2e_mcp_get_outline_empty_file() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let empty_uri = path_to_uri(&corpus_dir().join("empty.md"));
        let result = harness.call_tool("get-outline", serde_json::json!({ "uri": empty_uri }));

        let headings = result
            .get("headings")
            .and_then(|h| h.as_array())
            .expect("get-outline should return 'headings' even for empty file");

        assert!(
            headings.is_empty(),
            "empty.md should have no headings, got: {}",
            headings.len()
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}

#[test]
fn e2e_mcp_search_symbols_no_match() {
    run_with_timeout(|| {
        let mut harness = McpTestHarness::spawn(&corpus_dir());

        let result = harness.call_tool(
            "search-symbols",
            serde_json::json!({ "query": "zzzznonexistentxyzzy" }),
        );

        let symbols = result
            .get("symbols")
            .and_then(|s| s.as_array())
            .expect("search-symbols should return 'symbols' array");

        assert!(
            symbols.is_empty(),
            "search for nonsense query should return empty results, got: {}",
            symbols.len()
        );

        let exit = harness.shutdown();
        assert_eq!(exit, 0);
    });
}
