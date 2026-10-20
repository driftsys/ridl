//! The reader rules, the refusals and the absence rule of the proto3 codec,
//! each pinned by a hand-built buffer that is also fed to `prost`, so the
//! suite records where this codec is stricter than the reference reader.
//!
//! The conformance suite (`proto3_conformance.rs`) proves byte equality with
//! `prost` over the values a conforming writer produces. This target feeds
//! the generated `verify` and `decode` buffers no conforming writer produces:
//! a repeated field in unpacked form, fields out of order, a `bool` of 2,
//! unknown fields of each wire type, and every malformed form the design
//! refuses — a group, a wrong wire type, an over-wide varint, a duplicate, an
//! undeclared enum value, invalid UTF-8 — plus the absence rule at every
//! position a message has. Each case asserts the exact `VerifyError` of this
//! codec and `prost`'s verdict for the same bytes, against the checked-in
//! `prost` oracle, so a case states where the two agree and where this codec
//! refuses what `prost` reads (a duplicate, a group, an over-wide varint, a
//! value outside a typl constraint).
//!
//! The generated codec runs as the same program the conformance suite
//! builds (`support::proto3::Program`), once per test binary. The two
//! fixtures with no `prost` oracle — the zero-legality one and the evolution
//! pair — run as their own programs, and the `prost` half of those cases is
//! stated against the oracle's `Inner`, whose wire shape is theirs.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/proto3.rs"]
mod proto3;
#[path = "support/rustc.rs"]
mod rustc;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use ridl_backend_rust::WireEncoding;

/// The Rust `prost` writes for the fixture's two packages, as the
/// conformance suite includes it.
#[allow(
    warnings,
    reason = "third-party generated code, checked in verbatim so that a \
              regeneration can be compared byte for byte; editing it to \
              satisfy a lint would make that comparison impossible"
)]
mod prost_oracle {
    include!("generated/proto3_conformance_prost.rs");
}

use prost_oracle::pb::conformance as oracle;

/// The generated codec as a program, built once for this test binary, with
/// no named value: every case here hands the program bytes.
fn program() -> &'static proto3::Program {
    static PROGRAM: OnceLock<proto3::Program> = OnceLock::new();
    PROGRAM.get_or_init(|| proto3::Program::build("", &[]))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What the generated `verify` answers for `bytes` read as `message`: `ok`,
/// or the `Debug` text of the `VerifyError`.
fn verify(message: &str, bytes: &[u8]) -> String {
    program()
        .run(&[format!("{message} verify {}", to_hex(bytes))])
        .remove(0)
}

/// The hexadecimal re-encoding of `bytes` decoded as `message`, or the
/// `Debug` text of the `VerifyError` when `verify` refuses them.
fn decode_encode(message: &str, bytes: &[u8]) -> String {
    program()
        .run(&[format!("{message} decode-encode {}", to_hex(bytes))])
        .remove(0)
}

/// `prost`'s reading of `bytes` as `T`.
fn prost_read<T: prost::Message + Default>(bytes: &[u8]) -> Result<T, prost::DecodeError> {
    T::decode(bytes)
}

/// Asserts that this codec verifies `bytes` as `message` and reads them as
/// the value whose canonical encoding is `canonical`.
fn assert_reads(what: &str, message: &str, bytes: &[u8], canonical: &[u8]) {
    assert_eq!(verify(message, bytes), "ok", "{what}: verify");
    assert_eq!(
        decode_encode(message, bytes),
        to_hex(canonical),
        "{what}: the value read re-encodes to its canonical form"
    );
}

/// Asserts that this codec refuses `bytes` as `message` with exactly
/// `error`, from `verify` and from the `decode` path alike.
fn assert_refused(what: &str, message: &str, bytes: &[u8], error: &str) {
    assert_eq!(verify(message, bytes), error, "{what}: verify");
    assert_eq!(
        decode_encode(message, bytes),
        error,
        "{what}: a refused buffer is not decoded"
    );
}

fn structure(variant: &str) -> String {
    format!("Structure({variant})")
}

fn contract(type_name: &str, rule: &str) -> String {
    format!("Contract(Violation {{ type_name: Some({type_name:?}), rule: {rule} }})")
}

/// The tag of field `number` with wire type `wire`, as a varint.
fn tag(number: u32, wire: u8) -> Vec<u8> {
    varint((u64::from(number) << 3) | u64::from(wire))
}

fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// A length-delimited record of field `number` holding `body`.
fn record(number: u32, body: &[u8]) -> Vec<u8> {
    let mut out = tag(number, 2);
    out.extend(varint(body.len() as u64));
    out.extend_from_slice(body);
    out
}

/// A varint record of field `number`.
fn field(number: u32, v: u64) -> Vec<u8> {
    let mut out = tag(number, 0);
    out.extend(varint(v));
    out
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// One refused buffer of a table: its name, the message it is read as, the
/// bytes, and whether `prost` refuses the same bytes.
type RefusedCase = (
    &'static str,
    &'static str,
    Vec<u8>,
    Box<dyn Fn(&[u8]) -> bool>,
);

/// The `gear` every `Scalars` buffer must carry: `Gear` has no zero member,
/// so an absent `gear` is refused, and `FIRST` is 1 at field 22.
const GEAR: [u8; 3] = [0xb0, 0x01, 0x01];

/// The `three` every `Packed` buffer must carry: `[Count; 3]` is a fixed
/// array, written as a packed run of three zeros at field 16.
const THREE: [u8; 6] = [0x82, 0x01, 0x03, 0x00, 0x00, 0x00];

/// The canonical `Inner { speed: 5, label: "a" }`.
const INNER: [u8; 5] = [0x08, 0x05, 0x12, 0x01, 0x61];

/// The eight bytes of the `double` 1.0.
const ONE_F64: [u8; 8] = [0, 0, 0, 0, 0, 0, 0xf0, 0x3f];

/// A `Packed` with `three` at zero and the other fields as given.
fn packed(fill: impl FnOnce(&mut oracle::Packed)) -> oracle::Packed {
    let mut value = oracle::Packed {
        three: vec![0, 0, 0],
        ..Default::default()
    };
    fill(&mut value);
    value
}

/// A `Scalars` at `gear: FIRST` and the other fields as given.
fn scalars(fill: impl FnOnce(&mut oracle::Scalars)) -> oracle::Scalars {
    let mut value = oracle::Scalars {
        gear: oracle::Gear::First as i32,
        ..Default::default()
    };
    fill(&mut value);
    value
}

fn inner(speed: u32, label: &str) -> oracle::Inner {
    oracle::Inner {
        speed,
        label: label.to_string(),
    }
}

// ---------------------------------------------------------------------------
// The reader rules
// ---------------------------------------------------------------------------

/// A repeated numeric field is read in packed form, unpacked form, both
/// mixed, and as two packed runs, the elements concatenated in order; a
/// fixed-width element the same way. Each form decodes to the value whose
/// canonical encoding is one packed run, and `prost` reads each form to the
/// same value.
#[test]
fn unpacked_and_mixed_repeated_numerics_read() {
    let canonical = concat(&[&[0x0a, 0x02, 0x01, 0x02], &THREE]);
    let expected = packed(|v| v.unsigned8s = vec![1, 2]);
    for (name, head) in [
        ("packed", vec![0x0a, 0x02, 0x01, 0x02]),
        ("unpacked", vec![0x08, 0x01, 0x08, 0x02]),
        ("mixed", vec![0x08, 0x01, 0x0a, 0x01, 0x02]),
        ("unpacked after packed", vec![0x0a, 0x01, 0x01, 0x08, 0x02]),
        ("packed twice", vec![0x0a, 0x01, 0x01, 0x0a, 0x01, 0x02]),
    ] {
        let bytes = concat(&[&head, &THREE]);
        assert_reads(name, "Packed", &bytes, &canonical);
        assert_eq!(
            prost_read::<oracle::Packed>(&bytes),
            Ok(expected.clone()),
            "{name}: prost reads the same value"
        );
    }

    // A fixed-width element: `reals` is a `double` at field 11.
    let mut run = Vec::new();
    run.extend_from_slice(&ONE_F64);
    run.extend_from_slice(&ONE_F64);
    let canonical = concat(&[&record(11, &run), &THREE]);
    let expected = packed(|v| v.reals = vec![1.0, 1.0]);
    let one = concat(&[&tag(11, 1), &ONE_F64]);
    for (name, head) in [
        ("packed doubles", record(11, &run)),
        ("unpacked doubles", concat(&[&one, &one])),
        ("mixed doubles", concat(&[&record(11, &ONE_F64), &one])),
    ] {
        let bytes = concat(&[&head, &THREE]);
        assert_reads(name, "Packed", &bytes, &canonical);
        assert_eq!(
            prost_read::<oracle::Packed>(&bytes),
            Ok(expected.clone()),
            "{name}: prost reads the same value"
        );
    }
}

/// Fields in any order read as the same value, which re-encodes in number
/// order; `prost` reads them the same way.
#[test]
fn fields_out_of_order_read() {
    let reversed = [0x12, 0x01, 0x61, 0x08, 0x05];
    assert_reads("Inner reversed", "Inner", &reversed, &INNER);
    assert_eq!(prost_read::<oracle::Inner>(&reversed), Ok(inner(5, "a")));

    // `gear` (field 22) before `unsigned8` (field 1).
    let bytes = concat(&[&GEAR, &[0x08, 0x01]]);
    let canonical = concat(&[&[0x08, 0x01], &GEAR]);
    assert_reads("Scalars reversed", "Scalars", &bytes, &canonical);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.unsigned8 = 1))
    );

    // Inside a nested message, and the nested message after the scalar that
    // follows it.
    let bytes = concat(&[&[0x10, 0x01], &record(1, &[0x12, 0x01, 0x61, 0x08, 0x05])]);
    let canonical = concat(&[&record(1, &INNER), &[0x10, 0x01]]);
    assert_reads("Nested reversed", "Nested", &bytes, &canonical);
    assert_eq!(
        prost_read::<oracle::Nested>(&bytes),
        Ok(oracle::Nested {
            inner: Some(inner(5, "a")),
            count: 1,
        })
    );
}

