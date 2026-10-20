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

/// A declaration one of whose members the proto backend refuses — an enum
/// value outside int32 — has no proto3 message, so it is still refused with
/// an error naming the declaration and the cause, for the declaration itself
/// and for one that reaches it. A missing bound alone is not a refusal
/// ([`an_unbounded_type_gets_a_codec_without_a_size`]).
#[test]
fn a_refused_member_is_still_refused() {
    let package = ir::compile_fixture("proto3_unbounded.ridl");
    let err = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect_err("a declaration with a refused member is refused");
    assert!(
        err.message.contains("`pb.unbounded.Wide`") && err.message.contains("proto3"),
        "the refusal names the declaration and the encoding, got: {}",
        err.message
    );
    assert_eq!(
        err.message,
        "`pb.unbounded.Wide` has no proto3 codec: its value `BIG` is 4294967296, outside int32, \
         which the proto backend refuses",
        "the refusal names the value and the cause"
    );
    // The same package generates with no encoding, and with FlatBuffers
    // alone: the refusal is the proto3 codec's.
    ridl_backend_rust::generate_pipeline(&package, &[], &[]).expect("the types alone generate");
}

/// The generated source for `proto3_free.ridl` with a harness `main`
/// appended and no shared prelude, since the prelude's helpers name the
/// round-trip fixture's types.
fn free_program(main: &str) -> (String, String) {
    let package = ir::compile_fixture("proto3_free.ridl");
    let generated = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    let dense: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
    (format!("#![allow(dead_code)]\n{generated}\n{main}"), dense)
}

/// A type whose proto3 size state has no bound — a string with no length
/// bound, an array with no count bound, and a struct that reaches either —
/// gets a `Codec<Proto3>` with `BOUND = None` and no `Payload<Proto3>`,
/// while a bounded type of the same package keeps its `Payload<Proto3>`. The
/// codec round-trips through `Ref` into a buffer the caller sizes, and a
/// buffer too short is `Capacity { needed }` with the exact length.
#[test]
fn an_unbounded_type_gets_a_codec_without_a_size() {
    let main = r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{Codec, EncodeError, Payload, Ref};

fn main() {
    assert_eq!(<Free as Codec<Proto3>>::BOUND, None);
    assert_eq!(<Note as Codec<Proto3>>::BOUND, None);
    assert_eq!(<Measured as Codec<Proto3>>::BOUND, None);
    assert_eq!(<Count as Codec<Proto3>>::BOUND, Some(<Count as Payload<Proto3>>::MAX_SIZE));

    let value = Measured {
        count: Count::new_unchecked(7),
        note: Note {
            text: Free::new_unchecked(String::from("a text with no bound")),
            tags: vec![Free::new_unchecked(String::from("x")), Free::new_unchecked(String::new())],
        },
    };
    // Sized by the codec itself: the empty buffer reports the exact length.
    let needed = match Ref::<Measured, Proto3>::encode(&value, &mut []) {
        Err(EncodeError::Capacity { needed, available: 0 }) => needed,
        other => panic!("an empty buffer reports the length, got {:?}", other.map(|r| r.bytes().len())),
    };
    let mut out = vec![0xABu8; needed + 5];
    let proof = Ref::<Measured, Proto3>::encode(&value, &mut out).expect("the buffer holds it");
    let bytes = proof.bytes().to_vec();
    assert_eq!(bytes.len(), needed, "the length reported is the length written");
    // count = 7: tag 1 varint; note: tag 2 len, holding text (tag 1) and two
    // tag-2 records, the empty one written as a record of length 0.
    let expected = [
        &[0x08, 0x07, 0x12, 0x1b, 0x0a, 0x14][..],
        b"a text with no bound",
        &[0x12, 0x01, b'x', 0x12, 0x00][..],
    ]
    .concat();
    assert_eq!(bytes, expected);
    let back = Ref::<Measured, Proto3>::verify(&bytes).expect("verify").decode();
    assert_eq!(back, value);
    assert_eq!(
        Ref::<Measured, Proto3>::encode(&value, &mut out[..needed - 1]).err(),
        Some(EncodeError::Capacity { needed, available: needed - 1 })
    );

    // The box message of the named scalar, on its own.
    let free = Free::new_unchecked(String::from("hé"));
    let mut out = [0u8; 8];
    let proof = Ref::<Free, Proto3>::encode(&free, &mut out).expect("8 bytes is enough");
    assert_eq!(proof.bytes(), &[0x0a, 0x03, b'h', 0xc3, 0xa9]);
    assert_eq!(Ref::<Free, Proto3>::verify(&[0x0a, 0x03, b'h', 0xc3, 0xa9]).expect("verify").decode(), free);
}
"#;
    let (program, dense) = free_program(main);
    for name in ["Free", "Note", "Measured"] {
        assert!(
            dense.contains(&format!(
                "impl::ridl_rt::payload::Codec<::ridl_rt::encoding::Proto3>for{name}{{"
            )),
            "`{name}` carries a `Codec<Proto3>`"
        );
        assert!(
            !dense.contains(&format!(
                "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>for{name}{{"
            )),
            "`{name}` carries no `Payload<Proto3>`"
        );
    }
    assert!(
        dense.contains("impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>forCount{"),
        "`Count` keeps its `Payload<Proto3>`"
    );
    rustc::run_program("pb_free", &program);
}

