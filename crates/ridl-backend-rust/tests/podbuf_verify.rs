//! The podbuf verifier, view and decoder: the rules of §7 of the podbuf
//! design (`docs/wip/2026-10-24-podbuf-design.md`), the read-in-place view
//! (§10) and `decode` into the domain value (§14 "Decode").
//!
//! Each test compiles one program over the generated fixture through
//! `support::rustc`, as `podbuf_encode.rs` does: the program encodes a value,
//! patches a byte of the buffer where a test names one, runs
//! `__ridl_podbuf_verify` over it and prints one line per case. A refusal
//! prints the name of the `VerifyError` variant it carries (`Contract`), or
//! of the `Malformed` variant (`OutOfBounds`, `Stride`, ...).

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

/// The generated source for the fixture, with the podbuf codec.
fn generated() -> String {
    let package = ir::compile_fixture("podbuf_roundtrip.ridl");
    ridl_backend_rust::generate_pipeline(
        &package,
        &[ridl_backend_rust::WireEncoding::Podbuf],
        false,
        &[],
    )
    .expect("the fixture generates")
    .rust_source
}

/// The helpers every program draws on: value builders, the encoder call, an
/// aligned copy of a buffer, byte patches and the verdict of a `verify`.
const HELPERS: &str = r#"
use ridl_rt::payload::{VerifyError, View};
use ridl_rt::podbuf::{Aligned, Writer};

fn ev(code: i64, message: &str, timestamp: i64) -> FaultEvent {
    FaultEvent { code, message: message.into(), timestamp }
}

fn door(sensor_id: i64, is_open: bool) -> DoorPayload {
    DoorPayload { sensor_id, is_open }
}

fn map(pairs: &[(&str, i64)]) -> Vec<(Name, Weight)> {
    pairs
        .iter()
        .map(|(key, value)| (Name::new_unchecked((*key).into()), Weight::new_unchecked(*value)))
        .collect()
}

/// The buffer `value` encodes to.
macro_rules! enc {
    ($value:expr) => {{
        let value = $value;
        let mut out = Aligned([0u8; 4096]);
        let mut writer = Writer::new(&mut out.0);
        value.__ridl_podbuf_encode(&mut writer).expect("the encoder accepts the value");
        writer.finish().expect("the buffer finishes").to_vec()
    }};
}

/// An 8-aligned copy of `bytes`, which a `Vec<u8>` does not promise.
fn aligned(bytes: &[u8]) -> Aligned<[u8; 4096]> {
    let mut out = Aligned([0u8; 4096]);
    out.0[..bytes.len()].copy_from_slice(bytes);
    out
}

/// `OK`, or the name of the variant the refusal carries.
fn verdict<T>(result: Result<T, VerifyError>) -> String {
    match result {
        Ok(_) => "OK".to_string(),
        Err(VerifyError::Contract(_)) => "Contract".to_string(),
        Err(VerifyError::Structure(malformed)) => format!("{malformed:?}"),
        Err(other) => format!("{other:?}"),
    }
}

/// Verifies `bytes` as a `$ty` and prints the verdict.
macro_rules! check {
    ($ty:ty, $bytes:expr) => {{
        let bytes: Vec<u8> = $bytes;
        let buf = aligned(&bytes);
        verdict(<$ty>::__ridl_podbuf_verify(&buf.0[..bytes.len()]))
    }};
}

/// Encodes `$value`, verifies, decodes, and prints whether the round trip
/// is the identity.
macro_rules! roundtrip {
    ($ty:ty, $value:expr) => {{
        let value = $value;
        let bytes = enc!(value.clone());
        let buf = aligned(&bytes);
        let view = <$ty>::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
        let decoded = <$ty>::__ridl_podbuf_decode(view);
        (decoded == value).to_string()
    }};
}

fn patch(mut bytes: Vec<u8>, at: usize, value: u8) -> Vec<u8> {
    bytes[at] = value;
    bytes
}

