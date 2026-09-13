//! `ridl lsp` and `ridl mcp`: the two stdio servers the one binary hosts.
//!
//! Every test here spawns the built `ridl` binary and speaks a real protocol
//! to it over pipes. `crates/ridl-lsp/tests/` drives the language server
//! through an in-memory connection instead, which cannot show that the
//! subcommand wires the stdio transport at all.

use std::io::BufReader;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command as StdCommand, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use lsp_server::{Message, Notification, Request, RequestId, Response};
use rmcp::RoleClient;
use rmcp::ServiceExt as _;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use serde_json::json;
use tokio::process::Command;

/// How long a spawned server is given to answer or to exit. Generous, because
/// a loaded CI machine is slow; bounded, because `cargo test` has no per-test
/// timeout and a hung server would otherwise hang the whole run.
const TIMEOUT: Duration = Duration::from_secs(20);

/// One broken typl source, used by every diagnostic assertion below.
///
/// Deliberately no trailing newline: with one, the parser reports the missing
/// backing type at the position past it — line 3, column 1 — rather than at
/// the end of the `type X:` line. Measured against the built binary:
/// `FORM-101`, "expected a backing type", line 2 column 8, no fix-its.
const BROKEN_TYPL: &str = "package p\ntype X:";

// ---------------------------------------------------------------------------
// `ridl mcp`
// ---------------------------------------------------------------------------

/// Runs `ridl mcp` as a child, performs the MCP handshake, and returns the
/// client. The child is killed when the returned service is cancelled or
/// dropped.
async fn connect() -> RunningService<RoleClient, ()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ridl"));
    command.arg("mcp");
    let transport = TokioChildProcess::new(command).expect("spawn ridl mcp");
    ().serve(transport).await.expect("MCP initialize handshake")
}

/// The concatenated text of a tool result's text content blocks.
fn tool_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| content.as_text())
        .map(|text| text.text.clone())
        .collect()
}

/// Calls `ridl_check` over `source` under `profile` and returns the parsed
/// JSON the tool's text block carries.
async fn call_ridl_check(
    client: &RunningService<RoleClient, ()>,
    source: &str,
    profile: &str,
) -> serde_json::Value {
    let arguments = json!({ "source": source, "profile": profile })
        .as_object()
        .cloned()
        .expect("the arguments are a JSON object");
    let result = client
        .call_tool(CallToolRequestParams::new("ridl_check").with_arguments(arguments))
        .await
        .expect("tools/call");
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let text = tool_text(&result);
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("the tool returns JSON: {err}: {text}"))
}

#[tokio::test]
async fn ridl_mcp_advertises_ridl_check() {
    tokio::time::timeout(TIMEOUT, async {
        let client = connect().await;
        let tools = client
            .list_tools(Default::default())
            .await
            .expect("tools/list");
        let names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
        assert_eq!(names, ["ridl_check"]);
        client.cancel().await.expect("shutdown");
    })
    .await
    .expect("ridl_mcp_advertises_ridl_check did not finish within the timeout");
}

#[tokio::test]
async fn ridl_check_returns_the_diagnostic_contract() {
    tokio::time::timeout(TIMEOUT, async {
        let client = connect().await;
        let output = call_ridl_check(&client, BROKEN_TYPL, "typl").await;
        client.cancel().await.expect("shutdown");

        let diagnostics = output["diagnostics"]
            .as_array()
            .unwrap_or_else(|| panic!("a diagnostics array: {output}"));
        assert_eq!(diagnostics.len(), 1, "{output}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["code"], "FORM-101", "{output}");
        assert_eq!(diagnostic["severity"], "error", "{output}");
        assert_eq!(diagnostic["span"]["path"], "input.typl", "{output}");
        assert_eq!(diagnostic["span"]["start"]["line"], 2, "{output}");
        assert_eq!(diagnostic["span"]["start"]["column"], 8, "{output}");
        assert!(diagnostic["fixes"].is_array(), "{output}");
    })
    .await
    .expect("ridl_check_returns_the_diagnostic_contract did not finish within the timeout");
}

/// Blanks the file path out of every span in a diagnostic array, in place.
///
/// The two faces register the source under different names by construction —
/// the tool under the synthetic `input.typl`, the CLI under the real file it
/// read — so the path is the one field the agreement test must set aside
/// (`docs/wip/2026-09-13-ridl-mcp-v0-design.md` §2, "What the two faces share,
/// and where they differ"). No compiler pass emits a fix-it today, so the
/// inner loop is a no-op on current inputs; it is here because a `fixes` entry
/// carries a span of its own and would otherwise reintroduce the difference.
fn blank_span_paths(diagnostics: &mut serde_json::Value) {
    let blank = || serde_json::Value::String(String::new());
    for diagnostic in diagnostics.as_array_mut().into_iter().flatten() {
        diagnostic["span"]["path"] = blank();
        for fix in diagnostic["fixes"].as_array_mut().into_iter().flatten() {
            fix["span"]["path"] = blank();
        }
    }
}

/// The MCP tool and `ridl check --format json` report the same diagnostics.
///
/// The equality holds for this input class only: one standalone file, no
/// `ridl.toml` and no imports. The two faces run different front ends — the
/// CLI calls `ridlc::run_check`, which resolves a workspace, and the tool
/// calls `ridlc::check_source`, which does not — and a workspace member is
/// exactly where the two may legitimately diverge.
#[tokio::test]
async fn ridl_check_and_check_format_json_agree() {
    tokio::time::timeout(TIMEOUT, async {
        // The CLI side.
        let dir = TempDir::new("agree");
        let path = dir.write("agree.typl", BROKEN_TYPL);
        let cli = StdCommand::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&path)
            .output()
            .expect("run ridl check");
        // Checked first: on every exit-2 path `--format json` returns before it
        // prints anything, so stdout is empty and the parse below would report a
        // JSON error instead of the real failure.
        assert_eq!(cli.status.code(), Some(1), "{cli:?}");
        let mut cli: serde_json::Value =
            serde_json::from_slice(&cli.stdout).expect("the CLI prints JSON to stdout");

        // The MCP side.
        let client = connect().await;
        let mut mcp = call_ridl_check(&client, BROKEN_TYPL, "typl").await;
        client.cancel().await.expect("shutdown");
        // The tool wraps its array in an object; the CLI prints the bare array.
        let mut mcp = mcp["diagnostics"].take();

        assert!(
            cli.as_array().is_some_and(|array| !array.is_empty()),
            "the fixture must produce at least one diagnostic, or this proves nothing: {cli}"
        );
        blank_span_paths(&mut cli);
        blank_span_paths(&mut mcp);
        assert_eq!(cli, mcp);
    })
    .await
    .expect("ridl_check_and_check_format_json_agree did not finish within the timeout");
}

