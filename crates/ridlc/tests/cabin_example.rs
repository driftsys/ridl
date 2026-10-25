//! The end-to-end proof of driftsys/ridl#444: what
//! `ridl build --emit rust` writes is a crate an application outside this
//! workspace can link against and **run**.
//!
//! What is new here is the **path**, not the running. Other proofs already run
//! generated code: `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`
//! runs a program built from the codec's output, and
//! `crates/ridl-backend-rust/tests/interaction_face.rs` already runs these
//! same six round trips over `ridl-loopback`. Each of those runs the
//! backend's own output, in this workspace's own test crate.
//!
//! This one runs what **the CLI wrote**, linked as a separate crate into a
//! separate process. It runs the build over `examples/cabin/`, compiles the
//! emitted crate with plain `rustc`, compiles
//! `examples/cabin/consumer/src/main.rs`
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
//! manifest, no lock and no registry, and it is what runs under `just test`.
//!
//! That is also why the emitted crate is built here with the `std` cfg and
//! nothing else, not with the `validate-pattern` default its `Cargo.toml`
//! declares as well: a generated item gated behind `validate-pattern` is
//! outside this proof, and inside `just demo`'s, which builds the generated
//! crate with its default features through cargo. `std` is on because the
//! consumer's command and query round trips go through the generated
//! `blocking` client, which that feature gates.
//! The `ridl-rt` this links is built with the three encoding features and
//! with `std`, because the `blocking` client is `ridl_rt::task::block_on` over
//! the async one, and the consumer polls the async client's futures by hand
//! with `ridl_rt::task::noop_waker`; both are under `ridl-rt`'s `std`.

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
    let output = rustc_output(args);
    assert!(
        output.status.success(),
        "{what} must compile, rustc said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs `rustc` with `args` and hands back what it said, success or not. A
/// proof that a binding must be refused reads the diagnostics instead of
/// asserting on the exit status alone.
fn rustc_output(args: &[&std::ffi::OsStr]) -> std::process::Output {
    std::process::Command::new("rustc")
        .args(args)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful")
}

/// What the build and the two rlib compilations below produce, held together
/// so that a test can link against them. The two `TempDir` fields are the
/// directories the paths point into: dropping them removes the artifacts, so
/// they live as long as the value does.
struct EmittedCabin {
    _out_dir: tempfile::TempDir,
    _libs_dir: tempfile::TempDir,
    libs: std::path::PathBuf,
    ridl_rt: std::path::PathBuf,
    ridl_loopback: std::path::PathBuf,
    veh_cabin: std::path::PathBuf,
}

/// Runs the build over `examples/cabin/` and compiles what it wrote, together
/// with the two runtime crates a consumer names, into linkable rlibs. Both
/// proofs below start here: one links a program that runs, the other compiles
/// a program that must be refused.
fn emit_and_compile_cabin() -> EmittedCabin {
    let out_dir = tempfile::tempdir().expect("a temp dir is created");
    let libs_dir = tempfile::tempdir().expect("a temp dir is created");
    let out = out_dir.path().to_path_buf();
    let libs = libs_dir.path().to_path_buf();

    // 1 — the build under test, through the same entry point the CLI calls.
    let run = ridlc::run_build(&example_dir(), &out, &[Emit::Rust], false.into())
        .expect("the build runs");
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
    let ridl_rt = support::ridl_rt_rlib(&libs);
    let ridl_loopback = support::ridl_loopback_rlib(&libs, &ridl_rt);

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
            "--cfg".as_ref(),
            r#"feature="std""#.as_ref(),
            "--extern".as_ref(),
            format!("ridl_rt={}", ridl_rt.display()).as_ref(),
            "-L".as_ref(),
            dependency.as_ref(),
            out.join("lib.rs").as_ref(),
            "-o".as_ref(),
            veh_cabin.as_ref(),
        ],
    );

    EmittedCabin {
        _out_dir: out_dir,
        _libs_dir: libs_dir,
        libs,
        ridl_rt,
        ridl_loopback,
        veh_cabin,
    }
}

