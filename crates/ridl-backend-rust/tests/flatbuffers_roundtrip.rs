//! The FlatBuffers codec round trip — E11.7's own `Done when` (stage K5,
//! plan Task 4 of `docs/wip/2026-09-20-flatbuffers-codec-plan.md`).
//!
//! `generate` emits a `Payload<FlatBuffers>` implementation per root table
//! (design note D-1 as amended). This target compiles that output as a
//! program and runs it, because a compile proof shows only that the codec
//! type-checks: whether the bytes `encode` writes are the bytes `verify`
//! accepts and `decode` reads back is something only a run can say.
//!
//! The fixture carries one declaration per shape the codec has to reach, so
//! one round trip over `Report` exercises the scalar widths, the string, the
//! bytes, the enum, the enum set, the nested struct, the union's table arm
//! and its boxed arm, the induced tuple, the fixed and bounded arrays, the
//! map, and an optional field both present and absent.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

/// The generated source for the fixture, with a harness `main` appended.
fn program(main: &str) -> String {
    let package = ir::compile_fixture("flatbuffers_roundtrip.ridl");
    let generated = ridl_backend_rust::generate(&package)
        .expect("the fixture generates")
        .rust_source;
    format!(
        // A program's unused `pub` items are dead code, and a view's
        // accessors are read by a consumer rather than by this harness.
        "#![allow(dead_code)]\n{generated}\n{main}"
    )
}

/// The value every case below round-trips: every field populated, `note`
/// present and `spare` absent.
const VALUE: &str = r#"
fn sample() -> Report {
    Report {
        id: Count::new_unchecked(7),
        name: Label::new_unchecked(String::from("cabin")),
        blob: Blob::new_unchecked(vec![1, 2, 3]),
        ratio: Ratio::new_unchecked(0.25),
        engaged: Engaged::new(true),
        health: Health::WARN,
        flags: WarningFlags::HIGH,
        inner: Inner {
            speed: Speed::new_unchecked(120),
            label: Label::new_unchecked(String::from("inner")),
        },
        outcome: Outcome::Bad(Health::FAIL),
        range: ReportRange {
            min: Speed::new_unchecked(0),
            max: Speed::new_unchecked(300),
        },
        readings: [
            Speed::new_unchecked(1),
            Speed::new_unchecked(2),
            Speed::new_unchecked(3),
        ],
        faults: vec![Health::OK, Health::FAIL],
        meta: vec![
            (Label::new_unchecked(String::from("a")), Count::new_unchecked(1)),
            (Label::new_unchecked(String::from("bb")), Count::new_unchecked(2)),
        ],
        note: Some(Label::new_unchecked(String::from("note"))),
        spare: None,
    }
}
"#;

/// A value encodes, verifies and decodes back to itself, and the accessors
/// read the same fields in place without decoding (design note D-3).
#[test]
fn a_report_round_trips_through_the_generated_codec() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};

fn main() {
    let value = sample();
    let capacity = <Report as Payload<FlatBuffers>>::MAX_SIZE;
    let mut out = vec![0u8; capacity];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();

    // The buffer is a subslice of `out` ending at its end, not a prefix of
    // it (design note D-2).
    assert!(bytes.len() <= capacity, "the encoding is inside MAX_SIZE");

    let proof: Ref<'_, Report, FlatBuffers> = Ref::verify(&bytes).expect("verify");

    // Read in place, before any decode.
    let view = proof.view();
    assert_eq!(view.id(), value.id, "a scalar reads in place");
    assert_eq!(view.name(), value.name.get(), "a string reads in place");
    assert_eq!(view.blob(), value.blob.get(), "bytes read in place");
    assert_eq!(
        view.inner().speed(),
        value.inner.speed,
        "a nested table reads in place"
    );
    assert_eq!(
        view.note(),
        Some(value.note.as_ref().unwrap().get()),
        "a present optional reads in place"
    );
    assert_eq!(view.spare(), None, "an absent optional reads as absent");
    assert_eq!(view.outcome().value(), value.outcome, "a union reads back");

    let back = proof.decode();
    assert_eq!(back, value, "the value survives the round trip");
}
"#
    );
    rustc::run_program("fb_round_trip", &program(&main));
}

/// The encoder is deterministic: the same value encodes to the same bytes on
/// every run (design note D-8). A map's entries keep the order of the
/// generated `Vec<(K, V)>`, so nothing here depends on a hash order.
#[test]
fn the_encoding_is_deterministic() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;