/// A `bool` varint other than 0 or 1 reads as `true`, the protobuf rule
/// `prost` follows as well: at a box root, at a struct field, and inside a
/// packed run.
#[test]
fn a_bool_of_two_reads_as_true() {
    assert_reads(
        "EngagedBox of 2",
        "EngagedBox",
        &[0x08, 0x02],
        &[0x08, 0x01],
    );
    assert_eq!(
        prost_read::<oracle::EngagedBox>(&[0x08, 0x02]),
        Ok(oracle::EngagedBox { value: true })
    );
    assert_reads(
        "EngagedBox of 300",
        "EngagedBox",
        &field(1, 300),
        &[0x08, 0x01],
    );

    // `flag` is field 12 of `Scalars`.
    let bytes = concat(&[&[0x60, 0x02], &GEAR]);
    assert_reads(
        "Scalars.flag of 2",
        "Scalars",
        &bytes,
        &concat(&[&[0x60, 0x01], &GEAR]),
    );
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.flag = true))
    );

    // `flags` is the packed `[boolean; 0..4]` at field 12 of `Packed`.
    let bytes = concat(&[&record(12, &[0x02, 0x00, 0x7f]), &THREE]);
    let canonical = concat(&[&record(12, &[0x01, 0x00, 0x01]), &THREE]);
    assert_reads("Packed.flags of 2 and 127", "Packed", &bytes, &canonical);
    assert_eq!(
        prost_read::<oracle::Packed>(&bytes),
        Ok(packed(|v| v.flags = vec![true, false, true]))
    );
}

/// An unknown field of each wire type — varint, I64, length-delimited, I32
/// — is skipped at the start, in the middle and at the end of a message,
/// inside a nested message, at a number protobuf reserves (19000), and at a
/// retired ordinal (`Records.retired`, field 6). The value read carries none
/// of them: it re-encodes to the canonical bytes alone, which is the "dropped
/// on decode" answer. `prost` skips the same fields.
#[test]
fn unknown_fields_of_each_wire_type_are_skipped_anywhere() {
    let unknowns = concat(&[
        &field(99, 1),
        &tag(99, 1),
        &[1, 2, 3, 4, 5, 6, 7, 8],
        &record(99, &[0xaa, 0xbb]),
        &tag(99, 5),
        &[9, 10, 11, 12],
    ]);
    let speed = [0x08, 0x05];
    let label = [0x12, 0x01, 0x61];
    for (name, bytes) in [
        ("at the start", concat(&[&unknowns, &speed, &label])),
        ("in the middle", concat(&[&speed, &unknowns, &label])),
        ("at the end", concat(&[&speed, &label, &unknowns])),
    ] {
        assert_reads(name, "Inner", &bytes, &INNER);
        assert_eq!(
            prost_read::<oracle::Inner>(&bytes),
            Ok(inner(5, "a")),
            "{name}: prost skips the unknown fields"
        );
    }

    // Inside a nested message.
    let bytes = concat(&[
        &record(1, &concat(&[&speed, &record(99, &[0xff]), &label])),
        &[0x10, 0x01],
    ]);
    let canonical = concat(&[&record(1, &INNER), &[0x10, 0x01]]);
    assert_reads("inside Nested.inner", "Nested", &bytes, &canonical);
    assert_eq!(
        prost_read::<oracle::Nested>(&bytes),
        Ok(oracle::Nested {
            inner: Some(inner(5, "a")),
            count: 1,
        })
    );

    // A reserved number, and the highest field number.
    for (name, number) in [
        ("19000, reserved", 19000),
        ("the highest number", (1 << 29) - 1),
    ] {
        let bytes = concat(&[&speed, &field(number, 7), &label]);
        assert_reads(name, "Inner", &bytes, &INNER);
        assert_eq!(
            prost_read::<oracle::Inner>(&bytes),
            Ok(inner(5, "a")),
            "{name}"
        );
    }

    // A retired ordinal: `Records` reserves field 6, so a value there is an
    // unknown field, and the value read is the empty `Records`.
    let bytes = field(6, 1);
    assert_reads("a retired ordinal", "Records", &bytes, &[]);
    assert_eq!(
        prost_read::<oracle::Records>(&bytes),
        Ok(oracle::Records::default())
    );
}

/// A payload longer than this version's `MAX_SIZE` verifies when the excess
/// is a field this version skips: a newer writer may append a field, and the
/// bound that protects a receiver is its own buffer, not a refusal in
/// `verify`. No `TooLarge` is raised for a proto3 payload.
#[test]
fn a_newer_writers_payload_above_max_size_verifies() {
    let max_size: usize = program()
        .run(&["Inner max-size".to_string()])
        .remove(0)
        .parse()
        .expect("MAX_SIZE in decimal");
    assert!(max_size > 0);
    let padding = vec![0x41u8; max_size];
    let bytes = concat(&[&INNER, &record(99, &padding)]);
    assert!(
        bytes.len() > max_size,
        "the probe must pass MAX_SIZE ({max_size}), or it proves nothing"
    );
    assert_reads("above MAX_SIZE", "Inner", &bytes, &INNER);
    assert_eq!(prost_read::<oracle::Inner>(&bytes), Ok(inner(5, "a")));

    // The unknown field first, so the known fields sit past `MAX_SIZE`.
    let bytes = concat(&[&record(99, &padding), &INNER]);
    assert_reads("known fields past MAX_SIZE", "Inner", &bytes, &INNER);
    assert_eq!(prost_read::<oracle::Inner>(&bytes), Ok(inner(5, "a")));
}

// ---------------------------------------------------------------------------
// The refusals
// ---------------------------------------------------------------------------

