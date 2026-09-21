//! The FlatBuffers codec round trip — E11.7's own `Done when` (stage K5,
//! plan Task 4 of `docs/wip/2026-09-20-flatbuffers-codec-plan.md`).
//!
//! `generate` emits a `Payload<FlatBuffers>` implementation per root table
//! (design note D-1 as amended). This target compiles that output as a
//! program and runs it, because a compile proof shows only that the codec
//! type-checks: whether the bytes `encode` writes are the bytes `verify`
//! accepts and `decode` reads back is something only a run can say.
//!
//! What one round trip over `Report` runs: every scalar width, a string,
//! bytes, an enum, an enum set, a nested struct, a union's boxed arm, an
//! induced tuple, a fixed array, a bounded array of scalars, a map, a vector
//! of strings, a vector of tables, a vector of unions, a vector of tuples, a
//! reserved tombstone, an optional composite, an optional string and an
//! optional scalar — the last two once present and once absent.
//! [`a_union_round_trips_as_its_own_payload`] runs the union as a root
//! payload and decodes its table arm, which the `Report` round trip does not
//! reach on its own.

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

/// The two values every case below draws on.
///
/// `sample()` populates every field, with `pair` and `note` present and
/// `spare` absent. `defaults()` is the D-9 case: every field whose value
/// equals its FlatBuffers default, including the two optionals, which a
/// FlatBuffers writer omits by default and this encoder must not.
const VALUE: &str = r#"
fn inner(speed: i64, label: &str) -> Inner {
    Inner {
        speed: Speed::new_unchecked(speed),
        label: Label::new_unchecked(String::from(label)),
    }
}

fn sample() -> Report {
    Report {
        id: Count::new_unchecked(7),
        name: Label::new_unchecked(String::from("cabin")),
        blob: Blob::new_unchecked(vec![1, 2, 3]),
        ratio: Ratio::new_unchecked(0.25),
        engaged: Engaged::new(true),
        health: Health::WARN,
        flags: WarningFlags::HIGH,
        inner: inner(120, "inner"),
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
        names: vec![
            Label::new_unchecked(String::from("first")),
            Label::new_unchecked(String::from("second")),
        ],
        inners: vec![inner(1, "one"), inner(2, "two")],
        outcomes: vec![Outcome::Ok(inner(3, "three")), Outcome::Bad(Health::WARN)],
        points: vec![
            ReportPointsElement {
                x: Speed::new_unchecked(10),
                y: Speed::new_unchecked(20),
            },
            ReportPointsElement {
                x: Speed::new_unchecked(30),
                y: Speed::new_unchecked(40),
            },
        ],
        pair: Some(ReportPair {
            lo: Speed::new_unchecked(5),
            inner: inner(6, "six"),
        }),
        note: Some(Label::new_unchecked(String::from("note"))),
        spare: None,
    }
}

/// Every field at the value a FlatBuffers writer would omit: zero for every
/// scalar, `false`, the empty string, the empty collection — and the two
/// optionals **present** at those values rather than absent.
fn defaults() -> Report {
    Report {
        id: Count::new_unchecked(0),
        name: Label::new_unchecked(String::new()),
        blob: Blob::new_unchecked(Vec::new()),
        ratio: Ratio::new_unchecked(0.0),
        engaged: Engaged::new(false),
        health: Health::OK,
        flags: WarningFlags::LOW,
        inner: inner(0, ""),
        outcome: Outcome::Bad(Health::OK),
        range: ReportRange {
            min: Speed::new_unchecked(0),
            max: Speed::new_unchecked(0),
        },
        readings: [
            Speed::new_unchecked(0),
            Speed::new_unchecked(0),
            Speed::new_unchecked(0),
        ],
        faults: Vec::new(),
        meta: Vec::new(),
        names: Vec::new(),
        inners: Vec::new(),
        outcomes: Vec::new(),
        points: Vec::new(),
        pair: Some(ReportPair {
            lo: Speed::new_unchecked(0),
            inner: inner(0, ""),
        }),
        note: Some(Label::new_unchecked(String::new())),
        spare: Some(Speed::new_unchecked(0)),
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
    assert_eq!(view.names(), value.names, "a vector of strings reads back");
    assert_eq!(view.inners(), value.inners, "a vector of tables reads back");
    assert_eq!(view.outcomes(), value.outcomes, "a vector of unions reads back");
    assert_eq!(view.points(), value.points, "a vector of tuples reads back");
    // An optional composite reads back as a nested view, not as the domain
    // value: a table is what a view hands over in place.
    let pair = view.pair().expect("the optional composite is present");
    assert_eq!(pair.lo(), value.pair.as_ref().unwrap().lo);
    assert_eq!(pair.inner().speed(), value.pair.as_ref().unwrap().inner.speed);

    let back = proof.decode();
    assert_eq!(back, value, "the value survives the round trip");
}
"#
    );
    rustc::run_program("fb_round_trip", &program(&main));
}