/// An induced interaction root whose payload has no proto3 bound — the
/// argument message of a call with two parameters each near the `u32`
/// limit — gets a `Codec<Proto3>` with `BOUND = None` rather than a
/// refusal, while the package's declarations, which all have a bound, keep
/// their `Payload<Proto3>`. The interface gets a `Bounds<Proto3>` and no
/// `Sizes<Proto3>`, so nothing names the bound the root does not have.
#[test]
fn an_interaction_root_without_a_bound_gets_a_codec_without_a_size() {
    let package = ir::compile_fixture("proto3_unbounded_call.ridl");
    let generated = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("an interaction whose argument message has no proto3 bound generates")
        .rust_source;
    let dense: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        dense.contains(
            "impl::ridl_rt::payload::Codec<::ridl_rt::encoding::Proto3>forProbeSetParams{"
        ),
        "the induced root carries a `Codec<Proto3>`"
    );
    assert!(
        !dense.contains(
            "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>forProbeSetParams{"
        ),
        "the induced root carries no `Payload<Proto3>`"
    );
    for name in ["Count", "Big", "Inner"] {
        assert!(
            dense.contains(&format!(
                "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>for{name}{{"
            )),
            "`{name}` keeps its `Payload<Proto3>`"
        );
    }
    assert!(
        !dense.contains("Sizes<::ridl_rt::encoding::Proto3>"),
        "no proto3 size table is emitted"
    );
    assert!(
        dense.contains("impl::ridl_rt::contract::Bounds<::ridl_rt::encoding::Proto3>forProbe{"),
        "the interface states its rows as `Bounds<Proto3>`"
    );
    let layout = ridl_backend_rust::proto3_layout(&package, &[]).expect("the layout does not size");
    assert!(
        layout
            .iter()
            .any(|message| message.name == "ProbeSetParams")
    );
    ridl_backend_rust::generate_pipeline(&package, &[], &[]).expect("the types alone generate");
}

