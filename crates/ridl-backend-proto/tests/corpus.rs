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
//!
//! Neither of the two tests above compiles a genuinely multi-package source —
//! `cruise.ridl` is self-contained and the baseline corpus has no import
//! either — so [`the_cross_package_workspace_emits_valid_proto3`] below
//! closes that gap: a small dedicated two-package fixture, run through the
//! real workspace pipeline, with each package's schema emitted by
//! `generate_with` and the referencing package compiled together with the
//! referenced one so the emitted `import` line actually resolves.
//!
//! `crates/ridlc/tests/corpus/veh-cluster` — the two-package corpus entry
//! `crates/ridlc/tests/corpus.rs` already uses for its Rust and TypeScript
//! compile proofs — was tried first and does not fit here: every
//! cross-package reference in it (`Speed`, `Temperature`, `Ratio`) is to a
//! named scalar, which inlines whether local or foreign (design §3.1, §3.2),
//! and its one foreign *enum* reference (`GearPosition`) is a command
//! parameter, a position tier 2 never resolves a type for. Compiling it
//! through `generate_with` therefore emits no `import` at all, so
//! `compile_with_protox_and_siblings`'s sibling list would go unused — it
//! would not actually exercise the path this finding is about. The dedicated
//! fixture below exists specifically to put a foreign *struct* and a foreign
//! *enum* behind a struct field, the one case that does need an import.

mod support;

use std::path::Path;

use support::{compile_with_protox, compile_with_protox_and_siblings};

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

/// The cross-package path, driven through the real workspace pipeline
/// instead of hand-built IR — the coverage gap a review of this task found:
/// Task 6's own cross-package test skipped `compile_with_protox` on the
/// recorded justification that "Task 7 covers cross-package compilation over
/// the real corpus, where both files exist," and neither fixture above
/// carries an import. `generate_with`'s resolver is the newest code in this
/// crate — reworked mid-task after a Critical defect (see the Task 6 report)
/// — and until this test its only repeatable, protox-validated coverage was
/// hand-built IR, not a real compile.
///
/// `tests/fixtures/cross-package` is a two-package workspace built for this
/// one purpose: `proto.parts` declares a `struct` and an `enum`, and
/// `proto.vehicle` imports both and references them from struct fields —
/// the one case that needs an `import`, since a named scalar or an enum set
/// always inlines instead, local or foreign (design §3.1, §3.2). Both
/// packages are emitted with `generate_with`, and the referencing package
/// (`proto.vehicle`) is compiled together with the referenced one
/// (`proto.parts`) via `compile_with_protox_and_siblings`, so the emitted
/// `import "proto.parts.proto";` line is resolved by protox against a real
/// sibling file rather than merely asserted to be present as text.
#[test]
fn the_cross_package_workspace_emits_valid_proto3() {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cross-package");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output.diagnostics.is_empty(),
        "the cross-package fixture must compile with no diagnostic, got: {:?}",
        output.diagnostics,
    );

    let parts = &output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "proto.parts")
        .expect("the fixture declares a proto.parts member")
        .ir;
    let vehicle = &output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "proto.vehicle")
        .expect("the fixture declares a proto.vehicle member")
        .ir;

    let parts_generated =
        ridl_backend_proto::generate_with(parts, &[]).expect("proto.parts generates proto3");
    let vehicle_generated = ridl_backend_proto::generate_with(vehicle, &[parts])
        .expect("proto.vehicle generates proto3");

    assert!(
        vehicle_generated
            .proto_source
            .contains("import \"proto.parts.proto\";"),
        "the fixture exists to exercise a real import; got:\n{}",
        vehicle_generated.proto_source
    );

    // proto.parts references nothing foreign, so no sibling file is needed to
    // compile it standalone.
    compile_with_protox("proto.parts.proto", &parts_generated.proto_source);
    // proto.vehicle's schema names `import "proto.parts.proto";` — this call
    // is what actually resolves it, against the schema just generated above
    // rather than a second, hand-written stand-in for it.
    compile_with_protox_and_siblings(
        "proto.vehicle.proto",
        &vehicle_generated.proto_source,
        &[("proto.parts.proto", &parts_generated.proto_source)],
    );

    insta::assert_snapshot!("cross_package_parts", parts_generated.proto_source);
    insta::assert_snapshot!("cross_package_vehicle", vehicle_generated.proto_source);
}
