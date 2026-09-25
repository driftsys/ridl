//! A contract expression built as a long operator chain draws a diagnostic in
//! `ridl check`, `ridl mcp` and `ridl lsp`, and aborts none of them
//! (driftsys/ridl#346).
//!
//! A chain of a few thousand terms once built a syntax tree as deep as its term
//! count, and the checker's recursion over that tree overflowed the stack. A
//! stack overflow aborts the process instead of unwinding, so every test here
//! runs the built binary as a child: a regression fails the assertion on the
//! child's exit or its missing answer, and does not abort this test binary.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use lsp_server::{Message, Notification, Request, RequestId};
use serde_json::json;

/// How long a spawned binary is given to answer or to exit. Generous, because
/// a loaded CI machine is slow; bounded, because `cargo test` has no per-test
/// timeout and a hung child would otherwise hang the whole run.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The term count of every long chain below. `ridl mcp` checks on a 2 MiB
/// thread and aborted from about 860 terms in a debug build; `ridl check` and
/// `ridl lsp` check on the main thread and aborted below 5000.
const TERMS: usize = 5000;

/// A contract expression in a minimal ridl file that is otherwise clean: the
/// only diagnostics it draws come from the expression.
fn contract_file(expr: &str) -> String {
    format!(
        "package app\ntype N : integer [0..10]\ninterface I {{\n  command c(a: N) [ require {expr} ] @[..50ms]\n}}\n"
    )
}

/// A flat chain of `terms` operands under `op`, closed into a boolean where
/// the operator is not one already.
fn flat_chain(op: &str, terms: usize) -> String {
    match op {
        "||" | "&&" => vec!["a == 1"; terms].join(&format!(" {op} ")),
        "." => format!("a{} == 1", ".b".repeat(terms - 1)),
        _ => format!("{} == 1", vec!["a"; terms].join(&format!(" {op} "))),
    }
}

/// Every shape the tests send: one flat chain per chaining operator, plus two
/// shapes that stack chains — through parentheses, and through the precedence
/// levels — so that a bound charged per chain, not per tree, still fails.
fn shapes() -> Vec<(String, String)> {
    let mut shapes: Vec<(String, String)> = ["||", "&&", "+", "-", "*", "/", "%", "."]
        .into_iter()
        .map(|op| (format!("`{op}` chain"), flat_chain(op, TERMS)))
        .collect();
    let mut nested = "a".to_string();
    for _ in 0..120 {
        nested = format!("({nested}{})", " + a".repeat(119));
    }
    shapes.push(("nested chains".to_string(), format!("{nested} == 1")));
    shapes.push((
        "precedence ladder".to_string(),
        format!(
            "a{}{}{} == a{}{}",
            ".b".repeat(119),
            " * a".repeat(119),
            " + a".repeat(119),
            " && a == a".repeat(119),
            " || a == a".repeat(119),
        ),
    ));
    shapes
}

/// Waits for `child` to exit, killing it and failing the test if it does not
/// within [`TIMEOUT`].
fn wait_for_exit(child: &mut Child, what: &str) -> ExitStatus {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait().expect("poll the child") {
            Some(status) => return status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{what} did not exit within {TIMEOUT:?}");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// The child's stderr, for a failure message, once it has exited.
fn stderr_of(child: &mut Child) -> String {
    let mut text = String::new();
    if let Some(stderr) = child.stderr.as_mut() {
        let _ = std::io::Read::read_to_string(stderr, &mut text);
    }
    text
}

// ---------------------------------------------------------------------------
// `ridl check`
// ---------------------------------------------------------------------------

#[test]
fn ridl_check_reports_form_102_for_every_long_chain_shape() {
    let dir = TempDir::new("check");
    for (shape, expr) in shapes() {
        let path = dir.write("chain.ridl", &contract_file(&expr));
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&path)
            .output()
            .expect("run ridl check");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "the {shape} exits 1 with a diagnostic, not by a signal: {:?}\n{stderr}",
            output.status,
        );
        let diagnostics: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("stdout is JSON");
        let codes: Vec<&str> = diagnostics
            .as_array()
            .expect("a JSON array")
            .iter()
            .filter_map(|diagnostic| diagnostic["code"].as_str())
            .collect();
        assert!(
            codes.contains(&"FORM-102"),
            "the {shape} draws FORM-102: {codes:?}"
        );
    }
}

#[test]
fn ridl_check_accepts_a_long_chain_under_the_limit() {
    let dir = TempDir::new("check-short");
    for op in ["||", "&&", "+", "-", "*", "/", "%"] {
        let path = dir.write("chain.ridl", &contract_file(&flat_chain(op, 100)));
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .arg("check")
            .arg(&path)
            .output()
            .expect("run ridl check");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a 100-term `{op}` chain is clean: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

// ---------------------------------------------------------------------------
// `ridl mcp`
// ---------------------------------------------------------------------------

/// Reads newline-delimited JSON-RPC messages off `stdout` on a worker thread.
fn read_json_lines(stdout: ChildStdout) -> mpsc::Receiver<serde_json::Value> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match stdout.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let Ok(value) = serde_json::from_str(&line) else {
                        continue;
                    };
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        }
    });
    receiver
}

/// Waits for the JSON-RPC response to `id`, skipping anything else.
fn json_response(messages: &mpsc::Receiver<serde_json::Value>, id: i64) -> serde_json::Value {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match messages.recv_timeout(remaining) {
            Ok(message) if message["id"] == id => return message,
            Ok(_) => {}
            Err(err) => panic!("no response to request {id} within {TIMEOUT:?}: {err}"),
        }
    }
}

