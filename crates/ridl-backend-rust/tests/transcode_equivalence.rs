//! The equivalence check of the generated transcode functions.
//!
//! It brings in the checked-in generated module of `tests/generated/transcode.rs`
//! (regenerated and compared by `tests/transcode_regeneration.rs`) and, for each
//! value, compares the bytes of a transcode with the bytes of a direct
//! encoding: `T::transcode_proto3_to_flatbuffers` over the proto3 encoding of
//! the value must equal the FlatBuffers encoding of the value, and the reverse
//! direction likewise. A streaming transcoder that replaces the bodies of those
//! functions must pass the same comparisons.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};
use ridl_rt::payload::{EncodeError, Ref, Rule, TranscodeError, VerifyError};
use ridl_rt::podbuf::Aligned;

/// The checked-in output of `generate_pipeline` over `tests/fixtures/transcode.ridl`
/// with both encodings and `transcode = true`. It is `include!`d, never
/// hand-edited. Lint allows live on this wrapper module because the generated
/// file carries no inner attribute.
#[allow(
    dead_code,
    reason = "not every generated item is named from this file's own tests"
)]
#[allow(
    unused_imports,
    reason = "the generated face re-exports names this file's tests do not use"
)]
#[allow(
    clippy::derivable_impls,
    reason = "the domain-type Default emission writes a manual impl"
)]
#[allow(
    clippy::collapsible_if,
    reason = "the emitted code targets edition 2021, where an `if` and an `if let` cannot be joined"
)]
mod generated {
    include!("generated/transcode.rs");
}

use generated::{Count, GaugeSetParams, Inner, Key, Label, Level, Mode, Outcome, Panel, Reading};

const BUFFER: usize = 4096;

/// Checks both directions for one value of `$ty`.
macro_rules! assert_equivalent {
    ($ty:ty, $value:expr) => {{
        let value: $ty = $value;
        let mut a = [0u8; BUFFER];
        let mut b = [0u8; BUFFER];
        let mut c = [0u8; BUFFER];
        let proto3 = Ref::<$ty, Proto3>::encode(&value, &mut a).expect("encode proto3");
        let flatbuffers =
            Ref::<$ty, FlatBuffers>::encode(&value, &mut c).expect("encode flatbuffers");
        let to_flatbuffers = <$ty>::transcode_proto3_to_flatbuffers(proto3.bytes(), &mut b)
            .expect("transcode proto3 to flatbuffers");
        assert_eq!(
            to_flatbuffers.bytes(),
            flatbuffers.bytes(),
            "proto3 to flatbuffers differs from the direct encoding of {value:?}"
        );
        let mut d = [0u8; BUFFER];
        let to_proto3 = <$ty>::transcode_flatbuffers_to_proto3(flatbuffers.bytes(), &mut d)
            .expect("transcode flatbuffers to proto3");
        assert_eq!(
            to_proto3.bytes(),
            proto3.bytes(),
            "flatbuffers to proto3 differs from the direct encoding of {value:?}"
        );
    }};
}