/// **D-9's headline hazard.** A present field whose value equals its
/// FlatBuffers default is written explicitly, so a reader can tell it from an
/// absent one. Omitting it is what a FlatBuffers writer does by default, and
/// it is what the design note warns of: "a round trip that looks correct on a
/// non-default value silently loses information on a default one."
///
/// The two optionals carry the whole force of this: `note` is `Some("")` and
/// `spare` is `Some(0)`, and both are indistinguishable from `None` unless
/// the encoder writes them. The non-optional fields carry the rest — an
/// omitted one fails `verify` with `MissingRequired` rather than decoding
/// wrongly, so this case fails either way if the encoder starts skipping.
#[test]
fn a_present_default_valued_field_survives_the_round_trip() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};

fn main() {
    let value = defaults();
    let mut out = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();

    let proof: Ref<'_, Report, FlatBuffers> =
        Ref::verify(&bytes).expect("a value of all defaults still verifies");

    // Read in place first: an omitted optional reads as `None` here, which
    // is the loss this case exists to catch.
    let view = proof.view();
    assert_eq!(
        view.spare(),
        Some(Speed::new_unchecked(0)),
        "a present optional scalar equal to the FlatBuffers default must be written",
    );
    assert_eq!(
        view.note(),
        Some(""),
        "a present optional string equal to the FlatBuffers default must be written",
    );
    assert!(
        view.pair().is_some(),
        "a present optional composite must be written",
    );

    let back = proof.decode();
    assert_eq!(back.spare, Some(Speed::new_unchecked(0)), "Some(0) is not None");
    assert_eq!(
        back.note,
        Some(Label::new_unchecked(String::new())),
        "a present empty string is not an absent one",
    );
    assert_eq!(back, value, "a value of all defaults survives the round trip");

    // And the distinction is real in both directions: the same value with the
    // two optionals absent must encode to different bytes and decode to
    // `None`.
    let mut absent = defaults();
    absent.spare = None;
    absent.note = None;
    absent.pair = None;
    let mut other = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    let other_bytes = absent.encode(&mut other).expect("encode").bytes.to_vec();
    assert_ne!(
        bytes, other_bytes,
        "a present default-valued optional and an absent one are different buffers",
    );
    let back_absent = Ref::<'_, Report, FlatBuffers>::verify(&other_bytes)
        .expect("verify")
        .decode();
    assert_eq!(back_absent, absent, "the absent form round-trips as absent");
}
"#
    );
    rustc::run_program("fb_defaults", &program(&main));
}

/// A union is a root table of its own, so it is a payload of its own. This is
/// the only case that decodes the union's **table** arm: the `Report` round
/// trip carries `Outcome::Bad`, the boxed arm.
#[test]
fn a_union_round_trips_as_its_own_payload() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};

fn main() {
    for value in [Outcome::Ok(inner(42, "table arm")), Outcome::Bad(Health::FAIL)] {
        let mut out = vec![0u8; <Outcome as Payload<FlatBuffers>>::MAX_SIZE];
        let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();
        let proof: Ref<'_, Outcome, FlatBuffers> = Ref::verify(&bytes).expect("verify");
        assert_eq!(proof.view().value(), value, "the view decodes the arm");
        assert_eq!(proof.decode(), value, "the union survives the round trip");
    }

    // A struct is a payload of its own too, and the table arm above is one.
    let nested = inner(7, "seven");
    let mut out = vec![0u8; <Inner as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = nested.encode(&mut out).expect("encode").bytes.to_vec();
    let proof: Ref<'_, Inner, FlatBuffers> = Ref::verify(&bytes).expect("verify");
    assert_eq!(proof.decode(), nested);
}
"#
    );
    rustc::run_program("fb_union_payload", &program(&main));
}

