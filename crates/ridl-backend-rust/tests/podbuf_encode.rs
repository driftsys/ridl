//! The podbuf encoder: the bytes `__ridl_podbuf_encode` writes, pinned
//! against the first-fit law and the value rules of ADR-0031 (decisions 3 to 5,
//! 9 and 10; the `§` numbers in this file are those of the archived design,
//! `docs/archive/2026-10-24-podbuf-design.md`).
//!
//! The generated code runs as a program through `support::rustc`, as
//! `flatbuffers_roundtrip.rs` does: a compile proof shows only that the
//! encoder type-checks, and which byte lands where is something only a run
//! can say. Each program encodes one value and prints the buffer as
//! hexadecimal, or the error the encoder returned.

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

/// The value builders every case draws on.
const HELPERS: &str = r#"
use ridl_rt::podbuf::{Aligned, Writer};

fn ev(code: i64, message: &str) -> FaultEvent {
    FaultEvent { code, message: message.into(), timestamp: 0 }
}

fn map(pairs: &[(&str, i64)]) -> Vec<(Name, Weight)> {
    pairs
        .iter()
        .map(|(key, value)| (Name::new_unchecked((*key).into()), Weight::new_unchecked(*value)))
        .collect()
}

fn door(sensor_id: i64, is_open: bool) -> DoorPayload {
    DoorPayload { sensor_id, is_open }
}

fn smap(pairs: &[(f64, i64)]) -> Vec<(f64, Weight)> {
    pairs
        .iter()
        .map(|(key, value)| (*key, Weight::new_unchecked(*value)))
        .collect()
}
"#;

/// Encodes `expr` and returns the buffer, panicking when the encoder refuses
/// the value.
fn run(expr: &str) -> Vec<u8> {
    let transcript = run_result(expr);
    let hex = transcript
        .strip_prefix("OK ")
        .unwrap_or_else(|| panic!("the encoder refused the value: {transcript}"));
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
        .collect()
}

/// Encodes `expr` and returns the transcript: `OK <hex>` for a buffer, or
/// `ERR EncodeError::<variant>` for a refusal.
fn run_result(expr: &str) -> String {
    let name = format!("podbuf_{:x}", hash(expr));
    let transcript = rustc::run_program_capturing_stdout(&name, &program_of(expr));
    transcript.trim().to_string()
}

/// A short stable name for a program, from its expression text.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The program that encodes `expr` and prints the transcript. A program's
/// unused `pub` items are dead code: the fixture declares more types than any
/// one program encodes. `deprecated` is allowed because one case builds a
/// `Legacy` by naming its deprecated field.
fn program_of(expr: &str) -> String {
    let generated = generated();
    format!(
        "#![allow(dead_code, deprecated)]\n{generated}\n{HELPERS}\n{}",
        format_args!(
            r#"
fn main() {{
    let value = {expr};
    let mut out = Aligned([0u8; 4096]);
    let mut writer = Writer::new(&mut out.0);
    if let Err(error) = value.__ridl_podbuf_encode(&mut writer) {{
        println!("ERR EncodeError::{{error:?}}");
        return;
    }}
    match writer.finish() {{
        Ok(bytes) => {{
            let mut text = String::from("OK ");
            for byte in bytes {{
                text.push_str(&format!("{{byte:02x}}"));
            }}
            println!("{{text}}");
        }}
        Err(error) => println!("ERR EncodeError::{{error:?}}"),
    }}
}}
"#
        )
    )
}

/// The key of the first entry of the map at head offset 0: the entry block's
/// offset from the array descriptor, then the key's string descriptor at the
/// start of the first entry.
fn first_key(bytes: &[u8]) -> String {
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let entries = u32_at(8);
    let key_off = u32_at(entries);
    let key_len = u32_at(entries + 4);
    String::from_utf8(bytes[key_off..key_off + key_len].to_vec()).expect("UTF-8")
}

/// §5.3: two one-byte fields at 0 and 1, pad 2..8, after the 8-byte preamble
/// whose `head_size` is 8 and whose `reserved` word is 0.
#[test]
fn door_payload_encodes_to_the_measured_16_bytes() {
    let bytes = run("door(7, true)");
    assert_eq!(bytes, [8, 0, 0, 0, 0, 0, 0, 0, 7, 1, 0, 0, 0, 0, 0, 0]);
}