/// Every declaration that projects a type carries a `Payload<Proto3>`,
/// including one that holds or reaches a collection, and so does every
/// induced interaction tuple that crosses the wire on its own; a tuple
/// reached only from a field has the four functions and no `Payload`.
#[test]
fn every_declaration_and_interaction_root_is_emitted() {
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let source = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    for name in [
        "WithList",
        "ReachesList",
        "Outcome",
        "Tuples",
        "Lists",
        "Maps",
        "OptionalComposites",
        "ProbeSetBothParams",
        "ProbeStatusParams",
        "ProbeSpanReply",
    ] {
        assert!(
            source.contains(&format!("Payload<::ridl_rt::encoding::Proto3> for {name} ")),
            "`{name}` carries a `Payload<Proto3>`"
        );
    }
    for name in ["TuplesRange", "TuplesPair", "OptionalCompositesPair"] {
        assert!(
            !source.contains(&format!("Payload<::ridl_rt::encoding::Proto3> for {name} ")),
            "`{name}` is reached only from a field and is no root"
        );
        assert!(
            source.contains(&format!("__ridl_pb_len_{name}(")),
            "`{name}` has its functions, for the owner that encodes it"
        );
    }
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
    let layout = ridl_backend_rust::proto3_layout(&package, &[]).expect("the fixture lays out");
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
    // A collection is `Repeated`, named by its element's scalar or message,
    // or spelled as `map<K, V>`; a tuple field names the induced message.
    let lists = layout
        .iter()
        .find(|message| message.name == "Lists")
        .expect("`Lists` is laid out");
    assert_eq!(
        lists.fields[0],
        (
            1,
            "uint32".to_string(),
            ridl_backend_rust::Proto3Label::Repeated
        )
    );
    assert_eq!(
        lists.fields[5],
        (
            6,
            "Inner".to_string(),
            ridl_backend_rust::Proto3Label::Repeated
        )
    );
    assert_eq!(
        lists.fields[6],
        (
            7,
            "Outcome".to_string(),
            ridl_backend_rust::Proto3Label::Repeated
        )
    );
    assert_eq!(
        lists.fields.len(),
        9,
        "the reserved tombstone holds its number and lays out no field"
    );
    assert_eq!(lists.fields[8].0, 10);
    let maps = layout
        .iter()
        .find(|message| message.name == "Maps")
        .expect("`Maps` is laid out");
    assert_eq!(
        maps.fields,
        vec![
            (
                1,
                "map<string, uint32>".to_string(),
                ridl_backend_rust::Proto3Label::Repeated
            ),
            (
                2,
                "map<sint32, Inner>".to_string(),
                ridl_backend_rust::Proto3Label::Repeated
            ),
            (
                3,
                "map<bool, string>".to_string(),
                ridl_backend_rust::Proto3Label::Repeated
            ),
        ]
    );
    let tuples = layout
        .iter()
        .find(|message| message.name == "Tuples")
        .expect("`Tuples` is laid out");
    assert_eq!(tuples.fields[0].1, "TuplesRange");
    let range = layout
        .iter()
        .find(|message| message.name == "TuplesRange")
        .expect("the induced tuple message is laid out");
    assert_eq!(
        range.fields,
        vec![
            (
                1,
                "uint32".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
            (
                2,
                "uint32".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
        ]
    );
    let outcome = layout
        .iter()
        .find(|message| message.name == "Outcome")
        .expect("`Outcome` is laid out");
    assert_eq!(
        outcome.fields,
        vec![
            (
                1,
                "uint32".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
            (
                2,
                "string".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
            (
                3,
                "Health".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
            (
                4,
                "uint32".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
            (
                5,
                "Inner".to_string(),
                ridl_backend_rust::Proto3Label::Implicit
            ),
        ]
    );
    // An enum field is named by the enum, as the `.proto` names it; an enum
    // set is inlined as its scalar.
    let scalars = layout
        .iter()
        .find(|message| message.name == "Scalars")
        .expect("`Scalars` is laid out");
    let field = |name: &str| {
        let number = package
            .decls
            .iter()
            .find(|decl| decl.name == "Scalars")
            .and_then(|decl| match decl.kind.as_ref() {
                Some(ridl_ir::v2::decl::Kind::StructDef(def)) => {
                    def.members
                        .iter()
                        .find_map(|member| match member.member.as_ref() {
                            Some(ridl_ir::v2::struct_member::Member::Field(field))
                                if field.name == name =>
                            {
                                Some(field.ordinal)
                            }
                            _ => None,
                        })
                }
                _ => None,
            })
            .expect("the field is declared");
        scalars
            .fields
            .iter()
            .find(|field| field.0 == number)
            .map(|field| field.1.clone())
            .expect("the field is laid out")
    };
    assert_eq!(field("health"), "Health");
    assert_eq!(field("trend"), "Trend");
    assert_eq!(field("flags"), "uint32");
    let health = layout
        .iter()
        .find(|message| message.name == "HealthBox")
        .expect("`HealthBox` is laid out");
    assert_eq!(
        health.fields,
        vec![(
            1,
            "Health".to_string(),
            ridl_backend_rust::Proto3Label::Implicit
        )]
    );
}

/// A string or bytes field whose length lower bound excludes the empty
/// value is written even at its shortest legal value, and refused when
/// absent: the empty default is not a legal value of its type.
#[test]
fn a_field_whose_empty_value_is_illegal_is_written_and_required() {
    let main = r#"
fn main() {
    let value = Lengths {
        text: String::from("a"),
        code: Code::new_unchecked(String::from("ab")),
        data: vec![0],
    };
    let bytes = round_trip("lengths", &value);
    assert_eq!(hex(&bytes), "0a0161120261621a0100", "each field is written at its shortest legal value");
    assert_eq!(
        <Lengths as Payload<Proto3>>::verify(&[]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "every field absent",
    );
    // `code` alone absent: the named scalar's lower bound refuses the
    // empty default.
    assert_eq!(
        <Lengths as Payload<Proto3>>::verify(&[0x0a, 0x01, 0x61, 0x1a, 0x01, 0x00]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "the named string absent",
    );
    // `data` alone absent: the inline bytes bound refuses the empty default.
    assert_eq!(
        <Lengths as Payload<Proto3>>::verify(&[0x0a, 0x01, 0x61, 0x12, 0x02, 0x61, 0x62]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "the inline bytes absent",
    );
    // `text` alone absent: the inline string bound refuses the empty default.
    assert_eq!(
        <Lengths as Payload<Proto3>>::verify(&[0x12, 0x02, 0x61, 0x62, 0x1a, 0x01, 0x00]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "the inline string absent",
    );
    // An empty value that is present is refused by the length check, as a
    // contract violation rather than an absence.
    assert!(matches!(
        <Lengths as Payload<Proto3>>::verify(&[0x0a, 0x00, 0x12, 0x02, 0x61, 0x62, 0x1a, 0x01, 0x00]).err(),
        Some(VerifyError::Contract(_)),
    ), "a present empty string outside its bound");
}
"#;
    rustc::run_program("pb_lengths", &program(main));
}

/// A string field whose empty value is legal by length but which declares a
/// pattern: an absent field is checked as a present empty one, so under
/// `validate-pattern` both are refused by the pattern, and without the
/// feature both are accepted.
#[test]
fn an_absent_string_is_checked_against_its_pattern_as_an_empty_one() {
    let main = r#"
fn main() {
    let value = Patterned {
        tag: Tag::new_unchecked(String::from("ab")),
        text: String::from("cd"),
    };
    round_trip("patterned", &value);
    let absent = <Patterned as Payload<Proto3>>::verify(&[]).map(|_| ());
    // Both fields present and empty.
    let present = <Patterned as Payload<Proto3>>::verify(&[0x0a, 0x00, 0x12, 0x00]).map(|_| ());
    // Only the named `tag` absent, and only the inline `text` absent.
    let tag_absent = <Patterned as Payload<Proto3>>::verify(&[0x12, 0x02, 0x63, 0x64]).map(|_| ());
    let text_absent = <Patterned as Payload<Proto3>>::verify(&[0x0a, 0x02, 0x61, 0x62]).map(|_| ());
    if cfg!(feature = "validate-pattern") {
        for (name, verdict) in [("absent", absent), ("present", present), ("tag_absent", tag_absent), ("text_absent", text_absent)] {
            assert!(
                matches!(verdict, Err(VerifyError::Contract(v)) if v.rule == ridl_rt::payload::Rule::Pattern),
                "{name}: the pattern refuses the empty string, got {verdict:?}",
            );
        }
    } else {
        assert_eq!(absent, Ok(()));
        assert_eq!(present, Ok(()));
        assert_eq!(tag_absent, Ok(()));
        assert_eq!(text_absent, Ok(()));
    }
}
"#;
    let source = program(main);
    rustc::run_program("pb_pattern_off", &source);
    rustc::run_program_with_pattern("pb_pattern_on", &source, true);
}

/// The prelude items the collection cases share: a `Lists` value with every
/// array kind populated, a union value per arm, and the maps.
const COLLECTIONS: &str = r#"
fn count(v: i64) -> Count { Count::new_unchecked(v) }
fn label(s: &str) -> Label { Label::new_unchecked(String::from(s)) }

fn lists() -> Lists {
    Lists {
        counts: vec![count(1), count(2)],
        reals: vec![],
        flags: vec![true, false],
        healths: vec![Health::Ok, Health::Fail],
        labels: vec![label("")],
        inners: vec![inner(1, "a")],
        outcomes: vec![Outcome::Number(count(0))],
        three: [count(0); 3],
        at_least_one: vec![count(7)],
    }
}
"#;

/// Every collection, tuple and union shape round-trips, and the writer's
/// canonical form is pinned byte for byte: a numeric, boolean or enum array
/// is one packed record, any other array is one record per element, an
/// empty array is not written, a fixed-length array of zeros is still
/// written, a union writes its one arm even at the arm's default, a tuple
/// is a nested message with positional fields, and a map entry omits a key
/// or a value at its default.
#[test]
fn collections_tuples_and_unions_round_trip() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    let bytes = round_trip("lists", &lists());
    assert_eq!(
        hex(&bytes),
        "0a0201021a020100220200022a00320508011201613a0208004203000000520107",
        "packed runs, one record per element, the empty array omitted, the fixed array written"
    );
    let bytes = round_trip("lists_reals", &Lists { reals: vec![-0.5, 2.0], ..lists() });
    assert!(hex(&bytes).contains("1210000000000000e0bf0000000000000040"), "a packed double run: {}", hex(&bytes));

    // A union writes exactly its one arm, even at the arm's default.
    assert_eq!(hex(&round_trip("arm_number", &Outcome::Number(count(0)))), "0800");
    assert_eq!(hex(&round_trip("arm_text", &Outcome::Text(label("")))), "1200");
    assert_eq!(hex(&round_trip("arm_health", &Outcome::Health(Health::Ok))), "1800");
    assert_eq!(hex(&round_trip("arm_flags", &Outcome::Flags(WarningFlags::try_from(0i64).unwrap()))), "2000");
    assert_eq!(hex(&round_trip("arm_inner", &Outcome::Inner(inner(0, "")))), "2a00");
    assert_eq!(hex(&round_trip("arm_value", &Outcome::Inner(inner(3, "x")))), "2a050803120178");

    // A tuple is a nested message with positional fields numbered from 1.
    let tuples = Tuples {
        range: TuplesRange { min: count(1), max: count(2) },
        pair: TuplesPair { lo: count(0), inner: inner(0, "") },
    };
    assert_eq!(hex(&round_trip("tuples", &tuples)), "0a040801100212021200");

    // A map entry omits a key or a value at its default, and a struct value
    // whose own encoding is empty.
    let maps = Maps {
        by_label: vec![(label(""), count(0)), (label("a"), count(1))],
        by_int: vec![(-1, inner(0, "")), (3, inner(2, "b"))],
        by_bool: vec![(true, label("x"))],
    };
    assert_eq!(
        hex(&round_trip("maps", &maps)),
        "0a000a050a016110011202080112090806120508021201621a050801120178",
    );
    assert!(round_trip("maps_empty", &Maps { by_label: vec![], by_int: vec![], by_bool: vec![] }).is_empty());

    // The optional composites: present at any value, absent as `None`.
    let present = OptionalComposites {
        pair: Some(OptionalCompositesPair { lo: count(0), hi: count(0) }),
        outcome: Some(Outcome::Number(count(0))),
    };
    assert_eq!(hex(&round_trip("composites_present", &present)), "0a0012020800");
    assert!(round_trip("composites_absent", &OptionalComposites { pair: None, outcome: None }).is_empty());

    // The induced interaction roots are payloads of their own.
    assert_eq!(
        hex(&round_trip("set_both", &ProbeSetBothParams { level: Level::new_unchecked(2), count: count(0) })),
        "0802"
    );
    assert!(round_trip("status", &ProbeStatusParams {}).is_empty());
    assert_eq!(hex(&round_trip("span", &ProbeSpanReply { lo: count(1), hi: count(2) })), "08011002");
}
"#
    );
    rustc::run_program("pb_collections", &program(&main));
}

/// A repeated numeric field is read in packed form, unpacked form, or both
/// mixed, the elements concatenated in order; the writer's own form is the
/// packed one.
#[test]
fn packed_unpacked_and_mixed_runs_are_read() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    let expected = Lists {
        counts: vec![count(1), count(2), count(3)],
        reals: vec![], flags: vec![], healths: vec![], labels: vec![], inners: vec![], outcomes: vec![],
        three: [count(0); 3],
        at_least_one: vec![count(7)],
    };
    let tail: &[u8] = &[0x42, 0x03, 0, 0, 0, 0x52, 0x01, 0x07];
    for (name, head) in [
        ("packed", &[0x0a, 0x03, 0x01, 0x02, 0x03][..]),
        ("unpacked", &[0x08, 0x01, 0x08, 0x02, 0x08, 0x03][..]),
        ("mixed", &[0x08, 0x01, 0x0a, 0x02, 0x02, 0x03][..]),
        ("packed_twice", &[0x0a, 0x01, 0x01, 0x0a, 0x02, 0x02, 0x03][..]),
    ] {
        let mut buf = head.to_vec();
        buf.extend_from_slice(tail);
        let proof: Ref<'_, Lists, Proto3> = Ref::verify(&buf).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(proof.decode(), expected, "{name}");
    }
    // A double run whose length is not a multiple of eight.
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x12, 0x03, 0, 0, 0, 0x42, 0x03, 0, 0, 0, 0x52, 0x01, 0x07]).err(),
        Some(VerifyError::Structure(Malformed::OutOfBounds)),
    );
    // A record array under a wire type its element does not take.
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x28, 0x01, 0x42, 0x03, 0, 0, 0, 0x52, 0x01, 0x07]).err(),
        Some(VerifyError::Structure(Malformed::WireType)),
    );
}
"#
    );
    rustc::run_program("pb_packed_forms", &program(&main));
}

