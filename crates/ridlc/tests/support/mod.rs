//! Shared support for the compile proofs in this crate's integration tests.

use std::path::{Path, PathBuf};

/// Builds `ridl-rt` as an rlib with plain `rustc`, so a compile proof can
/// pass `--extern ridl_rt=<path>` for the generated source that names the
/// runtime. Every generated named scalar names
/// `::ridl_rt::payload::Violation` from its `new`, so every proof over
/// generated Rust needs it.
///
/// This is the same helper `crates/ridl-backend-rust/src/tests.rs` carries.
/// That one lives in a unit-test module, which an integration test of another
/// crate cannot reach, so the two are kept in step by hand.
///
/// One `rustc` call over its `lib.rs` is the whole build: `ridl-rt` is
/// `no_std` and has no dependency in any feature combination (ADR-0021
/// decision 8). It is built with the same `rustc` the proof itself spawns; an
/// rlib built by another toolchain is rejected with E0514.
///
/// **The three encoding features are enabled here**, which is the proof
/// mechanism E11.7 stage K3 settled. A generated `Payload<E>` implementation
/// names items that a feature gates — `ridl_rt::flatbuffers` is the first —
/// and this rlib is what every compile proof links, so a proof over generated
/// codec code does not compile against a bare build. One `--cfg` argument per
/// encoding is the whole mechanism; the alternative considered was a cargo
/// build of an emitted crate with path dependencies, which costs a manifest,
/// a target directory and a cargo run per proof and still does not build what
/// a consumer outside this repository builds. Enabling all three rather than
/// only `flatbuffers` is so that E11.8 and E11.12 inherit a working proof
/// without editing this helper again.
///
/// Edition 2021 is the edition `crates/ridl-rt/Cargo.toml` declares. The
/// generated code keeps compiling as edition 2024; the two are independent.
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
    // An rlib is an `ar` archive. Asserting the magic distinguishes a real
    // rlib from a metadata-only file under the same name, which a proof using
    // `--emit metadata` would accept while nothing that links could.
    let head = std::fs::read(&rlib).expect("the rlib is readable");
    assert!(
        head.starts_with(b"!<arch>\n"),
        "the helper must produce an rlib archive, not metadata under an rlib name"
    );
    rlib
}