/// A group (wire types 3 and 4) is refused as `WireType`, where `prost`
/// skips it: proto3 has no groups, so one can only come from a writer
/// outside the contract. Wire types 6 and 7 and field number 0 are refused
/// the same way, and `prost` refuses those too.
#[test]
fn a_group_is_refused_where_prost_skips_it() {
    let wire_type = structure("WireType");
    let group = concat(&[&tag(99, 3), &tag(99, 4)]);
    for (name, bytes) in [
        (
            "a group between the fields",
            concat(&[&[0x08, 0x05], &group, &[0x12, 0x01, 0x61]]),
        ),
        ("a group first", concat(&[&group, &INNER])),
        ("a group last", concat(&[&INNER, &group])),
        (
            "a group with a field inside",
            concat(&[&INNER, &tag(99, 3), &field(1, 1), &tag(99, 4)]),
        ),
    ] {
        assert_refused(name, "Inner", &bytes, &wire_type);
        assert_eq!(
            prost_read::<oracle::Inner>(&bytes),
            Ok(inner(5, "a")),
            "{name}: prost skips a group; if this changes, the test's claim changes"
        );
    }

    // In a nested message.
    let bytes = concat(&[&record(1, &group), &[0x10, 0x01]]);
    assert_refused("a group in Nested.inner", "Nested", &bytes, &wire_type);
    assert!(prost_read::<oracle::Nested>(&bytes).is_ok());

    // The wire types proto3 does not define, and field number 0: refused on
    // both sides.
    for (name, bytes) in [
        ("wire type 6", concat(&[&INNER, &tag(99, 6)])),
        ("wire type 7", concat(&[&INNER, &tag(99, 7)])),
        ("field number 0", concat(&[&[0x00, 0x01], &INNER])),
        (
            "field number 0 as a record",
            concat(&[&INNER, &[0x02, 0x00]]),
        ),
    ] {
        assert_refused(name, "Inner", &bytes, &wire_type);
        assert!(
            prost_read::<oracle::Inner>(&bytes).is_err(),
            "{name}: prost refuses it too"
        );
    }
}

/// A known field under a wire type its type does not take is refused as
/// `WireType`, at every shape: a varint scalar as a record, a string as a
/// varint, a nested message as a varint, a `double` as I32, a `float` as
/// I64, a packed array as I64, a map as a varint, and a union arm as a
/// record. `prost` refuses each.
#[test]
fn a_known_field_with_the_wrong_wire_type_is_refused() {
    let wire_type = structure("WireType");
    let cases: Vec<RefusedCase> = vec![
        (
            "a uint32 as a record",
            "Inner",
            concat(&[&record(1, &[0x05]), &[0x12, 0x01, 0x61]]),
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "a string as a varint",
            "Inner",
            concat(&[&[0x08, 0x05], &field(2, 1)]),
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "a nested message as a varint",
            "Nested",
            concat(&[&field(1, 1), &[0x10, 0x01]]),
            Box::new(|b| prost_read::<oracle::Nested>(b).is_err()),
        ),
        (
            "a double as I32",
            "Scalars",
            concat(&[&tag(11, 5), &[0, 0, 0, 0], &GEAR]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "a float as I64",
            "Scalars",
            concat(&[&tag(10, 1), &[0; 8], &GEAR]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "an enum as I32",
            "Scalars",
            concat(&[&tag(21, 5), &[1, 0, 0, 0], &GEAR]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "a packed array as I64",
            "Packed",
            concat(&[&tag(1, 1), &[0; 8], &THREE]),
            Box::new(|b| prost_read::<oracle::Packed>(b).is_err()),
        ),
        (
            "a record array as a varint",
            "Records",
            field(1, 1),
            Box::new(|b| prost_read::<oracle::Records>(b).is_err()),
        ),
        (
            "a map as a varint",
            "Maps",
            field(1, 1),
            Box::new(|b| prost_read::<oracle::Maps>(b).is_err()),
        ),
        (
            "a union arm as a record",
            "Outcome",
            record(1, &[]),
            Box::new(|b| prost_read::<oracle::Outcome>(b).is_err()),
        ),
        (
            "a box value as a record",
            "CountBox",
            record(1, &[0x05]),
            Box::new(|b| prost_read::<oracle::CountBox>(b).is_err()),
        ),
    ];
    for (name, message, bytes, prost_refuses) in cases {
        assert_refused(name, message, &bytes, &wire_type);
        assert!(prost_refuses(&bytes), "{name}: prost refuses it too");
    }
}

/// A varint wider than its field's scalar is refused as `Varint`: a
/// `uint32` above `u32::MAX`, a `sint32` outside `i32`, an enum outside
/// `int32`, the same inside a packed run, and a varint of eleven bytes or
/// whose tenth byte is above 1. `prost` truncates the first three to the
/// scalar's width and reads a value the bytes did not hold; it refuses the
/// last two.
#[test]
fn a_varint_wider_than_its_scalar_is_refused() {
    let wide = structure("Varint");
    let above_u32: u64 = 1 << 32;

    // `unsigned32` is field 3, a `uint32`.
    let bytes = concat(&[&field(3, above_u32), &GEAR]);
    assert_refused("uint32 above u32::MAX", "Scalars", &bytes, &wide);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|_| {})),
        "prost truncates 1 << 32 to 0"
    );

    // `signed32` is field 7, a `sint32`: zigzag 1 << 33 is outside i32.
    let bytes = concat(&[&field(7, 1 << 33), &GEAR]);
    assert_refused("sint32 outside i32", "Scalars", &bytes, &wide);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|_| {})),
        "prost truncates the zigzag value to 0"
    );

    // `health` is field 21, an enum: an `int32` on the wire.
    let bytes = concat(&[&field(21, above_u32), &GEAR]);
    assert_refused("enum outside int32", "Scalars", &bytes, &wide);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|_| {})),
        "prost truncates the enum value to 0"
    );

    // Inside a packed run: `unsigned32s` is field 3 of `Packed`.
    let bytes = concat(&[&record(3, &varint(above_u32)), &THREE]);
    assert_refused(
        "uint32 above u32::MAX in a packed run",
        "Packed",
        &bytes,
        &wide,
    );
    assert_eq!(
        prost_read::<oracle::Packed>(&bytes),
        Ok(packed(|v| v.unsigned32s = vec![0])),
        "prost truncates the element to 0"
    );

    // A varint the wire itself does not admit: refused on both sides.
    let ten_bytes_tenth_above_one = [
        0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02,
    ];
    let eleven_bytes = [
        0x08, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01,
    ];
    for (name, bytes) in [
        ("a tenth byte above 1", &ten_bytes_tenth_above_one[..]),
        ("eleven bytes", &eleven_bytes[..]),
    ] {
        assert_refused(name, "CountBox", bytes, &wide);
        assert!(
            prost_read::<oracle::CountBox>(bytes).is_err(),
            "{name}: prost refuses it too"
        );
    }

    // The contrast: a non-minimal varint is accepted, `80 00` for 0 and
    // `85 00` for 5.
    assert_reads("a non-minimal 0", "CountBox", &[0x08, 0x80, 0x00], &[]);
    assert_reads(
        "a non-minimal 5",
        "Inner",
        &[0x08, 0x85, 0x00, 0x12, 0x01, 0x61],
        &INNER,
    );
    assert_eq!(
        prost_read::<oracle::Inner>(&[0x08, 0x85, 0x00, 0x12, 0x01, 0x61]),
        Ok(inner(5, "a"))
    );
}

/// A varint that fits the wire scalar but not the declared width is a
/// range violation: 300 in a `u8` field (`uint32` on the wire) is
/// `Contract(Range)`, as is 201 in a `Count` (`[0..200]`), on the named
/// type's name. `prost` carries both, having no range to check.
#[test]
fn a_value_wider_than_its_width_is_a_range_violation() {
    let bytes = concat(&[&field(1, 300), &GEAR]);
    assert_refused(
        "300 in a u8",
        "Scalars",
        &bytes,
        &contract("Scalars", "Range"),
    );
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.unsigned8 = 300))
    );

    // `unsigned16` is field 2: 70000 fits a `uint32`, not a `u16`.
    let bytes = concat(&[&field(2, 70000), &GEAR]);
    assert_refused(
        "70000 in a u16",
        "Scalars",
        &bytes,
        &contract("Scalars", "Range"),
    );
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.unsigned16 = 70000))
    );

    // `signed8` is field 5, a `sint32`: the zigzag of 128 is 256, and 128 is
    // outside i8.
    let bytes = concat(&[&field(5, 256), &GEAR]);
    assert_refused(
        "128 in an i8",
        "Scalars",
        &bytes,
        &contract("Scalars", "Range"),
    );
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.signed8 = 128))
    );

    // A named scalar names its own type.
    let bytes = concat(&[&field(1, 201), &[0x12, 0x01, 0x61]]);
    assert_refused(
        "201 in a Count",
        "Inner",
        &bytes,
        &contract("Count", "Range"),
    );
    assert_eq!(prost_read::<oracle::Inner>(&bytes), Ok(inner(201, "a")));

    // Inside a packed run: `unsigned8s` is field 1 of `Packed`.
    let bytes = concat(&[&record(1, &varint(300)), &THREE]);
    assert_refused(
        "300 in a packed u8 run",
        "Packed",
        &bytes,
        &contract("Packed", "Range"),
    );
    assert_eq!(
        prost_read::<oracle::Packed>(&bytes),
        Ok(packed(|v| v.unsigned8s = vec![300]))
    );
}

