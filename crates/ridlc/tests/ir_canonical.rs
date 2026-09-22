//! The canonical encoding round trip, and the nesting bound the IR stability
//! specification states (`docs/specification/ir-specification.md`, roadmap
//! story E4.5a, ADR-0014 decision 9 as amended 2026-09-22).
//!
//! Two claims are pinned here. The first is the policy's central one: every IR
//! the front end admits round-trips through canonical protobuf JSON, and the
//! bytes of a second write are the bytes of the first. It is measured over
//! every package of every corpus entry, and over the lowered system where an
//! entry declares one. The second is the bound: the deepest package the front
//! end admits nests a stated number of message levels and of JSON levels, and
//! those numbers are asserted rather than described, so that an edit to
//! `MAX_TYPE_DEPTH` or to the schema's per-level cost fails here instead of
//! leaving the specification wrong.
//!
//! **Why this file is in `ridlc` and not in `ridl-ir`.** The design note
//! (`docs/wip/2026-09-22-ir-stability-design.md` §6) asks for both tests in
//! `crates/ridl-ir`. `ridl-ir` holds the schema and the encodings and depends
//! on nothing in this workspace; both tests need the front end, and the corpus
//! test needs the corpus fixtures, which live beside this file. Reaching either
//! from `ridl-ir` would mean a dev-dependency on `ridlc`, which depends on
//! `ridl-ir` — a dependency cycle added for a test. So the two front-end tests
//! sit here, at the layer that already owns the corpus, and the encodings' own
//! bounds that need no front end stay in `crates/ridl-ir/src/lib.rs`: the
//! binary round-trip bound of the policy's "binary is derived" clause is pinned
//! there, beside the prototext bound it matches.

use std::path::{Path, PathBuf};

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;

/// The canonical encoding, written and read back.
///
/// Three assertions, which together are what the specification means by
/// canonical: the write succeeds, the read returns a package equal to the one
/// written, and re-writing the package that came back produces the same bytes.
/// The third is the byte-identity claim; without it an asymmetric writer would
/// pass on equality alone.
fn round_trip(label: &str, package: &ridl_ir::v2::Package) -> String {
    let json = ridl_ir::v2::to_json_pretty(package)
        .unwrap_or_else(|err| panic!("{label} must serialize: {err}"));
    let decoded = ridl_ir::v2::from_json(&json)
        .unwrap_or_else(|err| panic!("{label} must parse back: {err}"));
    assert_eq!(package, &decoded, "{label} must round-trip equal");
    let again = ridl_ir::v2::to_json_pretty(&decoded)
        .unwrap_or_else(|err| panic!("{label} must serialize again: {err}"));
    assert_eq!(json, again, "{label} must re-serialize byte-identically");
    json
}

/// The same three assertions over a lowered system (ADR-0022).
fn round_trip_system(label: &str, system: &ridl_ir::v2::System) {
    let json = ridl_ir::v2::system_to_json_pretty(system)
        .unwrap_or_else(|err| panic!("{label} must serialize: {err}"));
    let decoded = ridl_ir::v2::system_from_json(&json)
        .unwrap_or_else(|err| panic!("{label} must parse back: {err}"));
    assert_eq!(system, &decoded, "{label} must round-trip equal");
    let again = ridl_ir::v2::system_to_json_pretty(&decoded)
        .unwrap_or_else(|err| panic!("{label} must serialize again: {err}"));
    assert_eq!(json, again, "{label} must re-serialize byte-identically");
}

/// The two depths of a canonical artifact, measured from its bytes.
///
/// The first is the nesting of JSON objects, which in the protobuf JSON
/// mapping is the nesting of messages: every message is an object, a repeated
/// field is an array of them, and the schema declares no `map<>` field, which
/// is the one other construct that would render an object without being a
/// message. The second is the nesting of brackets of either kind, which is the
/// unit the reader's ceiling is stated in (`MAX_JSON_NESTING`, ADR-0014
/// decision 14). Brackets inside a string literal do not count; an escaped
/// quote does not end a literal, and an escaped backslash does not disarm the
/// quote after it.
///
/// The scan is iterative on purpose: parsing this artifact into a tree to
/// measure it would recurse hundreds of levels deep to learn its depth.
fn depths(json: &str) -> (usize, usize) {
    let (mut objects, mut brackets) = (0usize, 0usize);
    let (mut max_objects, mut max_brackets) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for byte in json.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => {
                objects += 1;
                brackets += 1;
                max_objects = max_objects.max(objects);
                max_brackets = max_brackets.max(brackets);
            }
            b'[' => {
                brackets += 1;
                max_brackets = max_brackets.max(brackets);
            }
            b'}' => {
                objects = objects.saturating_sub(1);
                brackets = brackets.saturating_sub(1);
            }
            b']' => brackets = brackets.saturating_sub(1),
            _ => {}
        }
    }
    (max_objects, max_brackets)
}

