//! The equivalence check of the generated transcode functions.
//!
//! It brings in the checked-in generated module of `tests/generated/transcode.rs`
//! (regenerated and compared by `tests/transcode_regeneration.rs`) and, for each
//! value, compares the bytes of a transcode with the bytes of a direct
//! encoding: `T::transcode_proto3_to_flatbuffers` over the proto3 encoding of
//! the value must equal the FlatBuffers encoding of the value, and the reverse
//! direction likewise. A streaming transcoder that replaces the bodies of those
//! functions must pass the same comparisons.

use ridl_rt::encoding::{FlatBuffers, Proto3};
use ridl_rt::payload::{EncodeError, Ref, Rule, TranscodeError, VerifyError};

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
#[allow(
    clippy::identity_op,
    reason = "the emitted array codec multiplies a length by an element size of 1"
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
    // Each of these is within 16 characters, and the last two are longer in
    // bytes than in characters.
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
                (key("sixteen!"), count(0)),
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

#[test]
fn a_value_outside_a_declared_range_fails_with_a_contract_violation() {
    // A proto3 varint field 1 holding 201, one above the upper bound of
    // `Count`: well formed as proto3, so only the range rule rejects it.
    let outside = [0x08, 0xc9, 0x01];
    let mut out = [0u8; BUFFER];
    assert!(matches!(
        Count::transcode_proto3_to_flatbuffers(&outside, &mut out),
        Err(TranscodeError::Verify(VerifyError::Contract(violation)))
            if violation.rule == Rule::Range
    ));
    // The same bytes with the bound value 200 transcode.
    Count::transcode_proto3_to_flatbuffers(&[0x08, 0xc8, 0x01], &mut out)
        .expect("200 is inside the range");
}