/// A packed run cut inside a varint, or whose length is not a multiple of
/// its fixed width, is `OutOfBounds`; so is a record whose length runs past
/// the buffer, a tag with no value, and a fixed-width value cut short.
/// `prost` refuses each.
#[test]
fn a_packed_run_cut_inside_a_varint_is_out_of_bounds() {
    let out_of_bounds = structure("OutOfBounds");
    let cases: Vec<RefusedCase> = vec![
        (
            "a packed varint run cut inside a varint",
            "Packed",
            concat(&[&record(1, &[0x80]), &THREE]),
            Box::new(|b| prost_read::<oracle::Packed>(b).is_err()),
        ),
        (
            "a packed varint run cut inside its last varint",
            "Packed",
            concat(&[&record(1, &[0x01, 0x82]), &THREE]),
            Box::new(|b| prost_read::<oracle::Packed>(b).is_err()),
        ),
        (
            "a packed double run of three bytes",
            "Packed",
            concat(&[&record(11, &[0, 0, 0]), &THREE]),
            Box::new(|b| prost_read::<oracle::Packed>(b).is_err()),
        ),
        (
            "a packed float run of five bytes",
            "Packed",
            concat(&[&record(10, &[0; 5]), &THREE]),
            Box::new(|b| prost_read::<oracle::Packed>(b).is_err()),
        ),
        (
            "a record past the end",
            "Inner",
            vec![0x08, 0x05, 0x12, 0x05, 0x61],
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "a tag with no value",
            "Inner",
            vec![0x08],
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "a varint cut short",
            "Inner",
            vec![0x08, 0x85],
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "a fixed64 cut short",
            "Scalars",
            concat(&[&tag(11, 1), &[0, 0]]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "a fixed32 cut short",
            "Scalars",
            concat(&[&tag(10, 5), &[0, 0, 0]]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "a nested record past the end of its message",
            "Nested",
            record(1, &[0x12, 0x09, 0x61]),
            Box::new(|b| prost_read::<oracle::Nested>(b).is_err()),
        ),
    ];
    for (name, message, bytes, prost_refuses) in cases {
        assert_refused(name, message, &bytes, &out_of_bounds);
        assert!(prost_refuses(&bytes), "{name}: prost refuses it too");
    }
}

/// A field that is not repeated written twice is refused as `Duplicate`:
/// a scalar, a string, a nested message, an optional field, a box value and
/// a map entry's key. `prost` accepts each, reading the last scalar and the
/// merge of the two messages — the protobuf merge rule this codec does not
/// apply, so a case here states the value `prost` reads.
#[test]
fn a_singular_field_twice_is_a_duplicate() {
    let duplicate = structure("Duplicate");

    let bytes = [0x08, 0x05, 0x08, 0x06, 0x12, 0x01, 0x61];
    assert_refused("a scalar twice", "Inner", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Inner>(&bytes),
        Ok(inner(6, "a")),
        "prost reads the last scalar"
    );

    let bytes = [0x08, 0x05, 0x12, 0x01, 0x61, 0x12, 0x01, 0x62];
    assert_refused("a string twice", "Inner", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Inner>(&bytes),
        Ok(inner(5, "b")),
        "prost reads the last string"
    );

    // The same scalar twice at its default value: still two records.
    let bytes = [0x08, 0x00, 0x08, 0x00, 0x12, 0x01, 0x61];
    assert_refused("a scalar twice at 0", "Inner", &bytes, &duplicate);
    assert_eq!(prost_read::<oracle::Inner>(&bytes), Ok(inner(0, "a")));

    let bytes = concat(&[
        &record(1, &[0x08, 0x05]),
        &record(1, &[0x12, 0x01, 0x61]),
        &[0x10, 0x01],
    ]);
    assert_refused("a nested message twice", "Nested", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Nested>(&bytes),
        Ok(oracle::Nested {
            inner: Some(inner(5, "a")),
            count: 1,
        }),
        "prost merges the two messages"
    );

    // An optional field: `Optionals.count` is field 1.
    let bytes = [0x08, 0x01, 0x08, 0x02];
    assert_refused("an optional field twice", "Optionals", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Optionals>(&bytes),
        Ok(oracle::Optionals {
            count: Some(2),
            ..Default::default()
        })
    );

    // A box root's one field.
    assert_refused("a box value twice", "CountBox", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::CountBox>(&bytes),
        Ok(oracle::CountBox { value: 2 })
    );

    // A tuple's positional field: `Tuples.range` is `(min, max)`.
    let bytes = record(1, &[0x08, 0x01, 0x08, 0x02]);
    assert_refused("a tuple field twice", "Tuples", &bytes, &duplicate);
    assert!(prost_read::<oracle::Tuples>(&bytes).is_ok());

    // A map entry's key written twice inside one entry (`Maps.byString`,
    // field 1; the key is field 1 of the entry). Two entries with the same
    // key are not a wire duplicate: that is `Contract(Unique)`, pinned by the
    // round-trip suite.
    let entry = concat(&[&record(1, b"k"), &record(1, b"k")]);
    let bytes = record(1, &entry);
    assert_refused("a map entry key twice", "Maps", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Maps>(&bytes),
        Ok(oracle::Maps {
            by_string: [(String::from("k"), 0)].into(),
            ..Default::default()
        })
    );

    // The contrast: a repeated field written as several records is not a
    // duplicate, which `unpacked_and_mixed_repeated_numerics_read` pins.
}

/// A union message holding two arms, or one arm twice, is refused as
/// `Duplicate`, at the root and through a field; `prost` reads the last
/// arm.
#[test]
fn two_union_arms_are_a_duplicate() {
    let duplicate = structure("Duplicate");
    use oracle::outcome::Value;

    let bytes = [0x08, 0x01, 0x18, 0x01];
    assert_refused("two arms", "Outcome", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Outcome>(&bytes),
        Ok(oracle::Outcome {
            value: Some(Value::Health(1))
        }),
        "prost reads the last arm"
    );

    let bytes = [0x08, 0x01, 0x08, 0x02];
    assert_refused("one arm twice", "Outcome", &bytes, &duplicate);
    assert_eq!(
        prost_read::<oracle::Outcome>(&bytes),
        Ok(oracle::Outcome {
            value: Some(Value::Number(2))
        })
    );

    // Two arms with an unknown field between them.
    let bytes = concat(&[&[0x12, 0x01, 0x61], &field(99, 1), &[0x08, 0x01]]);
    assert_refused(
        "two arms around an unknown field",
        "Outcome",
        &bytes,
        &duplicate,
    );
    assert!(prost_read::<oracle::Outcome>(&bytes).is_ok());

    // Through an optional field: `OptionalComposites.outcome` is field 2.
    let bytes = record(2, &[0x08, 0x01, 0x18, 0x01]);
    assert_refused(
        "two arms through a field",
        "OptionalComposites",
        &bytes,
        &duplicate,
    );
    assert!(prost_read::<oracle::OptionalComposites>(&bytes).is_ok());

    // As an array element: `Records.outcomes` is field 4.
    let bytes = record(4, &[0x08, 0x01, 0x08, 0x01]);
    assert_refused("one arm twice in an element", "Records", &bytes, &duplicate);
    assert!(prost_read::<oracle::Records>(&bytes).is_ok());
}

/// An enum value the declaration does not name is `Contract(Variant)` on
/// the enum's name — at a box root, at a struct field, inside a packed run,
/// as a union arm, and for a negative value — where `prost` carries any
/// `int32`: the schema leaves enums open and the codec closes them.
#[test]
fn an_undeclared_enum_value_is_a_variant_violation() {
    let variant = contract("Health", "Variant");

    assert_refused("HealthBox of 7", "HealthBox", &[0x08, 0x07], &variant);
    assert_eq!(
        prost_read::<oracle::HealthBox>(&[0x08, 0x07]),
        Ok(oracle::HealthBox { value: 7 })
    );

    // `health` is field 21 of `Scalars`.
    let bytes = concat(&[&field(21, 7), &GEAR]);
    assert_refused("Scalars.health of 7", "Scalars", &bytes, &variant);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.health = 7))
    );

    // `healths` is field 13 of `Packed`.
    let bytes = concat(&[&record(13, &[0x00, 0x07]), &THREE]);
    assert_refused("a packed run with a 7", "Packed", &bytes, &variant);
    assert_eq!(
        prost_read::<oracle::Packed>(&bytes),
        Ok(packed(|v| v.healths = vec![0, 7]))
    );

    // The `health` arm of `Outcome` is field 3.
    assert_refused("an Outcome arm of 7", "Outcome", &[0x18, 0x07], &variant);
    assert_eq!(
        prost_read::<oracle::Outcome>(&[0x18, 0x07]),
        Ok(oracle::Outcome {
            value: Some(oracle::outcome::Value::Health(7))
        })
    );

    // A negative value `Trend` does not declare: -2, as `int32` writes it,
    // sign-extended to ten bytes.
    let minus_two = concat(&[&tag(1, 0), &varint(-2i64 as u64)]);
    assert_refused(
        "TrendBox of -2",
        "TrendBox",
        &minus_two,
        &contract("Trend", "Variant"),
    );
    assert_eq!(
        prost_read::<oracle::TrendBox>(&minus_two),
        Ok(oracle::TrendBox { value: -2 })
    );

    // `Gear` has no zero member, so 0 is undeclared too: the one value a
    // conforming writer never writes for it.
    assert_refused(
        "GearBox of 0",
        "GearBox",
        &[0x08, 0x00],
        &contract("Gear", "Variant"),
    );
    assert_eq!(
        prost_read::<oracle::GearBox>(&[0x08, 0x00]),
        Ok(oracle::GearBox { value: 0 })
    );
}