/// §5.3: the descriptor at 4..12 keeps `timestamp` out of 8..16; the pads
/// are zero; the string's bytes and its NUL follow the head, and `len` does
/// not count the NUL.
#[test]
fn fault_event_writes_descriptor_pads_and_tail_in_order() {
    let bytes = run(r#"FaultEvent { code: 0x1234, message: "ab".into(), timestamp: -1 }"#);
    assert_eq!(bytes.len(), 8 + 24 + 3);
    assert_eq!(&bytes[0..8], &[24, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&bytes[8..10], &[0x34, 0x12]);
    assert_eq!(&bytes[10..12], &[0, 0]); // _pad2
    assert_eq!(&bytes[12..20], &[32, 0, 0, 0, 2, 0, 0, 0]); // off 32, len 2
    assert_eq!(&bytes[20..24], &[0, 0, 0, 0]); // _pad12
    assert_eq!(&bytes[24..32], &[0xff; 8]);
    assert_eq!(&bytes[32..35], b"ab\0");
}

/// §5.1: a descriptor whose `len` is 0 is all zero bytes and points at no
/// tail data, so an empty string writes no NUL.
#[test]
fn an_empty_string_is_a_zero_descriptor_with_no_tail() {
    let bytes = run(r#"FaultEvent { code: 0, message: String::new(), timestamp: 0 }"#);
    assert_eq!(bytes.len(), 8 + 24);
    assert_eq!(&bytes[12..20], &[0; 8]);
}

/// §5.2 and §7 "Encoder duty": an absent optional leaves its flag bit and
/// every byte of its descriptor zero; a present one sets bit 0 of the flags
/// byte at offset 1 and writes its descriptor.
#[test]
fn an_absent_optional_is_all_zero_and_a_present_one_sets_its_bit() {
    let absent = run("DiagFilter { severity: 2, category: None }");
    assert_eq!(absent.len(), 8 + 16);
    assert_eq!(
        &absent[8..24],
        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let present = run(r#"DiagFilter { severity: 2, category: Some("x".into()) }"#);
    assert_eq!(present[9], 1);
    assert_eq!(&present[12..20], &[24, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&present[24..26], b"x\0");
}

/// §5.1 and §14: the elements of an array of structs are 24-byte heads in
/// one block at stride `elem_size`, and each element's own tail (its
/// string) follows the block, in element order.
#[test]
fn fault_page_elements_are_24_byte_heads_with_their_strings_after_the_block() {
    let bytes = run(r#"FaultPage { faults: vec![ev(1, "a"), ev(2, "bc")] }"#);
    assert_eq!(&bytes[8..16], &[16, 0, 0, 0, 2, 0, 24, 0]); // off 16, len 2, elem_size 24
    assert_eq!(bytes.len(), 16 + 48 + 2 + 3);
    // The first element's head starts at 16 and its string at 64; the second
    // element's head starts at 40 and its string at 66.
    assert_eq!(&bytes[16..18], &[1, 0]);
    assert_eq!(&bytes[20..28], &[64, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&bytes[40..42], &[2, 0]);
    assert_eq!(&bytes[44..52], &[66, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(&bytes[64..69], b"a\0bc\0");
}

/// §6: an empty array is a zero descriptor with no block.
#[test]
fn an_empty_array_is_a_zero_descriptor() {
    let bytes = run("FaultPage { faults: Vec::new() }");
    assert_eq!(bytes.len(), 16);
    assert_eq!(&bytes[8..16], &[0; 8]);
}

/// §6: map entries are written sorted by key, whatever the order of the
/// domain value's `Vec`.
#[test]
fn map_entries_are_sorted_by_key() {
    let bytes = run(r#"Labels { by_name: map(&[("b", 2), ("a", 1)]) }"#);
    assert_eq!(first_key(&bytes), "a");
    // Two 16-byte entries at 16, each `{ key: desc, value: u8 }`, then the
    // keys' strings in entry order.
    assert_eq!(&bytes[8..16], &[16, 0, 0, 0, 2, 0, 16, 0]);
    assert_eq!(bytes.len(), 16 + 32 + 2 + 2);
    assert_eq!(bytes[24], 1); // the value of the entry whose key is "a"
    assert_eq!(bytes[40], 2);
    assert_eq!(&bytes[48..52], b"a\0b\0");
}

/// §6: two entries with one key are refused with the `Unique` rule.
#[test]
fn a_duplicate_map_key_is_refused_by_the_encoder() {
    let transcript = run_result(r#"Labels { by_name: map(&[("a", 2), ("a", 1)]) }"#);
    assert!(
        transcript.contains("EncodeError::Contract") && transcript.contains("Unique"),
        "got: {transcript}"
    );
}

/// §6: a string that holds U+0000 cannot carry its NUL terminator, so the
/// encoder refuses it.
#[test]
fn a_string_holding_u0000_is_refused_by_the_encoder() {
    let transcript = run_result(r#"FaultEvent { code: 1, message: "a\0b".into(), timestamp: 0 }"#);
    assert!(
        transcript.contains("EncodeError::Contract") && transcript.contains("Pattern"),
        "got: {transcript}"
    );
}

/// §14 and §5.1: the tail is written in field order (the string's bytes,
/// then the block of `i64`, then the block of `FaultEvent` heads with its
/// element's string after it), and each block starts at the alignment of
/// its element: the 3-byte string ends at 35 and the next block starts at
/// 40, not 35.
#[test]
fn tail_blocks_follow_field_order_at_their_element_alignment() {
    let bytes = run(r#"Ordered { label: "ab".into(), xs: vec![1, 2], faults: vec![ev(1, "a")] }"#);
    assert_eq!(bytes.len(), 82);
    assert_eq!(&bytes[8..16], &[32, 0, 0, 0, 2, 0, 0, 0]); // label: off 32, len 2
    assert_eq!(&bytes[16..24], &[40, 0, 0, 0, 2, 0, 8, 0]); // xs: off 40, len 2, elem_size 8
    assert_eq!(&bytes[24..32], &[56, 0, 0, 0, 1, 0, 24, 0]); // faults: off 56, len 1, elem_size 24
    assert_eq!(&bytes[32..35], b"ab\0");
    assert_eq!(&bytes[35..40], &[0; 5]); // the gap before the i64 block
    assert_eq!(&bytes[40..48], &1i64.to_le_bytes());
    assert_eq!(&bytes[48..56], &2i64.to_le_bytes());
    assert_eq!(&bytes[56..58], &[1, 0]); // the element's code
    assert_eq!(&bytes[60..68], &[80, 0, 0, 0, 1, 0, 0, 0]); // the element's message
    assert_eq!(&bytes[80..82], b"a\0");
}

/// §5.1: a map's entry block starts at alignment 8 after a 3-byte string.
#[test]
fn a_map_block_after_a_string_starts_at_alignment_8() {
    let bytes = run(r#"Keyed { label: "ab".into(), by_name: map(&[("a", 1)]) }"#);
    assert_eq!(bytes.len(), 50);
    assert_eq!(&bytes[8..16], &[24, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(&bytes[16..24], &[32, 0, 0, 0, 1, 0, 16, 0]); // off 32, not 27
    assert_eq!(&bytes[32..40], &[48, 0, 0, 0, 1, 0, 0, 0]); // the key's descriptor
    assert_eq!(bytes[40], 1);
    assert_eq!(&bytes[48..50], b"a\0");
}

/// §5.1: the arm head of a union inline in a struct starts at alignment 8
/// after that struct's 3-byte string.
#[test]
fn a_union_arm_after_a_string_starts_at_alignment_8() {
    let bytes = run(r#"Wrapped { label: "ab".into(), outcome: Outcome::Bad(Health::Warn) }"#);
    assert_eq!(bytes.len(), 48);
    assert_eq!(&bytes[8..16], &[32, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(bytes[16], 2); // the tag, inline at head offset 8
    assert_eq!(&bytes[20..28], &[40, 0, 0, 0, 8, 0, 0, 0]); // the arm: off 40, not 35
    assert_eq!(&bytes[32..35], b"ab\0");
    assert_eq!(&bytes[40..48], &[1, 0, 0, 0, 0, 0, 0, 0]);
}

/// §6: float keys sort numerically, a NaN key is refused with the `Range`
/// rule, and `0.0` and `-0.0` are one key, refused with `Unique`.
#[test]
fn float_map_keys_sort_numerically_and_refuse_nan_and_signed_zero_pairs() {
    let bytes = run("Scored { by_score: smap(&[(2.5, 1), (-1.0, 2)]) }");
    assert_eq!(bytes.len(), 48);
    assert_eq!(&bytes[8..16], &[16, 0, 0, 0, 2, 0, 16, 0]);
    assert_eq!(&bytes[16..24], &(-1.0f64).to_le_bytes());
    assert_eq!(bytes[24], 2);
    assert_eq!(&bytes[32..40], &2.5f64.to_le_bytes());
    assert_eq!(bytes[40], 1);
    let nan = run_result("Scored { by_score: smap(&[(f64::NAN, 1)]) }");
    assert!(
        nan.contains("EncodeError::Contract") && nan.contains("Range"),
        "got: {nan}"
    );
    let zeros = run_result("Scored { by_score: smap(&[(0.0, 1), (-0.0, 2)]) }");
    assert!(
        zeros.contains("EncodeError::Contract") && zeros.contains("Unique"),
        "got: {zeros}"
    );
}

/// §8.1: a deprecated field is written as its init value, 5, not as the
/// domain value.
#[test]
fn a_deprecated_field_is_written_as_its_init_value() {
    assert_eq!(run("Legacy { legacy: 7 }")[8], 5);
}

/// §5.2 and §6: a nested struct is inline at its head's alignment, an enum
/// is its narrowest integer, a tuple is an inline struct rounded to 8, and an
/// exact-length array is inline with no length; a later small field fills
/// the gap an earlier alignment left (`readings` at 10, before `span` at 16).
#[test]
fn a_nested_struct_an_enum_a_tuple_and_a_fixed_array_are_inline() {
    let bytes = run("Nested { door: door(3, false), health: Health::Fail, \
         span: NestedSpan { lo: 1, hi: 0x0302 }, readings: [0x0a0b, 0x0c0d] }");
    assert_eq!(bytes.len(), 8 + 24);
    assert_eq!(&bytes[8..16], &[3, 0, 0, 0, 0, 0, 0, 0]); // door, inline
    assert_eq!(bytes[16], 2); // health
    assert_eq!(bytes[17], 0); // _pad9
    assert_eq!(&bytes[18..22], &[0x0b, 0x0a, 0x0d, 0x0c]); // readings
    assert_eq!(&bytes[22..24], &[0, 0]); // _pad14
    assert_eq!(&bytes[24..32], &[1, 0, 0x02, 0x03, 0, 0, 0, 0]); // span
}

/// §6: a union is a tag in the head and a descriptor to the active arm's
/// head in the tail; an arm of struct type writes that struct's head, and an
/// arm of enum type writes a one-field head.
#[test]
fn a_union_writes_its_tag_and_its_arm_in_the_tail() {
    let ok = run("Outcome::Ok(door(7, true))");
    assert_eq!(ok.len(), 8 + 16 + 8);
    assert_eq!(
        &ok[8..24],
        &[1, 0, 0, 0, 24, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(&ok[24..32], &[7, 1, 0, 0, 0, 0, 0, 0]);
    let bad = run("Outcome::Bad(Health::Warn)");
    assert_eq!(bad.len(), 8 + 16 + 8);
    assert_eq!(
        &bad[8..24],
        &[2, 0, 0, 0, 24, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(&bad[24..32], &[1, 0, 0, 0, 0, 0, 0, 0]);
}

/// The encoder is deterministic and writes every byte it reserves: a dirty
/// buffer gives the same bytes as a zeroed one.
#[test]
fn every_reserved_byte_is_written() {
    let generated = generated();
    let main = r#"
fn main() {
    let value = FaultPage { faults: vec![ev(1, "a"), ev(2, "bc")] };
    let mut clean = Aligned([0u8; 256]);
    let mut dirty = Aligned([0xabu8; 256]);
    let mut first = Writer::new(&mut clean.0);
    value.__ridl_podbuf_encode(&mut first).expect("encode");
    let a = first.finish().expect("finish").to_vec();
    let mut second = Writer::new(&mut dirty.0);
    value.__ridl_podbuf_encode(&mut second).expect("encode");
    let b = second.finish().expect("finish").to_vec();
    assert_eq!(a, b, "a dirty buffer leaks a byte the encoder did not write");
}
"#;
    rustc::run_program(
        "podbuf_dirty",
        &format!("#![allow(dead_code, deprecated)]\n{generated}\n{HELPERS}\n{main}"),
    );
}