/// Every corpus entry, in directory order.
fn corpus_entries() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the corpus directory is readable")
        .map(|entry| entry.expect("a corpus directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "the corpus must not be empty");
    entries
}

/// The policy's central claim, over the whole corpus: every package the front
/// end produces round-trips through the canonical encoding, and writing it
/// twice gives the same bytes.
///
/// Every entry counts, including the diagnostic showcases: their IR is lowered
/// partially, and a partially lowered package is still an IR the encoding must
/// carry. `ridl.std` counts too — it is a package every consumer resolves
/// against.
#[test]
fn every_corpus_package_round_trips_through_the_canonical_encoding() {
    for entry in corpus_entries() {
        let name = entry.file_name().expect("a named entry").to_string_lossy();
        let mut db = RidlDatabase::default();
        let output = ridlc::compile_workspace(&mut db, &entry).expect("a corpus entry loads");
        for checked in &output.checked {
            round_trip(&format!("{name}: package {}", checked.ir.name), &checked.ir);
        }
        round_trip(&format!("{name}: ridl.std"), &output.std_ir);
        if let Some(system) = &output.system {
            round_trip_system(&format!("{name}: system {}", system.name), system);
        }
    }
}

/// The source depth the parser admits: `MAX_TYPE_DEPTH` is 128 and FORM-102 is
/// drawn at 128, so 127 is the deepest type nesting that compiles.
const DEEPEST_ADMITTED: usize = 127;

/// One package whose single struct field nests `depth` levels of the given
/// shape, in the three shapes the design note measured.
fn shaped_source(shape: &str, depth: usize) -> String {
    let payload = match shape {
        "arrays" => {
            let mut payload = "integer".to_string();
            for _ in 0..depth {
                payload = format!("[{payload}; 1]");
            }
            payload
        }
        "maps" => {
            let mut payload = "integer".to_string();
            for _ in 0..depth {
                payload = format!("[string : {payload}; 0..1]");
            }
            payload
        }
        "tuples" => {
            let mut payload = "integer".to_string();
            for level in (0..depth).rev() {
                payload = format!("(f{level}: {payload})");
            }
            payload
        }
        other => panic!("unknown shape {other}"),
    };
    format!("package veh.deep\n\nstruct Deep {{\n  payload : {payload}\n}}\n")
}

/// The bound the specification states, asserted rather than described.
///
/// Per shape: the deepest source the front end admits, the message levels
/// below the `Package` root that its IR nests, and the JSON levels its
/// canonical artifact nests. The specification quotes the tuple row, which is
/// the deepest of the three and therefore the bound a reader of the canonical
/// encoding must provision for.
///
/// These numbers are the schema's per-level cost multiplied by
/// `MAX_TYPE_DEPTH`. An edit to either moves them, and moving them without
/// moving the specification fails here.
const SHAPE_BOUNDS: &[(&str, usize, usize)] = &[
    ("arrays", 259, 262),
    ("maps", 261, 264),
    ("tuples", 386, 516),
];

/// The deepest package the front end admits round-trips through the canonical
/// encoding, and nests exactly the levels the specification states.
///
/// Run on an explicitly sized stack: the writer recurses on the caller's stack
/// (ADR-0014 decision 14 — only the reader sizes a thread of its own), and the
/// front end recurses once per nesting level as well, so the default
/// test-thread stack is not the right place to measure the front end's own
/// ceiling.
#[test]
fn the_deepest_package_the_front_end_admits_round_trips_and_pins_the_bound() {
    with_sized_stack(|| {
        for (shape, message_levels, json_levels) in SHAPE_BOUNDS {
            let source = shaped_source(shape, DEEPEST_ADMITTED);
            let compiled = ridlc::compile("deep.typl", &source);
            let errors: Vec<_> = compiled
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(|diagnostic| format!("{}: {}", diagnostic.code.0, diagnostic.message))
                .collect();
            assert!(
                errors.is_empty(),
                "{shape} at source depth {DEEPEST_ADMITTED} must compile clean, got: {errors:?}"
            );

            let json = round_trip(
                &format!("{shape} at depth {DEEPEST_ADMITTED}"),
                &compiled.package,
            );
            let (objects, brackets) = depths(&json);
            assert_eq!(
                objects - 1,
                *message_levels,
                "{shape}: the message levels below the package root are the bound the \
                 specification states"
            );
            assert_eq!(
                brackets, *json_levels,
                "{shape}: the JSON levels are the bound the specification states"
            );
        }
    });
}

/// One level past the parser's budget is FORM-102, which is what makes the
/// numbers above a ceiling rather than a sample.
#[test]
fn one_level_past_the_budget_is_refused_by_the_front_end() {
    with_sized_stack(|| {
        for (shape, _, _) in SHAPE_BOUNDS {
            let source = shaped_source(shape, DEEPEST_ADMITTED + 1);
            let run = ridlc::check_source("deep.typl", &source);
            assert!(
                run.diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.0 == "FORM-102"),
                "{shape} one level past the budget must draw FORM-102, got: {:?}",
                run.diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.0)
                    .collect::<Vec<_>>()
            );
        }
    });
}

/// Runs `test` on a thread whose stack fits the recursion the deep fixtures
/// drive — the same helper, and the same reason, as the deep JSON tests in
/// `crates/ridl-ir/src/lib.rs`.
fn with_sized_stack(test: impl FnOnce() + Send + 'static) {
    let outcome = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .expect("spawn the large-stack test thread")
        .join();
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}