/// An empty array is not written, and an absent array whose count lower
/// bound is 0 reads as empty; one whose lower bound excludes 0 is refused
/// when absent.
#[test]
fn an_empty_array_is_not_written() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    let value = Lists {
        counts: vec![], reals: vec![], flags: vec![], healths: vec![], labels: vec![], inners: vec![], outcomes: vec![],
        three: [count(0); 3],
        at_least_one: vec![count(1)],
    };
    let bytes = round_trip("empty_arrays", &value);
    assert_eq!(hex(&bytes), "4203000000520101", "only the fixed array and the required one are written");
    // `at_least_one` absent.
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x42, 0x03, 0, 0, 0]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
    );
    // `three` absent.
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x52, 0x01, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
    );
    let empty = round_trip("with_list_empty", &WithList { items: vec![] });
    assert!(empty.is_empty());
    let proof: Ref<'_, WithList, Proto3> = Ref::verify(&[]).expect("an absent array reads as empty");
    assert_eq!(proof.decode(), WithList { items: vec![] });
}
"#
    );
    rustc::run_program("pb_empty_arrays", &program(&main));
}

/// A map with a key twice is refused on both sides: the encoder with
/// `EncodeError::Contract(Unique)`, the verifier with
/// `VerifyError::Contract(Unique)` over a hand-built buffer, including one
/// whose repeated key is the omitted default.
#[test]
fn a_map_with_a_repeated_key_is_refused_on_both_sides() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    use ridl_rt::payload::{Rule, Violation};
    let unique = Violation { type_name: Some("Maps"), rule: Rule::Unique };
    let twice = Maps {
        by_label: vec![(label("a"), count(1)), (label("a"), count(2))],
        by_int: vec![], by_bool: vec![],
    };
    let mut out = vec![0u8; <Maps as Payload<Proto3>>::MAX_SIZE];
    assert_eq!(twice.encode(&mut out).err(), Some(EncodeError::Contract(unique)));
    let twice = Maps {
        by_label: vec![],
        by_int: vec![(5, inner(0, "")), (5, inner(1, ""))], by_bool: vec![],
    };
    assert_eq!(twice.encode(&mut out).err(), Some(EncodeError::Contract(unique)));
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[0x0a, 0x05, 0x0a, 0x01, 0x61, 0x10, 0x01, 0x0a, 0x05, 0x0a, 0x01, 0x61, 0x10, 0x02]).err(),
        Some(VerifyError::Contract(unique)),
        "the same string key twice",
    );
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[0x0a, 0x00, 0x0a, 0x00]).err(),
        Some(VerifyError::Contract(unique)),
        "the omitted default key twice",
    );
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[0x12, 0x02, 0x08, 0x02, 0x12, 0x04, 0x08, 0x02, 0x12, 0x00]).err(),
        Some(VerifyError::Contract(unique)),
        "the same integer key twice",
    );
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[0x1a, 0x02, 0x08, 0x01, 0x1a, 0x00]).err(),
        None,
        "two different boolean keys",
    );
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[0x1a, 0x02, 0x08, 0x01, 0x1a, 0x02, 0x08, 0x01]).err(),
        Some(VerifyError::Contract(unique)),
        "the same boolean key twice",
    );
}
"#
    );
    rustc::run_program("pb_map_unique", &program(&main));
}

