//! The proto3 codec round trip: the codec's own `Done when`.
//!
//! `generate_pipeline` emits a `Payload<Proto3>` implementation per
//! declaration that projects a type. This target compiles that output as a
//! program and runs it, because a compile proof shows only that the codec
//! type-checks: whether the bytes `encode` writes are the bytes `verify`
//! accepts and `decode` reads back is something only a run can say. Each
//! program prints the encodings it makes as hexadecimal, so a case can pin
//! the bytes the writer produces and not only the value that comes back.
//!
//! Conformance against `prost` is a separate target; this one runs the codec
//! against itself.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::WireEncoding;

/// The generated source for the fixture, with a harness `main` appended.
fn program(main: &str) -> String {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    program_over(&package, main)
}

fn program_over(package: &ridl_ir::v2::Package, main: &str) -> String {
    let generated = ridl_backend_rust::generate_pipeline(package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    format!("#![allow(dead_code)]\n{generated}\n{PRELUDE}\n{main}")
}

/// What every program below shares: the imports, a hexadecimal printer, and a
/// generic round trip that encodes into a buffer of `MAX_SIZE`, prints the
/// bytes, verifies them, decodes them, and compares.
const PRELUDE: &str = r#"
use ridl_rt::encoding::Proto3;
// Each case reads the error types it needs; the program is built under
// `-D warnings`, so the others are allowed.
#[allow(unused_imports)]
use ridl_rt::payload::{Payload, Ref, VerifyError, Malformed, EncodeError};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Encodes `value`, prints `<name>=<hex>`, verifies, decodes, and asserts
/// the decoded value equals `value`. Returns the bytes.
fn round_trip<T: Payload<Proto3> + PartialEq + std::fmt::Debug>(name: &str, value: &T) -> Vec<u8> {
    let mut out = vec![0u8; <T as Payload<Proto3>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes.to_vec();
    println!("{name}={}", hex(&bytes));
    assert!(bytes.len() <= <T as Payload<Proto3>>::MAX_SIZE, "{name}: inside MAX_SIZE");
    let proof: Ref<'_, T, Proto3> = Ref::verify(&bytes).expect("verify");
    assert_eq!(proof.bytes(), &bytes[..], "{name}: the proof carries the bytes");
    let back = proof.decode();
    assert_eq!(&back, value, "{name}: the value survives the round trip");
    bytes
}

fn inner(speed: i64, label: &str) -> Inner {
    Inner {
        speed: Count::new_unchecked(speed),
        label: Label::new_unchecked(String::from(label)),
    }
}

fn scalars() -> Scalars {
    Scalars {
        unsigned8: 200,
        unsigned16: 60000,
        unsigned32: 4000000000,
        unsigned64: 9223372036854775807,
        signed8: -100,
        signed16: -30000,
        signed32: -2000000000,
        signed64: -4294967296,
        plain: -7,
        narrow: 12.5,
        broad: -0.75,
        real: 1.0e300,
        flag: true,
        text: String::from("caf\u{e9} \u{1F697}"),
        data: vec![0, 255, 7],
        count: Count::new_unchecked(7),
        ratio: Ratio::new_unchecked(100.5),
        label: Label::new_unchecked(String::from("label")),
        blob: Blob::new_unchecked(vec![9, 8]),
        engaged: Engaged::new(true),
        health: Health::Warn,
        gear: Gear::Second,
        trend: Trend::Down,
        flags: WarningFlags::HIGH,
    }
}
"#;

/// Every scalar kind the codec carries at a field position, with a
/// multi-byte character in the string and a negative enum value.
#[test]
fn a_struct_of_every_scalar_round_trips() {
    let main = r#"
fn main() {
    round_trip("scalars", &scalars());
    // A second value with the other sign of every signed field, so that the
    // zigzag encoding is exercised in both directions.
    let mut other = scalars();
    other.signed8 = 100;
    other.signed16 = 30000;
    other.signed32 = 2000000000;
    other.signed64 = 4294967296;
    other.plain = 7;
    other.trend = Trend::Up;
    other.gear = Gear::First;
    round_trip("other", &other);
}
"#;
    rustc::run_program("pb_round_trip", &program(main));
}

/// An optional field is written when present at any value, including the
/// proto3 default, and omitted when absent: the two forms decode to `Some`
/// and `None`.
#[test]
fn optional_fields_round_trip_present_at_zero_and_absent() {
    let main = r#"
fn main() {
    let at_zero = Optionals {
        count: Some(Count::new_unchecked(0)),
        label: Some(Label::new_unchecked(String::new())),
        flag: Some(false),
        health: Some(Health::Ok),
        inner: Some(inner(0, "")),
    };
    let bytes = round_trip("at_zero", &at_zero);
    assert!(!bytes.is_empty(), "a present optional at its default is written");

    let absent = Optionals { count: None, label: None, flag: None, health: None, inner: None };
    let bytes = round_trip("absent", &absent);
    assert!(bytes.is_empty(), "an absent optional is not written");

    let present = Optionals {
        count: Some(Count::new_unchecked(9)),
        label: Some(Label::new_unchecked(String::from("x"))),
        flag: Some(true),
        health: Some(Health::Fail),
        inner: Some(inner(3, "in")),
    };
    round_trip("present", &present);
}
"#;
    rustc::run_program("pb_optionals", &program(main));
}

/// A nested struct is a length-delimited record, written even when its own
/// encoding is empty, and an absent one decodes as the message read from zero
/// bytes.
#[test]
fn a_nested_struct_round_trips() {
    let main = r#"
fn main() {
    round_trip("nested", &Nested { inner: inner(5, "five"), count: Count::new_unchecked(1) });
    // The nested struct at its default is a zero-length record, so the
    // encoding is its tag and a zero length, then nothing for `count`.
    let bytes = round_trip("nested_zero", &Nested { inner: inner(0, ""), count: Count::new_unchecked(0) });
    assert_eq!(hex(&bytes), "0a00", "a non-optional nested struct is always written");
    // A buffer with no record for `inner` reads it as the message decoded
    // from zero bytes.
    let proof: Ref<'_, Nested, Proto3> = Ref::verify(&[0x10, 0x02]).expect("verify");
    assert_eq!(proof.decode(), Nested { inner: inner(0, ""), count: Count::new_unchecked(2) });
    // `Inner` is a payload of its own.
    round_trip("inner", &inner(7, "seven"));
}
"#;
    rustc::run_program("pb_nested", &program(main));
}

/// A named scalar, an enum and an enum set are rooted in a box message with
/// one field, `value = 1`, and each is a payload of its own.
#[test]
fn each_box_root_round_trips() {
    let main = r#"
fn main() {
    let bytes = round_trip("count", &Count::new_unchecked(150));
    assert_eq!(hex(&bytes), "089601", "a box writes its value at field 1");
    round_trip("label", &Label::new_unchecked(String::from("box")));
    round_trip("blob", &Blob::new_unchecked(vec![1, 2]));
    round_trip("ratio", &Ratio::new_unchecked(2.5));
    round_trip("engaged", &Engaged::new(true));
    round_trip("health", &Health::Fail);
    round_trip("gear", &Gear::First);
    let bytes = round_trip("trend", &Trend::Down);
    assert_eq!(hex(&bytes), "08ffffffffffffffffff01", "a negative enum value is a ten-byte varint");
    round_trip("flags", &WarningFlags::HIGH);
    let empty = round_trip("flags_empty", &WarningFlags::try_from(0i64).expect("the empty set"));
    assert!(empty.is_empty(), "an empty set is a box of 0");
    // `LOW` is bit 0, the value 1, and is written.
    assert_eq!(hex(&round_trip("flags_low", &WarningFlags::LOW)), "0801");
}
"#;
    rustc::run_program("pb_box_roots", &program(main));
}

/// Fields are written in field-number order whatever their declaration
/// order: the fixture's `Nested` is reordered in the IR so that its second
/// field is declared first, and the encoding still starts with the tag of
/// field 1.
#[test]
fn fields_are_written_in_number_order() {
    let mut package = ir::compile_fixture("proto3_roundtrip.ridl");
    let nested = package
        .decls
        .iter_mut()
        .find(|decl| decl.name == "Nested")
        .expect("the fixture declares `Nested`");
    let Some(ridl_ir::v2::decl::Kind::StructDef(def)) = nested.kind.as_mut() else {
        panic!("`Nested` is a struct");
    };
    // `inner` is ordinal 1 and `count` ordinal 2; reverse the declaration
    // order and keep the ordinals, so that the field with number 2 is
    // declared first. An emitter that wrote fields in declaration order
    // would start with `count`'s tag `10`.
    def.members.reverse();
    let ordinals: Vec<u32> = def
        .members
        .iter()
        .filter_map(|member| match member.member.as_ref() {
            Some(ridl_ir::v2::struct_member::Member::Field(field)) => Some(field.ordinal),
            _ => None,
        })
        .collect();
    assert_eq!(
        ordinals,
        [2, 1],
        "the fixture now declares ordinal 2 before 1"
    );
    let main = r#"
fn main() {
    let value = Nested { inner: inner(5, "five"), count: Count::new_unchecked(3) };
    let bytes = round_trip("reordered", &value);
    // `inner` carries number 1 and is written first, as the record
    // `0a <len> (08 05) (12 04 "five")`, then `count` at number 2: `10 03`.
    assert_eq!(hex(&bytes), "0a0808051204666976651003");
}
"#;
    rustc::run_program("pb_number_order", &program_over(&package, main));
}

/// An implicit-presence field at its proto3 default is not written, so a
/// value whose every field is at a legal default encodes to nothing, and a
/// box of 0 is the empty message. An empty buffer decodes back to it.
#[test]
fn a_field_at_its_default_is_not_written() {
    let main = r#"
fn main() {
    let zero = Zeroable {
        count: Count::new_unchecked(0),
        flag: false,
        label: Label::new_unchecked(String::new()),
        blob: Blob::new_unchecked(Vec::new()),
        health: Health::Ok,
        flags: WarningFlags::try_from(0i64).expect("the empty set"),
        real: 0.0,
    };
    let bytes = round_trip("zero", &zero);
    assert!(bytes.is_empty(), "every field at its default encodes to nothing, got {}", hex(&bytes));
    let proof: Ref<'_, Zeroable, Proto3> = Ref::verify(&[]).expect("the empty buffer verifies");
    assert_eq!(proof.decode(), zero);

    assert!(round_trip("count_zero", &Count::new_unchecked(0)).is_empty(), "a box of 0 is empty");
    assert!(round_trip("health_zero", &Health::Ok).is_empty(), "a box of the zero member is empty");
    assert!(round_trip("label_empty", &Label::new_unchecked(String::new())).is_empty());

    // A field whose type excludes 0 is always written, and refused when
    // absent.
    let required = Required { level: Level::new_unchecked(1), gear: Gear::First };
    let bytes = round_trip("required", &required);
    assert_eq!(hex(&bytes), "08011001", "a field that is never at its default is always written");
    assert_eq!(
        <Required as Payload<Proto3>>::verify(&[0x10, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "an absent field of a type that excludes 0",
    );
    assert_eq!(
        <Required as Payload<Proto3>>::verify(&[0x08, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "an absent field of an enum with no zero member",
    );
    assert_eq!(
        <Level as Payload<Proto3>>::verify(&[]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "an empty box of a type that excludes 0",
    );
}
"#;
    rustc::run_program("pb_defaults", &program(main));
}

/// A float is compared by its bits, so `-0.0` is written and `+0.0` is not;
/// the written `-0.0` keeps its sign through the round trip.
#[test]
fn minus_zero_is_written() {
    let main = r#"
fn main() {
    let bytes = round_trip("minus_zero", &Signs { ignored: 0, value: -0.0 });
    assert_eq!(hex(&bytes), "110000000000000080");
    let back: Signs = Ref::<'_, Signs, Proto3>::verify(&bytes).expect("verify").decode();
    assert!(back.value.is_sign_negative(), "the sign of zero survives");
    let bytes = round_trip("plus_zero", &Signs { ignored: 0, value: 0.0 });
    assert!(bytes.is_empty(), "+0.0 is the default and is omitted");
}
"#;
    rustc::run_program("pb_minus_zero", &program(main));
}

/// `encode` sizes the value first and reports the exact length it needs when
/// the buffer is one byte short, having written nothing it would then have
/// to undo.
#[test]
fn encode_reports_the_exact_capacity_needed() {
    let main = r#"
fn main() {
    let value = scalars();
    let len = round_trip("scalars", &value).len();
    let mut short = vec![0u8; len - 1];
    assert_eq!(
        value.encode(&mut short).err(),
        Some(EncodeError::Capacity { needed: len, available: len - 1 }),
    );
    let mut exact = vec![0u8; len];
    assert_eq!(value.encode(&mut exact).expect("an exact buffer fits").bytes.len(), len);
    let nested = Nested { inner: inner(5, "five"), count: Count::new_unchecked(1) };
    let len = round_trip("nested", &nested).len();
    let mut short = vec![0u8; len - 1];
    assert_eq!(
        nested.encode(&mut short).err(),
        Some(EncodeError::Capacity { needed: len, available: len - 1 }),
    );
}
"#;
    rustc::run_program("pb_capacity", &program(main));
}

/// The writer fills the buffer forwards, so `Encoded.bytes` is a prefix of
/// the output buffer, and the bytes after it are untouched.
#[test]
fn the_encoded_bytes_are_a_prefix_of_the_buffer() {
    let main = r#"
fn main() {
    let value = scalars();
    let mut out = vec![0xABu8; <Scalars as Payload<Proto3>>::MAX_SIZE];
    let encoded = value.encode(&mut out).expect("encode");
    let len = encoded.bytes.len();
    let bytes = encoded.bytes.to_vec();
    assert_eq!(&out[..len], &bytes[..], "the encoding is a prefix of the buffer");
    assert!(out[len..].iter().all(|byte| *byte == 0xAB), "the rest of the buffer is untouched");
    // And the same value encodes to the same bytes into a zeroed buffer.
    let mut zeroed = vec![0u8; <Scalars as Payload<Proto3>>::MAX_SIZE];
    assert_eq!(value.encode(&mut zeroed).expect("encode").bytes, &bytes[..]);
}
"#;
    rustc::run_program("pb_prefix", &program(main));
}

/// Each emitted `MAX_SIZE` is the proto3 bound the codegen model carries for
/// the declaration, and the largest legal value of a type fits in it.
#[test]
fn max_size_is_the_shared_bound() {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let mut pins = String::new();
    let mut checks = String::new();
    for decl in &model.declarations {
        let name = decl
            .name
            .as_ref()
            .map(|name| name.declared.as_str())
            .unwrap_or_default();
        if matches!(name, "WithList" | "ReachesList") {
            continue;
        }
        let Some(ridl_ir::codegen::v1::size_state::State::Bounded(bound)) =
            decl.proto3.as_ref().and_then(|state| state.state.as_ref())
        else {
            panic!("`{name}` has a proto3 bound in the model");
        };
        pins.push_str(&format!(
            "    assert_eq!(<{name} as Payload<Proto3>>::MAX_SIZE, {bound}usize, \"{name}\");\n"
        ));
        checks.push_str(&format!(
            "    assert_eq!(<{name} as Payload<Proto3>>::BUFFER.len(), {bound}usize, \"{name}\");\n"
        ));
    }
    assert!(!pins.is_empty(), "the model carries declarations");
    let main = format!(
        r#"
fn main() {{
{pins}
{checks}
    // The widest `Inner`: a sixteen-character label at four UTF-8 bytes a
    // character, and the largest `Count`.
    let widest = "\u{{10348}}".repeat(16);
    let value = Inner {{ speed: Count::new_unchecked(200), label: Label::new_unchecked(widest) }};
    let mut out = vec![0u8; <Inner as Payload<Proto3>>::MAX_SIZE];
    value.encode(&mut out).expect("the bound holds the largest value");
    // The widest `Scalars`: every varint at its widest, the string at
    // sixteen four-byte characters, the bytes at eight.
    let mut value = scalars();
    value.unsigned8 = 255;
    value.unsigned16 = 65535;
    value.unsigned32 = 4294967295;
    value.unsigned64 = 9223372036854775807;
    value.signed8 = -128;
    value.signed16 = -32768;
    value.signed32 = -2147483648;
    value.signed64 = -4294967296;
    value.plain = i64::MIN;
    value.text = "\u{{10348}}".repeat(16);
    value.data = vec![0xFF; 8];
    value.label = Label::new_unchecked("\u{{10348}}".repeat(16));
    value.blob = Blob::new_unchecked(vec![0xFF; 8]);
    value.trend = Trend::Down;
    let mut out = vec![0u8; <Scalars as Payload<Proto3>>::MAX_SIZE];
    value.encode(&mut out).expect("the bound holds the largest value");
}}
"#
    );
    rustc::run_program("pb_max_size", &program(&main));
}

/// A declaration whose proto3 state is not bounded is refused with an error
/// naming the declaration and the cause, for the declaration itself and for
/// one that reaches it.
#[test]
fn a_declaration_without_a_proto3_bound_is_refused() {
    let package = ir::compile_fixture("proto3_unbounded.ridl");
    let err = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect_err("a declaration with no proto3 bound is refused");
    assert!(
        err.message.contains("`pb.unbounded.Wide`") && err.message.contains("proto3"),
        "the refusal names the declaration and the encoding, got: {}",
        err.message
    );
    assert!(
        err.message.contains("refuse") || err.message.contains("int32"),
        "the refusal names the cause, got: {}",
        err.message
    );
    // The same package generates with no encoding, and with FlatBuffers
    // alone: the refusal is the proto3 codec's.
    ridl_backend_rust::generate_pipeline(&package, &[], &[]).expect("the types alone generate");
}

/// A declaration whose shape reaches a collection, a tuple or a union gets no
/// `Payload<Proto3>` yet and draws no error: the package still generates.
#[test]
fn a_declaration_that_reaches_a_collection_is_not_emitted_yet() {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let source = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    for name in ["WithList", "ReachesList"] {
        assert!(
            !source.contains(&format!("Payload<::ridl_rt::encoding::Proto3> for {name} ")),
            "`{name}` reaches an array and is not emitted yet"
        );
        assert!(
            !source.contains(&format!("__ridl_pb_len_{name}(")),
            "`{name}` reaches an array and has no functions yet"
        );
    }
    assert!(
        source.contains("Payload<::ridl_rt::encoding::Proto3> for Nested "),
        "every other declaration is emitted"
    );
    // With both encodings, both codecs are emitted side by side.
    let both = ridl_backend_rust::generate_pipeline(
        &package,
        &[WireEncoding::FlatBuffers, WireEncoding::Proto3],
        &[],
    )
    .expect("both codecs generate")
    .rust_source;
    assert!(both.contains("Payload<::ridl_rt::encoding::FlatBuffers> for Nested "));
    assert!(both.contains("Payload<::ridl_rt::encoding::Proto3> for Nested "));
}

/// `verify` never panics on any input: every byte string up to a few bytes,
/// and a set of longer shapes a fuzzer would reach first, is refused or
/// accepted and never aborts the program. An accepted buffer also decodes.
#[test]
fn verify_is_total_over_arbitrary_bytes() {
    let main = r#"
fn probe<T: Payload<Proto3>>(buf: &[u8]) {
    if let Ok(proof) = Ref::<'_, T, Proto3>::verify(buf) {
        let _ = proof.decode();
    }
}

fn main() {
    let mut buf = Vec::new();
    for len in 0..=2usize {
        let count = 1usize << (8 * len);
        for n in 0..count {
            buf.clear();
            for i in 0..len {
                buf.push((n >> (8 * i)) as u8);
            }
            probe::<Scalars>(&buf);
            probe::<Nested>(&buf);
            probe::<Optionals>(&buf);
            probe::<Count>(&buf);
            probe::<Label>(&buf);
            probe::<Health>(&buf);
        }
    }
    // Longer shapes: a truncated length, a length past the end, an
    // over-wide varint, a wrong wire type for a known field, a duplicate.
    for bytes in [
        &[0x72, 0x05, 0x61][..],
        &[0x72, 0xff, 0xff, 0xff, 0xff, 0x0f][..],
        &[0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02][..],
        &[0x08, 0x80, 0x80, 0x80, 0x80, 0x10][..],
        &[0x09, 1, 2, 3, 4, 5, 6, 7, 8][..],
        &[0x08, 0x01, 0x08, 0x01][..],
        &[0x0a, 0x02, 0x08, 0x01, 0x0a, 0x00][..],
    ] {
        probe::<Scalars>(bytes);
        probe::<Nested>(bytes);
        probe::<Optionals>(bytes);
    }
    // The refusals that fall out of the field walk.
    assert_eq!(
        <Scalars as Payload<Proto3>>::verify(&[0x08, 0x01, 0x08, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::Duplicate)),
        "a singular field written twice",
    );
    assert_eq!(
        <Scalars as Payload<Proto3>>::verify(&[0x09, 1, 2, 3, 4, 5, 6, 7, 8]).err(),
        Some(VerifyError::Structure(Malformed::WireType)),
        "a known field with a wire type its type does not take",
    );
    assert_eq!(
        <Scalars as Payload<Proto3>>::verify(&[0x08, 0x80, 0x80, 0x80, 0x80, 0x10]).err(),
        Some(VerifyError::Structure(Malformed::Varint)),
        "a uint32 above u32::MAX",
    );
    // 300 fits the wire scalar but not the declared `[0..255]`.
    assert!(matches!(
        <Scalars as Payload<Proto3>>::verify(&[0x08, 0xac, 0x02]).err(),
        Some(VerifyError::Contract(_)),
    ), "a varint outside the declared range");
    // An unknown field number of each wire type is skipped.
    let proof = <Nested as Payload<Proto3>>::verify(&[
        0xf8, 0x07, 0x01, // field 127, varint
        0x81, 0x08, 1, 2, 3, 4, 5, 6, 7, 8, // field 128, i64
        0x8a, 0x08, 0x01, 0xff, // field 129, len
        0x95, 0x08, 1, 2, 3, 4, // field 130, i32
        0x10, 0x05, // count = 5
    ]).expect("unknown fields are skipped");
    let _ = proof;
}
"#;
    rustc::run_program("pb_total", &program(main));
}

/// The emitted codec checks for `wasm32-unknown-unknown`, which is what
/// `just wasm-check` owes over generated code and cannot reach: the codec is
/// text with no manifest.
#[test]
fn the_generated_proto3_codec_checks_for_wasm32() {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let generated = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    let source = format!("#![allow(dead_code)]\n{generated}");
    assert!(
        rustc::check_for_wasm32("pb_wasm32", &source),
        "the wasm32-unknown-unknown target must be installed for this check to \
         mean anything; `just wasm-check` adds it with rustup, and so does this \
         test when rustup is present"
    );
}

/// The layout the emitter exposes to the drift test: one entry per emitted
/// message, keyed by the `.proto` message name, with the box roots named
/// `<Name>Box`.
#[test]
fn the_layout_names_each_message_and_its_fields() {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let layout = ridl_backend_rust::proto3_layout(&package, &[]);
    let nested = layout
        .iter()
        .find(|message| message.name == "Nested")
        .expect("`Nested` is laid out");
    assert_eq!(nested.fields.len(), 2);
    assert_eq!(nested.fields[0].0, 1);
    assert_eq!(nested.fields[0].1, "Inner");
    assert_eq!(
        nested.fields[1],
        (
            2,
            "uint32".to_string(),
            ridl_backend_rust::Proto3Label::Implicit
        )
    );
    let count = layout
        .iter()
        .find(|message| message.name == "CountBox")
        .expect("`CountBox` is laid out");
    assert_eq!(
        count.fields,
        vec![(
            1,
            "uint32".to_string(),
            ridl_backend_rust::Proto3Label::Implicit
        )]
    );
    let optionals = layout
        .iter()
        .find(|message| message.name == "Optionals")
        .expect("`Optionals` is laid out");
    assert!(
        optionals
            .fields
            .iter()
            .all(|field| field.2 == ridl_backend_rust::Proto3Label::Optional)
    );
    assert!(!layout.iter().any(|message| message.name == "WithList"));
}