#[test]
fn the_emitted_cabin_crate_runs_six_round_trips_against_a_consumer() {
    let built = emit_and_compile_cabin();
    let EmittedCabin {
        libs,
        ridl_rt,
        ridl_loopback,
        veh_cabin,
        ..
    } = &built;
    let dependency = format!("dependency={}", libs.display());

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
    // Whole lines, with the value each round trip carried, as `just demo`
    // matches them: a substring match on "command ok" would be satisfied by
    // the blocking round trip's "blocking command ok" line, and the async
    // round trip would then be proven by nothing.
    for round_trip in [
        "signal ok 21",
        "event ok 5",
        "command ok 42",
        "query ok 7",
        "blocking command ok 43",
        "blocking query ok 9",
    ] {
        assert!(
            stdout.lines().any(|line| line == round_trip),
            "the consumer must report the line `{round_trip}`, it said:\n{stdout}"
        );
    }
}

/// A program that binds a client of the generated `Cabin` face over the
/// loopback `runtime` builds. The two programs below differ in that
/// expression alone. The generated prelude is imported for `Bind`, which the
/// cabin consumer imports the same way; the `Podbuf` import is unused in one
/// of the two programs, which the crate-level attribute covers.
fn binding_program(runtime: &str) -> String {
    format!(
        r#"#![allow(unused_imports)]
use veh_cabin::veh::cabin as api;
use api::cabin::prelude::*;
use ridl_loopback::Loopback;
use ridl_rt::contract::{{CatalogRef, Interface}};
use ridl_rt::encoding::Podbuf;
const CATALOG: CatalogRef = *<api::Cabin as Interface>::CATALOG;
fn main() {{
    let rt = {runtime};
    let _client = api::cabin::Client::new(rt.attach());
}}
"#
    )
}

/// Compiles `source` the way the consumer program above is compiled, and
/// hands back what `rustc` said. `error_format` is the only flag that may
/// differ between two calls: a proof that reads the diagnostics asks for
/// `json`, and every other flag stays the one the consumer program is built
/// with, so the two compilations cannot drift apart.
fn compile_against_cabin(
    built: &EmittedCabin,
    name: &str,
    source: &str,
    error_format: ErrorFormat,
) -> std::process::Output {
    let path = built.libs.join(format!("{name}.rs"));
    std::fs::write(&path, source).expect("the program source is written");
    let dependency = format!("dependency={}", built.libs.display());
    let veh_cabin = format!("veh_cabin={}", built.veh_cabin.display());
    let ridl_rt = format!("ridl_rt={}", built.ridl_rt.display());
    let ridl_loopback = format!("ridl_loopback={}", built.ridl_loopback.display());
    let program = built.libs.join(name);
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "--edition".as_ref(),
        "2024".as_ref(),
        "--crate-name".as_ref(),
        name.as_ref(),
        "--extern".as_ref(),
        veh_cabin.as_ref(),
        "--extern".as_ref(),
        ridl_rt.as_ref(),
        "--extern".as_ref(),
        ridl_loopback.as_ref(),
        "-L".as_ref(),
        dependency.as_ref(),
        path.as_ref(),
        "-o".as_ref(),
        program.as_ref(),
    ];
    if let ErrorFormat::Json = error_format {
        args.push("--error-format=json".as_ref());
    }
    rustc_output(&args)
}

/// How `rustc` is asked to write its diagnostics: the default human text, or
/// one JSON object per line, which a proof can read field by field.
enum ErrorFormat {
    Human,
    Json,
}

/// One `rustc` diagnostic, read from the JSON `--error-format=json` writes:
/// its error code, its message, the source text each primary span points at,
/// and the human rendering, which only a failure message uses.
struct Diagnostic {
    code: Option<String>,
    message: String,
    primary_spans: Vec<String>,
    rendered: String,
}

/// Reads the diagnostics out of a JSON `rustc` stderr. A line that is not a
/// JSON object is not a diagnostic and is skipped.
fn diagnostics(stderr: &str) -> Vec<Diagnostic> {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|value| Diagnostic {
            code: value["code"]["code"].as_str().map(str::to_owned),
            message: value["message"].as_str().unwrap_or_default().to_owned(),
            primary_spans: value["spans"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter(|span| span["is_primary"].as_bool() == Some(true))
                .filter_map(span_source)
                .collect(),
            rendered: value["rendered"].as_str().unwrap_or_default().to_owned(),
        })
        .collect()
}

