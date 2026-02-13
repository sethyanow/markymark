//! Shared infrastructure for dual-process LSP alignment tests.
//!
//! Provides LspProcess (single server wrapper), response normalization,
//! comparison logic, and report generation types.

// Test infrastructure code - allow unused items for future test expansion
#![allow(dead_code)]

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
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
pub fn markymark_bin() -> PathBuf {
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

/// Try to find the marksman binary. Returns None if not found.
pub fn marksman_bin() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/marksman",
        "/usr/local/bin/marksman",
        "/usr/bin/marksman",
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(output) = Command::new("which").arg("marksman").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}

/// Skip the current test if marksman is not available.
#[macro_export]
macro_rules! require_marksman {
    () => {
        match $crate::alignment_support::marksman_bin() {
            Some(bin) => bin,
            None => {
                eprintln!("SKIP: marksman not found — alignment test requires marksman binary");
                return;
            }
        }
    };
}

pub fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

pub fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn lsp_frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

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

/// Run a test inside a thread with a 30-second overall timeout.
pub fn run_with_timeout<F, R>(test_fn: F) -> R
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
        .expect("alignment test timed out after 30 seconds");
    handle.join().expect("test thread panicked");
    result
}

// ---------------------------------------------------------------------------
// LspProcess — wraps a single LSP server (marksman or markymark)
// ---------------------------------------------------------------------------

pub struct LspProcess {
    _guard: ChildGuard,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
    pub diagnostics: HashMap<String, Vec<Value>>,
    name: String,
}

