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
use ridl_rt::payload::{EncodeError, Ref, TranscodeError};

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

use generated::{Count, GaugeSetParams, Label, Level, Mode, Outcome, Reading};

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
