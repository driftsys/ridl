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
/// does not compile. It is checked twice: with the `std` cfg on, against a
/// `ridl-rt` built with `std`, so the `blocking` module is part of what is
/// checked; and with the cfg off, against a `ridl-rt` built without `std`,
/// so an item the emitter leaves outside `cfg(feature = "std")` — the
/// `blocking` module, which names `ridl_rt::task::block_on`, or the call
/// future's `sent()`, which only that module calls and which is dead code
/// without it — fails the build.
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
    for std in [true, false] {
        let ridl_rt = if std {
            rustc::ridl_rt_rlib(dir.path())
        } else {
            rustc::ridl_rt_rlib_without_std(dir.path())
        };
        let mut command = std::process::Command::new("rustc");
        command.args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-D",
            "warnings",
        ]);
        if std {
            command.arg("--cfg").arg(r#"feature="std""#);
        }
        let compiled = command
            .arg("-o")
            .arg(dir.path().join(format!("lib{name}_std_{std}.rmeta")))
            .arg("--extern")
            .arg(format!("ridl_rt={}", ridl_rt.display()))
            .arg(&source_path)
            .output()
            .expect("rustc must be installed and runnable for this test to be meaningful");
        assert!(
            compiled.status.success(),
            "the emitted face must compile with the std cfg {}, rustc said:\n{}\nsource:\n{face}",
            if std { "on" } else { "off" },
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

/// A ridl member or parameter may carry a name the emitter uses for a local
/// of its own. Story E11.21's first half put `port`, `deadline`, `this` and
/// `cx` beside a binding that carries the ridl parameter's name, in the call
/// method, the call future's `poll` and the internal send; each of the four
/// compiled before it. Those three bodies now rebind the ridl-named argument
/// to an emitter-owned name first, and the internal send names its port
/// parameter the same way, with the `__` prefix the codec's own locals use
/// (`__p`, `__v`). The blocking module's deadline helper is a module-level
/// function, and a parameter named `deadlineAfter` is bound as `deadline_after`
/// in the method body, so the function is `__deadline_after` for the same
/// reason. The generated `dispatch` has the same treatment; its cases follow.
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
  command deadlineAfter(deadlineAfter: Level)
}
"#,
    );
}

/// The generated `dispatch` binds a claim's decoded argument next to its own
/// parameters and locals. Before issue #570 it bound the argument under the
/// ridl parameter's own name, so a parameter named like one of them failed
/// to compile; it now binds the argument as `__arg`, which no ridl identifier
/// can be. These six are the names that collided: the parameters `h`, `p`
/// and `buf`, and the locals `claim`, `accepted` and `reply`. Each name is a
/// parameter of one command and of one query, and the member names say which
/// arm and which parameter, so a failing compile names both. One face holds
/// every case, because each `face_compiles` call builds ridl-rt twice.
#[test]
fn a_call_parameter_named_like_a_dispatch_local_compiles() {
    let members: String = ["claim", "h", "accepted", "buf", "p", "reply"]
        .iter()
        .map(|param| {
            let suffix = format!("{}{}", param[..1].to_uppercase(), &param[1..]);
            format!(
                "  command set{suffix}({param}: Level) @[..50ms]\n  \
                 query read{suffix}({param}: Window): Average @[..50ms]\n"
            )
        })
        .collect();
    face_compiles(
        "dispatch_locals",
        &format!(
            r#"
package face.dispatchlocals

type Level : integer [0..100]
type Window : integer [0..100000]
type Average : integer [0..1000]

interface Dispatch {{
{members}}}
"#
        ),
    );
}