/// The source text a span points at: the lines it covers, with the
/// indentation trimmed, which for a diagnostic on an expression is the
/// statement that expression sits in.
fn span_source(span: &serde_json::Value) -> Option<String> {
    let lines: Vec<&str> = span["text"]
        .as_array()?
        .iter()
        .filter_map(|line| line["text"].as_str())
        .collect();
    Some(lines.join("\n").trim().to_owned())
}

/// The predicate `Self: Trait<Arguments>` a bound diagnostic reports, with
/// every path reduced to its last segment, so that a fully qualified
/// rendering and a short one compare equal.
#[derive(Debug, PartialEq, Eq)]
struct Predicate {
    self_type: String,
    trait_name: String,
    arguments: Vec<String>,
}

/// Every predicate a diagnostic message names. `rustc` writes a predicate
/// between backticks, so each backticked run is read and the ones that are
/// not predicates are dropped. Reading the predicate rather than matching the
/// sentence around it means a change of wording does not break the proof.
fn predicates(message: &str) -> Vec<Predicate> {
    message
        .split('`')
        .skip(1)
        .step_by(2)
        .filter_map(parse_predicate)
        .collect()
}

/// Reads `Self: Trait<A, B>` out of one backticked run, or hands back `None`
/// when the run is not a predicate.
fn parse_predicate(quoted: &str) -> Option<Predicate> {
    let (self_type, trait_ref) = quoted.split_once(": ")?;
    let (trait_name, arguments) = match trait_ref.split_once('<') {
        Some((name, rest)) => (
            name,
            split_arguments(rest.strip_suffix('>')?)
                .iter()
                .map(|argument| last_segment(argument))
                .collect(),
        ),
        None => (trait_ref, Vec::new()),
    };
    Some(Predicate {
        self_type: last_segment(self_type),
        trait_name: last_segment(trait_name),
        arguments,
    })
}

/// Splits a generic argument list at its top-level commas, so that a nested
/// argument list stays one argument.
fn split_arguments(list: &str) -> Vec<&str> {
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in list.char_indices() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                arguments.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if !list[start..].trim().is_empty() {
        arguments.push(&list[start..]);
    }
    arguments
}