/// Checks the six ordered pairs of the three encodings for one value of
/// `$ty`: the transcode of the bytes of one encoding to another must equal the
/// direct encoding of the value in the other. A podbuf buffer starts on an
/// 8-byte boundary, so every output is an `Aligned` array.
macro_rules! assert_all_pairs_equivalent {
    ($ty:ty, $value:expr) => {{
        let value: $ty = $value;
        let mut proto3_buf = Aligned([0u8; BUFFER]);
        let mut flatbuffers_buf = Aligned([0u8; BUFFER]);
        let mut podbuf_buf = Aligned([0u8; BUFFER]);
        let proto3 = Ref::<$ty, Proto3>::encode(&value, &mut proto3_buf.0).expect("encode proto3");
        let flatbuffers = Ref::<$ty, FlatBuffers>::encode(&value, &mut flatbuffers_buf.0)
            .expect("encode flatbuffers");
        let podbuf = Ref::<$ty, Podbuf>::encode(&value, &mut podbuf_buf.0).expect("encode podbuf");
        let mut out = Aligned([0u8; BUFFER]);
        assert_eq!(
            <$ty>::transcode_proto3_to_podbuf(proto3.bytes(), &mut out.0)
                .expect("proto3 to podbuf")
                .bytes(),
            podbuf.bytes(),
            "proto3 to podbuf differs from the direct encoding of {value:?}"
        );
        assert_eq!(
            <$ty>::transcode_flatbuffers_to_podbuf(flatbuffers.bytes(), &mut out.0)
                .expect("flatbuffers to podbuf")
                .bytes(),
            podbuf.bytes(),
            "flatbuffers to podbuf differs from the direct encoding of {value:?}"
        );
        assert_eq!(
            <$ty>::transcode_podbuf_to_proto3(podbuf.bytes(), &mut out.0)
                .expect("podbuf to proto3")
                .bytes(),
            proto3.bytes(),
            "podbuf to proto3 differs from the direct encoding of {value:?}"
        );
        assert_eq!(
            <$ty>::transcode_podbuf_to_flatbuffers(podbuf.bytes(), &mut out.0)
                .expect("podbuf to flatbuffers")
                .bytes(),
            flatbuffers.bytes(),
            "podbuf to flatbuffers differs from the direct encoding of {value:?}"
        );
        assert_eq!(
            <$ty>::transcode_proto3_to_flatbuffers(proto3.bytes(), &mut out.0)
                .expect("proto3 to flatbuffers")
                .bytes(),
            flatbuffers.bytes(),
            "proto3 to flatbuffers differs from the direct encoding of {value:?}"
        );
        assert_eq!(
            <$ty>::transcode_flatbuffers_to_proto3(flatbuffers.bytes(), &mut out.0)
                .expect("flatbuffers to proto3")
                .bytes(),
            proto3.bytes(),
            "flatbuffers to proto3 differs from the direct encoding of {value:?}"
        );
    }};
}

fn count(value: i64) -> Count {
    Count::new(value).expect("inside the range")
}

fn level(value: i64) -> Level {
    Level::new(value).expect("inside the range")
}

fn label(value: &str) -> Label {
    Label::new(String::from(value)).expect("inside the length bounds")
}

fn reading(
    count_value: i64,
    level_value: i64,
    text: &str,
    mode: Mode,
    ratio: f64,
    flag: bool,
) -> Reading {
    Reading {
        count: count(count_value),
        level: level(level_value),
        label: label(text),
        mode,
        ratio,
        flag,
    }
}

#[test]
fn a_range_bounded_scalar_survives_transcoding_at_each_bound() {
    for value in [0, 1, 100, 199, 200] {
        assert_equivalent!(Count, count(value));
    }
    for value in [-5, -1, 0, 1, 10] {
        assert_equivalent!(Level, level(value));
    }
}

#[test]
fn a_string_survives_transcoding_at_each_length_bound() {
    for value in ["", "a", "sixteen chars..."] {
        assert_equivalent!(Label, label(value));
    }
}

#[test]
fn an_enum_survives_transcoding_for_each_member() {
    for value in [Mode::Idle, Mode::Active, Mode::Fault] {
        assert_equivalent!(Mode, value);
    }
}

#[test]
fn a_struct_survives_transcoding() {
    assert_equivalent!(Reading, Reading::default());
    assert_equivalent!(
        Reading,
        reading(200, 10, "sixteen chars...", Mode::Fault, 1.0, true)
    );
    assert_equivalent!(Reading, reading(0, -5, "", Mode::Active, -1.0, false));
    assert_equivalent!(Reading, reading(17, 3, "mid", Mode::Idle, 0.25, true));
}

#[test]
fn a_union_survives_transcoding_for_each_arm_and_bound() {
    assert_equivalent!(Outcome, Outcome::default());
    assert_equivalent!(Outcome, Outcome::Number(count(0)));
    assert_equivalent!(Outcome, Outcome::Number(count(200)));
    assert_equivalent!(Outcome, Outcome::Text(label("")));
    assert_equivalent!(Outcome, Outcome::Text(label("sixteen chars...")));
    assert_equivalent!(Outcome, Outcome::Mode(Mode::Fault));
    assert_equivalent!(Outcome, Outcome::Mode(Mode::Idle));
    assert_equivalent!(
        Outcome,
        Outcome::Reading(reading(5, -5, "x", Mode::Active, -1.0, true))
    );
    assert_equivalent!(Outcome, Outcome::Reading(Reading::default()));
}