fn patch_u16(mut bytes: Vec<u8>, at: usize, value: u16) -> Vec<u8> {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn patch_u32(mut bytes: Vec<u8>, at: usize, value: u32) -> Vec<u8> {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

/// A one-descriptor head (`Short2`, `Blob` with a zero second descriptor
/// aside) whose tail holds `tail`, with the descriptor `{ off: 16, len }`.
fn one_desc(len: u32, tail: &[u8]) -> Vec<u8> {
    let mut bytes = vec![8, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0];
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(tail);
    bytes
}

/// A `Scored` buffer by hand: two entries of `{ key: f64, value: u8 }` at
/// 16 and 32, in the order given.
fn scored(keys: &[f64]) -> Vec<u8> {
    let mut bytes = vec![8, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0];
    bytes.extend_from_slice(&(keys.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    for (i, key) in keys.iter().enumerate() {
        bytes.extend_from_slice(&key.to_le_bytes());
        bytes.push(i as u8 + 1);
        bytes.extend_from_slice(&[0; 7]);
    }
    bytes
}

/// Swaps the two 16-byte map entries of a `Labels` buffer.
fn swap_entries(mut bytes: Vec<u8>) -> Vec<u8> {
    let (head, tail) = bytes.split_at_mut(32);
    head[16..32].swap_with_slice(&mut tail[..16]);
    bytes
}
"#;

/// Runs a program whose `main` prints one line per expression of `cases`,
/// and returns the lines.
fn run(name: &str, cases: &[&str]) -> Vec<String> {
    let generated = generated();
    let prints: String = cases
        .iter()
        .map(|case| format!("    println!(\"{{}}\", {case});\n"))
        .collect();
    let source = format!(
        "#![allow(dead_code, deprecated, unused_imports, unused_macros)]\n{generated}\n{HELPERS}\n\
         fn main() {{\n{prints}}}\n"
    );
    rustc::run_program_capturing_stdout(name, &source)
        .lines()
        .map(str::to_string)
        .collect()
}

/// §7 rules 1 to 9 over the encoder's own output, and the view reads each
/// field in place.
#[test]
fn verify_accepts_what_encode_wrote_and_the_view_reads_it_in_place() {
    let out = run(
        "podbuf_view_reads",
        &[
            r#"{
                let bytes = enc!(ev(0x1234, "ab", -1));
                let buf = aligned(&bytes);
                let v = FaultEvent::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                format!("{} {} {} {}", v.code(), v.message(), v.timestamp(), View::bytes(&v).len())
            }"#,
            r#"check!(DoorPayload, enc!(door(7, true)))"#,
            r#"check!(FaultPage, enc!(FaultPage { faults: vec![ev(1, "a", 0), ev(2, "bc", 1)] }))"#,
            r#"check!(Labels, enc!(Labels { by_name: map(&[("b", 2), ("a", 1)]) }))"#,
            r#"check!(Nested, enc!(Nested { door: door(3, false), health: Health::Fail, span: NestedSpan { lo: 1, hi: 2 }, readings: [1, 2] }))"#,
            r#"check!(Outcome, enc!(Outcome::Ok(door(7, true))))"#,
            r#"check!(Ordered, enc!(Ordered { label: "ab".into(), xs: vec![1, 2], faults: vec![ev(1, "a", 0)] }))"#,
        ],
    );
    assert_eq!(out, ["4660 ab -1 35", "OK", "OK", "OK", "OK", "OK", "OK"]);
}

/// §7 rule 2: a boolean above 1, a strict enum value the declaration does not
/// name, and a bit the enum set does not declare are refused as contract
/// violations.
#[test]
fn a_boolean_above_1_a_strict_enum_value_not_declared_and_an_undeclared_bit_are_refused() {
    let out = run(
        "podbuf_rule_2",
        &[
            r#"check!(DoorPayload, patch(enc!(door(7, true)), 9, 2))"#,
            r#"check!(Shifter, patch(enc!(Shifter { gear: GearPosition::Drive }), 8, 7))"#,
            r#"check!(Warnings, patch(enc!(Warnings { flags: WarningFlags::LOW_OIL }), 8, 0x80))"#,
            r#"check!(Warnings, patch(enc!(Warnings { flags: WarningFlags::LOW_OIL }), 8, 0x07))"#,
        ],
    );
    assert_eq!(out, ["Contract", "Contract", "Contract", "OK"]);
}

/// §7 rule 2: an absent optional whose descriptor bytes are not zero is
/// refused; a present one with a value is accepted.
#[test]
fn an_absent_optional_whose_bytes_are_not_zero_is_refused() {
    let out = run(
        "podbuf_absent_optional",
        &[
            r#"check!(DiagFilter, patch(enc!(DiagFilter { severity: 2, category: None }), 12, 24))"#,
            r#"check!(DiagFilter, enc!(DiagFilter { severity: 2, category: None }))"#,
            r#"check!(DiagFilter, enc!(DiagFilter { severity: 2, category: Some("x".into()) }))"#,
            r#"check!(DiagFilter, patch(enc!(DiagFilter { severity: 2, category: Some("x".into()) }), 9, 0x03))"#,
        ],
    );
    assert_eq!(out, ["Contract", "OK", "OK", "OK"]);
}

/// §7 rule 4: a descriptor inside the head, one whose end overflows, a string
/// length above four times its bound, and a `len` of 0 with a non-zero `off`.
#[test]
fn rule_4_descriptor_edges() {
    let out = run(
        "podbuf_rule_4",
        &[
            r#"check!(FaultEvent, patch_u32(enc!(ev(1, "ab", 0)), 12, 8))"#,
            r#"check!(FaultEvent, patch_u32(enc!(ev(1, "ab", 0)), 12, u32::MAX - 1))"#,
            r#"check!(FaultEvent, patch_u32(enc!(ev(1, "ab", 0)), 16, 4 * 256 + 1))"#,
            r#"check!(FaultEvent, patch_u32(enc!(ev(1, "", 0)), 12, 32))"#,
            r#"check!(FaultEvent, patch_u32(enc!(ev(1, "ab", 0)), 12, 33))"#,
        ],
    );
    assert_eq!(
        out,
        [
            "OutOfBounds",
            "OutOfBounds",
            "OutOfBounds",
            "OutOfBounds",
            "OutOfBounds"
        ]
    );
}

/// §7 "Not checked, by design": two descriptors over the same bytes are
/// accepted.
#[test]
fn two_descriptors_over_the_same_bytes_are_accepted() {
    let out = run(
        "podbuf_overlap",
        &[
            r#"check!(TwoStrings, patch_u32(enc!(TwoStrings { a: "ab".into(), b: "ab".into() }), 16, 24))"#,
        ],
    );
    assert_eq!(out, ["OK"]);
}

/// §7 rules 3 and 6: the bytes are checked first (terminator, interior NUL,
/// UTF-8), then the scalar-value count against the bound.
#[test]
fn rule_3_counts_scalars_after_rule_6_checks_bytes() {
    let out = run(
        "podbuf_rule_3_6",
        &[
            r#"check!(Short2, one_desc(3, b"abc\0"))"#,
            r#"check!(Short2, one_desc(8, "𝄞𝄞\0".as_bytes()))"#,
            r#"check!(Short2, one_desc(2, b"ab\xff"))"#,
            r#"check!(Short2, one_desc(3, b"a\0b\0"))"#,
            r#"check!(Short2, one_desc(2, b"\xff\xfe\0"))"#,
            r#"check!(Short2, one_desc(2, b"ab"))"#,
            // Nine ASCII bytes are a valid string of nine scalar values, but
            // `len` is above four bytes per scalar value of the bound, so
            // rule 4 refuses it before the bytes are read.
            r#"check!(Short2, one_desc(9, b"abcdefghi\0"))"#,
        ],
    );
    assert_eq!(
        out,
        [
            "Contract",
            "OK",
            "Terminator",
            "Terminator",
            "Utf8",
            "OutOfBounds",
            "OutOfBounds"
        ]
    );
}

/// §7 rule 5: a struct element's stride is a multiple of 8, a scalar
/// element's stride is its exact size.
#[test]
fn rule_5_stride_mismatch_is_refused() {
    let out = run(
        "podbuf_rule_5",
        &[
            r#"check!(FaultPage, patch_u16(enc!(FaultPage { faults: vec![ev(1, "a", 0), ev(2, "b", 0)] }), 14, 20))"#,
            r#"check!(Readings, patch_u16(enc!(Readings { xs: vec![1, 2, 3] }), 14, 4))"#,
            r#"check!(Readings, enc!(Readings { xs: vec![1, 2, 3] }))"#,
        ],
    );
    assert_eq!(out, ["Stride", "Stride", "OK"]);
}

/// §7 rules 7 and 8: a tag that names no arm and an arm length that is not a
/// multiple of 8 are `Union`; an empty arm is a zero descriptor; map keys
/// are strictly increasing, with `0.0` and `-0.0` one key and NaN refused.
#[test]
fn rule_7_and_8_unions_and_maps() {
    let out = run(
        "podbuf_rule_7_8",
        &[
            r#"check!(Outcome, patch(enc!(Outcome::Ok(door(1, false))), 8, 3))"#,
            r#"check!(Outcome, patch_u32(enc!(Outcome::Ok(door(1, false))), 16, 12))"#,
            r#"check!(Maybe, enc!(Maybe::None(Empty {})))"#,
            r#"check!(Maybe, patch_u32(enc!(Maybe::None(Empty {})), 12, 32))"#,
            r#"check!(Maybe, enc!(Maybe::Some(door(1, true))))"#,
            r#"check!(Labels, swap_entries(enc!(Labels { by_name: map(&[("a", 1), ("b", 2)]) })))"#,
            r#"check!(Labels, enc!(Labels { by_name: map(&[("a", 1), ("b", 2)]) }))"#,
            r#"check!(Scored, scored(&[0.0, -0.0]))"#,
            r#"check!(Scored, scored(&[-1.0, 2.5]))"#,
            r#"check!(Scored, scored(&[f64::NAN]))"#,
        ],
    );
    assert_eq!(
        out,
        [
            "Union",
            "Union",
            "OK",
            "OutOfBounds",
            "OK",
            "KeyOrder",
            "OK",
            "KeyOrder",
            "OK",
            "Contract"
        ]
    );
}

/// §14 "Floats in verify": a float with no range accepts NaN. A `float` with
/// no range is an `f64` in Rust, so the value is `f64::NAN`.
#[test]
fn a_float_with_no_range_accepts_nan() {
    let out = run(
        "podbuf_nan",
        &[r#"check!(FreeFloat, enc!(FreeFloat { x: f64::NAN }))"#],
    );
    assert_eq!(out, ["OK"]);
}

/// §7 rule 4 for bytes and arrays: the count is checked against the bound
/// (`Contract`), and a required array with no element is refused.
#[test]
fn bytes_and_array_counts_are_checked_against_their_bounds() {
    let out = run(
        "podbuf_counts",
        &[
            r#"check!(Blob, enc!(Blob { data: vec![1, 2, 3], marks: vec![4] }))"#,
            r#"check!(Blob, patch_u32(enc!(Blob { data: vec![1, 2, 3], marks: vec![4] }), 12, 9))"#,
            r#"check!(Blob, patch_u32(patch_u32(enc!(Blob { data: vec![1, 2, 3], marks: vec![4] }), 16, 0), 20, 0))"#,
            r#"check!(Blob, patch_u16(enc!(Blob { data: vec![1, 2, 3], marks: vec![4] }), 20, 5))"#,
        ],
    );
    assert_eq!(out, ["OK", "OutOfBounds", "Contract", "OutOfBounds"]);
}

/// §8.1: a head shorter than the reader's reads its missing fields as 0, and
/// a head longer than the reader's is accepted with its extra bytes ignored.
#[test]
fn a_producer_head_of_another_size_is_read_at_its_own_size() {
    let out = run(
        "podbuf_head_size",
        &[
            // `FaultEvent` with a 16-byte head: `code` at 0, the descriptor at
            // 4..12, no `timestamp`.
            r#"{
                let bytes = vec![16, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                let buf = aligned(&bytes);
                match FaultEvent::__ridl_podbuf_verify(&buf.0[..bytes.len()]) {
                    Ok(v) => format!("{} {} {}", v.code(), v.message().len(), v.timestamp()),
                    Err(e) => verdict::<()>(Err(e)),
                }
            }"#,
            // The same 16-byte head with a message in the tail at 24: the
            // bytes where the reader's `timestamp` would be are the string's,
            // and still read as 0 because they lie past the producer's size.
            r#"{
                let bytes = vec![16, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 24, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, b'a', b'b', 0];
                let buf = aligned(&bytes);
                match FaultEvent::__ridl_podbuf_verify(&buf.0[..bytes.len()]) {
                    Ok(v) => format!("{} {} {}", v.code(), v.message(), v.timestamp()),
                    Err(e) => verdict::<()>(Err(e)),
                }
            }"#,
            // `DoorPayload` with a 16-byte head whose second half is not zero.
            r#"check!(DoorPayload, vec![16, 0, 0, 0, 0, 0, 0, 0, 7, 1, 0, 0, 0, 0, 0, 0, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 1, 2])"#,
            // A pad byte that is not zero is not checked.
            r#"check!(DoorPayload, patch(enc!(door(7, true)), 15, 0xff))"#,
            // The preamble: a head size that is not a multiple of 8, and one
            // past the buffer.
            r#"check!(DoorPayload, patch_u32(enc!(door(7, true)), 0, 12))"#,
            r#"check!(DoorPayload, patch_u32(enc!(door(7, true)), 0, 16))"#,
            r#"check!(DoorPayload, vec![8, 0, 0, 0])"#,
        ],
    );
    assert_eq!(
        out,
        [
            "5 0 0", "5 ab 0", "OK", "OK", "Preamble", "Preamble", "Preamble"
        ]
    );
}

/// The view's accessors over every shape: an optional, a nested head, an
/// exact-length array, a bounded array of heads with their strings, a map,
/// a union's arm and a deprecated field.
#[test]
fn the_view_reads_every_shape_in_place() {
    let out = run(
        "podbuf_view_shapes",
        &[
            r#"{
                let bytes = enc!(DiagFilter { severity: 2, category: Some("x".into()) });
                let buf = aligned(&bytes);
                let v = DiagFilter::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                format!("{} {:?}", v.severity(), v.category())
            }"#,
            r#"{
                let bytes = enc!(DiagFilter { severity: 2, category: None });
                let buf = aligned(&bytes);
                let v = DiagFilter::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                format!("{:?}", v.category())
            }"#,
            r#"{
                let bytes = enc!(Nested { door: door(3, true), health: Health::Warn, span: NestedSpan { lo: 1, hi: 0x0302 }, readings: [10, 11] });
                let buf = aligned(&bytes);
                let v = Nested::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                let readings: Vec<i64> = v.readings().collect();
                format!("{} {} {:?} {} {} {:?}", v.door().sensor_id(), v.door().is_open(), v.health(), v.span().lo(), v.span().hi(), readings)
            }"#,
            r#"{
                let bytes = enc!(FaultPage { faults: vec![ev(1, "a", 0), ev(2, "bc", -1)] });
                let buf = aligned(&bytes);
                let v = FaultPage::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                let faults: Vec<String> = v.faults().map(|f| format!("{}:{}:{}", f.code(), f.message(), f.timestamp())).collect();
                faults.join(",")
            }"#,
            r#"{
                let bytes = enc!(Labels { by_name: map(&[("b", 2), ("a", 1)]) });
                let buf = aligned(&bytes);
                let v = Labels::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                let entries: Vec<String> = v.by_name().map(|(k, w)| format!("{}={}", k, w.get())).collect();
                entries.join(",")
            }"#,
            r#"{
                let bytes = enc!(Outcome::Ok(door(7, true)));
                let buf = aligned(&bytes);
                let v = Outcome::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                match v.arm() {
                    OutcomePodbufArm::Ok(d) => format!("ok {} {}", d.sensor_id(), d.is_open()),
                    OutcomePodbufArm::Bad(h) => format!("bad {h:?}"),
                }
            }"#,
            r#"{
                let bytes = enc!(Outcome::Bad(Health::Warn));
                let buf = aligned(&bytes);
                let v = Outcome::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                match v.arm() {
                    OutcomePodbufArm::Ok(d) => format!("ok {} {}", d.sensor_id(), d.is_open()),
                    OutcomePodbufArm::Bad(h) => format!("bad {h:?}"),
                }
            }"#,
            r#"{
                let bytes = enc!(Legacy { legacy: 7 });
                let buf = aligned(&bytes);
                let v = Legacy::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                format!("{}", v.legacy())
            }"#,
            r#"{
                let bytes = enc!(Blob { data: vec![1, 2, 3], marks: vec![4, 5] });
                let buf = aligned(&bytes);
                let v = Blob::__ridl_podbuf_verify(&buf.0[..bytes.len()]).expect("verifies");
                let marks: Vec<i64> = v.marks().collect();
                format!("{:?} {:?}", v.data(), marks)
            }"#,
        ],
    );
    assert_eq!(
        out,
        [
            "2 Some(\"x\")",
            "None",
            "3 true Warn 1 770 [10, 11]",
            "1:a:0,2:bc:-1",
            "a=1,b=2",
            "ok 7 true",
            "bad Warn",
            "5",
            "[1, 2, 3] [4, 5]",
        ]
    );
}

