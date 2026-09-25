//! A `[bool]` field in the FlatBuffers codec (driftsys/ridl#518).
//!
//! `flatbuffers_roundtrip.ridl`'s `Report.engaged` field is an inline scalar
//! of the named type `Engaged`, encoded through `Field::Bool(value)`, which
//! takes a `bool` and needs no widening. A vector element is encoded on a
//! different path: every scalar element's raw value is widened to bytes with
//! `.to_le_bytes()`, a method `bool` does not have, so the generated code did
//! not compile for a vector of `boolean` — the bare primitive and a named
//! scalar over it alike, since both are `Scalar::prim == Prim::Bool`, which
//! is what the fix tests rather than `Scalar::repr`. This target proves the
//! fix for both shapes: the generated codec for
//! `flatbuffers_bool_vector.ridl`'s `Flags.values` (a vector of the bare
//! `boolean` primitive) and `Flags.engaged` (a vector of the named scalar
//! `Engaged`) compiles as a program and round-trips a populated and an empty
//! vector of each.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

/// The generated source for the fixture, with a harness `main` appended.
fn program(main: &str) -> String {
    let package = ir::compile_fixture("flatbuffers_bool_vector.ridl");
    let generated = ridl_backend_rust::generate(&package)
        .expect("the fixture generates")
        .rust_source;
    format!("#![allow(dead_code)]\n{generated}\n{main}")
}

/// A vector of the bare `boolean` primitive and a vector of `Engaged`, a
/// named scalar over the same backing, both encode, verify and decode back
/// to themselves, in place and after a full decode, for both a populated and
/// an empty vector.
#[test]
fn a_bool_vector_round_trips_through_the_generated_codec() {
    let main = r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};

fn round_trip(values: Vec<bool>, engaged: Vec<Engaged>) {
    let value = Flags { values: values.clone(), engaged: engaged.clone() };
    let mut out = vec![0u8; <Flags as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();

    let proof: Ref<'_, Flags, FlatBuffers> = Ref::verify(&bytes).expect("verify");
    assert_eq!(proof.view().values(), values, "a bool vector reads back");
    assert_eq!(
        proof.view().engaged(),
        engaged,
        "a named-scalar bool vector reads back"
    );
    assert_eq!(proof.decode(), value, "the value survives the round trip");
}

fn main() {
    round_trip(
        vec![true, false, true, false],
        vec![
            Engaged::new(true),
            Engaged::new(false),
            Engaged::new(true),
            Engaged::new(false),
        ],
    );
    round_trip(Vec::new(), Vec::new());
}
"#;
    rustc::run_program("fb_bool_vector", &program(main));
}