#[test]
fn an_induced_call_struct_survives_transcoding() {
    assert_equivalent!(GaugeSetParams, GaugeSetParams::default());
    assert_equivalent!(
        GaugeSetParams,
        GaugeSetParams {
            count: count(200),
            level: level(-5),
        }
    );
    assert_equivalent!(
        GaugeSetParams,
        GaugeSetParams {
            count: count(7),
            level: level(10),
        }
    );
}

fn key(value: &str) -> Key {
    Key::new(String::from(value)).expect("inside the length bounds")
}

fn inner(count_value: i64, text: &str) -> Inner {
    Inner {
        count: count(count_value),
        label: label(text),
    }
}

#[test]
fn a_non_ascii_string_survives_transcoding() {
    // Each of these is within 16 characters, and each is longer in bytes than
    // in characters.
    for value in ["é", "naïve café", "日本語", "😀😀"] {
        assert_equivalent!(Label, label(value));
    }
    assert_equivalent!(
        Reading,
        reading(3, -2, "température", Mode::Active, 0.5, false)
    );
}

#[test]
fn a_negative_float_survives_transcoding() {
    for ratio in [-1.0, -0.5, -0.125, -0.0] {
        assert_equivalent!(Reading, reading(1, 1, "r", Mode::Idle, ratio, true));
    }
}

#[test]
fn an_optional_field_survives_transcoding_present_and_absent() {
    assert_equivalent!(Panel, Panel::default());
    for note in ["", "x", "naïve"] {
        let panel = Panel {
            note: Some(label(note)),
            ..Panel::default()
        };
        assert_equivalent!(Panel, panel);
    }
}

#[test]
fn an_array_field_survives_transcoding_empty_and_full() {
    for levels in [vec![], vec![-5], vec![0, 10], vec![-5, -1, 0, 10]] {
        let panel = Panel {
            levels: levels.into_iter().map(level).collect(),
            ..Panel::default()
        };
        assert_equivalent!(Panel, panel);
    }
}

#[test]
fn a_map_field_survives_transcoding_empty_and_full() {
    assert_equivalent!(
        Panel,
        Panel {
            by_key: vec![],
            ..Panel::default()
        }
    );
    assert_equivalent!(
        Panel,
        Panel {
            by_key: vec![(key("a"), count(0))],
            ..Panel::default()
        }
    );
    assert_equivalent!(
        Panel,
        Panel {
            by_key: vec![
                (key("clé"), count(200)),
                (key("b"), count(7)),
                // Exactly 8 characters, the upper length bound of `Key`.
                (key("12345678"), count(0)),
            ],
            ..Panel::default()
        }
    );
}

#[test]
fn a_nested_struct_field_survives_transcoding() {
    assert_equivalent!(
        Panel,
        Panel {
            inner: inner(200, "naïve"),
            ..Panel::default()
        }
    );
    assert_equivalent!(
        Panel,
        Panel {
            note: Some(label("n")),
            levels: vec![level(-1), level(2)],
            by_key: vec![(key("k"), count(5))],
            inner: inner(9, "日本語"),
        }
    );
}

#[test]
fn bytes_that_do_not_verify_fail_with_a_verify_error() {
    let garbage = [0xffu8; 8];
    let mut out = [0u8; BUFFER];
    assert!(matches!(
        Reading::transcode_flatbuffers_to_proto3(&garbage, &mut out),
        Err(TranscodeError::Verify(_))
    ));
    assert!(matches!(
        Reading::transcode_proto3_to_flatbuffers(&garbage, &mut out),
        Err(TranscodeError::Verify(_))
    ));
}

#[test]
fn an_output_buffer_that_is_too_small_fails_with_a_capacity_error() {
    let value = reading(200, 10, "sixteen chars...", Mode::Fault, 1.0, true);
    let mut a = [0u8; BUFFER];
    let mut c = [0u8; BUFFER];
    let proto3 = Ref::<Reading, Proto3>::encode(&value, &mut a).expect("encode proto3");
    let flatbuffers =
        Ref::<Reading, FlatBuffers>::encode(&value, &mut c).expect("encode flatbuffers");
    let mut small = [0u8; 1];
    assert!(matches!(
        Reading::transcode_proto3_to_flatbuffers(proto3.bytes(), &mut small),
        Err(TranscodeError::Encode(EncodeError::Capacity { .. }))
    ));
    assert!(matches!(
        Reading::transcode_flatbuffers_to_proto3(flatbuffers.bytes(), &mut small),
        Err(TranscodeError::Encode(EncodeError::Capacity { .. }))
    ));
}