/// An enum set with a bit no member declares is `Contract(Variant)` on the
/// set's name, at width u8 and u64, at a box root, at a struct field and
/// inside a packed run; `prost` carries any integer.
#[test]
fn an_undeclared_enum_set_bit_is_a_variant_violation() {
    let variant = contract("WarningFlags", "Variant");

    // `WarningFlags` declares bits 0 and 1: bit 2 is undeclared.
    assert_refused(
        "WarningFlagsBox with bit 2",
        "WarningFlagsBox",
        &[0x08, 0x04],
        &variant,
    );
    assert_eq!(
        prost_read::<oracle::WarningFlagsBox>(&[0x08, 0x04]),
        Ok(oracle::WarningFlagsBox { value: 4 })
    );

    // Declared bits beside an undeclared one.
    assert_refused(
        "WarningFlagsBox with bits 0, 1 and 7",
        "WarningFlagsBox",
        &[0x08, 0x83, 0x01],
        &variant,
    );

    // `WideFlags` declares bits 0, 32 and 63: bit 1 is undeclared, and so
    // is bit 62.
    let wide = contract("WideFlags", "Variant");
    assert_refused(
        "WideFlagsBox with bit 1",
        "WideFlagsBox",
        &[0x08, 0x02],
        &wide,
    );
    assert_eq!(
        prost_read::<oracle::WideFlagsBox>(&[0x08, 0x02]),
        Ok(oracle::WideFlagsBox { value: 2 })
    );
    let bit_62 = field(1, 1 << 62);
    assert_refused("WideFlagsBox with bit 62", "WideFlagsBox", &bit_62, &wide);
    assert_eq!(
        prost_read::<oracle::WideFlagsBox>(&bit_62),
        Ok(oracle::WideFlagsBox { value: 1 << 62 })
    );

    // `flags` is field 24 of `Scalars`.
    let bytes = concat(&[&field(24, 4), &GEAR]);
    assert_refused("Scalars.flags with bit 2", "Scalars", &bytes, &variant);
    assert_eq!(
        prost_read::<oracle::Scalars>(&bytes),
        Ok(scalars(|v| v.flags = 4))
    );

    // `sets` is field 15 of `Packed`.
    let bytes = concat(&[&record(15, &[0x03, 0x04]), &THREE]);
    assert_refused("a packed run with bit 2", "Packed", &bytes, &variant);
    assert_eq!(
        prost_read::<oracle::Packed>(&bytes),
        Ok(packed(|v| v.sets = vec![3, 4]))
    );
}

/// A string that is not UTF-8 is `Utf8`, at a box root, at a named and an
/// inline string field, as a map key, as a union arm, and in a string with
/// no constraint at all; `prost` refuses it too. A `bytes` field holding
/// the same byte is the contrast: read.
#[test]
fn invalid_utf8_is_refused() {
    let utf8 = structure("Utf8");
    let cases: Vec<RefusedCase> = vec![
        (
            "LabelBox",
            "LabelBox",
            record(1, &[0xff]),
            Box::new(|b| prost_read::<oracle::LabelBox>(b).is_err()),
        ),
        (
            "a named string field",
            "Inner",
            concat(&[&[0x08, 0x05], &record(2, &[0xc3])]),
            Box::new(|b| prost_read::<oracle::Inner>(b).is_err()),
        ),
        (
            "an inline string field",
            "Scalars",
            concat(&[&record(13, &[0xff]), &GEAR]),
            Box::new(|b| prost_read::<oracle::Scalars>(b).is_err()),
        ),
        (
            "a map key",
            "Maps",
            record(1, &record(1, &[0xff])),
            Box::new(|b| prost_read::<oracle::Maps>(b).is_err()),
        ),
        (
            "a union arm",
            "Outcome",
            record(2, &[0xff]),
            Box::new(|b| prost_read::<oracle::Outcome>(b).is_err()),
        ),
        (
            "an array element",
            "Records",
            record(1, &[0xff]),
            Box::new(|b| prost_read::<oracle::Records>(b).is_err()),
        ),
        (
            "a truncated multi-byte sequence",
            "LabelBox",
            record(1, &[0xe2, 0x82]),
            Box::new(|b| prost_read::<oracle::LabelBox>(b).is_err()),
        ),
    ];
    for (name, message, bytes, prost_refuses) in cases {
        assert_refused(name, message, &bytes, &utf8);
        assert!(prost_refuses(&bytes), "{name}: prost refuses it too");
    }

    // The contrast.
    let bytes = record(1, &[0xff]);
    assert_reads("BlobBox of 0xff", "BlobBox", &bytes, &bytes);
    assert_eq!(
        prost_read::<oracle::BlobBox>(&bytes),
        Ok(oracle::BlobBox { value: vec![0xff] })
    );

    // A string with no declared constraint, `pb.zero`'s `Free`, at a field
    // and at a box root: the checker gives it a default length bound, so it
    // is checked as every other string is, UTF-8 first. No string of a
    // checked source reaches the codec without a constraint, so this is the
    // least-constrained string a test can hold. `prost` refuses the same
    // bytes as a `LabelBox`, asserted above.
    rustc::run_program(
        "pb_utf8_unconstrained",
        &fixture_program(
            "proto3_zero_legality.ridl",
            r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{Malformed, Payload, Ref, VerifyError};

fn main() {
    let invalid: &[u8] = &[0x0a, 0x01, 0xff];
    assert_eq!(
        <ValueFree as Payload<Proto3>>::verify(invalid).err(),
        Some(VerifyError::Structure(Malformed::Utf8)),
        "a string field with no declared constraint, of 0xff",
    );
    assert_eq!(
        <Free as Payload<Proto3>>::verify(invalid).err(),
        Some(VerifyError::Structure(Malformed::Utf8)),
        "a string box with no declared constraint, of 0xff",
    );
    let valid: &[u8] = &[0x0a, 0x01, 0x78];
    let proof: Ref<'_, ValueFree, Proto3> = Ref::verify(valid).expect("a valid string reads");
    assert_eq!(
        proof.decode(),
        ValueFree { value: Free::new_unchecked(String::from("x")), label: Label::new_unchecked(String::new()) },
    );
    let proof: Ref<'_, Free, Proto3> = Ref::verify(valid).expect("a valid string box reads");
    assert_eq!(proof.decode(), Free::new_unchecked(String::from("x")));
    // The default bound the checker gave `Free` is a real check: 257
    // characters are refused by length, so `Free` is not unconstrained.
    let long = "x".repeat(257);
    // The tag, then 257 as a varint.
    let mut over = vec![0x0a, 0x81, 0x02];
    over.extend_from_slice(long.as_bytes());
    assert!(
        matches!(
            <Free as Payload<Proto3>>::verify(&over).err(),
            Some(VerifyError::Contract(v)) if v.rule == ridl_rt::payload::Rule::Length
        ),
        "the default length bound is checked",
    );
}
"#,
        ),
    );
}

