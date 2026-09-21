//! The end-to-end proof of story E11.14 (driftsys/ridl#444): what
//! `ridl build --emit rust` writes is a crate an application outside this
//! workspace can link against and **run**.
//!
//! What is new here is the **path**, not the running. Other proofs already run
//! generated code: `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`
//! runs a program built from the codec's output, and
//! `crates/ridl-backend-rust/tests/interaction_face.rs` already runs these
//! same four round trips over `ridl-loopback`. Each of those runs the
//! backend's own output, in this workspace's own test crate.
//!
//! This one runs what **the CLI wrote**, linked as a separate crate into a
//! separate process. It runs the build over `examples/cabin/`, compiles the
//! emitted crate with plain `rustc`, compiles `examples/cabin/consumer.rs`
//! against it, runs the program, and requires it to exit zero having
//! completed one round trip of every interaction kind that carries a payload
//! — a signal, an event, a command and a query — through the generated
//! `Client`, `Publisher`, `Provider` and `dispatch` over `Loopback`.
//!
//! **What this does not establish.** The round trip is symmetric, so it
//! cannot see an identity error: a wrong `InterfaceNo`, a wrong ordinal or a
//! wrong catalog hash would be written and read back consistently and every
//! assertion here would still hold. Those are pinned by
//! `crates/ridl-backend-rust/tests/descriptor_generation.rs`. Nor does the
//! consumer drive a failing `require` or `ensure`; the clause paths are
//! `interaction_face.rs`'s.
//!
//! Nothing here goes through cargo. `rustc` is spawned directly, the same
//! mechanism the crate's other compile proofs use, because a cargo build of
//! an emitted crate costs a manifest, a target directory and a cargo run per
//! proof and still does not build what a consumer outside this repository
//! builds.
//!
//! One source serves both proofs. `examples/cabin/consumer/src/main.rs` is
//! also `just demo`'s program, built there by cargo against the same schema,
//! so the crate name here is `veh_cabin` — the package name `ridlc` writes
//! into the emitted `Cargo.toml` — rather than one this test invents. That
//! manifest is read by the demo and written but not read here.
//!
//! The two proofs differ in what they hold fixed. The demo builds the way a
//! person does, through cargo and the committed `examples/cabin/Cargo.lock`,
//! with `ridl-rt` patched to this repository's copy. This test builds with
//! bare `rustc` against an `ridl-rt` rlib it builds itself, so it needs no
//! manifest, no lock and no registry, and it is what runs under `just test`. For the same reason the `ridl-rt` this links is
//! built with the three encoding features and not with the `std` and
//! `validate-pattern` defaults that `Cargo.toml` declares, so a generated
//! item gated behind either of those two is outside this proof.

use std::path::Path;

use ridlc::Emit;

mod support;

/// The example package: the schema the build compiles and the consumer that
/// links against what it produces.
fn example_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("cabin")
}

/// Runs `rustc` with `args`, and panics with its own diagnostics when it
/// fails. `what` names the artifact, so a failure says which of the four
/// compilations broke without the reader counting calls.
fn rustc(what: &str, args: &[&std::ffi::OsStr]) {
    let output = std::process::Command::new("rustc")
        .args(args)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        output.status.success(),
        "{what} must compile, rustc said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_emitted_cabin_crate_runs_four_round_trips_against_a_consumer() {
    let out = tempfile::tempdir().expect("a temp dir is created");
    let libs = tempfile::tempdir().expect("a temp dir is created");
    let out = out.path();
    let libs = libs.path();

    // 1 — the build under test, through the same entry point the CLI calls.
    let run =
        ridlc::run_build(&example_dir(), out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(
        !run.has_error(),
        "the example must compile clean, diagnostics: {:?}",
        run.diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.message)
            .collect::<Vec<_>>()
    );
    assert!(
        out.join("lib.rs").is_file() && out.join("veh.cabin.rs").is_file(),
        "the build writes a crate root and the package module"
    );

    // 2 — the runtime crates the generated code and the consumer name,
    // built the way every compile proof in this crate builds `ridl-rt`.
    let ridl_rt = support::ridl_rt_rlib(libs);
    let ridl_loopback = support::ridl_loopback_rlib(libs, &ridl_rt);

    // 3 — the emitted crate itself, as a linkable rlib rather than the
    // metadata the other proofs stop at: a consumer that runs must link it.
    let veh_cabin = libs.join("libveh_cabin.rlib");
    let dependency = format!("dependency={}", libs.display());
    rustc(
        "the emitted crate",
        &[
            "--edition".as_ref(),
            "2024".as_ref(),
            "--crate-type".as_ref(),
            "rlib".as_ref(),
            "--crate-name".as_ref(),
            "veh_cabin".as_ref(),
            "--extern".as_ref(),
            format!("ridl_rt={}", ridl_rt.display()).as_ref(),
            "-L".as_ref(),
            dependency.as_ref(),
            out.join("lib.rs").as_ref(),
            "-o".as_ref(),
            veh_cabin.as_ref(),
        ],
    );

    // 4 — the hand-written consumer, which names the generated face and the
    // loopback and nothing of the compiler that produced the crate.
    let program = libs.join("consumer");
    rustc(
        "the consumer program",
        &[
            "--edition".as_ref(),
            "2024".as_ref(),
            "--crate-name".as_ref(),
            "consumer".as_ref(),
            "--extern".as_ref(),
            format!("veh_cabin={}", veh_cabin.display()).as_ref(),
            "--extern".as_ref(),
            format!("ridl_rt={}", ridl_rt.display()).as_ref(),
            "--extern".as_ref(),
            format!("ridl_loopback={}", ridl_loopback.display()).as_ref(),
            "-L".as_ref(),
            dependency.as_ref(),
            example_dir()
                .join("consumer")
                .join("src")
                .join("main.rs")
                .as_ref(),
            "-o".as_ref(),
            program.as_ref(),
        ],
    );

    // 5 — running it is the proof. The program asserts each round trip and
    // exits non-zero on the first that does not hold.
    let run = std::process::Command::new(&program)
        .output()
        .expect("the consumer program runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "the consumer must complete every round trip, it said:\n{stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    for round_trip in ["signal ok", "event ok", "command ok", "query ok"] {
        assert!(
            stdout.contains(round_trip),
            "the consumer must report `{round_trip}`, it said:\n{stdout}"
        );
    }
}
