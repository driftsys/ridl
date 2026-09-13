//! `ridl --version` and `ridl mcp`'s `serverInfo.version` report the build
//! version: the `editor-v*` tag when the release workflow set
//! `RIDL_BUILD_VERSION` (`build.rs`), else the crate version — the same
//! string from both, so a bug report names one build regardless of which
//! server the reporter queried.
//!
//! The version is baked in at compile time — `src/main.rs` reads it through
//! `env!("RIDL_BUILD_VERSION")` — so the injected branch is only observable
//! by rebuilding the binary with the variable set. Comparing the already-
//! built `CARGO_BIN_EXE_ridl` against `std::env::var` at test-run time (as a
//! first draft of this test did) cannot exercise that branch under `just
//! test`, which never sets the variable: the comparison passes whether or
//! not the injection code exists at all, because both sides independently
//! fall back to the crate version. This test instead rebuilds the binary
//! itself, with the variable set and then removed, and checks each build's
//! output.
//!
//! `build.rs` declares `cargo:rerun-if-env-changed=RIDL_BUILD_VERSION`, so a
//! changed value invalidates only the `ridl` crate's own build script and
//! compile unit — every dependency stays cached, and a rebuild costs a
//! couple of seconds, not a workspace recompile. That is true for a plain
//! shell invocation; a cargo build spawned *from inside a running test*
//! inherits this process's own environment, which cargo has already
//! populated with this package's `CARGO_MANIFEST_DIR`, `CARGO_PKG_*`, and
//! `OUT_DIR` (the variables it sets for every test binary of a package that
//! has a build script). A couple of `ridl-core`'s optional dependencies
//! (`ureq`, and transitively `rustls`/`ring`) read some of those in their
//! own build scripts; inherited instead of values scoped to the nested
//! build, they read as "changed" and force an unrelated rebuild of that
//! whole chain (confirmed with `cargo build -v`: `Dirty ring: the env
//! variable CARGO_MANIFEST_DIR changed`). `rebuild_and_run_version` strips
//! the leaked variables so the nested build sees the same environment a
//! plain shell invocation would, and reliably costs only the couple of
//! seconds `ridl` itself takes to recompile.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// A value distinct from the crate's own version, so the injected and
/// fallback cases cannot pass by coincidence.
const INJECTED_VERSION: &str = "editor-v9.9.9-test";

/// Rebuilds the `ridl` binary with `RIDL_BUILD_VERSION` set to `version` (or
/// removed, for `None`) and returns `ridl --version`'s trimmed stdout.
///
/// Uses `CARGO_MANIFEST_DIR` (this crate's own directory) rather than relying
/// on the test process's current directory, and `CARGO` (the path to the
/// cargo binary running this test, which cargo always sets for a spawned
/// test) rather than assuming `cargo` is on `PATH`.
fn rebuild_and_run_version(version: Option<&str>) -> String {
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut command = Command::new(&cargo);
    command.args(["build", "--locked", "--manifest-path", &manifest]);
    // Variables cargo sets for this test binary's own runtime, which a
    // dependency's build script (see the module doc) can misread as this
    // nested build's environment rather than as leftovers from the outer
    // one.
    for key in std::env::vars().map(|(key, _)| key) {
        if key.starts_with("CARGO_PKG_")
            || key.starts_with("CARGO_BIN_EXE_")
            || matches!(
                key.as_str(),
                "CARGO_MANIFEST_DIR" | "CARGO_MANIFEST_PATH" | "OUT_DIR"
            )
        {
            command.env_remove(key);
        }
    }
    match version {
        Some(version) => {
            command.env("RIDL_BUILD_VERSION", version);
        }
        None => {
            command.env_remove("RIDL_BUILD_VERSION");
        }
    }
    let status = command.status().expect("run `cargo build` for `ridl`");
    assert!(status.success(), "rebuilding `ridl` failed");

    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("--version")
        .output()
        .expect("run `ridl --version`");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Spawns `ridl mcp` from the currently-built `CARGO_BIN_EXE_ridl` (so it
/// reflects whichever `rebuild_and_run_version` call ran last), sends one
/// `initialize` request, and returns the `serverInfo.version` the response
/// carries.
///
/// This is the minimal roundtrip needed to read `serverInfo` back — not the
/// full handshake and clean-shutdown sequence
/// `crates/ridl/tests/servers.rs` performs for its own, more thorough MCP
/// coverage; killing the child once the response is read is enough here.
fn mcp_server_info_version() -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn `ridl mcp`");
    let mut stdin = child.stdin.take().expect("piped stdin");
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

    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .expect("read the initialize response");
    let response: serde_json::Value =
        serde_json::from_str(&line).expect("the response is one JSON-RPC line");
    let _ = child.kill();
    let _ = child.wait();

    response["result"]["serverInfo"]["version"]
        .as_str()
        .unwrap_or_else(|| panic!("serverInfo.version is a string: {response}"))
        .to_string()
}

/// Pins the actual injection mechanism: rebuilding with `RIDL_BUILD_VERSION`
/// set changes the reported version to exactly that value — in both
/// `ridl --version` and `ridl mcp`'s `serverInfo.version` — and rebuilding
/// again with the variable unset reports exactly the crate version — so
/// either half of `build.rs`'s fallback (`unwrap_or_else`) being broken, or
/// `src/main.rs` no longer reading `RIDL_BUILD_VERSION` at all for either
/// server, fails this test.
///
/// The MCP check has to live here rather than as a standalone assertion:
/// `crates/ridl/tests/servers.rs`'s own `serverInfo.version` assertion
/// compares against `env!("RIDL_BUILD_VERSION")`, which is *this build's*
/// compile-time value — a meaningful check when the whole test run was
/// invoked with the variable already set, but not under plain `just test`,
/// where it never is: every workspace crate shares the same `0.0.0` version
/// (`version.workspace = true`), so under an unset variable,
/// `ridl-mcp`'s own `CARGO_PKG_VERSION` and `ridl`'s `RIDL_BUILD_VERSION`
/// fallback are numerically the same value regardless of whether `run_mcp`
/// actually wires one into the other. Only a build with a *distinct*,
/// non-default value — this test's own rebuild — tells the two apart.
///
/// The second rebuild also leaves `target/debug/ridl` — the same binary
/// `CARGO_BIN_EXE_ridl` and every other integration test in this crate
/// spawns — back in its ordinary, un-injected state.
#[test]
fn version_reports_the_injected_build_version_and_falls_back_without_one() {
    let injected = rebuild_and_run_version(Some(INJECTED_VERSION));
    assert_eq!(
        injected,
        format!("ridl {INJECTED_VERSION}"),
        "with RIDL_BUILD_VERSION set, `ridl --version` must report the injected value"
    );
    assert_eq!(
        mcp_server_info_version(),
        INJECTED_VERSION,
        "with RIDL_BUILD_VERSION set, `ridl mcp`'s serverInfo.version must report the same \
         injected value as `ridl --version`"
    );

    let fallback = rebuild_and_run_version(None);
    assert_eq!(
        fallback,
        format!("ridl {}", env!("CARGO_PKG_VERSION")),
        "with RIDL_BUILD_VERSION unset, `ridl --version` must report the crate version"
    );

    assert_ne!(
        injected, fallback,
        "the injected and fallback versions must differ, or this test cannot tell them apart"
    );
}