/// `MAX_SIZE` is the largest buffer any legal value encodes to, and this
/// pins it as a number rather than as an inequality.
///
/// An inequality cannot do the job: the bound charges seven bytes of
/// alignment slack per vtable slot, so a bound short by one still holds every
/// value the fixture can build, and `bound.saturating_sub(1)` passes every
/// other case in this file. The constant is wire-visible — a consumer sizes a
/// stack buffer from it, and E11.7's face sizes real buffers from the maximum
/// over these — so pinning the number is what the constant deserves. A
/// deliberate change to the projection's charges changes these two literals
/// in the same commit.
///
/// The second half is what keeps the pinned numbers honest: the largest legal
/// `Inner` — a sixteen-character label at four UTF-8 bytes a character, the
/// widest typl §5.3 admits — is encoded and must fit.
#[test]
fn max_size_is_pinned_and_holds_the_largest_legal_value() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;

fn main() {
    assert_eq!(
        <Inner as Payload<FlatBuffers>>::MAX_SIZE,
        INNER_MAX_SIZE,
        "the bound of `Inner` changed; change this literal deliberately",
    );
    assert_eq!(
        <Report as Payload<FlatBuffers>>::MAX_SIZE,
        REPORT_MAX_SIZE,
        "the bound of `Report` changed; change this literal deliberately",
    );

    // Sixteen characters, each four UTF-8 bytes: the largest `Label` typl
    // §5.3 admits, which is what the bound charges four bytes a character
    // for.
    let widest = "\u{10348}".repeat(16);
    assert_eq!(widest.len(), 64, "the fixture value really is four bytes a character");
    let value = Inner {
        speed: Speed::new_unchecked(300),
        label: Label::new_unchecked(widest),
    };
    let mut out = vec![0u8; INNER_MAX_SIZE];
    let written = value.encode(&mut out).expect("the bound holds the largest value").bytes.len();
    assert!(
        written <= INNER_MAX_SIZE,
        "the largest legal value encodes to {written} bytes, over the bound",
    );
}
"#
    );
    rustc::run_program("fb_max_size", &program(&format!("{MAX_SIZE_PINS}{main}")));
}

/// The two pinned bounds, read by
/// [`max_size_is_pinned_and_holds_the_largest_legal_value`].
const MAX_SIZE_PINS: &str = r#"
const INNER_MAX_SIZE: usize = 133;
const REPORT_MAX_SIZE: usize = 2388;
"#;

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

/// A union discriminant naming no arm is `Malformed::Union`, checked by
/// corrupting **only** that byte.
///
/// [`verify_refuses_an_undeclared_discriminant`] flips every byte in turn and
/// asserts that the contract half is reached at least once, which other
/// fields satisfy on their own: accepting any discriminant in the union's
/// wildcard arm leaves that case passing. This one writes one byte, at the
/// position the union's own wrapper table puts its discriminant, and asserts
/// the specific refusal.
#[test]
fn verify_refuses_a_union_discriminant_that_names_no_arm() {
    let main = format!(
        "{VALUE}{}",
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::flatbuffers::{field, root};
use ridl_rt::payload::{Malformed, Payload, Ref, VerifyError};

fn main() {
    // A union as the root payload, so the root table *is* the wrapper table
    // and its discriminant is reachable through the public helpers alone.
    let value = Outcome::Ok(inner(11, "arm"));
    let mut out = vec![0u8; <Outcome as Payload<FlatBuffers>>::MAX_SIZE];
    let mut bytes = value.encode(&mut out).expect("encode").bytes.to_vec();

    Ref::<'_, Outcome, FlatBuffers>::verify(&bytes)
        .expect("the untouched buffer verifies, so the mutation below is the only difference");

    // The wrapper's discriminant sits at the implicit id 0, one byte wide
    // (ADR-0019 decision 1).
    let table = root(&bytes).expect("a buffer this encoder wrote has a root");
    let at = field(&bytes, table, 0u16, 1usize)
        .expect("the wrapper's vtable is well formed")
        .expect("the discriminant is always written");

    let declared = bytes[at];
    assert_ne!(declared, 0u8, "an arm's ordinal is 1-based, so zero names none");

    for wrong in [0u8, declared.wrapping_add(100), u8::MAX] {
        bytes[at] = wrong;
        match Ref::<'_, Outcome, FlatBuffers>::verify(&bytes).err() {
            Some(VerifyError::Structure(Malformed::Union)) => {}
            other => panic!("discriminant {wrong} names no arm, got {other:?}"),
        }
    }

    // Restored, the same buffer verifies again: nothing else was disturbed.
    bytes[at] = declared;
    Ref::<'_, Outcome, FlatBuffers>::verify(&bytes).expect("the restored buffer verifies");
}
"#
    );
    rustc::run_program("fb_union_discriminant", &program(&main));
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