#[test]
fn ridl_mcp_reports_form_102_for_every_long_chain_shape_and_keeps_serving() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl mcp");
    let mut stdin = child.stdin.take().expect("piped stdin");
    let messages = read_json_lines(child.stdout.take().expect("piped stdout"));

    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "deep-expression-test", "version": "0.0.0" }
        }
    });
    writeln!(stdin, "{initialize}").expect("write initialize");
    json_response(&messages, 1);
    writeln!(
        stdin,
        "{}",
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
    )
    .expect("write initialized");

    for (id, (shape, expr)) in (2..).zip(shapes()) {
        let call = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "ridl_check",
                "arguments": { "source": contract_file(&expr), "profile": "ridl" }
            }
        });
        writeln!(stdin, "{call}").expect("write tools/call");
        let deadline = Instant::now() + TIMEOUT;
        let response = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match messages.recv_timeout(remaining) {
                Ok(message) if message["id"] == id => break message,
                Ok(_) => {}
                Err(_) => {
                    let _ = child.kill();
                    let status = child.wait().expect("reap ridl mcp");
                    panic!(
                        "ridl mcp gave no answer for the {shape} (status {status:?}):\n{}",
                        stderr_of(&mut child)
                    );
                }
            }
        };
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("a text result for the {shape}: {response}"));
        let output: serde_json::Value = serde_json::from_str(text).expect("the tool returns JSON");
        let codes: Vec<&str> = output["diagnostics"]
            .as_array()
            .unwrap_or_else(|| panic!("a diagnostics array: {output}"))
            .iter()
            .filter_map(|diagnostic| diagnostic["code"].as_str())
            .collect();
        assert!(
            codes.contains(&"FORM-102"),
            "the {shape} draws FORM-102: {codes:?}"
        );
    }

    drop(stdin);
    let status = wait_for_exit(&mut child, "ridl mcp");
    assert_eq!(status.code(), Some(0), "{}", stderr_of(&mut child));
}

// ---------------------------------------------------------------------------
// `ridl lsp`
// ---------------------------------------------------------------------------

/// Reads LSP messages off `stdout` on a worker thread.
fn read_messages(stdout: ChildStdout) -> mpsc::Receiver<Message> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        while let Ok(Some(message)) = Message::read(&mut stdout) {
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}

/// A `file://` URI for `path`.
fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[test]
fn ridl_lsp_publishes_form_102_for_every_long_chain_shape_and_keeps_serving() {
    let dir = TempDir::new("lsp");
    dir.write(
        "ridl.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl lsp");
    let mut stdin = child.stdin.take().expect("piped stdin");
    let messages = read_messages(child.stdout.take().expect("piped stdout"));

    let initialize = RequestId::from(1);
    Message::Request(Request::new(
        initialize.clone(),
        "initialize".to_string(),
        json!({ "capabilities": {}, "rootUri": file_uri(&dir.0) }),
    ))
    .write(&mut stdin)
    .expect("write initialize");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match messages.recv_timeout(remaining) {
            Ok(Message::Response(response)) if response.id == initialize => break,
            Ok(_) => {}
            Err(err) => panic!("no initialize response within {TIMEOUT:?}: {err}"),
        }
    }
    Message::Notification(Notification::new("initialized".to_string(), json!({})))
        .write(&mut stdin)
        .expect("write initialized");

    for (index, (shape, expr)) in shapes().into_iter().enumerate() {
        let path = dir.write(&format!("chain{index}.ridl"), &contract_file(&expr));
        let uri = file_uri(&path);
        Message::Notification(Notification::new(
            "textDocument/didOpen".to_string(),
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "ridl",
                    "version": 1,
                    "text": contract_file(&expr),
                }
            }),
        ))
        .write(&mut stdin)
        .expect("write didOpen");
        let deadline = Instant::now() + TIMEOUT;
        let codes: Vec<String> = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match messages.recv_timeout(remaining) {
                Ok(Message::Notification(notification))
                    if notification.method == "textDocument/publishDiagnostics"
                        && notification.params["uri"] == uri =>
                {
                    break notification.params["diagnostics"]
                        .as_array()
                        .expect("a diagnostics array")
                        .iter()
                        .filter_map(|diagnostic| diagnostic["code"].as_str())
                        .map(str::to_string)
                        .collect();
                }
                Ok(_) => {}
                Err(_) => {
                    let _ = child.kill();
                    let status = child.wait().expect("reap ridl lsp");
                    panic!(
                        "ridl lsp published nothing for the {shape} (status {status:?}):\n{}",
                        stderr_of(&mut child)
                    );
                }
            }
        };
        assert!(
            codes.iter().any(|code| code == "FORM-102"),
            "the {shape} draws FORM-102: {codes:?}"
        );
    }

    let shutdown = RequestId::from(2);
    Message::Request(Request::new(
        shutdown.clone(),
        "shutdown".to_string(),
        json!(null),
    ))
    .write(&mut stdin)
    .expect("write shutdown");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match messages.recv_timeout(remaining) {
            Ok(Message::Response(response)) if response.id == shutdown => break,
            Ok(_) => {}
            Err(err) => panic!("no shutdown response within {TIMEOUT:?}: {err}"),
        }
    }
    Message::Notification(Notification::new("exit".to_string(), json!(null)))
        .write(&mut stdin)
        .expect("write exit");
    drop(stdin);
    let status = wait_for_exit(&mut child, "ridl lsp");
    assert_eq!(status.code(), Some(0), "{}", stderr_of(&mut child));
}

// ---------------------------------------------------------------------------
// Scratch files
// ---------------------------------------------------------------------------

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-deep-expression-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        // Canonical, so the `file://` URIs the language server is sent match
        // the paths it resolves (macOS puts the temp dir behind a symlink).
        Self(path.canonicalize().expect("canonicalize the temp dir"))
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::write(&path, text).expect("write the fixture file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