fn main() {
    let value = sample();
    let capacity = <Report as Payload<FlatBuffers>>::MAX_SIZE;
    let mut first = vec![0u8; capacity];
    let a = value.encode(&mut first).expect("encode").bytes.to_vec();
    // A second buffer, dirty rather than zeroed, so that a byte the encoder
    // leaves untouched cannot make two encodes agree by accident.
    let mut second = vec![0xABu8; capacity];
    let b = value.encode(&mut second).expect("encode").bytes.to_vec();
    assert_eq!(a, b, "one value encodes to one byte sequence");
}
"#
    );
    rustc::run_program("fb_deterministic", &program(&main));
}

/// A buffer too small for the value is a `Capacity` error rather than a
/// panic or a truncated encoding, and a buffer of `MAX_SIZE` always suffices.
#[test]
fn a_short_buffer_reports_capacity() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{EncodeError, Payload};

fn main() {
    let value = sample();
    let mut tiny = [0u8; 8];
    match value.encode(&mut tiny) {
        Err(EncodeError::Capacity { available, .. }) => assert_eq!(available, 8),
        Err(other) => panic!("expected a capacity error, got {other:?}"),
        Ok(_) => panic!("a value larger than the buffer must not encode"),
    }
    let mut exact = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    value.encode(&mut exact).expect("MAX_SIZE always suffices");
}
"#
    );
    rustc::run_program("fb_capacity", &program(&main));
}

/// `verify` refuses bytes that are not this type's encoding, and refuses a
/// buffer larger than `MAX_SIZE` before it reads anything (design note D-5).
#[test]
fn verify_refuses_bytes_it_did_not_write() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Malformed, Payload, Ref, VerifyError};

fn main() {
    let value = sample();
    let capacity = <Report as Payload<FlatBuffers>>::MAX_SIZE;
    let mut out = vec![0u8; capacity];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();

    // A truncated buffer: the root offset points past its end.
    let short = &bytes[..bytes.len() / 2];
    assert!(
        matches!(
            Ref::<'_, Report, FlatBuffers>::verify(short),
            Err(VerifyError::Structure(_))
        ),
        "a truncated buffer is malformed"
    );

    // Nothing at all.
    assert!(
        matches!(
            Ref::<'_, Report, FlatBuffers>::verify(&[]),
            Err(VerifyError::Structure(Malformed::OutOfBounds))
        ),
        "an empty buffer is out of bounds"
    );

    // Larger than the bound: refused before the walk (design note D-5).
    let oversized = vec![0u8; capacity + 1];
    assert!(
        matches!(
            Ref::<'_, Report, FlatBuffers>::verify(&oversized),
            Err(VerifyError::Structure(Malformed::TooLarge))
        ),
        "a buffer larger than MAX_SIZE is refused on its length"
    );
}
"#
    );
    rustc::run_program("fb_verify_refuses", &program(&main));
}

/// A union arm this type does not declare is `Malformed::Union`, and an enum
/// discriminant no variant carries is a contract violation — the two checks
/// that keep `decode` from having to answer for a value it cannot build.
#[test]
fn verify_refuses_an_undeclared_discriminant() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref, Rule, VerifyError};

fn main() {
    // A union whose table arm is present verifies, so that the mutation
    // below is the only difference between the two runs.
    let mut value = sample();
    value.outcome = Outcome::Ok(Inner {
        speed: Speed::new_unchecked(9),
        label: Label::new_unchecked(String::from("x")),
    });
    let mut out = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();
    Ref::<'_, Report, FlatBuffers>::verify(&bytes).expect("the arm is declared");

    // An enum value outside the declared set, written through the wire
    // rather than through the generated `TryFrom`, is a contract violation.
    let mut broken = bytes.clone();
    let mut hits = 0usize;
    for index in 0..broken.len() {
        let original = broken[index];
        broken[index] = 0x7F;
        if let Err(VerifyError::Contract(violation)) =
            Ref::<'_, Report, FlatBuffers>::verify(&broken)
        {
            assert!(
                matches!(violation.rule, Rule::Variant | Rule::Length),
                "a wire-level corruption shows up as a variant or a length \
                 violation, got {:?}",
                violation.rule
            );
            hits += 1;
        }
        broken[index] = original;
    }
    assert!(
        hits > 0,
        "corrupting the buffer must reach the contract half of verify at least once"
    );
}
"#
    );
    rustc::run_program("fb_undeclared", &program(&main));
}