/// Asserts that `result` is a range violation of the typl type `type_name`.
fn assert_range_violation<T>(result: Result<T, TranscodeError>, type_name: &str) {
    match result {
        Err(TranscodeError::Verify(VerifyError::Contract(violation))) => {
            assert_eq!(violation.rule, Rule::Range);
            assert_eq!(violation.type_name, Some(type_name));
        }
        Err(error) => panic!("expected a range violation of {type_name}, got {error:?}"),
        Ok(_) => panic!("expected a range violation of {type_name}, got a success"),
    }
}

#[test]
fn a_value_outside_a_declared_range_fails_with_a_contract_violation() {
    let mut out = [0u8; BUFFER];
    // A proto3 varint field 1 holding 201, one above the upper bound of
    // `Count`: well formed as proto3, so only the range rule rejects it.
    assert_range_violation(
        Count::transcode_proto3_to_flatbuffers(&[0x08, 0xc9, 0x01], &mut out),
        "Count",
    );
    // The same bytes with the bound value 200 transcode.
    Count::transcode_proto3_to_flatbuffers(&[0x08, 0xc8, 0x01], &mut out)
        .expect("200 is inside the range");
    // The lower bound of `Level` is -5. Proto3 writes a signed integer as a
    // zigzag varint: -5 is 9 and -6 is 11.
    assert_range_violation(
        Level::transcode_proto3_to_flatbuffers(&[0x08, 0x0b], &mut out),
        "Level",
    );
    Level::transcode_proto3_to_flatbuffers(&[0x08, 0x09], &mut out)
        .expect("-5 is inside the range");
    // The upper bound is 10: zigzag 20 is 0x14 and 22 is 0x16.
    assert_range_violation(
        Level::transcode_proto3_to_flatbuffers(&[0x08, 0x16], &mut out),
        "Level",
    );
    Level::transcode_proto3_to_flatbuffers(&[0x08, 0x14], &mut out)
        .expect("10 is inside the range");
}

#[test]
fn a_range_violation_inside_a_composite_fails_in_both_directions() {
    let mut a = [0u8; BUFFER];
    let mut out = [0u8; BUFFER];
    let value = reading(200, 0, "x", Mode::Idle, 0.0, false);

    // Proto3: field 1 of `Reading` is the count, a varint. 200 is `c8 01`
    // and 201 is `c9 01`.
    let mut proto3 = Ref::<Reading, Proto3>::encode(&value, &mut a)
        .expect("encode proto3")
        .bytes()
        .to_vec();
    assert_eq!(proto3[..3], [0x08, 0xc8, 0x01]);
    Reading::transcode_proto3_to_flatbuffers(&proto3, &mut out).expect("200 is inside the range");
    proto3[1] = 0xc9;
    assert_range_violation(
        Reading::transcode_proto3_to_flatbuffers(&proto3, &mut out),
        "Count",
    );

    // FlatBuffers: change the one stored byte that holds the count.
    let mut flatbuffers = Ref::<Reading, FlatBuffers>::encode(&value, &mut a)
        .expect("encode flatbuffers")
        .bytes()
        .to_vec();
    Reading::transcode_flatbuffers_to_proto3(&flatbuffers, &mut out)
        .expect("200 is inside the range");
    let at = flatbuffers.iter().position(|&b| b == 200).expect("a 200");
    assert_eq!(flatbuffers.iter().filter(|&&b| b == 200).count(), 1);
    flatbuffers[at] = 201;
    assert_range_violation(
        Reading::transcode_flatbuffers_to_proto3(&flatbuffers, &mut out),
        "Count",
    );
}

