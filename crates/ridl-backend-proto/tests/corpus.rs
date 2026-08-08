//! The story's acceptance check (E9.8 task 7): the cruise-control package
//! emits valid proto3, and its text is pinned so a later change has to be
//! looked at.
//!
//! The committed baseline-corpus package
//! (`crates/ridl/tests/baseline-corpus/cluster.ridl`) is run through the same
//! pipeline alongside it: that package declares only named scalars and
//! interfaces, so on its own it exercises tier 2 (the interaction identity
//! table) and almost none of tier 1 (the typl surface) — `cruise.ridl` is
//! what closes that gap, so together they are the corpus-wide validation the
//! story asks for.
//!
//! `insta` writes its snapshots beside this file, under `tests/snapshots/` —
//! this is an integration test (`tests/corpus.rs`), unlike the other two
//! backends' unit tests (`src/tests.rs`), whose snapshots land in
//! `src/snapshots/`. Both are `insta`'s own convention for where the test
//! lives, not a choice made here.

mod support;

use std::path::Path;

use support::compile_with_protox;

/// Compiles the source file at `relative_to_fixtures`, resolved against
/// `tests/fixtures/` — so `cruise.ridl` reads the fixture beside this file,
/// and `../../../ridl/tests/baseline-corpus/cluster.ridl` climbs back out of
/// it to reach the committed baseline-corpus member in the sibling `ridl`
/// crate.
///
/// Built over [`ridlc::compile`] (`crates/ridlc/src/lib.rs`), the same
/// source-to-IR path `crates/ridlc/tests/golden.rs` and
/// `crates/ridlc/tests/totality.rs` already drive: it wraps one file as a
/// single-file synthetic package and runs it through parse, resolve, and
/// check, rather than a second compile pipeline written for this crate.
fn compile_fixture(relative_to_fixtures: &str) -> ridl_ir::v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative_to_fixtures);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let output = ridlc::compile(&path.display().to_string(), &text);
    assert!(
        output.diagnostics.is_empty(),
        "{} must compile with no diagnostic, got: {:?}",
        path.display(),
        output.diagnostics,
    );
    output.package
}

#[test]
fn the_cruise_control_package_emits_valid_proto3() {
    let package = compile_fixture("cruise.ridl");
    let generated = ridl_backend_proto::generate(&package).expect("generate");
    compile_with_protox("veh.cruise.proto", &generated.proto_source);
    insta::assert_snapshot!(generated.proto_source);
}

#[test]
fn the_baseline_corpus_emits_valid_proto3() {
    let package = compile_fixture("../../../ridl/tests/baseline-corpus/cluster.ridl");
    let generated = ridl_backend_proto::generate(&package).expect("generate");
    compile_with_protox("corpus.baseline.proto", &generated.proto_source);
    insta::assert_snapshot!(generated.proto_source);
}
