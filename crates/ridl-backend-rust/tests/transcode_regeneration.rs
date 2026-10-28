//! The regeneration guard of the transcode fixture.
//!
//! `tests/generated/transcode.rs` is `generate_pipeline`'s output for
//! `tests/fixtures/transcode.ridl` with all three encodings and `transcode = true`,
//! checked in and `include!`d by `tests/transcode_equivalence.rs`. The guard
//! lives in its own test target so that when a change to the emitter leaves
//! the equivalence tests unable to compile against a stale fixture,
//! `RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test
//! transcode_regeneration` still builds and rewrites it.

#[path = "support/ir.rs"]
mod ir;

const ENCODINGS: &[ridl_backend_rust::WireEncoding] = &[
    ridl_backend_rust::WireEncoding::FlatBuffers,
    ridl_backend_rust::WireEncoding::Proto3,
    ridl_backend_rust::WireEncoding::Podbuf,
];

/// Regenerates the fixture and compares it byte-for-byte against the
/// checked-in file. `RIDL_UPDATE_GENERATED=1` writes the fresh output instead.
#[test]
fn generated_transcode_matches_the_emitter() {
    let package = ir::compile_fixture("transcode.ridl");
    let source = ridl_backend_rust::generate_pipeline(&package, ENCODINGS, true, &[])
        .expect("generate_pipeline")
        .rust_source;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generated/transcode.rs");

    if std::env::var_os("RIDL_UPDATE_GENERATED").is_some() {
        std::fs::write(&path, &source)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        return;
    }

    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        source, checked_in,
        "generated transcode.rs is stale; regenerate it with \
         RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test transcode_regeneration"
    );
}

/// A repeated encoding takes its owner list once, so the output is the one
/// the list without the repeat gives, and no transcode function is emitted
/// twice.
#[test]
fn a_repeated_encoding_emits_each_transcode_function_once() {
    let package = ir::compile_fixture("transcode.ridl");
    let repeated = [
        ridl_backend_rust::WireEncoding::FlatBuffers,
        ridl_backend_rust::WireEncoding::Proto3,
        ridl_backend_rust::WireEncoding::Podbuf,
        ridl_backend_rust::WireEncoding::FlatBuffers,
    ];
    let once = ridl_backend_rust::generate_pipeline(&package, ENCODINGS, true, &[])
        .expect("generate_pipeline")
        .rust_source;
    let twice = ridl_backend_rust::generate_pipeline(&package, &repeated, true, &[])
        .expect("generate_pipeline")
        .rust_source;
    assert_eq!(twice, once);
}

/// The same as above with proto3 listed first: the repeat of the first
/// listed encoding does not change the order either.
#[test]
fn a_repeated_encoding_in_proto3_first_order_emits_each_transcode_function_once() {
    let package = ir::compile_fixture("transcode.ridl");
    let once = [
        ridl_backend_rust::WireEncoding::Proto3,
        ridl_backend_rust::WireEncoding::FlatBuffers,
    ];
    let repeated = [
        ridl_backend_rust::WireEncoding::Proto3,
        ridl_backend_rust::WireEncoding::FlatBuffers,
        ridl_backend_rust::WireEncoding::Proto3,
    ];
    let once = ridl_backend_rust::generate_pipeline(&package, &once, true, &[])
        .expect("generate_pipeline")
        .rust_source;
    let twice = ridl_backend_rust::generate_pipeline(&package, &repeated, true, &[])
        .expect("generate_pipeline")
        .rust_source;
    assert_eq!(twice, once);
}

/// The transcode functions follow the order the encodings are listed in,
/// not the order the codecs are emitted in: listing proto3 first puts each
/// type's proto3-to-FlatBuffers function before its FlatBuffers-to-proto3
/// function, in every `impl` block that has both.
#[test]
fn the_transcode_functions_follow_the_listed_encoding_order() {
    let package = ir::compile_fixture("transcode.ridl");
    let reversed = [
        ridl_backend_rust::WireEncoding::Proto3,
        ridl_backend_rust::WireEncoding::FlatBuffers,
    ];
    let source = ridl_backend_rust::generate_pipeline(&package, &reversed, true, &[])
        .expect("generate_pipeline")
        .rust_source;
    let mut checked = 0;
    for block in source.split("\nimpl ").skip(1) {
        let (Some(proto3), Some(flatbuffers)) = (
            block.find("fn transcode_proto3_to_flatbuffers"),
            block.find("fn transcode_flatbuffers_to_proto3"),
        ) else {
            continue;
        };
        assert!(
            proto3 < flatbuffers,
            "proto3-to-FlatBuffers comes first in the block starting {:?}",
            block.chars().take(40).collect::<String>()
        );
        checked += 1;
    }
    // Count, Level, Label, Mode, Reading, Inner, Panel, Outcome, Key and
    // GaugeSetParams: every payload type of the fixture has both encodings.
    assert_eq!(checked, 10, "the blocks that have both functions");
}