/// §14 "Decode": the owned domain value is built from the view, and the round
/// trip through the encoder is the identity.
#[test]
fn decode_builds_the_domain_value_from_the_view() {
    let out = run(
        "podbuf_decode",
        &[
            r#"roundtrip!(FaultPage, FaultPage { faults: vec![ev(1, "a", 0), ev(2, "bc", -1), ev(3, "", 7)] })"#,
            r#"roundtrip!(DoorPayload, door(7, true))"#,
            r#"roundtrip!(DiagFilter, DiagFilter { severity: 2, category: Some("x".into()) })"#,
            r#"roundtrip!(DiagFilter, DiagFilter { severity: 2, category: None })"#,
            r#"roundtrip!(Labels, Labels { by_name: map(&[("a", 1), ("b", 2)]) })"#,
            r#"roundtrip!(Nested, Nested { door: door(3, false), health: Health::Fail, span: NestedSpan { lo: 1, hi: 0x0302 }, readings: [0x0a0b, 0x0c0d] })"#,
            r#"roundtrip!(Outcome, Outcome::Ok(door(7, true)))"#,
            r#"roundtrip!(Outcome, Outcome::Bad(Health::Warn))"#,
            r#"roundtrip!(Maybe, Maybe::None(Empty {}))"#,
            r#"roundtrip!(Ordered, Ordered { label: "ab".into(), xs: vec![1, -2], faults: vec![ev(1, "a", 0)] })"#,
            r#"roundtrip!(Keyed, Keyed { label: "ab".into(), by_name: map(&[("a", 1)]) })"#,
            r#"roundtrip!(Wrapped, Wrapped { label: "ab".into(), outcome: Outcome::Bad(Health::Warn) })"#,
            r#"roundtrip!(Scored, Scored { by_score: vec![(-1.0, Weight::new_unchecked(2)), (2.5, Weight::new_unchecked(1))] })"#,
            r#"roundtrip!(Blob, Blob { data: vec![1, 2, 3], marks: vec![4, 5] })"#,
            r#"roundtrip!(Warnings, Warnings { flags: WarningFlags::DOOR_AJAR })"#,
        ],
    );
    assert_eq!(out, vec!["true"; 15]);
}

