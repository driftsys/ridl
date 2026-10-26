//! The podbuf `Payload` implementation: a value encodes through
//! `Ref::<T, Podbuf>::encode`, the bytes verify through
//! `Ref::<T, Podbuf>::verify`, and `decode` builds the value again; the
//! bound `MAX_SIZE` and the 8-aligned buffers are what the encoding contract
//! of `ridl-rt` asks of a generated type.
//!
//! Each test compiles one program over the generated fixture through
//! `support::rustc`, as `podbuf_encode.rs` does, and the program asserts.
//! `tests/fixtures/podbuf_roundtrip.ridl` holds the structs, `podbuf_face.ridl`
//! the interface whose size table is read.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

/// The generated source for `fixture`, with the podbuf codec.
fn generated(fixture: &str) -> String {
    let package = ir::compile_fixture(fixture);
    ridl_backend_rust::generate_pipeline(
        &package,
        &[ridl_backend_rust::WireEncoding::Podbuf],
        false,
        &[],
    )
    .expect("the fixture generates")
    .rust_source
}

/// Compiles and runs the `main` body `body` over the generated `fixture`.
/// A program's unused `pub` items are dead code: the fixture declares more
/// types than any one program uses.
fn run(fixture: &str, name: &str, body: &str) {
    let generated = generated(fixture);
    let values = if fixture == "podbuf_roundtrip.ridl" {
        VALUES
    } else {
        ""
    };
    let program = format!(
        "#![allow(dead_code, deprecated, unused_imports)]\n{generated}\n{HELPERS}\n{values}\nfn main() {{\n{body}\n}}\n"
    );
    rustc::run_program(name, &program);
}

/// The helpers every program draws on.
const HELPERS: &str = r#"
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{EncodeError, Payload, Ref, VerifyError};
use ridl_rt::podbuf::Aligned;

/// Encodes `value` into the type's own buffer, verifies the bytes through
/// `Ref::verify`, and requires `decode` to give the value back.
fn round_trip<T>(value: T) -> usize
where
    T: Payload<Podbuf> + PartialEq + core::fmt::Debug,
{
    let mut buf = <T as Payload<Podbuf>>::BUFFER;
    let proof = Ref::<T, Podbuf>::encode(&value, buf.as_mut()).expect("encode");
    let bytes = proof.bytes();
    assert!(bytes.len() <= <T as Payload<Podbuf>>::MAX_SIZE);
    let verified = Ref::<T, Podbuf>::verify(bytes).expect("the bytes verify");
    let decoded = verified.decode();
    assert_eq!(decoded, value);
    bytes.len()
}
"#;

/// Value builders over `podbuf_roundtrip.ridl`.
const VALUES: &str = r#"
fn ev(code: i64, message: &str) -> FaultEvent {
    FaultEvent { code, message: message.into(), timestamp: 0 }
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

"#;

#[test]
fn a_report_round_trips_through_ref_verify_and_decode() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_round_trips",
        r#"
    round_trip(door(7, true));
    round_trip(ev(7, "stuck"));
    round_trip(ev(0, ""));
    round_trip(FaultPage { faults: vec![ev(1, "a"), ev(2, "bcd"), ev(3, "")] });
    round_trip(Labels { by_name: map(&[("a", 1), ("b", 2), ("c", 3)]) });
    round_trip(DiagFilter { severity: 3, category: Some("engine".into()) });
    round_trip(DiagFilter { severity: 3, category: None });
    round_trip(Outcome::Ok(door(1, false)));
    round_trip(Outcome::Bad(Health::Fail));
    round_trip(Health::Warn);
    round_trip(WarningFlags(5));
    round_trip(Name::new_unchecked("a name".into()));
    round_trip(Weight::new_unchecked(200));
    "#,
    );
}

/// The `Ref` path of the verifier and the decoder: the view `Ref::verify`
/// hands back reads the fields in place, and `decode` builds the domain value
/// from that view.
#[test]
fn decode_builds_the_domain_value_from_the_view() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_decode_from_view",
        r#"
    let page = FaultPage { faults: vec![ev(1, "alpha"), ev(2, "beta")] };
    let mut buf = <FaultPage as Payload<Podbuf>>::BUFFER;
    let proof = Ref::<FaultPage, Podbuf>::encode(&page, buf.as_mut()).expect("encode");
    let verified = Ref::<FaultPage, Podbuf>::verify(proof.bytes()).expect("verify");
    let codes: Vec<i64> = verified.view().faults().map(|fault| fault.code()).collect();
    assert_eq!(codes, [1, 2]);
    let messages: Vec<&str> = verified.view().faults().map(|fault| fault.message()).collect();
    assert_eq!(messages, ["alpha", "beta"]);
    assert_eq!(verified.decode(), page);

    let mut buf = <Health as Payload<Podbuf>>::BUFFER;
    let proof = Ref::<Health, Podbuf>::encode(&Health::Warn, buf.as_mut()).expect("encode");
    let verified = Ref::<Health, Podbuf>::verify(proof.bytes()).expect("verify");
    assert_eq!(verified.view().value(), Health::Warn);
    assert_eq!(verified.decode(), Health::Warn);
    "#,
    );
}

#[test]
fn max_size_is_pinned_and_holds_the_largest_legal_value() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_max_size",
        r#"
    assert_eq!(<FaultPage as Payload<Podbuf>>::MAX_SIZE, 8 + 8 + 64 * 24 + 64 * 1025);
    // 256 characters of four bytes each: the longest legal message.
    let longest = "\u{1F600}".repeat(256);
    let page = FaultPage { faults: (0..64).map(|code| ev(code, &longest)).collect() };
    let written = round_trip(page);
    assert_eq!(written, <FaultPage as Payload<Podbuf>>::MAX_SIZE);
    "#,
    );
}