/// A collection longer than its declared count is refused by `encode` and
/// by `verify` with `Rule::Length`, for an array and for a map; a fixed
/// array is refused by `verify` at any other count.
#[test]
fn a_collection_over_its_count_is_refused() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    use ridl_rt::payload::{Rule, Violation};
    let length = Violation { type_name: Some("Lists"), rule: Rule::Length };
    let mut out = vec![0u8; 256];
    let long = Lists { counts: vec![count(1); 5], ..lists() };
    assert_eq!(long.encode(&mut out).err(), Some(EncodeError::Contract(length)));
    let long = Lists { labels: vec![label("a"); 4], ..lists() };
    assert_eq!(long.encode(&mut out).err(), Some(EncodeError::Contract(length)));
    let tail: &[u8] = &[0x42, 0x03, 0, 0, 0, 0x52, 0x01, 0x07];
    let mut buf = vec![0x0a, 0x05, 1, 2, 3, 4, 5];
    buf.extend_from_slice(tail);
    assert_eq!(<Lists as Payload<Proto3>>::verify(&buf).err(), Some(VerifyError::Contract(length)), "a packed run over the count");
    let mut buf = vec![0x08, 1, 0x08, 2, 0x08, 3, 0x08, 4, 0x08, 5];
    buf.extend_from_slice(tail);
    assert_eq!(<Lists as Payload<Proto3>>::verify(&buf).err(), Some(VerifyError::Contract(length)), "unpacked records over the count");
    let mut buf = vec![0x2a, 0x00, 0x2a, 0x00, 0x2a, 0x00, 0x2a, 0x00];
    buf.extend_from_slice(tail);
    assert_eq!(<Lists as Payload<Proto3>>::verify(&buf).err(), Some(VerifyError::Contract(length)), "string records over the count");
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x42, 0x02, 0, 0, 0x52, 0x01, 0x07]).err(),
        Some(VerifyError::Contract(length)),
        "a fixed array of another count",
    );
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x42, 0x03, 0, 0, 0, 0x52, 0x04, 1, 2, 3, 4]).err(),
        Some(VerifyError::Contract(length)),
        "a bounded array over its count",
    );

    let length = Violation { type_name: Some("Maps"), rule: Rule::Length };
    let long = Maps {
        by_label: vec![(label("a"), count(1)), (label("b"), count(1)), (label("c"), count(1)), (label("d"), count(1))],
        by_int: vec![], by_bool: vec![],
    };
    assert_eq!(long.encode(&mut out).err(), Some(EncodeError::Contract(length)));
    assert_eq!(
        <Maps as Payload<Proto3>>::verify(&[
            0x0a, 0x03, 0x0a, 0x01, 0x61, 0x0a, 0x03, 0x0a, 0x01, 0x62,
            0x0a, 0x03, 0x0a, 0x01, 0x63, 0x0a, 0x03, 0x0a, 0x01, 0x64,
        ]).err(),
        Some(VerifyError::Contract(length)),
        "four distinct entries in a map of at most three",
    );
}
"#
    );
    rustc::run_program("pb_counts", &program(&main));
}