/// §8.1 in `verify`, not only in the view: a constrained field past the
/// producer's size reads as 0 whatever the bytes there hold, for an array
/// element (its `elem_size`), a union arm (its descriptor `len`) and the
/// root head (`head_size`). Each case lays a byte above `[0..15]` where the
/// reader's `small` or `sensorId` would be, and expects acceptance and a
/// read of 0.
#[test]
fn a_constrained_field_past_the_producer_size_reads_as_zero_in_verify() {
    let out = run(
        "podbuf_producer_size",
        &[
            // `Talls` with one element of `elem_size` 8: `big` at 16..24,
            // then eight 0xff bytes where `small` (offset 8) would be.
            r#"{
                let mut bytes = vec![8, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 1, 0, 8, 0];
                bytes.extend_from_slice(&[0; 8]);
                bytes.extend_from_slice(&[0xff; 8]);
                let buf = aligned(&bytes);
                match Talls::__ridl_podbuf_verify(&buf.0[..bytes.len()]) {
                    Ok(v) => format!("OK {}", v.items().map(|t| t.small()).sum::<i64>()),
                    Err(e) => verdict::<()>(Err(e)),
                }
            }"#,
            // `Outcome::Ok` with the arm descriptor zeroed (`len` 0): the
            // arm reads as all zero although the tail still holds the door
            // and the buffer starts with a preamble byte of 16.
            r#"{
                let bytes = patch_u32(patch_u32(enc!(Outcome::Ok(door(7, true))), 12, 0), 16, 0);
                let buf = aligned(&bytes);
                match Outcome::__ridl_podbuf_verify(&buf.0[..bytes.len()]) {
                    Ok(v) => match v.arm() {
                        OutcomePodbufArm::Ok(d) => format!("OK {} {}", d.sensor_id(), d.is_open()),
                        OutcomePodbufArm::Bad(h) => format!("OK {h:?}"),
                    },
                    Err(e) => verdict::<()>(Err(e)),
                }
            }"#,
            // `Tall` with an 8-byte head: `small` lies past it, over 0xff.
            r#"{
                let mut bytes = vec![8, 0, 0, 0, 0, 0, 0, 0];
                bytes.extend_from_slice(&[0; 8]);
                bytes.extend_from_slice(&[0xff; 8]);
                let buf = aligned(&bytes);
                match Tall::__ridl_podbuf_verify(&buf.0[..bytes.len()]) {
                    Ok(v) => format!("OK {} {}", v.big(), v.small()),
                    Err(e) => verdict::<()>(Err(e)),
                }
            }"#,
        ],
    );
    assert_eq!(out, ["OK 0", "OK 0 false", "OK 0 0"]);
}