#[test]
fn the_buffer_is_8_aligned_and_exactly_max_size() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_buffer",
        r#"
    assert_eq!(core::mem::align_of::<<FaultPage as Payload<Podbuf>>::Buffer>(), 8);
    assert_eq!(core::mem::size_of::<<DoorPayload as Payload<Podbuf>>::Buffer>(), 16);
    assert_eq!(
        <DoorPayload as Payload<Podbuf>>::BUFFER.as_ref().len(),
        <DoorPayload as Payload<Podbuf>>::MAX_SIZE
    );
    assert_eq!(core::mem::align_of::<<Health as Payload<Podbuf>>::Buffer>(), 8);
    "#,
    );
}

#[test]
fn a_short_buffer_reports_capacity() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_capacity",
        r#"
    let mut out = Aligned([0u8; 8]);
    assert!(matches!(
        Ref::<FaultEvent, Podbuf>::encode(&ev(1, "a"), &mut out.0),
        Err(EncodeError::Capacity { .. })
    ));
    "#,
    );
}

#[test]
fn an_unaligned_output_is_refused_and_an_unaligned_input_does_not_verify() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_alignment",
        r#"
    let mut out = Aligned([0u8; 64]);
    assert!(matches!(
        Ref::<FaultEvent, Podbuf>::encode(&ev(1, "a"), &mut out.0[1..]),
        Err(EncodeError::Unaligned)
    ));
    let proof = Ref::<FaultEvent, Podbuf>::encode(&ev(1, "a"), &mut out.0).expect("encode");
    let mut copy = Aligned([0u8; 64]);
    let n = proof.bytes().len();
    copy.0[1..1 + n].copy_from_slice(proof.bytes());
    assert!(matches!(
        Ref::<FaultEvent, Podbuf>::verify(&copy.0[1..1 + n]),
        Err(VerifyError::Structure(_))
    ));
    "#,
    );
}

/// The view of an exact-length `bytes [4]` reads the four bytes at their
/// offsets, not shifted by one.
#[test]
fn an_exact_length_bytes_view_reads_the_bytes_in_place() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_exact_bytes",
        r#"
    let value = Digest { hash: vec![1, 2, 3, 4] };
    let mut buf = <Digest as Payload<Podbuf>>::BUFFER;
    let proof = Ref::<Digest, Podbuf>::encode(&value, buf.as_mut()).expect("encode");
    let verified = Ref::<Digest, Podbuf>::verify(proof.bytes()).expect("verify");
    let seen: Vec<u8> = verified.view().hash().collect();
    assert_eq!(seen, [1, 2, 3, 4]);
    assert_eq!(verified.decode(), value);
    "#,
    );
}

#[test]
fn a_buffer_above_max_size_does_not_verify() {
    run(
        "podbuf_roundtrip.ridl",
        "podbuf_payload_too_large",
        r#"
    let big = Aligned([0u8; 64]);
    assert!(matches!(
        Ref::<DoorPayload, Podbuf>::verify(&big.0[..24]),
        Err(VerifyError::Structure(_))
    ));
    // Exactly MAX_SIZE (16) is not too large; one byte more is.
    assert!(Ref::<DoorPayload, Podbuf>::verify(&big.0[..16]).is_ok());
    assert!(matches!(
        Ref::<DoorPayload, Podbuf>::verify(&big.0[..17]),
        Err(VerifyError::Structure(_))
    ));
    "#,
    );
}

/// The size table of an interface: the claim and event buffers are 8-aligned,
/// every number derives from the payloads' `MAX_SIZE`, and a call with two
/// parameters is sized as the induced struct it sends.
#[test]
fn the_size_table_is_aligned_and_derives_from_the_payload_bounds() {
    run(
        "podbuf_face.ridl",
        "podbuf_sizes_table",
        r#"
    use ridl_rt::contract::Sizes;
    assert_eq!(core::mem::align_of::<<Panel as Sizes<Podbuf>>::ClaimBuffer>(), 8);
    assert_eq!(core::mem::align_of::<<Panel as Sizes<Podbuf>>::EventBuffer>(), 8);

    let move_to = <PanelMoveToParams as Payload<Podbuf>>::MAX_SIZE;
    let level = <Level as Payload<Podbuf>>::MAX_SIZE;
    let alarm = <Alarm as Payload<Podbuf>>::MAX_SIZE;
    // The two-parameter call is sized as its induced struct, which has two
    // one-byte fields: preamble 8 plus a head of 8.
    assert_eq!(move_to, 16);
    assert_eq!(level, 16);

    // Members, in order: the signal, the event, the command, the query.
    let rows = <Panel as Sizes<Podbuf>>::RESERVATIONS;
    assert_eq!(rows[0], level as u64);
    assert_eq!(rows[1], alarm as u64);
    assert_eq!(rows[2], move_to as u64);
    assert_eq!(rows[3], (level + <Mode as Payload<Podbuf>>::MAX_SIZE) as u64);

    assert_eq!(<Panel as Sizes<Podbuf>>::MAX_BUFFER_SIZE, move_to.max(level).max(<Mode as Payload<Podbuf>>::MAX_SIZE));
    assert_eq!(<Panel as Sizes<Podbuf>>::EVENT_SOURCE_BUFFER_SIZE, alarm);
    assert_eq!(<Panel as Sizes<Podbuf>>::CLAIM_BUFFER.as_ref().len(), <Panel as Sizes<Podbuf>>::MAX_BUFFER_SIZE);

    // The induced struct round trips through the same path.
    round_trip(PanelMoveToParams { x: Level::new_unchecked(3), y: Level::new_unchecked(4) });
    "#,
    );
}
