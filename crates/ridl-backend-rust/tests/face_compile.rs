//! Compile proofs of the emitted face over sources the checked-in fixture does
//! not hold: the face is emitted from an inline ridl source, written to a
//! temporary directory, and checked with a bare `rustc` that links `ridl-rt`,
//! the way the codec proofs in `flatbuffers_roundtrip.rs` compile emitted
//! code. What is proven is that the source compiles, not what it does; the
//! round trips in `interaction_face.rs` are the behavioural proofs.

#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::generate_face;

/// Emits the face of `source` and checks it as a library crate
/// (`--emit=metadata -D warnings`), panicking with rustc's diagnostics when it
/// does not compile. The `std` cfg is on, so the `blocking` module the face
/// emits under it is part of what is checked.
fn face_compiles(name: &str, source: &str) {
    let output = ridlc::compile(&format!("{name}.ridl"), source);
    // Errors only: a call with no response bound draws RIDL-112, a warning,
    // and such a call is one of the shapes proven here.
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{name}.ridl must compile with no error, got: {errors:?}"
    );
    let face = generate_face(&output.package)
        .expect("generate_face")
        .rust_source;

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    std::fs::write(&source_path, &face).expect("the generated source is written");
    let ridl_rt = rustc::ridl_rt_rlib(dir.path());
    let compiled = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-D",
            "warnings",
        ])
        .arg("--cfg")
        .arg(r#"feature="std""#)
        .arg("-o")
        .arg(dir.path().join(format!("lib{name}.rmeta")))
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg(&source_path)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        compiled.status.success(),
        "the emitted face must compile, rustc said:\n{}\nsource:\n{face}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}

/// A ridl member or parameter may carry a name the emitter uses for a local
/// of its own. Story E11.21's first half put `port`, `deadline`, `this` and
/// `cx` beside a binding that carries the ridl parameter's name, in the call
/// method, the call future's `poll` and the internal send; each of the four
/// compiled before it. Those three bodies now rebind the ridl-named argument
/// to an emitter-owned name first, and the internal send names its port
/// parameter the same way, with the `__` prefix the codec's own locals use
/// (`__p`, `__v`). The generated `dispatch` still binds the ridl name directly
/// beside its own locals (`claim`, `h`, `accepted`, and `buf` for a query);
/// that collision predates the story and is not covered here.
#[test]
fn a_call_parameter_named_like_a_future_local_compiles() {
    face_compiles(
        "names",
        r#"
package face.names

type Level : integer [0..100]
type Window : integer [0..100000]
type Average : integer [0..1000]

interface Names {
  command port(port: Level) @[..50ms]
  command deadline(deadline: Level)
  query this(this: Window): Average @[..50ms]
  query cx(cx: Window): Average
}
"#,
    );
}
