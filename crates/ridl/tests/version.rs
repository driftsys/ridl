//! The binary and MCP server report the same normalized build stamp.
//! Pure normalization tests live in `build_version.rs`; no test rebuilds a binary.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
const MCP_TIMEOUT: Duration = Duration::from_secs(20);

/// Kills and reaps the wrapped child on drop — including when the drop runs
/// while unwinding past a panic. `std::process::Child`'s own `Drop` does
/// neither, so a test failure between spawning `ridl mcp` and reading its
/// response would otherwise leak the process.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawns the current `ridl mcp` binary, sends one
/// `initialize` request, and returns the `serverInfo.version` the response
/// carries.
///
/// This is the minimal roundtrip needed to read `serverInfo` back — not the
/// full handshake and clean-shutdown sequence
/// `crates/ridl/tests/servers.rs` performs for its own, more thorough MCP
/// coverage. The response is read on a worker thread so the wait for it can
/// be bounded by [`MCP_TIMEOUT`]: an unresponsive `ridl mcp` fails this test
/// within that bound instead of hanging the run until the CI job's own
/// timeout, which is what a plain blocking `read_line` would do.
fn mcp_server_info_version() -> String {
    let mut child = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_ridl"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn `ridl mcp`"),
    );
    let mut stdin = child.0.stdin.take().expect("piped stdin");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "version-test", "version": "0.0.0" }
            }
        })
    )
    .expect("write the initialize request");

    let stdout = child.0.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        if stdout.read_line(&mut line).is_ok() {
            let _ = sender.send(line);
        }
    });
    let line = receiver
        .recv_timeout(MCP_TIMEOUT)
        .unwrap_or_else(|err| panic!("no response from `ridl mcp` within {MCP_TIMEOUT:?}: {err}"));
    let response: serde_json::Value =
        serde_json::from_str(&line).expect("the response is one JSON-RPC line");

    response["result"]["serverInfo"]["version"]
        .as_str()
        .unwrap_or_else(|| panic!("serverInfo.version is a string: {response}"))
        .to_string()
}

#[test]
fn version_reports_the_normalized_build_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    let expected = env!("RIDL_BUILD_VERSION");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("ridl {expected}")
    );
    assert_eq!(mcp_server_info_version(), expected);
}
