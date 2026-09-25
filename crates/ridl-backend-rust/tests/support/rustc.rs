//! Building `ridl-rt` as an rlib with plain `rustc`, so an integration test
//! can compile — and run — generated code that names the runtime.
//!
//! This is the twin of `ridl_rt_rlib` in `crates/ridl-backend-rust/src/tests.rs`
//! and of the helper in `crates/ridlc/tests/support/mod.rs`, and it carries
//! the same `--cfg` arguments for the same reason: stage K3 settled
//! `--cfg feature="<encoding>"` on the existing bare-`rustc` helpers as this
//! lane's proof mechanism, because a generated `Payload<E>` implementation
//! names items a cargo feature gates and every proof links `ridl-rt`'s source
//! rather than a release. All three encoding features are enabled so that
//! E11.8 and E11.12 inherit a working proof without editing a helper again.
//!
//! One `rustc` call over `lib.rs` is the whole build: `ridl-rt` is `no_std`
//! with the encoding features and has no dependency in any feature combination
//! (ADR-0021 decision 8). It is built with the same `rustc` the proof itself
//! spawns; an rlib built by
//! another toolchain is rejected with E0514.

// Three integration test targets pull this module in with `#[path]`, and
// each uses the helpers it needs: `flatbuffers_roundtrip.rs` and
// `flatbuffers_bool_vector.rs` run programs, `flatbuffers_conformance.rs`
// captures their output and checks for wasm32. A helper unused by one of
// them is not dead code, it is used by another, and a `#[path]` module is
// compiled once per target that names it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Builds `ridl-rt` as an rlib in `dir` and returns its path.
pub fn ridl_rt_rlib(dir: &Path) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ridl-rt")
        .join("src")
        .join("lib.rs");
    let rlib = dir.join("libridl_rt.rlib");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "rlib",
            "--crate-name",
            "ridl_rt",
        ])
        .arg("--cfg")
        .arg(r#"feature="flatbuffers""#)
        .arg("--cfg")
        .arg(r#"feature="proto3""#)
        .arg("--cfg")
        .arg(r#"feature="repr-c""#)
        .arg(&source)
        .arg("-o")
        .arg(&rlib)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "ridl-rt must build as an rlib for the compile proofs to see it"
    );
    let head = std::fs::read(&rlib).expect("the rlib is readable");
    assert!(
        head.starts_with(b"!<arch>\n"),
        "the helper must produce an rlib archive, not metadata under an rlib name"
    );
    rlib
}

/// Compiles `source` as a program linking `ridl-rt`, runs it, and asserts it
/// exits zero.
///
/// A compile proof shows that generated code type-checks. Only a run shows
/// that the bytes it writes are the bytes it reads back, which is E11.7's own
/// `Done when`.
pub fn run_program(name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    let bin_path = dir.path().join(name);
    std::fs::write(&source_path, source).expect("the generated source is written");
    let ridl_rt = ridl_rt_rlib(dir.path());

    let status = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "bin", "-D", "warnings"])
        .arg("-o")
        .arg(&bin_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the generated codec must compile as a program, source:\n{source}"
    );

    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled program runs");
    assert!(
        run.status.success(),
        "the generated codec must behave as declared, stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// Compiles `source` as a program linking `ridl-rt`, runs it, asserts it
/// exits zero, and returns what it wrote to standard output.
///
/// The conformance test needs bytes to cross between the generated codec and
/// planus. The codec is compiled and run as a separate program — it is text
/// this crate emits, not a crate this test binary links — so the two cannot
/// share a value. They share a hexadecimal transcript on standard output
/// instead.
pub fn run_program_capturing_stdout(name: &str, source: &str) -> String {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    let bin_path = dir.path().join(name);
    std::fs::write(&source_path, source).expect("the generated source is written");
    let ridl_rt = ridl_rt_rlib(dir.path());

    let status = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "bin", "-D", "warnings"])
        .arg("-o")
        .arg(&bin_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the generated codec must compile as a program, source:\n{source}"
    );

    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled program runs");
    assert!(
        run.status.success(),
        "the generated codec must behave as declared, stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8(run.stdout).expect("the program writes UTF-8 on standard output")
}

/// Checks `source` for `wasm32-unknown-unknown`, the way [`run_program`]
/// compiles it for the host.
///
/// This is what `just wasm-check` owes over generated code. The recipe runs
/// `cargo check --target wasm32-unknown-unknown` over a fixed `-p` list of
/// workspace packages, and generated code is on no such list: it is text
/// this crate emits, which a test `include!`s or compiles as a program, with
/// no manifest of its own. `--emit=metadata` is what `cargo check` runs per
/// unit, so this performs the recipe's check through stage K3's proof
/// mechanism — a bare `rustc` over the emitted source, linking a `ridl-rt`
/// built the same way — rather than through a manifest that does not exist.
///
/// Returns `false`, having checked nothing, when `wasm32-unknown-unknown` is
/// not installed and cannot be added: the same guard `just wasm-check` puts
/// on `rustup`. The caller reports that rather than passing in silence.
pub fn check_for_wasm32(name: &str, source: &str) -> bool {
    if !ensure_wasm32_target() {
        return false;
    }
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    std::fs::write(&source_path, source).expect("the generated source is written");

    let ridl_rt_source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ridl-rt")
        .join("src")
        .join("lib.rs");
    let rlib = dir.path().join("libridl_rt.rlib");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "rlib",
            "--crate-name",
            "ridl_rt",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .arg("--cfg")
        .arg(r#"feature="flatbuffers""#)
        .arg("--cfg")
        .arg(r#"feature="proto3""#)
        .arg("--cfg")
        .arg(r#"feature="repr-c""#)
        .arg(&ridl_rt_source)
        .arg("-o")
        .arg(&rlib)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "ridl-rt must build for wasm32 before generated code can be checked against it"
    );

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--target",
            "wasm32-unknown-unknown",
            "--emit=metadata",
            "-D",
            "warnings",
        ])
        .arg("-o")
        .arg(dir.path().join(format!("lib{name}.rmeta")))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the generated code must check for wasm32-unknown-unknown, source:\n{source}"
    );
    true
}

/// Installs `wasm32-unknown-unknown` if it is missing, and reports whether
/// the target is available afterwards.
fn ensure_wasm32_target() -> bool {
    let Ok(installed) = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    else {
        return false;
    };
    if String::from_utf8_lossy(&installed.stdout)
        .lines()
        .any(|line| line.trim() == "wasm32-unknown-unknown")
    {
        return true;
    }
    std::process::Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