/// §7 rule 9: a nested inline head, a struct arm, a map value, a present
/// optional and an array element are each checked with the same rules.
#[test]
fn rule_9_checks_nested_heads_arms_entries_optionals_and_elements() {
    let out = run(
        "podbuf_rule_9",
        &[
            // `Nested.door.isOpen` at head offset 1 -> byte 9.
            r#"check!(Nested, patch(enc!(Nested { door: door(3, false), health: Health::Fail, span: NestedSpan { lo: 1, hi: 2 }, readings: [1, 2] }), 9, 2))"#,
            // The `Ok` arm's head at 24: `isOpen` at byte 25.
            r#"check!(Outcome, patch(enc!(Outcome::Ok(door(7, true))), 25, 2))"#,
            // One entry at 16: the key's descriptor at 16..24, the `Score`
            // byte at 24; 200 is above 100.
            r#"check!(Ranked, patch(enc!(Ranked { by_name: vec![(Name::new_unchecked("a".into()), Score::new_unchecked(1))] }), 24, 200))"#,
            // The present `category` is `x ` at 24: a byte that is not
            // UTF-8.
            r#"check!(DiagFilter, patch(enc!(DiagFilter { severity: 2, category: Some("x".into()) }), 24, 0xff))"#,
            // The one element's string `a ` at 40.
            r#"check!(FaultPage, patch(enc!(FaultPage { faults: vec![ev(1, "a", 0)] }), 40, 0xff))"#,
        ],
    );
    assert_eq!(out, ["Contract", "Contract", "Contract", "Utf8", "Utf8"]);
}

