//! The story's acceptance check (E9.9 task 7): the cruise-control package
//! emits a valid FlatBuffers schema, and its text is pinned so a later change
//! has to be looked at.
//!
//! `insta` writes its snapshots beside this file, under `tests/snapshots/` —
//! this is an integration test (`tests/corpus.rs`), unlike the crate's own
//! unit tests (`src/tests.rs`), whose snapshots land in `src/snapshots/`.
//! Both are `insta`'s own convention for where the test lives, not a choice
//! made here.
//!
//! Neither fixture below carries a cross-package reference on its own, so
//! [`the_cross_package_workspace_emits_valid_flatbuffers_schemas`] closes
//! that gap: the same two-package fixture `ridl-backend-proto`'s task 7
//! built (`tests/fixtures/cross-package`, reached here via a relative path
//! back into that crate), run through the real workspace pipeline, with each
//! package's schema emitted by `generate_with` and the referencing package
//! compiled together with the referenced one so the emitted `include` line
//! actually resolves. Reusing that fixture rather than writing a second one
//! keeps a foreign struct and a foreign enum behind a struct field as the
//! one corpus entry both backends exercise, instead of two sources of truth
//! that could drift apart.

mod support;

use std::path::Path;

use support::{compile_with_planus, compile_with_planus_and_siblings};

/// Compiles the source file at `relative_to_fixtures`, resolved against
/// `../ridl-backend-proto/tests/fixtures/` — the proto3 backend's fixture
/// directory, reused here rather than duplicated (task 7 brief: reuse the
/// existing fixtures, do not write new ridl source).
///
/// Built over [`ridlc::compile`] (`crates/ridlc/src/lib.rs`), the same
/// source-to-IR path `crates/ridlc/tests/golden.rs` and
/// `crates/ridlc/tests/totality.rs` already drive: it wraps one file as a
/// single-file synthetic package and runs it through parse, resolve, and
/// check, rather than a second compile pipeline written for this crate.
fn compile_fixture(relative_to_fixtures: &str) -> ridl_ir::v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures")
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
fn the_cruise_control_package_emits_a_valid_flatbuffers_schema() {
    let package = compile_fixture("cruise.ridl");
    let generated = ridl_backend_flatbuffers::generate(&package).expect("generate");
    compile_with_planus("veh.cruise.fbs", &generated.fbs_source);
    insta::assert_snapshot!(generated.fbs_source);
}

/// The cross-package path, driven through the real workspace pipeline
/// instead of hand-built IR — the same fixture and the same reasoning as
/// `ridl-backend-proto`'s `the_cross_package_workspace_emits_valid_proto3`:
/// `tests/fixtures/cross-package` (in the proto3 crate) is a two-package
/// workspace built for this one purpose. `proto.parts` declares a `struct`
/// and an `enum`, and `proto.vehicle` imports both and references them from
/// struct fields — the one case that needs a FlatBuffers `include`, since a
/// named scalar or an enum set always inlines instead, local or foreign
/// (ADR-0017 decision 1). Both packages are emitted with `generate_with`,
/// and the referencing package (`proto.vehicle`) is compiled together with
/// the referenced one (`proto.parts`) via `compile_with_planus_and_siblings`,
/// so the emitted `include "proto.parts.fbs";` line is resolved by `planus`
/// against a real sibling file rather than merely asserted to be present as
/// text.
fn compile_cross_package_fixture() -> (ridl_ir::v2::Package, ridl_ir::v2::Package) {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures/cross-package");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output.diagnostics.is_empty(),
        "the cross-package fixture must compile with no diagnostic, got: {:?}",
        output.diagnostics,
    );

    let parts = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "proto.parts")
        .expect("the fixture declares a proto.parts member")
        .ir
        .clone();
    let vehicle = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "proto.vehicle")
        .expect("the fixture declares a proto.vehicle member")
        .ir
        .clone();
    (parts, vehicle)
}

#[test]
fn the_cross_package_workspace_emits_valid_flatbuffers_schemas() {
    let (parts, vehicle) = compile_cross_package_fixture();
    let parts_out = ridl_backend_flatbuffers::generate(&parts).expect("parts");
    let vehicle_out =
        ridl_backend_flatbuffers::generate_with(&vehicle, &[&parts]).expect("vehicle");

    assert!(
        vehicle_out
            .fbs_source
            .contains("include \"proto.parts.fbs\";"),
        "the fixture exists to exercise a real include; got:\n{}",
        vehicle_out.fbs_source
    );

    // proto.parts references nothing foreign, so no sibling file is needed
    // to compile it standalone.
    compile_with_planus("proto.parts.fbs", &parts_out.fbs_source);
    // proto.vehicle's schema names `include "proto.parts.fbs";` — this call
    // is what actually resolves it, against the schema just generated above
    // rather than a second, hand-written stand-in for it.
    compile_with_planus_and_siblings(
        "proto.vehicle.fbs",
        &vehicle_out.fbs_source,
        &[("proto.parts.fbs", &parts_out.fbs_source)],
    );

    insta::assert_snapshot!("cross_package_parts", parts_out.fbs_source);
    insta::assert_snapshot!("cross_package_vehicle", vehicle_out.fbs_source);
}
