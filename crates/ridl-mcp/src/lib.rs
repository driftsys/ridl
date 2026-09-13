//! The RIDL MCP server (ADR-0005 Layer B; docs/ROADMAP.md epic E8.6).
//!
//! One tool, `ridl_check`: the compiler's single-file check
//! (`ridlc::check_source`) over a source string an agent supplies, returned
//! as the JSON diagnostic contract (`ridl_core::diag::JsonDiagnostic`). The
//! server is a thin consumer of the shared compiler crates — no parser, no
//! checker of its own — and runs over stdio behind `ridl mcp`.

use ridl_core::diag::{JsonDiagnostic, to_json};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::schemars::JsonSchema;
use rmcp::transport::stdio;
use rmcp::{ErrorData, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

// This enum is the only gate on the profile, and its doc comment is the
// description the tool's JSON schema carries to an agent. `ridlc` reads the
// profile off the file extension and treats every extension that is not
// `.ridl` as typl, so a profile name this enum did not reject would be
// checked as typl rather than refused.
/// Which language a source text is parsed as. These are the two profiles the
/// compiler has; the `.rxdl` form does not exist yet (epic E3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(crate = "rmcp::schemars")]
pub enum Profile {
    Typl,
    Ridl,
}

impl Profile {
    /// The path the source is registered under: `ridlc` selects the profile
    /// from the extension, and this path is what spans report.
    fn synthetic_path(self) -> &'static str {
        match self {
            Profile::Typl => "input.typl",
            Profile::Ridl => "input.ridl",
        }
    }
}

/// The `ridl_check` tool's input.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CheckParams {
    /// The full text of one `.typl` or `.ridl` file.
    pub source: String,
    /// Which language `source` is parsed as.
    pub profile: Profile,
}

/// The `ridl_check` tool's output.
#[derive(Debug, Serialize)]
pub struct CheckOutput {
    pub diagnostics: Vec<JsonDiagnostic>,
}

/// Checks `params.source` under `params.profile` against the embedded
/// `ridl.std`. Pure: no transport, no I/O.
pub fn check(params: &CheckParams) -> CheckOutput {
    let run = ridlc::check_source(params.profile.synthetic_path(), &params.source);
    CheckOutput {
        diagnostics: to_json(&run.diagnostics, &run.sources),
    }
}

/// The MCP server: the tool router and the version it advertises.
#[derive(Clone)]
pub struct RidlMcp {
    tool_router: ToolRouter<Self>,
    /// Reported as `serverInfo.version` in `get_info`. The `ridl` binary
    /// passes its own build version here (`with_version`), so `ridl mcp` and
    /// `ridl lsp` agree; a caller driving this library directly (including
    /// this crate's own tests) gets this crate's version through `new`.
    version: String,
    /// What `ridl_check` runs: [`check`], except in this crate's own tests.
    /// No input is known to make the compiler panic, so a test installs a
    /// function that does, to reach the handler's failure path.
    check: fn(&CheckParams) -> CheckOutput,
}

impl Default for RidlMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RidlMcp {
    /// A server reporting this crate's own version — the sensible default for
    /// anything that drives this library directly rather than through the
    /// `ridl` binary.
    pub fn new() -> Self {
        Self::with_version(env!("CARGO_PKG_VERSION"))
    }

    /// A server reporting `version` instead of the crate's own version.
    pub fn with_version(version: impl Into<String>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            version: version.into(),
            check,
        }
    }

    #[tool(
        name = "ridl_check",
        description = "Type-check one typl or ridl source text against the embedded ridl.std. Returns the compiler's coded diagnostics with their spans and fix-its, verbatim. Every span reports the path `input.typl` or `input.ridl`, a fixed synthetic name for the text you supplied rather than a file on disk."
    )]
    async fn ridl_check(
        &self,
        Parameters(params): Parameters<CheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // The check is synchronous compiler work, so it runs on the blocking
        // pool rather than on the worker that drives the transport. That is
        // also what turns a panic in the compiler into an error this call
        // reports: `rmcp` spawns one task per request and sends the response
        // from it, so a panic that unwound through this handler would end
        // that task before any response was sent, leave the client waiting
        // until its own timeout, and leave the request's cancellation token
        // in the SDK's pool. Tokio catches the blocking task's panic instead
        // and hands it back here as a `JoinError`.
        let check = self.check;
        let output = tokio::task::spawn_blocking(move || check(&params))
            .await
            .map_err(checker_failed)?;
        Ok(CallToolResult::success(vec![ContentBlock::json(&output)?]))
    }
}

/// The error a `tools/call` gets when the blocking task running the check
/// did not return: the compiler panicked, or the runtime shut down under
/// it. The message says the checker failed and carries the panic's own
/// message when it has one — the backtrace, when there is one, stays on
/// stderr where the panic hook wrote it, never in the response.
fn checker_failed(err: tokio::task::JoinError) -> ErrorData {
    let mut message = String::from("the checker failed on this input");
    if err.is_panic() {
        let payload = err.into_panic();
        let text = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        if let Some(text) = text {
            message.push_str(": ");
            message.push_str(text);
        }
    }
    ErrorData::internal_error(message, None)
}