#[test]
fn a_flatbuffers_value_outside_a_declared_range_fails_with_a_contract_violation() {
    let mut a = [0u8; BUFFER];
    let mut out = [0u8; BUFFER];
    // Encode the bound value, then change its one stored byte, so the buffer
    // stays well formed and only the range rule rejects it.
    let mut count_bytes = Ref::<Count, FlatBuffers>::encode(&count(200), &mut a)
        .expect("encode flatbuffers")
        .bytes()
        .to_vec();
    Count::transcode_flatbuffers_to_proto3(&count_bytes, &mut out)
        .expect("200 is inside the range");
    let at = count_bytes.iter().position(|&b| b == 200).expect("a 200");
    assert_eq!(count_bytes.iter().filter(|&&b| b == 200).count(), 1);
    count_bytes[at] = 201;
    assert_range_violation(
        Count::transcode_flatbuffers_to_proto3(&count_bytes, &mut out),
        "Count",
    );

    // `Level` is stored as an i8, so -5 is 0xfb and -6 is 0xfa.
    let mut level_bytes = Ref::<Level, FlatBuffers>::encode(&level(-5), &mut a)
        .expect("encode flatbuffers")
        .bytes()
        .to_vec();
    let at = level_bytes.iter().position(|&b| b == 0xfb).expect("a -5");
    assert_eq!(level_bytes.iter().filter(|&&b| b == 0xfb).count(), 1);
    level_bytes[at] = 0xfa;
    assert_range_violation(
        Level::transcode_flatbuffers_to_proto3(&level_bytes, &mut out),
        "Level",
    );
    // The upper bound is 10 (0x0a); 11 is 0x0b.
    let mut level_bytes = Ref::<Level, FlatBuffers>::encode(&level(10), &mut a)
        .expect("encode flatbuffers")
        .bytes()
        .to_vec();
    Level::transcode_flatbuffers_to_proto3(&level_bytes, &mut out).expect("10 is inside the range");
    let at = level_bytes.iter().position(|&b| b == 0x0a).expect("a 10");
    assert_eq!(level_bytes.iter().filter(|&&b| b == 0x0a).count(), 1);
    level_bytes[at] = 0x0b;
    assert_range_violation(
        Level::transcode_flatbuffers_to_proto3(&level_bytes, &mut out),
        "Level",
    );
}

#[test]
fn podbuf_transcodes_agree_with_the_direct_encodings() {
    // A named scalar, an enum, a struct, a union and the induced call struct
    // each carry the three encodings.
    for value in [0, 1, 100, 200] {
        assert_all_pairs_equivalent!(Count, count(value));
    }
    for value in [-5, 0, 10] {
        assert_all_pairs_equivalent!(Level, level(value));
    }
    for value in ["", "a", "naïve café", "sixteen chars..."] {
        assert_all_pairs_equivalent!(Label, label(value));
    }
    for value in [Mode::Idle, Mode::Active, Mode::Fault] {
        assert_all_pairs_equivalent!(Mode, value);
    }
    assert_all_pairs_equivalent!(Reading, Reading::default());
    assert_all_pairs_equivalent!(
        Reading,
        reading(200, 10, "sixteen chars...", Mode::Fault, 1.0, true)
    );
    assert_all_pairs_equivalent!(
        Reading,
        reading(17, -3, "température", Mode::Idle, -0.25, true)
    );
    assert_all_pairs_equivalent!(Outcome, Outcome::Number(count(200)));
    assert_all_pairs_equivalent!(Outcome, Outcome::Text(label("mid")));
    assert_all_pairs_equivalent!(Outcome, Outcome::Mode(Mode::Fault));
    assert_all_pairs_equivalent!(
        Outcome,
        Outcome::Reading(reading(5, -5, "x", Mode::Active, -1.0, true))
    );
    assert_all_pairs_equivalent!(
        GaugeSetParams,
        GaugeSetParams {
            count: count(200),
            level: level(-5),
        }
    );
    assert_all_pairs_equivalent!(
        Panel,
        Panel {
            note: Some(label("n")),
            levels: vec![level(-1), level(2)],
            // Sorted by key: podbuf writes a map in key order, so a map in
            // any other order would decode into a different sequence.
            by_key: vec![(key("clé"), count(200)), (key("k"), count(5))],
            inner: inner(9, "日本語"),
        }
    );
}