/// An optional array element and an optional map value are refused at
/// generation, as the FlatBuffers codec refuses them: the schema emits
/// neither with presence, so a `None` would have no form on the wire.
#[test]
fn an_optional_array_element_is_refused_at_generation() {
    use ridl_ir::v2;
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let with_optional =
        |decl: &str, field: &str, pick: fn(&mut v2::FieldType) -> &mut v2::FieldType| {
            let mut package = package.clone();
            let decl = package
                .decls
                .iter_mut()
                .find(|candidate| candidate.name == decl)
                .expect("the fixture declares it");
            let Some(v2::decl::Kind::StructDef(def)) = decl.kind.as_mut() else {
                panic!("a struct");
            };
            let field = def
                .members
                .iter_mut()
                .find_map(|member| match member.member.as_mut() {
                    Some(v2::struct_member::Member::Field(candidate))
                        if candidate.name == field =>
                    {
                        Some(candidate)
                    }
                    _ => None,
                })
                .expect("the field is declared");
            pick(field.r#type.as_mut().expect("typed")).optional = true;
            package
        };
    let element = with_optional("WithList", "items", |ty| match ty.kind.as_mut() {
        Some(v2::field_type::Kind::Array(array)) => array.element.as_mut().expect("an element"),
        _ => panic!("an array"),
    });
    let err = ridl_backend_rust::generate_pipeline(&element, &[WireEncoding::Proto3], &[])
        .expect_err("an optional array element is refused");
    assert!(
        err.message.contains("an array element is optional"),
        "the refusal names the position, got: {}",
        err.message
    );
    ridl_backend_rust::generate_pipeline(&element, &[], &[]).expect("the types alone generate");

    let value = with_optional("Maps", "byLabel", |ty| match ty.kind.as_mut() {
        Some(v2::field_type::Kind::Map(map)) => map.value.as_mut().expect("a value"),
        _ => panic!("a map"),
    });
    let err = ridl_backend_rust::generate_pipeline(&value, &[WireEncoding::Proto3], &[])
        .expect_err("an optional map value is refused");
    assert!(
        err.message.contains("a map value is optional"),
        "the refusal names the position, got: {}",
        err.message
    );
}

/// Each induced interaction root — the argument message of a call with two
/// parameters, the empty argument message of a query with none, and the
/// reply message of a query that replies with a tuple — takes as `MAX_SIZE`
/// the bound the model carries for that interaction's payload.
#[test]
fn an_induced_interaction_root_takes_the_interactions_bound() {
    use ridl_ir::codegen::v1;
    let package = ir::compile_fixture("proto3_roundtrip.ridl");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let bound = |sizes: Option<&v1::PayloadSizes>| -> u32 {
        match sizes
            .and_then(|sizes| sizes.proto3.as_ref())
            .and_then(|state| state.state.as_ref())
        {
            Some(v1::size_state::State::Bounded(bound)) => *bound,
            other => panic!("the interaction's payload has a proto3 bound, got {other:?}"),
        }
    };
    let tuple_name = |reference: Option<&v1::TupleRef>| -> String {
        let reference = reference.expect("the interaction induces a tuple");
        model.tuples[reference.index as usize]
            .name
            .as_ref()
            .map(|name| name.rust.clone())
            .expect("the tuple is named")
    };
    let mut pins: Vec<(String, u32)> = Vec::new();
    for interface in &model.interfaces {
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_ref()
            else {
                continue;
            };
            match interaction.shape.as_ref() {
                Some(v1::interaction::Shape::Command(command)) => {
                    pins.push((
                        tuple_name(command.request_tuple.as_ref()),
                        bound(command.request_sizes.as_ref()),
                    ));
                }
                Some(v1::interaction::Shape::Query(query)) => {
                    pins.push((
                        tuple_name(query.request_tuple.as_ref()),
                        bound(query.request_sizes.as_ref()),
                    ));
                    if query.reply_tuple.is_some() {
                        pins.push((
                            tuple_name(query.reply_tuple.as_ref()),
                            bound(query.reply_sizes.as_ref()),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    let names: Vec<&str> = pins.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "ProbeSetBothParams",
            "ProbeStatusParams",
            "ProbeSpanParams",
            "ProbeSpanReply"
        ]
    );
    let asserts: String = pins
        .iter()
        .map(|(name, bound)| {
            format!(
                "    assert_eq!(<{name} as Payload<Proto3>>::MAX_SIZE, {bound}usize, \"{name}\");\n"
            )
        })
        .collect();
    let main = format!(
        r#"
fn main() {{
{asserts}
    // The widest value of each fits its bound.
    let mut out = vec![0u8; <ProbeSetBothParams as Payload<Proto3>>::MAX_SIZE];
    ProbeSetBothParams {{ level: Level::new_unchecked(10), count: Count::new_unchecked(200) }}
        .encode(&mut out).expect("fits");
    let mut out = vec![0u8; <ProbeSpanReply as Payload<Proto3>>::MAX_SIZE];
    ProbeSpanReply {{ lo: Count::new_unchecked(200), hi: Count::new_unchecked(200) }}
        .encode(&mut out).expect("fits");
    assert_eq!(<ProbeStatusParams as Payload<Proto3>>::MAX_SIZE, 0usize);
}}
"#
    );
    rustc::run_program("pb_interaction_roots", &program(&main));
}

/// A non-optional union field that is absent, and a union message with no
/// arm, are `MissingRequired`; two arms, or one arm twice, are `Duplicate`.
#[test]
fn a_union_holds_exactly_one_arm() {
    let main = format!(
        "{COLLECTIONS}{}",
        r#"
fn main() {
    assert_eq!(
        <Outcome as Payload<Proto3>>::verify(&[]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
        "no arm",
    );
    assert_eq!(
        <Outcome as Payload<Proto3>>::verify(&[0x08, 0x01, 0x18, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::Duplicate)),
        "two arms",
    );
    assert_eq!(
        <Outcome as Payload<Proto3>>::verify(&[0x08, 0x01, 0x08, 0x01]).err(),
        Some(VerifyError::Structure(Malformed::Duplicate)),
        "one arm twice",
    );
    // An unknown field beside the arm is skipped.
    let proof: Ref<'_, Outcome, Proto3> = Ref::verify(&[0x38, 0x01, 0x08, 0x05]).expect("verify");
    assert_eq!(proof.decode(), Outcome::Number(count(5)));
    // A union element of an array with no arm is refused through its owner.
    assert_eq!(
        <Lists as Payload<Proto3>>::verify(&[0x3a, 0x00, 0x42, 0x03, 0, 0, 0, 0x52, 0x01, 0x07]).err(),
        Some(VerifyError::Structure(Malformed::MissingRequired)),
    );
    // An arm's own constraint is checked: `Count` is `[0..200]`.
    assert!(matches!(
        <Outcome as Payload<Proto3>>::verify(&[0x08, 0xc9, 0x01]).err(),
        Some(VerifyError::Contract(_)),
    ));
}
"#
    );
    rustc::run_program("pb_union_arms", &program(&main));
}