// ---------------------------------------------------------------------------
// Absence
// ---------------------------------------------------------------------------

/// An absent non-optional field reads as its proto3 default when that
/// default is a legal value of its type, one buffer per row of the rule:
/// an integer, a float, a boolean, an enum with a zero member, an enum set,
/// a string, bytes, an array, a map, a struct, a tuple and an optional
/// field — at a struct field, at a tuple field, at a map entry's key and at
/// its value, and at a box root, where the empty payload is the default.
/// The value read re-encodes to the canonical bytes, which omit the field
/// again; `prost` reads each buffer.
#[test]
fn absence_reads_as_the_default_when_it_is_legal() {
    // At a struct field: every row of the table in one struct whose fields
    // all admit their default, then the collection, struct and tuple rows.
    assert_reads("Zeroable: every scalar row absent", "Zeroable", &[], &[]);
    assert_eq!(prost_read::<oracle::Zeroable>(&[]), Ok(Default::default()));
    assert_reads("Records: every array absent", "Records", &[], &[]);
    assert_eq!(prost_read::<oracle::Records>(&[]), Ok(Default::default()));
    assert_reads("Maps: every map absent", "Maps", &[], &[]);
    assert_eq!(prost_read::<oracle::Maps>(&[]), Ok(Default::default()));
    assert_reads("Optionals: every optional absent", "Optionals", &[], &[]);
    assert_eq!(prost_read::<oracle::Optionals>(&[]), Ok(Default::default()));
    assert_reads("OptionalComposites absent", "OptionalComposites", &[], &[]);

    // A struct field absent reads as the message decoded from zero bytes,
    // which the writer then writes as an empty record (a message value is
    // present or absent, and `prost` writes `Some` the same way).
    let nested_canonical = concat(&[&record(1, &[]), &[0x10, 0x01]]);
    assert_reads(
        "Nested.inner absent",
        "Nested",
        &[0x10, 0x01],
        &nested_canonical,
    );
    assert_eq!(
        prost_read::<oracle::Nested>(&[0x10, 0x01]),
        Ok(oracle::Nested {
            inner: None,
            count: 1,
        }),
        "prost reads the absent message as None; this codec as its empty value"
    );

    // A tuple field absent, and the fields of a tuple absent: `Tuples` is
    // `range: (min, max)` at 1 and `pair: (lo, inner)` at 2; the canonical
    // all-zero value writes `range` empty and `pair` holding its empty
    // `inner`.
    let tuples_canonical = concat(&[&record(1, &[]), &record(2, &record(2, &[]))]);
    assert_reads(
        "Tuples: both tuples absent",
        "Tuples",
        &[],
        &tuples_canonical,
    );
    assert_reads(
        "Tuples.pair absent",
        "Tuples",
        &record(1, &[]),
        &tuples_canonical,
    );
    assert_reads(
        "Tuples.pair present, its fields absent",
        "Tuples",
        &record(2, &[]),
        &tuples_canonical,
    );
    assert_eq!(
        prost_read::<oracle::Tuples>(&record(2, &[])),
        Ok(oracle::Tuples {
            range: None,
            pair: Some(oracle::TuplesPair::default()),
        })
    );
    // An induced root tuple with every field absent.
    assert_reads("ProbeSpanReply absent fields", "ProbeSpanReply", &[], &[]);
    assert_eq!(
        prost_read::<oracle::ProbeSpanReply>(&[]),
        Ok(Default::default())
    );

    // A map entry's key and value absent: `byLabel` is field 7 (its key
    // admits the empty string), `byUint32` field 2 holds a `Label`,
    // `bySint32` field 4 holds an `Inner`.
    let empty_entry = record(7, &[]);
    assert_reads(
        "a map entry with key and value absent",
        "Maps",
        &empty_entry,
        &empty_entry,
    );
    assert_eq!(
        prost_read::<oracle::Maps>(&empty_entry),
        Ok(oracle::Maps {
            by_label: [(String::new(), 0)].into(),
            ..Default::default()
        })
    );
    let key_absent = record(7, &[0x10, 0x01]);
    assert_reads(
        "a map entry with its key absent",
        "Maps",
        &key_absent,
        &key_absent,
    );
    let value_absent = record(7, &record(1, b"a"));
    assert_reads(
        "a map entry with its value absent",
        "Maps",
        &value_absent,
        &value_absent,
    );
    let string_value_absent = record(2, &[0x08, 0x07]);
    assert_reads(
        "a map entry with its string value absent",
        "Maps",
        &string_value_absent,
        &string_value_absent,
    );
    assert_eq!(
        prost_read::<oracle::Maps>(&string_value_absent),
        Ok(oracle::Maps {
            by_uint32: [(7, String::new())].into(),
            ..Default::default()
        })
    );
    let struct_value_absent = record(4, &field(1, 2));
    assert_reads(
        "a map entry with its struct value absent",
        "Maps",
        &struct_value_absent,
        &struct_value_absent,
    );
    assert_eq!(
        prost_read::<oracle::Maps>(&struct_value_absent),
        Ok(oracle::Maps {
            by_sint32: [(1, oracle::Inner::default())].into(),
            ..Default::default()
        })
    );

    // At a box root: the empty payload is the default of every box whose
    // type admits it.
    for message in [
        "CountBox",
        "WideBox",
        "RatioBox",
        "LabelBox",
        "BlobBox",
        "EngagedBox",
        "HealthBox",
        "TrendBox",
        "WarningFlagsBox",
        "WideFlagsBox",
        "GradeBox",
        "TorqueBox",
        "Wheel",
        "ProbeStatusParams",
    ] {
        assert_reads(&format!("{message} empty"), message, &[], &[]);
    }
    use prost_oracle::pb::parts;
    assert_eq!(prost_read::<oracle::CountBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::WideBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::RatioBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::LabelBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::BlobBox>(&[]), Ok(Default::default()));
    assert_eq!(
        prost_read::<oracle::EngagedBox>(&[]),
        Ok(Default::default())
    );
    assert_eq!(prost_read::<oracle::HealthBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::TrendBox>(&[]), Ok(Default::default()));
    assert_eq!(
        prost_read::<oracle::WarningFlagsBox>(&[]),
        Ok(Default::default())
    );
    assert_eq!(
        prost_read::<oracle::WideFlagsBox>(&[]),
        Ok(Default::default())
    );
    assert_eq!(prost_read::<parts::GradeBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<parts::TorqueBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<parts::Wheel>(&[]), Ok(Default::default()));
    assert_eq!(
        prost_read::<oracle::ProbeStatusParams>(&[]),
        Ok(Default::default())
    );
}