impl LspProcess {
    /// Spawn an LSP server and perform initialize handshake.
    pub fn spawn(bin: &Path, args: &[&str], workspace_root: &Path, name: &str) -> Self {
        assert!(bin.exists(), "{name} binary not found at {}", bin.display());

        let mut guard = ChildGuard::new(
            Command::new(bin)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap_or_else(|e| panic!("failed to spawn {name}: {e}")),
        );

        let child = guard.child_mut();
        let stdin = child.stdin.take().expect("failed to take stdin");
        let stdout = child.stdout.take().expect("failed to take stdout");
        let reader = BufReader::new(stdout);

        let mut proc = Self {
            _guard: guard,
            stdin,
            reader,
            next_id: 1,
            diagnostics: HashMap::new(),
            name: name.to_string(),
        };

        let root_uri = path_to_uri(workspace_root);
        let init_result = proc.send_request(
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "completion": { "completionItem": {} },
                        "rename": { "prepareSupport": true },
                        "publishDiagnostics": {}
                    }
                }
            }),
        );
        assert!(
            init_result.get("capabilities").is_some(),
            "{name} initialize should return capabilities"
        );

        proc.send_notification("initialized", serde_json::json!({}));
        proc
    }

    pub fn send_request(&mut self, method: &str, params: Value) -> Value {
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
            .unwrap_or_else(|e| panic!("failed to write to {}: {e}", self.name));
        self.stdin
            .flush()
            .unwrap_or_else(|e| panic!("failed to flush {}: {e}", self.name));

        loop {
            let response = read_lsp_message(&mut self.reader);

            if response.get("id").is_none() {
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

            if response.get("id").and_then(|v| v.as_i64()) == Some(id) {
                if response.get("error").is_some() {
                    return Value::Null;
                }
                return response.get("result").cloned().unwrap_or(Value::Null);
            }
        }
    }

    pub fn send_notification(&mut self, method: &str, params: Value) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let frame = lsp_frame(&msg.to_string());
        self.stdin
            .write_all(&frame)
            .unwrap_or_else(|e| panic!("failed to write notification to {}: {e}", self.name));
        self.stdin
            .flush()
            .unwrap_or_else(|e| panic!("failed to flush {}: {e}", self.name));
    }

    pub fn open_file(&mut self, path: &Path) {
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

    pub fn drain_notifications(&mut self) {
        std::thread::sleep(Duration::from_millis(300));
        loop {
            let buf = self.reader.buffer();
            if buf.is_empty() {
                break;
            }
            let response = read_lsp_message(&mut self.reader);
            if response.get("id").is_none()
                && response.get("method").and_then(|m| m.as_str())
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

    pub fn shutdown_and_exit(mut self) -> i32 {
        let _shutdown = self.send_request("shutdown", Value::Null);
        self.send_notification("exit", Value::Null);
        drop(self.stdin);
        let child = self._guard.take();
        let output = child.wait_with_output().expect("failed to wait on child");
        output.status.code().unwrap_or(-1)
    }
}

// ---------------------------------------------------------------------------
// Alignment types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AlignmentResult {
    Match,
    Superset { extra_count: usize },
    Mismatch { marksman: Value, markymark: Value },
    MarksmanOnly,
    MarkymarkOnly,
}

impl fmt::Display for AlignmentResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlignmentResult::Match => write!(f, "MATCH"),
            AlignmentResult::Superset { extra_count } => {
                write!(f, "SUPERSET (markymark +{extra_count})")
            }
            AlignmentResult::Mismatch { .. } => write!(f, "MISMATCH"),
            AlignmentResult::MarksmanOnly => write!(f, "MARKSMAN_ONLY"),
            AlignmentResult::MarkymarkOnly => write!(f, "MARKYMARK_ONLY"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MethodComparison {
    pub method: String,
    pub file: String,
    pub result: AlignmentResult,
    pub notes: String,
}

#[derive(Debug)]
pub struct AlignmentReport {
    pub comparisons: Vec<MethodComparison>,
}

impl AlignmentReport {
    pub fn new() -> Self {
        Self {
            comparisons: Vec::new(),
        }
    }

    pub fn add(&mut self, method: &str, file: &str, result: AlignmentResult, notes: &str) {
        self.comparisons.push(MethodComparison {
            method: method.to_string(),
            file: file.to_string(),
            result,
            notes: notes.to_string(),
        });
    }

    pub fn to_json(&self) -> Value {
        let entries: Vec<Value> = self
            .comparisons
            .iter()
            .map(|c| {
                serde_json::json!({
                    "method": c.method,
                    "file": c.file,
                    "result": c.result.to_string(),
                    "notes": c.notes,
                })
            })
            .collect();
        serde_json::json!({
            "comparisons": entries,
            "summary": self.summary_counts(),
        })
    }

    fn summary_counts(&self) -> Value {
        let mut match_count = 0;
        let mut superset_count = 0;
        let mut mismatch_count = 0;
        let mut marksman_only = 0;
        let mut markymark_only = 0;
        for c in &self.comparisons {
            match &c.result {
                AlignmentResult::Match => match_count += 1,
                AlignmentResult::Superset { .. } => superset_count += 1,
                AlignmentResult::Mismatch { .. } => mismatch_count += 1,
                AlignmentResult::MarksmanOnly => marksman_only += 1,
                AlignmentResult::MarkymarkOnly => markymark_only += 1,
            }
        }
        serde_json::json!({
            "total": self.comparisons.len(),
            "match": match_count,
            "superset": superset_count,
            "mismatch": mismatch_count,
            "marksman_only": marksman_only,
            "markymark_only": markymark_only,
        })
    }

    pub fn summary_text(&self) -> String {
        let counts = self.summary_counts();
        let mut out = String::new();
        out.push_str("=== Alignment Report ===\n");
        out.push_str(&format!("Total: {} comparisons\n", counts["total"]));
        out.push_str(&format!("  Match:          {}\n", counts["match"]));
        out.push_str(&format!("  Superset:       {}\n", counts["superset"]));
        out.push_str(&format!("  Mismatch:       {}\n", counts["mismatch"]));
        out.push_str(&format!("  Marksman-only:  {}\n", counts["marksman_only"]));
        out.push_str(&format!("  Markymark-only: {}\n", counts["markymark_only"]));
        out.push_str("\nDetails:\n");
        for c in &self.comparisons {
            out.push_str(&format!(
                "  [{}] {} on {} {}\n",
                c.result,
                c.method,
                c.file,
                if c.notes.is_empty() {
                    String::new()
                } else {
                    format!("— {}", c.notes)
                },
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Normalization & comparison
// ---------------------------------------------------------------------------

fn is_empty_response(v: &Value) -> bool {
    v.is_null()
        || v.as_array().is_some_and(|a| a.is_empty())
        || v.as_object().is_some_and(|o| o.is_empty())
}

fn location_sort_key(loc: &Value) -> (String, i64, i64) {
    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let range = loc
        .get("range")
        .or_else(|| loc.get("targetRange"))
        .unwrap_or(&Value::Null);
    let line = range
        .pointer("/start/line")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let char = range
        .pointer("/start/character")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    (uri, line, char)
}

fn sort_locations(arr: &mut [Value]) {
    arr.sort_by(|a, b| {
        let ka = location_sort_key(a);
        let kb = location_sort_key(b);
        ka.cmp(&kb)
    });
}

fn uri_filename(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

fn normalize_location(loc: &Value) -> Value {
    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let filename = uri_filename(uri);
    let range = loc
        .get("range")
        .or_else(|| loc.get("targetRange"))
        .cloned()
        .unwrap_or(Value::Null);
    serde_json::json!({
        "file": filename,
        "range": range,
    })
}

pub fn compare_responses(method: &str, marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_empty = is_empty_response(marksman);
    let mm_empty = is_empty_response(markymark);

    if ms_empty && mm_empty {
        return AlignmentResult::Match;
    }
    if ms_empty && !mm_empty {
        return AlignmentResult::MarkymarkOnly;
    }
    if !ms_empty && mm_empty {
        return AlignmentResult::MarksmanOnly;
    }

    match method {
        "textDocument/definition" | "textDocument/references" => {
            compare_locations(marksman, markymark)
        }
        "textDocument/hover" => compare_hover(marksman, markymark),
        "textDocument/completion" => compare_completions(marksman, markymark),
        "textDocument/rename" => compare_workspace_edits(marksman, markymark),
        "textDocument/documentSymbol" => compare_document_symbols(marksman, markymark),
        "workspace/symbol" => compare_workspace_symbols(marksman, markymark),
        _ => {
            if marksman == markymark {
                AlignmentResult::Match
            } else {
                AlignmentResult::Mismatch {
                    marksman: marksman.clone(),
                    markymark: markymark.clone(),
                }
            }
        }
    }
}

fn compare_locations(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let mut ms_locs = to_location_array(marksman);
    let mut mm_locs = to_location_array(markymark);

    sort_locations(&mut ms_locs);
    sort_locations(&mut mm_locs);

    let mut ms_sorted: Vec<Value> = ms_locs.iter().map(normalize_location).collect();
    let mut mm_sorted: Vec<Value> = mm_locs.iter().map(normalize_location).collect();
    ms_sorted.sort_by_key(|v| format!("{v}"));
    mm_sorted.sort_by_key(|v| format!("{v}"));

    if ms_sorted == mm_sorted {
        return AlignmentResult::Match;
    }

    let ms_set: std::collections::HashSet<String> =
        ms_sorted.iter().map(|v| v.to_string()).collect();
    let mm_set: std::collections::HashSet<String> =
        mm_sorted.iter().map(|v| v.to_string()).collect();

    if ms_set.is_subset(&mm_set) && mm_set.len() > ms_set.len() {
        return AlignmentResult::Superset {
            extra_count: mm_set.len() - ms_set.len(),
        };
    }

    AlignmentResult::Mismatch {
        marksman: Value::Array(ms_sorted),
        markymark: Value::Array(mm_sorted),
    }
}

fn to_location_array(v: &Value) -> Vec<Value> {
    if let Some(arr) = v.as_array() {
        arr.clone()
    } else if v.is_object() {
        vec![v.clone()]
    } else {
        vec![]
    }
}

fn compare_hover(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_content = marksman
        .get("contents")
        .and_then(|c| c.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mm_content = markymark
        .get("contents")
        .and_then(|c| c.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if ms_content.trim() == mm_content.trim() {
        AlignmentResult::Match
    } else {
        AlignmentResult::Mismatch {
            marksman: Value::String(ms_content.to_string()),
            markymark: Value::String(mm_content.to_string()),
        }
    }
}

fn compare_completions(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_items = extract_completion_labels(marksman);
    let mm_items = extract_completion_labels(markymark);

    if ms_items == mm_items {
        return AlignmentResult::Match;
    }

    let ms_set: std::collections::HashSet<&String> = ms_items.iter().collect();
    let mm_set: std::collections::HashSet<&String> = mm_items.iter().collect();

    if ms_set.is_subset(&mm_set) && mm_set.len() > ms_set.len() {
        return AlignmentResult::Superset {
            extra_count: mm_set.len() - ms_set.len(),
        };
    }

    AlignmentResult::Mismatch {
        marksman: serde_json::json!(ms_items),
        markymark: serde_json::json!(mm_items),
    }
}

fn extract_completion_labels(v: &Value) -> Vec<String> {
    let items = v
        .get("items")
        .and_then(|i| i.as_array())
        .or_else(|| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut labels: Vec<String> = items
        .iter()
        .filter_map(|item| item.get("label").and_then(|l| l.as_str()).map(String::from))
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

fn compare_workspace_edits(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_edits = flatten_workspace_edits(marksman);
    let mm_edits = flatten_workspace_edits(markymark);

    if ms_edits == mm_edits {
        AlignmentResult::Match
    } else {
        AlignmentResult::Mismatch {
            marksman: serde_json::json!(ms_edits),
            markymark: serde_json::json!(mm_edits),
        }
    }
}

fn flatten_workspace_edits(v: &Value) -> Vec<(String, Value)> {
    let mut result = Vec::new();
    if let Some(changes) = v.get("changes").and_then(|c| c.as_object()) {
        for (uri, edits) in changes {
            let filename = uri_filename(uri).to_string();
            if let Some(arr) = edits.as_array() {
                for edit in arr {
                    result.push((filename.clone(), edit.clone()));
                }
            }
        }
    }
    result.sort_by(|a, b| {
        let cmp = a.0.cmp(&b.0);
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
        let a_line =
            a.1.pointer("/range/start/line")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        let b_line =
            b.1.pointer("/range/start/line")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        a_line.cmp(&b_line)
    });
    result
}

fn compare_document_symbols(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_names = extract_symbol_names(marksman);
    let mm_names = extract_symbol_names(markymark);

    if ms_names == mm_names {
        return AlignmentResult::Match;
    }

    let ms_set: std::collections::HashSet<&String> = ms_names.iter().collect();
    let mm_set: std::collections::HashSet<&String> = mm_names.iter().collect();

    if ms_set.is_subset(&mm_set) && mm_set.len() > ms_set.len() {
        return AlignmentResult::Superset {
            extra_count: mm_set.len() - ms_set.len(),
        };
    }

    AlignmentResult::Mismatch {
        marksman: serde_json::json!(ms_names),
        markymark: serde_json::json!(mm_names),
    }
}

fn extract_symbol_names(v: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(arr) = v.as_array() {
        for sym in arr {
            collect_symbol_names_recursive(sym, &mut names);
        }
    }
    names.sort();
    names
}

fn collect_symbol_names_recursive(sym: &Value, names: &mut Vec<String>) {
    if let Some(name) = sym.get("name").and_then(|n| n.as_str()) {
        names.push(name.to_string());
    }
    if let Some(children) = sym.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_symbol_names_recursive(child, names);
        }
    }
}

fn compare_workspace_symbols(marksman: &Value, markymark: &Value) -> AlignmentResult {
    let ms_names = extract_flat_symbol_names(marksman);
    let mm_names = extract_flat_symbol_names(markymark);

    if ms_names == mm_names {
        return AlignmentResult::Match;
    }

    let ms_set: std::collections::HashSet<&String> = ms_names.iter().collect();
    let mm_set: std::collections::HashSet<&String> = mm_names.iter().collect();

    if ms_set.is_subset(&mm_set) && mm_set.len() > ms_set.len() {
        return AlignmentResult::Superset {
            extra_count: mm_set.len() - ms_set.len(),
        };
    }

    AlignmentResult::Mismatch {
        marksman: serde_json::json!(ms_names),
        markymark: serde_json::json!(mm_names),
    }
}

fn extract_flat_symbol_names(v: &Value) -> Vec<String> {
    let items = v.as_array().cloned().unwrap_or_default();
    let mut names: Vec<String> = items
        .iter()
        .filter_map(|sym| sym.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();
    names.sort();
    names
}

/// Helper to truncate JSON for display in notes.
pub fn truncate_json(v: &Value, max: usize) -> String {
    let s = v.to_string();
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s
    }
}
