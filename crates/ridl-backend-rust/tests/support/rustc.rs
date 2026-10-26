//! Building `ridl-rt` as an rlib with plain `rustc`, so an integration test
//! can compile — and run — generated code that names the runtime.
//!
//! This is the twin of `ridl_rt_rlib` in `crates/ridl-backend-rust/src/tests.rs`
//! and of the helper in `crates/ridlc/tests/support/mod.rs`, and it carries
//! the same `--cfg` arguments for the same reason: the project settled
//! `--cfg feature="<encoding>"` on the existing bare-`rustc` helpers as the
//! proof mechanism, because a generated `Payload<E>` implementation
//! names items a cargo feature gates and every proof links `ridl-rt`'s source
//! rather than a release. All three encoding features are enabled so that
//! the other encodings inherit a working proof without editing a helper again.
//!
//! One `rustc` call over `lib.rs` is the whole build: `ridl-rt` is `no_std`
//! with the encoding features and has no dependency in any feature combination
//! (ADR-0021 decision 8). It is built with the same `rustc` the proof itself
//! spawns; an rlib built by
//! another toolchain is rejected with E0514. The `std` feature is on as well,
//! because the face proof in `face_compile.rs` compiles the emitted `blocking`
//! module, which names `ridl_rt::task::block_on`.

// Three integration test targets pull this module in with `#[path]`, and
// each uses the helpers it needs: `flatbuffers_roundtrip.rs` and
// `flatbuffers_bool_vector.rs` run programs, `flatbuffers_conformance.rs`
// captures their output and checks for wasm32. A helper unused by one of
// them is not dead code, it is used by another, and a `#[path]` module is
// compiled once per target that names it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Builds `ridl-rt` as an rlib in `dir`, with the `std` feature, and
/// returns its path.
pub fn ridl_rt_rlib(dir: &Path) -> PathBuf {
    ridl_rt_rlib_with(dir, true)
}

/// Builds `ridl-rt` as an rlib in `dir` without the `std` feature, so a
/// proof can compile emitted code with `std` off against a runtime that has
/// no `task` module: an item the emitter left outside `cfg(feature = "std")`
/// then fails to resolve.
pub fn ridl_rt_rlib_without_std(dir: &Path) -> PathBuf {
    ridl_rt_rlib_with(dir, false)
}

/// Builds `ridl-rt` as an rlib in `dir` with no feature at all — no encoding
/// and no `std` — so a proof can compile a types-only package against the
/// runtime a consumer links when its generated `Cargo.toml` enables no
/// encoding feature: `::ridl_rt::flatbuffers` does not exist in that build,
/// so a codec item left in a types-only package fails to resolve.
pub fn ridl_rt_rlib_bare(dir: &Path) -> PathBuf {
    ridl_rt_rlib_features(dir, "libridl_rt_bare.rlib", &[])
}

fn ridl_rt_rlib_with(dir: &Path, std: bool) -> PathBuf {
    let mut features = vec!["flatbuffers", "proto3", "podbuf"];
    if std {
        features.push("std");
    }
    let file = if std {
        "libridl_rt.rlib"
    } else {
        "libridl_rt_no_std.rlib"
    };
    ridl_rt_rlib_features(dir, file, &features)
}

/// One `rustc` call over `ridl-rt`'s `lib.rs`, with one `--cfg feature="…"`
/// per entry of `features`, written to `<dir>/<file>`.
fn ridl_rt_rlib_features(dir: &Path, file: &str, features: &[&str]) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ridl-rt")
        .join("src")
        .join("lib.rs");
    let rlib = dir.join(file);
    let mut command = std::process::Command::new("rustc");
    command.args([
        "--edition",
        "2021",
        "--crate-type",
        "rlib",
        "--crate-name",
        "ridl_rt",
    ]);
    for feature in features {
        command.arg("--cfg").arg(format!(r#"feature="{feature}""#));
    }
    let status = command
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
/// that the bytes it writes are the bytes it reads back, which is the codec's own
/// `Done when`.
pub fn run_program(name: &str, source: &str) {
    run_program_with_pattern(name, source, false);
}

/// Runs emitted pattern validation against the regex library already built by
/// Cargo for the source compiler's semantic checks.
pub fn run_program_with_pattern(name: &str, source: &str, validate_pattern: bool) {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    let bin_path = dir.path().join(name);
    std::fs::write(&source_path, source).expect("the generated source is written");
    let ridl_rt = ridl_rt_rlib(dir.path());

    let mut command = std::process::Command::new("rustc");
    if validate_pattern {
        let executable = std::env::current_exe().expect("the test executable has a path");
        let dependencies = executable
            .parent()
            .expect("the executable is in Cargo's deps directory");
        let regex = std::fs::read_dir(dependencies)
            .expect("Cargo's dependencies are readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("libregex-") && name.ends_with(".rlib")
                })
            })
            .expect("the semantic compiler builds regex");
        command
            .arg("--cfg")
            .arg(r#"feature="validate-pattern""#)
            .arg("--extern")
            .arg(format!("regex={}", regex.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()));
    }
    let status = command
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
/// unit, so this performs the recipe's check through the bare-`rustc` proof
/// mechanism — a bare `rustc` over the emitted source, linking a `ridl-rt`
/// built the same way — rather than through a manifest that does not exist.
///
/// Returns `false`, having checked nothing, when `wasm32-unknown-unknown` is
/// not installed and cannot be added: the same guard `just wasm-check` puts
/// on `rustup`. The caller reports that rather than passing in silence.
pub fn check_for_wasm32(name: &str, source: &str) -> bool {
    check_for_target(
        "wasm32-unknown-unknown",
        &["flatbuffers", "proto3", "podbuf"],
        name,
        source,
    )
}

/// Checks `source` for `thumbv7em-none-eabihf`, a target with no standard
/// library, against a `ridl-rt` built for it with only the `podbuf` feature.
/// `source` has to be `no_std` itself. Returns `false`, having checked
/// nothing, when the target is not installed and cannot be added.
pub fn check_for_thumbv7em(name: &str, source: &str) -> bool {
    check_for_target("thumbv7em-none-eabihf", &["podbuf"], name, source)
}

/// [`check_for_wasm32`] for any `target`, with `ridl-rt` built for it with
/// the features in `features`.
fn check_for_target(target: &str, features: &[&str], name: &str, source: &str) -> bool {
    if !ensure_target(target) {
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
            target,
        ])
        .args(
            features
                .iter()
                .flat_map(|feature| ["--cfg".to_string(), format!(r#"feature="{feature}""#)]),
        )
        .arg(&ridl_rt_source)
        .arg("-o")
        .arg(&rlib)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "ridl-rt must build for {target} before generated code can be checked against it"
    );

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--target",
            target,
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
        "the generated code must check for {target}, source:\n{source}"
    );
    true
}

/// Installs `target` if it is missing, and reports whether the target is
/// available afterwards.
fn ensure_target(target: &str) -> bool {
    let Ok(installed) = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    else {
        return false;
    };
    if String::from_utf8_lossy(&installed.stdout)
        .lines()
        .any(|line| line.trim() == target)
    {
        return true;
    }
    std::process::Command::new("rustup")
        .args(["target", "add", target])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