/// An absent non-optional field whose default is not a legal value of its
/// type is `MissingRequired`: an integer whose range excludes 0, a float
/// whose step excludes 0, an enum with no zero member, a string whose
/// length excludes the empty string, an array whose count excludes the
/// empty array, a union, a nested struct whose own field excludes 0, and
/// the empty payload of a box whose type excludes 0. `prost` reads every one of these buffers: it has no rule to refuse
/// an absent field, which is where this codec is stricter.
///
/// The rows the conformance fixture holds run through the program with
/// their `prost` verdict; the rows only the zero-legality fixture holds
/// (`pb.zero`, no oracle) run as their own program, and `prost`'s verdict
/// for their one buffer is stated once against the oracle's `Inner`, whose
/// wire shape every struct of that fixture shares.
#[test]
fn absence_is_missing_required_when_the_default_is_illegal() {
    let missing = structure("MissingRequired");

    // `Required` is `level: Level [1..10]` at 1 and `gear: Gear` at 2.
    assert_refused("Required.level absent", "Required", &[0x10, 0x01], &missing);
    assert_refused("Required.gear absent", "Required", &[0x08, 0x01], &missing);
    assert_refused("Required empty", "Required", &[], &missing);
    assert_eq!(
        prost_read::<oracle::Required>(&[0x10, 0x01]),
        Ok(oracle::Required { level: 0, gear: 1 })
    );
    assert_eq!(prost_read::<oracle::Required>(&[]), Ok(Default::default()));

    // `Lengths` is `text: string [1..8]` at 1, `code: Code [2..4]` at 2,
    // `data: bytes [1..4]` at 3.
    let code_and_data = concat(&[&record(2, b"ab"), &record(3, &[0])]);
    assert_refused("Lengths.text absent", "Lengths", &code_and_data, &missing);
    let text_and_data = concat(&[&record(1, b"a"), &record(3, &[0])]);
    assert_refused("Lengths.code absent", "Lengths", &text_and_data, &missing);
    let text_and_code = concat(&[&record(1, b"a"), &record(2, b"ab")]);
    assert_refused("Lengths.data absent", "Lengths", &text_and_code, &missing);
    assert!(prost_read::<oracle::Lengths>(&code_and_data).is_ok());

    // `Packed.three` is `[Count; 3]`: a fixed count excludes the empty
    // array.
    assert_refused("Packed.three absent", "Packed", &[], &missing);
    assert_eq!(prost_read::<oracle::Packed>(&[]), Ok(Default::default()));

    // A union: the root with no arm, an element with no arm, an optional
    // union present with no arm. (A non-optional union field is in the
    // zero-legality fixture below.)
    assert_refused("Outcome with no arm", "Outcome", &[], &missing);
    assert_eq!(prost_read::<oracle::Outcome>(&[]), Ok(Default::default()));
    let element = record(4, &[]);
    assert_refused(
        "a Records.outcomes element with no arm",
        "Records",
        &element,
        &missing,
    );
    assert!(prost_read::<oracle::Records>(&element).is_ok());
    let present_empty = record(2, &[]);
    assert_refused(
        "OptionalComposites.outcome present with no arm",
        "OptionalComposites",
        &present_empty,
        &missing,
    );
    assert!(prost_read::<oracle::OptionalComposites>(&present_empty).is_ok());

    // A map entry whose key is absent while its type excludes the empty
    // string: `byString` is field 1, keyed by `Key [1..8]`.
    let key_absent = record(1, &[0x10, 0x01]);
    assert_refused(
        "a map entry whose Key is absent",
        "Maps",
        &key_absent,
        &missing,
    );
    assert_eq!(
        prost_read::<oracle::Maps>(&key_absent),
        Ok(oracle::Maps {
            by_string: [(String::new(), 1)].into(),
            ..Default::default()
        })
    );

    // An induced root whose field excludes 0: `ProbeSetBothParams` is
    // `(level: Level, count: Count)`.
    assert_refused(
        "ProbeSetBothParams.level absent",
        "ProbeSetBothParams",
        &[],
        &missing,
    );
    assert_eq!(
        prost_read::<oracle::ProbeSetBothParams>(&[]),
        Ok(Default::default())
    );

    // The empty payload of a box whose type excludes 0.
    for message in ["LevelBox", "GearBox", "CodeBox", "KeyBox"] {
        assert_refused(&format!("{message} empty"), message, &[], &missing);
    }
    assert_eq!(prost_read::<oracle::LevelBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::GearBox>(&[]), Ok(Default::default()));
    assert_eq!(prost_read::<oracle::CodeBox>(&[]), Ok(Default::default()));

    // The rows only the zero-legality fixture holds. One buffer, the label
    // alone, is read as each struct; `prost` reads it as `Inner { 0, "x" }`
    // and the empty payload as the default `Inner`, whatever the typl type.
    let label_only = [0x12, 0x01, 0x78];
    assert_eq!(prost_read::<oracle::Inner>(&label_only), Ok(inner(0, "x")));
    assert_eq!(prost_read::<oracle::Inner>(&[]), Ok(Default::default()));
    rustc::run_program(
        "pb_absence_illegal",
        &fixture_program(
            "proto3_zero_legality.ridl",
            r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{Malformed, Ref, VerifyError};

const LABEL_ONLY: &[u8] = &[0x12, 0x01, 0x78];

fn refused<T>(what: &str, result: Result<T, VerifyError>) {
    match result {
        Err(VerifyError::Structure(Malformed::MissingRequired)) => {}
        Err(other) => panic!("{what}: expected MissingRequired, got {other:?}"),
        Ok(_) => panic!("{what}: an absent field whose default is illegal must not verify"),
    }
}

fn main() {
    refused("an absent integer [1..10]", Ref::<'_, ValueRange, Proto3>::verify(LABEL_ONLY));
    refused("an absent Odd", Ref::<'_, ValueOdd, Proto3>::verify(LABEL_ONLY));
    refused("an absent Gear", Ref::<'_, ValueGear, Proto3>::verify(LABEL_ONLY));
    refused("an absent Short", Ref::<'_, ValueShort, Proto3>::verify(LABEL_ONLY));
    refused("an absent string [1..8]", Ref::<'_, ValueInlineShort, Proto3>::verify(LABEL_ONLY));
    refused("an absent [Count; 1..4]", Ref::<'_, ValueList, Proto3>::verify(LABEL_ONLY));
    // A nested struct absent: its empty message is verified by the same rule
    // and refused by its own `integer [1..10]` field.
    refused("an absent ValueRange", Ref::<'_, ValueNested, Proto3>::verify(LABEL_ONLY));
    refused("an absent Choice", Ref::<'_, ValueChoice, Proto3>::verify(LABEL_ONLY));

    refused("an empty Odd box", Ref::<'_, Odd, Proto3>::verify(&[]));
    refused("an empty Gear box", Ref::<'_, Gear, Proto3>::verify(&[]));
    refused("an empty Short box", Ref::<'_, Short, Proto3>::verify(&[]));
    refused("an empty Choice", Ref::<'_, Choice, Proto3>::verify(&[]));
}
"#,
        ),
    );
}

/// Whether an absent field reads as 0 follows the field's own range, step
/// and members: the FlatBuffers suite's case in proto3 terms. One buffer
/// carrying the label alone is read as structs that differ only in the type
/// of the absent field at number 1:
///
/// - a bare `integer` and a bare `float`, with no constraint, read as 0 and
///   0.0;
/// - `integer [0..10]` and `Even` (`[-1.0..1.0 step 1.0]`, whose grid holds
///   0) read as 0;
/// - `Mode`, whose zero member `OFF` is declared after `ON`, reads as
///   `OFF`, not as the first variant;
/// - `integer [1..10]` and `Odd` (`[-1.5..1.5 step 1.0]`, whose grid is
///   -1.5, -0.5, 0.5 and 1.5) are refused with `MissingRequired`: 0 is
///   inside `Odd`'s range but is not one of its values.
///
/// The empty payload, read as a `Mode`, an `Even`, a `Count` and an `Odd`
/// box, is checked the same way. `prost` reads the buffer as the oracle's
/// `Inner { 0, "x" }` whatever the type, which the previous test states.
#[test]
fn whether_absence_reads_as_zero_follows_the_type() {
    rustc::run_program(
        "pb_zero_legality",
        &fixture_program(
            "proto3_zero_legality.ridl",
            r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{Malformed, Ref, VerifyError};

const LABEL_ONLY: &[u8] = &[0x12, 0x01, 0x78];

fn refused<T>(what: &str, result: Result<T, VerifyError>) {
    match result {
        Err(VerifyError::Structure(Malformed::MissingRequired)) => {}
        Err(other) => panic!("{what}: expected MissingRequired, got {other:?}"),
        Ok(_) => panic!("{what}: an absent field whose type excludes 0 must not verify"),
    }
}

fn main() {
    let label = || Label::new_unchecked(String::from("x"));

    let proof: Ref<'_, ValueMode, Proto3> =
        Ref::verify(LABEL_ONLY).expect("an absent Mode reads as its zero member");
    assert_eq!(proof.decode(), ValueMode { value: Mode::Off, label: label() });

    let proof: Ref<'_, ValueZero, Proto3> =
        Ref::verify(LABEL_ONLY).expect("an absent integer [0..10] reads as 0");
    assert_eq!(proof.decode(), ValueZero { value: 0, label: label() });

    let proof: Ref<'_, ValueEven, Proto3> =
        Ref::verify(LABEL_ONLY).expect("an absent Even reads as 0.0");
    assert_eq!(proof.decode(), ValueEven { value: Even::new_unchecked(0.0), label: label() });

    let proof: Ref<'_, ValueBareInteger, Proto3> =
        Ref::verify(LABEL_ONLY).expect("an absent bare integer reads as 0");
    assert_eq!(proof.decode(), ValueBareInteger { value: 0, label: label() });

    let proof: Ref<'_, ValueBareFloat, Proto3> =
        Ref::verify(LABEL_ONLY).expect("an absent bare float reads as 0.0");
    assert_eq!(proof.decode(), ValueBareFloat { value: 0.0, label: label() });

    refused("an absent integer [1..10]", Ref::<'_, ValueRange, Proto3>::verify(LABEL_ONLY));
    refused("an absent Odd", Ref::<'_, ValueOdd, Proto3>::verify(LABEL_ONLY));

    let proof: Ref<'_, Mode, Proto3> =
        Ref::verify(&[]).expect("an empty Mode box reads as its zero member");
    assert_eq!(proof.decode(), Mode::Off);
    let proof: Ref<'_, Even, Proto3> = Ref::verify(&[]).expect("an empty Even box reads as 0.0");
    assert_eq!(proof.decode(), Even::new_unchecked(0.0));
    let proof: Ref<'_, Count, Proto3> = Ref::verify(&[]).expect("an empty Count box reads as 0");
    assert_eq!(proof.decode(), Count::new_unchecked(0));
    refused("an empty Odd box", Ref::<'_, Odd, Proto3>::verify(&[]));
}
"#,
        ),
    );
}

/// The appends `ridl diff` calls breaking that the proto3 codec reads all
/// the same: a non-optional string, struct, tuple, array or map whose every
/// element's default is legal. `ridl diff` calls them breaking because the
/// FlatBuffers codec refuses an absent offset, and the rule is held to the
/// stricter codec; proto3 reads an absent one as its empty value. The list
/// is pinned so that a change in either the diff or this codec is seen
/// here: a name here must be one the diff calls breaking and the codec
/// reads.
const BREAKING_BUT_READ_UNDER_PROTO3: &[&str] =
    &["WithInner", "WithLabel", "WithList", "WithMap", "WithTuple"];

/// **Every append `ridl diff` calls compatible is one this codec reads.**
///
/// Each `With*` struct of the evolution fixture gains one field from v1 to
/// v2. `ridl_diff::diff_packages` classifies each append; this codec,
/// generated from v1, writes one buffer per struct, which the codec
/// generated from v2 verifies. A compatible append must verify, so the diff
/// is never more lenient than this codec. A breaking append is refused with
/// `MissingRequired`, or read when it is one of
/// [`BREAKING_BUT_READ_UNDER_PROTO3`], the appends the FlatBuffers codec
/// refuses and this one does not.
#[test]
fn every_append_ridl_diff_calls_compatible_reads_an_old_payload() {
    let v1 = ir::compile_fixture("flatbuffers_evolution_v1.ridl");
    let v2 = ir::compile_fixture("flatbuffers_evolution_v2.ridl");
    let report = ridl_diff::diff_packages(&v1, &v2);

    let mut verdicts = BTreeMap::new();
    for change in &report.changes {
        assert_eq!(
            change.category,
            ridl_diff::Category::DeclAdded,
            "the fixture only appends fields, got {change:?}"
        );
        let [_, container, _] = change.path.split('/').collect::<Vec<_>>()[..] else {
            panic!("an appended field's path is package/struct/field, got {change:?}");
        };
        verdicts.insert(container.to_string(), change.verdict);
    }
    assert_eq!(
        verdicts.len(),
        16,
        "one append per `With*` struct: {verdicts:?}"
    );
    for name in BREAKING_BUT_READ_UNDER_PROTO3 {
        assert_eq!(
            verdicts.get(*name),
            Some(&ridl_diff::Verdict::Breaking),
            "`{name}` is listed as an append the diff calls breaking"
        );
    }

    let writes: String = verdicts
        .keys()
        .map(|name| format!("    println!(\"{{}}\", hex(&{name} {{ a: count.clone() }}));\n"))
        .collect();
    let transcript = rustc::run_program_capturing_stdout(
        "pb_append_v1",
        &fixture_program(
            "flatbuffers_evolution_v1.ridl",
            &format!(
                r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::Payload;

fn hex<T: Payload<Proto3>>(value: &T) -> String {{
    let mut out = vec![0u8; <T as Payload<Proto3>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes;
    bytes.iter().map(|byte| format!("{{byte:02x}}")).collect()
}}

fn main() {{
    let count = Count::new_unchecked(7);
{writes}}}
"#
            ),
        ),
    );
    let buffers: Vec<&str> = transcript.lines().collect();
    assert_eq!(
        buffers.len(),
        verdicts.len(),
        "one buffer per struct: {transcript:?}"
    );
    assert!(
        buffers.iter().all(|buffer| *buffer == "0807"),
        "every v1 buffer is `a: 7` alone: {buffers:?}"
    );

    let reads: String = verdicts
        .keys()
        .map(|name| format!("    read::<{name}>(\"{name}\");\n"))
        .collect();
    let transcript = rustc::run_program_capturing_stdout(
        "pb_append_v2",
        &fixture_program(
            "flatbuffers_evolution_v2.ridl",
            &format!(
                r#"
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{{Malformed, Payload, Ref, VerifyError}};

const V1_BUFFER: &[u8] = &[0x08, 0x07];

fn read<T: Payload<Proto3>>(name: &str) {{
    match Ref::<'_, T, Proto3>::verify(V1_BUFFER) {{
        Ok(proof) => {{
            let _ = proof.decode();
            println!("{{name}} read");
        }}
        Err(VerifyError::Structure(Malformed::MissingRequired)) => {{
            println!("{{name}} refused");
        }}
        Err(other) => panic!("{{name}}: expected a read or MissingRequired, got {{other:?}}"),
    }}
}}

fn main() {{
{reads}}}
"#
            ),
        ),
    );
    let mut outcomes = BTreeMap::new();
    for line in transcript.lines() {
        let (name, outcome) = line.split_once(' ').expect("`<struct> <outcome>`");
        outcomes.insert(name.to_string(), outcome.to_string());
    }
    for (name, verdict) in &verdicts {
        let expected = match verdict {
            ridl_diff::Verdict::Compatible => "read",
            _ if BREAKING_BUT_READ_UNDER_PROTO3.contains(&name.as_str()) => "read",
            _ => "refused",
        };
        assert_eq!(
            outcomes.get(name).map(String::as_str),
            Some(expected),
            "`ridl diff` calls the append to `{name}` {verdict:?}, so a v1 buffer of it \
             must be {expected} by the v2 proto3 codec"
        );
    }
}

/// The generated proto3 codec's source for a fixture, with `main` appended.
fn fixture_program(fixture: &str, main: &str) -> String {
    let package = ir::compile_fixture(fixture);
    let generated = ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::Proto3], &[])
        .expect("the fixture generates")
        .rust_source;
    format!("#![allow(dead_code)]\n{generated}\n{main}")
}