/// §7 rules 4 and 5 on a block's placement: a block or an arm at an offset
/// that is not a multiple of its alignment, an empty array whose
/// descriptor is not all zero, and a map entry stride that is not a
/// multiple of 8.
#[test]
fn rule_4_and_5_block_placement_is_checked() {
    let out = run(
        "podbuf_block_placement",
        &[
            // The element's 9-byte string leaves room for the 24-byte block
            // at 20 instead of 16: in bounds, not 8-aligned.
            r#"check!(FaultPage, patch_u32(enc!(FaultPage { faults: vec![ev(1, "abcdefgh", 0)] }), 8, 20))"#,
            // `Wrapped`: the arm at 40..48 of 48 bytes, moved to 36 (above
            // the tail start 24, in bounds, not 8-aligned).
            r#"check!(Wrapped, patch_u32(enc!(Wrapped { label: "ab".into(), outcome: Outcome::Bad(Health::Warn) }), 20, 36))"#,
            // An empty array whose `elem_size` is 8.
            r#"check!(FaultPage, patch_u16(enc!(FaultPage { faults: Vec::new() }), 14, 8))"#,
            // A map entry stride of 12.
            r#"check!(Labels, patch_u16(enc!(Labels { by_name: map(&[("a", 1)]) }), 14, 12))"#,
        ],
    );
    assert_eq!(out, ["Unaligned", "Unaligned", "OutOfBounds", "Stride"]);
}