// `router = self.tool_router` rather than the macro's default
// `Self::tool_router()`, which would rebuild the router on every request.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for RidlMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            // Without this, `ServerInfo::new` keeps
            // `Implementation::from_build_env()`, whose `env!` calls expand
            // inside rmcp: every host would display and log this server as
            // `rmcp` at the SDK's version, not this crate's own version or
            // (through `with_version`) the `ridl` binary's build version.
            .with_server_info(Implementation::new("ridl-mcp", self.version.clone()))
            .with_instructions(
                "RIDL compiler tools. Call ridl_check with a source text and a profile \
                 (typl or ridl) to get coded diagnostics with fix-its.",
            )
    }
}

/// Serves the MCP protocol over this process's stdin and stdout until the
/// client disconnects, reporting `version` instead of this crate's own
/// version when one is given — the `ridl` binary passes its build version
/// here, so `ridl mcp` and `ridl lsp` agree. `None` is this crate's own
/// version, `RidlMcp::new`'s default; the `ridl` binary is this function's
/// only caller today, and it always passes `Some`.
pub async fn serve_stdio_with_version(
    version: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = match version {
        Some(version) => RidlMcp::with_version(version),
        None => RidlMcp::new(),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rmcp::service::QuitReason;
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

    use super::*;

    // The fixture deliberately has no trailing newline: with one, the parser
    // reports the missing backing type at the position past it, which is
    // line 3 column 1 rather than the end of the `type X:` line.
    const BROKEN_TYPL: &str = "package p\ntype X:";

    #[test]
    fn check_reports_an_error_for_a_broken_typl_source() {
        let output = check(&CheckParams {
            source: BROKEN_TYPL.to_string(),
            profile: Profile::Typl,
        });
        let error = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == "error")
            .expect("an error diagnostic");
        assert_eq!(error.span.path, "input.typl");
        assert_eq!(error.span.start.line, 2);
        assert_eq!(error.span.start.column, 8);
    }

    #[test]
    fn check_output_serializes_as_a_diagnostics_array() {
        let output = check(&CheckParams {
            source: BROKEN_TYPL.to_string(),
            profile: Profile::Typl,
        });
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&output).expect("serializes"))
                .expect("valid JSON");
        let object = value.as_object().expect("a JSON object");
        assert_eq!(object.keys().collect::<Vec<_>>(), vec!["diagnostics"]);
        let first = object["diagnostics"][0]
            .as_object()
            .expect("a diagnostic object");
        let mut keys: Vec<&str> = first.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["code", "fixes", "labels", "message", "severity", "span"]
        );
    }

    #[test]
    fn check_selects_the_profile_from_the_parameter() {
        let source = "package p\ninterface I {}\n".to_string();
        let ridl = check(&CheckParams {
            source: source.clone(),
            profile: Profile::Ridl,
        });
        let typl = check(&CheckParams {
            source,
            profile: Profile::Typl,
        });
        assert!(ridl.diagnostics.iter().all(|d| d.severity != "error"));
        assert!(typl.diagnostics.iter().any(|d| d.severity == "error"));
    }

    #[test]
    fn profile_deserializes_lowercase_only() {
        assert!(serde_json::from_str::<Profile>("\"typl\"").is_ok());
        assert!(serde_json::from_str::<Profile>("\"ridl\"").is_ok());
        assert!(serde_json::from_str::<Profile>("\"rxdl\"").is_err());
        assert!(serde_json::from_str::<Profile>("\"Typl\"").is_err());
    }

    #[test]
    fn the_server_advertises_exactly_one_tool_named_ridl_check() {
        let server = RidlMcp::new();
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        assert_eq!(names, vec!["ridl_check".to_string()]);
    }

    #[test]
    fn the_server_announces_itself_rather_than_the_sdk() {
        let info = RidlMcp::new().get_info();
        // rmcp's own default is `Implementation::from_build_env()`, which
        // reports the SDK's crate name and version, not this crate's.
        assert_eq!(info.server_info.name, "ridl-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn with_version_reports_the_given_version_instead_of_the_crate_version() {
        let info = RidlMcp::with_version("editor-v1.2.3").get_info();
        assert_eq!(info.server_info.name, "ridl-mcp");
        assert_eq!(info.server_info.version, "editor-v1.2.3");
    }

    #[tokio::test]
    async fn the_tool_returns_the_check_output_as_one_json_text_block() {
        let server = RidlMcp::new();
        let result = server
            .ridl_check(Parameters(CheckParams {
                source: BROKEN_TYPL.to_string(),
                profile: Profile::Typl,
            }))
            .await
            .expect("the tool succeeds");
        assert_eq!(result.is_error, Some(false));
        let [ContentBlock::Text(block)] = result.content.as_slice() else {
            panic!("expected exactly one text content block");
        };
        let value: serde_json::Value =
            serde_json::from_str(&block.text).expect("the block holds JSON");
        assert_eq!(value["diagnostics"][0]["code"], json!("FORM-101"));
        assert_eq!(value["diagnostics"][0]["span"]["path"], json!("input.typl"));
    }

    /// The check the panic test installs: it panics on one sentinel source
    /// and is the real [`check`] on every other, so one server can show both
    /// the failed call and the call after it.
    fn check_or_panic(params: &CheckParams) -> CheckOutput {
        if params.source == "panic" {
            panic!("a checker panic driven by the test");
        }
        check(params)
    }

    /// Writes `message` as one newline-delimited JSON-RPC line.
    async fn send(writer: &mut WriteHalf<DuplexStream>, message: serde_json::Value) {
        let mut line = message.to_string();
        line.push('\n');
        writer
            .write_all(line.as_bytes())
            .await
            .expect("write to the pipe");
    }

    /// Reads the next JSON-RPC line the server wrote.
    async fn next(
        reader: &mut tokio::io::Lines<BufReader<ReadHalf<DuplexStream>>>,
    ) -> serde_json::Value {
        let line = reader
            .next_line()
            .await
            .expect("read from the pipe")
            .expect("a line before the server closes the pipe");
        serde_json::from_str(&line).expect("the server writes JSON-RPC lines")
    }

    /// One `tools/call` of `ridl_check` over `source` as typl.
    fn call(id: u32, source: &str) -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "ridl_check",
                "arguments": { "source": source, "profile": "typl" }
            }
        })
    }

    // Without `spawn_blocking` in `ridl_check`, a panic ends the task `rmcp`
    // spawned for the request before it sends any response, so the client
    // waits for an answer that never comes: this test then fails on its
    // timeout, with the second call never made.
    #[tokio::test]
    async fn a_checker_panic_fails_that_call_and_the_server_answers_the_next() {
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut server = RidlMcp::new();
            server.check = check_or_panic;
            // The server is served over one end of an in-process pipe and the
            // test speaks newline-delimited JSON-RPC on the other end: the
            // framing stdio uses, without the `client` feature of `rmcp`.
            let (server_end, client_end) = tokio::io::duplex(64 * 1024);
            let served = tokio::spawn(async move {
                let service = server
                    .serve(server_end)
                    .await
                    .expect("the initialize handshake");
                service.waiting().await.expect("the serve loop task joins")
            });
            let (client_read, mut client_write) = tokio::io::split(client_end);
            let mut responses = BufReader::new(client_read).lines();

            send(
                &mut client_write,
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "clientInfo": { "name": "lib-test", "version": "0.0.0" }
                    }
                }),
            )
            .await;
            let initialized = next(&mut responses).await;
            assert_eq!(
                initialized["result"]["serverInfo"]["name"],
                json!("ridl-mcp"),
                "{initialized}"
            );
            send(
                &mut client_write,
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            )
            .await;

            // The panicking call gets an error response, not silence.
            send(&mut client_write, call(2, "panic")).await;
            let failed = next(&mut responses).await;
            assert_eq!(failed["id"], json!(2), "{failed}");
            assert!(failed.get("result").is_none(), "{failed}");
            let message = failed["error"]["message"]
                .as_str()
                .unwrap_or_else(|| panic!("an error message: {failed}"));
            assert!(
                message.starts_with("the checker failed on this input"),
                "{failed}"
            );
            assert!(
                message.contains("a checker panic driven by the test"),
                "{failed}"
            );
            assert!(
                !message.contains("panicked at"),
                "no panic location in the response: {failed}"
            );

            // The server still serves: the next call is answered in full.
            send(&mut client_write, call(3, BROKEN_TYPL)).await;
            let answered = next(&mut responses).await;
            assert_eq!(answered["id"], json!(3), "{answered}");
            let text = answered["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_else(|| panic!("one text block: {answered}"));
            let output: serde_json::Value =
                serde_json::from_str(text).expect("the block holds JSON");
            assert_eq!(
                output["diagnostics"][0]["code"],
                json!("FORM-101"),
                "{answered}"
            );

            // Closing the write side is the host closing stdin: a clean end.
            client_write.shutdown().await.expect("close the pipe");
            let reason = served.await.expect("the server task joins");
            assert!(matches!(reason, QuitReason::Closed), "{reason:?}");
        })
        .await
        .expect("the test did not finish within the timeout");
    }

    #[test]
    fn the_advertised_schema_offers_only_the_two_profiles() {
        let server = RidlMcp::new();
        let tools = server.tool_router.list_all();
        let tool = tools.first().expect("one tool");
        let schema = serde_json::to_value(&*tool.input_schema).expect("the schema serializes");
        assert_eq!(schema["$defs"]["Profile"]["enum"], json!(["typl", "ridl"]));
        assert_eq!(schema["required"], json!(["source", "profile"]));
    }
}