/// The last segment of a path, which is the name a reader writes.
fn last_segment(path: &str) -> String {
    path.trim()
        .rsplit("::")
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The generated face reads its payload encoding from its port, and the
/// generated package carries the encoding tables of the encodings it was
/// built with — for the cabin example, FlatBuffers and proto3. So a port that
/// carries `Podbuf` has no table in that package, and binding a face over it
/// must be refused.
///
/// What this pins is **where** the refusal lands: at the binding, on the
/// `Sizes` bound the face declares, and not inside the body of a call that a
/// reader would have to step into to understand. The port does implement
/// `Encoded`, so the refusal must not be that the port is not `Encoded`; the
/// missing item is the package's `Sizes<Podbuf>` table.
///
/// The same program over `Loopback::new(CATALOG)`, whose encoding is
/// FlatBuffers, compiles, and so does the same program over a proto3
/// loopback. Without that twin a refusal could come from a
/// broken compile command rather than from the bound.
#[test]
fn a_port_whose_encoding_the_package_lacks_is_refused_at_the_binding() {
    let built = emit_and_compile_cabin();

    let refused = compile_against_cabin(
        &built,
        "encoding_the_package_lacks",
        &binding_program("Loopback::<Podbuf>::with_encoding(CATALOG)"),
        ErrorFormat::Json,
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    let emitted = diagnostics(&stderr);
    let rendered: String = emitted
        .iter()
        .map(|diagnostic| diagnostic.rendered.as_str())
        .collect();
    assert!(
        !refused.status.success(),
        "binding a face over a port whose encoding the package lacks must not compile, rustc said:\n{rendered}"
    );

    // A diagnostic carries the whole refusal: the code, the unsatisfied
    // predicate and the span. Reading the three off the same diagnostic is
    // what ties them together; three independent searches of the output
    // would be satisfied by three unrelated diagnostics. With two tables in
    // the package, rustc reports the predicate once for the constructor path
    // and once for its argument, both inside the binding statement, so every
    // diagnostic naming the predicate must point at that statement, and
    // there are one or two of them.
    let expected = Predicate {
        self_type: "Cabin".to_owned(),
        trait_name: "Sizes".to_owned(),
        arguments: vec!["Podbuf".to_owned()],
    };
    // The statement the refusal must point at. `rustc` highlights the
    // argument inside it, and the span carries the whole statement, so the
    // statement is what a proof compares.
    let binding = "let _client = api::cabin::Client::new(rt.attach());";
    let refusals: Vec<&Diagnostic> = emitted
        .iter()
        .filter(|diagnostic| {
            diagnostic.code.as_deref() == Some("E0277")
                && predicates(&diagnostic.message).contains(&expected)
        })
        .collect();
    assert!(
        (1..=2).contains(&refusals.len()),
        "one or two diagnostics must be an E0277 naming the predicate {expected:?}, got {}, rustc said:\n{rendered}",
        refusals.len()
    );
    for refusal in &refusals {
        assert!(
            refusal.primary_spans.iter().any(|span| span == binding),
            "every E0277 naming the predicate {expected:?} must have a primary span on `{binding}`, rustc said:\n{rendered}"
        );
    }

    // The port is `Encoded`; what the package lacks is its `Sizes` table. So
    // no unsatisfied-bound diagnostic may report a predicate whose trait is
    // `Encoded`. The name does appear elsewhere in the output — in the
    // projection path `<Loopback<Podbuf> as Encoded>::Encoding` of the
    // companion type-mismatch diagnostic, and in the
    // `ridl_rt::payload::Encoded` struct the generated FlatBuffers codec
    // names, which is a different item — so the predicate is read rather
    // than the prose around it.
    for diagnostic in &emitted {
        if diagnostic.code.as_deref() != Some("E0277") {
            continue;
        }
        let unmet: Vec<Predicate> = predicates(&diagnostic.message)
            .into_iter()
            .filter(|predicate| predicate.trait_name == "Encoded")
            .collect();
        assert!(
            unmet.is_empty(),
            "the port is `Encoded`; the missing item is the package's `Sizes` table, so no unsatisfied bound may name `Encoded`, this one named {unmet:?}, rustc said:\n{rendered}"
        );
    }

    let accepted = compile_against_cabin(
        &built,
        "the_encoding_the_package_carries",
        &binding_program("Loopback::new(CATALOG)"),
        ErrorFormat::Human,
    );
    assert!(
        accepted.status.success(),
        "the same program over the package's own encoding must compile, rustc said:\n{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let accepted = compile_against_cabin(
        &built,
        "the_second_encoding_the_package_carries",
        &binding_program("Loopback::<ridl_rt::encoding::Proto3>::with_encoding(CATALOG)"),
        ErrorFormat::Human,
    );
    assert!(
        accepted.status.success(),
        "the same program over proto3, the package's second encoding, must compile, rustc said:\n{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}

/// The cabin workspace declares one system with one deployment, `Bench`, whose
/// three consumer links cover both crossing kinds: two on one machine and one
/// between machines. Later tests read this deployment by name.
#[test]
fn cabin_lowers_one_deployment_with_three_links() {
    use ridl_ir::v2::Crossing;

    let output = ridlc::compile_workspace(&mut ridl_core::RidlDatabase::default(), &example_dir())
        .expect("the cabin workspace loads");
    let system = output.system.expect("the cabin workspace lowers a system");

    assert_eq!(system.deployments.len(), 1);
    let bench = &system.deployments[0];
    assert_eq!(bench.name, "Bench");
    let crossings: Vec<i32> = bench.links.iter().map(|link| link.crossing).collect();
    assert_eq!(
        crossings,
        vec![
            Crossing::SameMachine as i32,
            Crossing::SameMachine as i32,
            Crossing::DifferentMachine as i32,
        ]
    );
    assert_eq!(system.regions.len(), 1);
    assert_eq!(system.regions[0].catalog, "veh.cabin");
    assert_eq!(system.regions[0].interfaces.len(), 2);
}