#[test]
fn podbuf_bytes_that_do_not_verify_fail_with_a_verify_error() {
    let garbage = Aligned([0xffu8; 8]);
    let mut out = Aligned([0u8; BUFFER]);
    assert!(matches!(
        Reading::transcode_podbuf_to_proto3(&garbage.0, &mut out.0),
        Err(TranscodeError::Verify(_))
    ));
    assert!(matches!(
        Reading::transcode_podbuf_to_flatbuffers(&garbage.0, &mut out.0),
        Err(TranscodeError::Verify(_))
    ));
}

#[test]
fn a_podbuf_output_buffer_that_is_too_small_fails_with_a_capacity_error() {
    let value = reading(200, 10, "sixteen chars...", Mode::Fault, 1.0, true);
    let mut a = [0u8; BUFFER];
    let proto3 = Ref::<Reading, Proto3>::encode(&value, &mut a).expect("encode proto3");
    let mut small = Aligned([0u8; 8]);
    assert!(matches!(
        Reading::transcode_proto3_to_podbuf(proto3.bytes(), &mut small.0),
        Err(TranscodeError::Encode(EncodeError::Capacity { .. }))
    ));
}

// ---------------------------------------------------------------------------
// Open enums
// ---------------------------------------------------------------------------

/// proto3 carries an enum as an int32, so an `Unknown` whose raw integer does
/// not fit int32 can reach the proto3 encoder only from another encoding.
/// The encoder refuses it with the contract error for an out-of-range value,
/// `Rule::Range`, naming the enum; the transcode reports the same error.
#[test]
fn an_unknown_outside_int32_is_refused_by_the_proto3_encoder() {
    let package = ir::compile_fixture("open_enums.ridl");
    let generated = ridl_backend_rust::generate_pipeline(
        &package,
        &[
            ridl_backend_rust::WireEncoding::FlatBuffers,
            ridl_backend_rust::WireEncoding::Proto3,
        ],
        true,
        &[],
    )
    .expect("the open enums fixture generates")
    .rust_source;
    rustc::run_program(
        "transcode_open_enum_outside_int32",
        &format!(
            "#![allow(dead_code)]\n{generated}\n{}",
            r#"
use ridl_rt::encoding::{FlatBuffers, Proto3};
use ridl_rt::payload::{EncodeError, Ref, Rule, TranscodeError};

fn main() {
    let wide = OpenMsg { status: DtcStatus::from(1i64 << 40), flags: Flags::from(0) };
    let mut a = [0u8; 64];
    let flatbuffers = Ref::<OpenMsg, FlatBuffers>::encode(&wide, &mut a)
        .expect("a FlatBuffers long holds any i64")
        .bytes()
        .to_vec();
    let mut out = [0u8; 64];
    match OpenMsg::transcode_flatbuffers_to_proto3(&flatbuffers, &mut out) {
        Err(TranscodeError::Encode(EncodeError::Contract(violation))) => {
            assert_eq!(violation.rule, Rule::Range);
            assert_eq!(violation.type_name, Some("DtcStatus"));
        }
        Err(other) => panic!("expected a range violation, got {other:?}"),
        Ok(_) => panic!("expected a range violation, got a success"),
    }
    match Ref::<OpenMsg, Proto3>::encode(&wide, &mut out) {
        Err(EncodeError::Contract(violation)) => {
            assert_eq!(violation.rule, Rule::Range);
            assert_eq!(violation.type_name, Some("DtcStatus"));
        }
        Err(other) => panic!("expected a range violation, got {other:?}"),
        Ok(_) => panic!("expected a range violation, got a success"),
    }
    // The int32 bounds themselves transcode.
    for raw in [i64::from(i32::MIN), i64::from(i32::MAX)] {
        let edge = OpenMsg { status: DtcStatus::from(raw), flags: Flags::from(0) };
        let flatbuffers = Ref::<OpenMsg, FlatBuffers>::encode(&edge, &mut a)
            .expect("encode")
            .bytes()
            .to_vec();
        let proto3 = OpenMsg::transcode_flatbuffers_to_proto3(&flatbuffers, &mut out)
            .expect("an int32 bound transcodes");
        let back = Ref::<OpenMsg, Proto3>::verify(proto3.bytes()).expect("verify").decode();
        assert_eq!(i64::from(back.status), raw);
    }
}
"#
        ),
    );
}