// ---------------------------------------------------------------------------
// `ridl lsp`
// ---------------------------------------------------------------------------

/// Spawns `ridl lsp` with all three standard streams piped.
fn spawn_lsp() -> Child {
    StdCommand::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl lsp")
}

/// Reads LSP messages off `stdout` on a worker thread and forwards them.
/// Reading on a thread is what lets the test bound each wait: a blocking
/// `Message::read` against a server that never answers would hang the run.
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

/// Waits for the response to `id`, skipping the notifications the server
/// sends on its own (`textDocument/publishDiagnostics`).
fn response_to(messages: &mpsc::Receiver<Message>, id: RequestId) -> Response {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match messages.recv_timeout(remaining) {
            Ok(Message::Response(response)) if response.id == id => return response,
            Ok(_) => {}
            Err(err) => panic!("no response to request {id} within {TIMEOUT:?}: {err}"),
        }
    }
}

/// Waits for `child` to exit, killing it and failing the test if it does not
/// within [`TIMEOUT`]. Polling rather than a blocking `wait` is what lets the
/// child be killed instead of orphaned when the test fails.
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

#[test]
fn ridl_lsp_serves_the_handshake_and_exits_zero_on_shutdown() {
    let mut child = spawn_lsp();
    let mut stdin = child.stdin.take().expect("piped stdin");
    let messages = read_messages(child.stdout.take().expect("piped stdout"));

    let initialize = RequestId::from(1);
    Message::Request(Request::new(
        initialize.clone(),
        "initialize".to_string(),
        json!({ "capabilities": {} }),
    ))
    .write(&mut stdin)
    .expect("write initialize");
    let result = response_to(&messages, initialize)
        .response_result
        .expect("an initialize result, not an error");
    // Not merely "something answered": the capability set is the language
    // server's own, so a subcommand that only opened a transport fails here.
    assert!(
        result["capabilities"]["textDocumentSync"].is_object(),
        "{result}"
    );

    Message::Notification(Notification::new("initialized".to_string(), json!({})))
        .write(&mut stdin)
        .expect("write initialized");
    let shutdown = RequestId::from(2);
    Message::Request(Request::new(
        shutdown.clone(),
        "shutdown".to_string(),
        json!(null),
    ))
    .write(&mut stdin)
    .expect("write shutdown");
    response_to(&messages, shutdown)
        .response_result
        .expect("a shutdown result, not an error");
    Message::Notification(Notification::new("exit".to_string(), json!(null)))
        .write(&mut stdin)
        .expect("write exit");
    drop(stdin);

    let status = wait_for_exit(&mut child, "ridl lsp");
    assert_eq!(status.code(), Some(0), "a clean shutdown exits 0");
}

#[test]
fn ridl_lsp_exits_two_when_stdin_closes_before_initialize() {
    let mut child = spawn_lsp();
    // Close stdin without sending a request: the server must notice the
    // transport ending and exit, not hang.
    drop(child.stdin.take().expect("piped stdin"));

    let status = wait_for_exit(&mut child, "ridl lsp");
    // A client that disappears before the handshake is a transport error, not
    // a clean shutdown: exit 2, the "could not answer" code of ADR-0010
    // decision 1.
    assert_eq!(status.code(), Some(2), "a lost transport exits 2");
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
            "ridl-servers-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
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