/// §14 "Generated Rust": `verify` and the in-place reads allocate nothing.
/// The program installs a counting global allocator and asserts zero
/// allocations across `verify` and every accessor of the view.
#[test]
fn verify_allocates_nothing() {
    let generated = generated();
    let main = r#"
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() {
    let pages = enc!(FaultPage { faults: vec![ev(1, "a", 0), ev(2, "bc", -1)] });
    let labels = enc!(Labels { by_name: map(&[("b", 2), ("a", 1)]) });
    let nested = enc!(Nested { door: door(3, true), health: Health::Warn, span: NestedSpan { lo: 1, hi: 2 }, readings: [10, 11] });
    let outcome = enc!(Outcome::Ok(door(7, true)));
    let filter = enc!(DiagFilter { severity: 2, category: Some("x".into()) });
    let pages_buf = aligned(&pages);
    let labels_buf = aligned(&labels);
    let nested_buf = aligned(&nested);
    let outcome_buf = aligned(&outcome);
    let filter_buf = aligned(&filter);
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    let mut sink = 0i64;
    let v = FaultPage::__ridl_podbuf_verify(&pages_buf.0[..pages.len()]).expect("verifies");
    for f in v.faults() {
        sink += f.code() + f.timestamp() + f.message().len() as i64;
    }
    let v = Labels::__ridl_podbuf_verify(&labels_buf.0[..labels.len()]).expect("verifies");
    for (k, w) in v.by_name() {
        sink += k.len() as i64 + w.get();
    }
    let v = Nested::__ridl_podbuf_verify(&nested_buf.0[..nested.len()]).expect("verifies");
    sink += v.door().sensor_id() + i64::from(v.door().is_open()) + i64::from(v.health());
    sink += v.span().lo() + v.span().hi() + v.readings().sum::<i64>();
    let v = Outcome::__ridl_podbuf_verify(&outcome_buf.0[..outcome.len()]).expect("verifies");
    sink += match v.arm() {
        OutcomePodbufArm::Ok(d) => d.sensor_id(),
        OutcomePodbufArm::Bad(h) => i64::from(h),
    };
    let v = DiagFilter::__ridl_podbuf_verify(&filter_buf.0[..filter.len()]).expect("verifies");
    sink += v.severity() + v.category().map_or(0, |c| c.len() as i64);
    let after = ALLOCATIONS.load(Ordering::SeqCst);
    std::hint::black_box(sink);
    assert_eq!(after - before, 0, "verify or an accessor allocated");
}
"#;
    rustc::run_program(
        "podbuf_no_alloc",
        &format!(
            "#![allow(dead_code, deprecated, unused_imports, unused_macros)]\n{generated}\n\
             {HELPERS}\n{main}"
        ),
    );
}
