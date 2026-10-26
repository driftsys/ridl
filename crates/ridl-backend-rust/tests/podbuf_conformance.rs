//! Conformance of the podbuf codec: the bytes of a value, the evolution of a
//! type between two versions, the drift between the emitted head layouts and
//! the layout law, and the behaviour of `verify` over damaged buffers.
//!
//! The bytes of a podbuf value are fully determined by the first-fit law
//! (`docs/wip/2026-10-24-podbuf-design.md`, §5), so the oracle is the law
//! itself: every expected buffer below is written by hand, field by field,
//! from §5.1 to §5.3 and §6, and the codec is held to those bytes. No other
//! implementation of the encoding exists to compare against.
//!
//! Each test that runs generated code compiles a program through
//! `support::rustc`, as the other podbuf targets do. The table, the
//! exhaustive mutation pass and the seeded random pass share one program and
//! one compile; the evolution tests compile one program per version.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/podbuf.rs"]
mod podbuf;
#[path = "support/rustc.rs"]
mod rustc;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::podbuf::{exceeds_descriptor_limits, max_size, struct_head, union_head};
use ridl_ir::v2;

// ---------------------------------------------------------------------------
// The byte table
// ---------------------------------------------------------------------------

/// One expected buffer: the type, a Rust expression for a value of it over
/// `podbuf_roundtrip.ridl`, and the bytes the law gives that value. `|`
/// separates the preamble, the head fields and the tail blocks.
struct Row {
    name: &'static str,
    ty: &'static str,
    value: &'static str,
    bytes: &'static str,
}

/// Each row's bytes are derived by hand. The preamble is `head_size: u32`
/// then `reserved: u32 = 0`. A pad is zero. A descriptor is `off: u32` (from
/// the buffer start) then `len: u32`, or `len: u16` then `elem_size: u16` for
/// an array or a map. The first tail byte is at `8 + head_size`, and each
/// tail block starts at its alignment.
const VALUE_TABLE: &[Row] = &[
    // §5.3: `sensor_id` at 0, `is_open` at 1, pad 2..8; head 8.
    Row {
        name: "DoorPayload",
        ty: "DoorPayload",
        value: "door(7, true)",
        bytes: "08000000 00000000 | 07 01 000000000000",
    },
    // §5.3: 0x0102 at 0, 0x0304 at 2, 5 at 4, pad 5..8; head 8.
    Row {
        name: "ClimateReport",
        ty: "ClimateReport",
        value: "ClimateReport { cabin: 0x0102, setpoint: 0x0304, fan: 5 }",
        bytes: "08000000 00000000 | 0201 0403 05 000000",
    },
    // §5.3: severity at 0, the flags byte at 1 (no bit set), pad 2..4, the
    // descriptor at 4..12 (all zero for an absent optional), pad 12..16;
    // head 16.
    Row {
        name: "DiagFilter, absent",
        ty: "DiagFilter",
        value: "DiagFilter { severity: 2, category: None }",
        bytes: "10000000 00000000 | 02 00 0000 | 0000000000000000 | 00000000",
    },
    // The same with bit 0 of the flags byte set: the descriptor holds
    // `off` 24 (8 + 16), `len` 2, and the tail holds "ab" and a NUL.
    Row {
        name: "DiagFilter, present",
        ty: "DiagFilter",
        value: r#"DiagFilter { severity: 3, category: Some("ab".into()) }"#,
        bytes: "10000000 00000000 | 03 01 0000 | 18000000 02000000 | 00000000 | 6162 00",
    },
    // §5.3: code at 0, pad 2..4, the descriptor at 4..12 (`off` 32, `len` 4
    // bytes of the three scalar values of "h\u{e9}y"), pad 12..16, the i64 at
    // 16..24; head 24. The tail holds the UTF-8 bytes and a NUL.
    Row {
        name: "FaultEvent",
        ty: "FaultEvent",
        value: r#"ev(0x0102, "h\u{e9}y", 0x0102030405060708)"#,
        bytes: "18000000 00000000 | 0201 0000 | 20000000 04000000 | 00000000 \
                | 0807060504030201 | 68c3a979 00",
    },
    // §6: an empty array is an all-zero descriptor and no block.
    Row {
        name: "FaultPage, no events",
        ty: "FaultPage",
        value: "FaultPage { faults: Vec::new() }",
        bytes: "08000000 00000000 | 0000000000000000",
    },
    // §5.3: one descriptor (`off` 16, `len` 1, `elem_size` 24); the element
    // head at 16..40, then its string at 40, the end of the block.
    Row {
        name: "FaultPage, one event",
        ty: "FaultPage",
        value: r#"FaultPage { faults: vec![ev(7, "a", 0)] }"#,
        bytes: "08000000 00000000 | 10000000 0100 1800 \
                | 0700 0000 | 28000000 01000000 | 00000000 | 0000000000000000 \
                | 6100",
    },
    // Four elements at 16, 40, 64 and 88; the block ends at 112, where the
    // strings follow in element order: "a" at 112, "bc" at 114. The first and
    // the last element have an empty string, so a zero descriptor.
    Row {
        name: "FaultPage, four events",
        ty: "FaultPage",
        value: r#"FaultPage { faults: vec![
            ev(1, "", 0), ev(2, "a", 0), ev(3, "bc", 0), ev(4, "", -1)] }"#,
        bytes: "08000000 00000000 | 10000000 0400 1800 \
                | 0100 0000 | 0000000000000000 | 00000000 | 0000000000000000 \
                | 0200 0000 | 70000000 01000000 | 00000000 | 0000000000000000 \
                | 0300 0000 | 72000000 02000000 | 00000000 | 0000000000000000 \
                | 0400 0000 | 0000000000000000 | 00000000 | ffffffffffffffff \
                | 6100 626300",
    },
    // §6: tag 1 (the ordinal of `ok`) at 0, pad 1..4, the arm descriptor at
    // 4..12 (`off` 24, `len` 8), pad 12..16; head 16. The arm is the
    // `DoorPayload` head, 8 bytes, with no preamble.
    Row {
        name: "Outcome, ok",
        ty: "Outcome",
        value: "Outcome::Ok(door(9, false))",
        bytes: "10000000 00000000 | 01 000000 | 18000000 08000000 | 00000000 \
                | 09 00 000000000000",
    },
    // The `bad` arm (tag 2) holds an enum, so its arm head is one field at
    // offset 0: `Health::Fail` is 2.
    Row {
        name: "Outcome, bad",
        ty: "Outcome",
        value: "Outcome::Bad(Health::Fail)",
        bytes: "10000000 00000000 | 02 000000 | 18000000 08000000 | 00000000 \
                | 02 000000 00000000",
    },
    // §6: a map is a descriptor to entries sorted by key. An entry is
    // `{ key: descriptor at 0..8, value: u8 at 8 }`, 16 bytes. The block is
    // at 16..48, so "a" is at 48 and "b" at 50. The domain value is listed
    // in key order, the order `decode` returns; the table phase also writes
    // it in the other order.
    Row {
        name: "Labels",
        ty: "Labels",
        value: r#"Labels { by_name: map(&[("a", 1), ("b", 2)]) }"#,
        bytes: "08000000 00000000 | 10000000 0200 1000 \
                | 30000000 01000000 | 01 | 00000000000000 \
                | 32000000 01000000 | 02 | 00000000000000 \
                | 6100 6200",
    },
    // §5.2: the inline `DoorPayload` head at 0..8, the enum at 8, pad 9,
    // the two-element array at 10..14 (a later small field fills the gap),
    // pad 14..16, the tuple at 16..24 (`lo` at 0, `hi` at 2); head 24.
    Row {
        name: "Nested",
        ty: "Nested",
        value: "Nested { door: door(5, true), health: Health::Warn, \
                span: NestedSpan { lo: 1, hi: 0x0302 }, readings: [0x0a0b, 0x0c0d] }",
        bytes: "18000000 00000000 | 0501 000000000000 | 01 00 | 0b0a 0d0c | 0000 \
                | 01 00 0203 00000000",
    },
];

/// The values of the table over `podbuf_roundtrip.ridl`.
const VALUES: &str = r#"
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
"#;

/// The program over the table: one phase each for the table, the stride
/// refusals, the exhaustive pass and the random pass. Run once.
fn table_transcript() -> &'static str {
    static TRANSCRIPT: OnceLock<String> = OnceLock::new();
    TRANSCRIPT.get_or_init(|| {
        let package = ir::compile_fixture("podbuf_roundtrip.ridl");
        let generated = podbuf::generated(&package, &[]);
        let rows: String = VALUE_TABLE
            .iter()
            .map(|row| {
                format!(
                    "        row::<{}>(&mut rows, {:?}, {}, {:?});\n",
                    row.ty, row.name, row.value, row.bytes
                )
            })
            .collect();
        let main = format!(
            r#"
    let mut rows: Vec<Row> = Vec::new();
    phase("table", || {{
{rows}
        let labels = rows.iter().find(|row| row.name == "Labels").expect("the Labels row");
        let unsorted = Labels {{ by_name: map(&[("b", 2), ("a", 1)]) }};
        assert_eq!(encode(&unsorted), labels.bytes, "the entries are written in key order");
        format!("rows={{}}", rows.len())
    }});
    phase("stride", stride_refusals);
    phase("exhaustive", || exhaustive(&rows));
    phase("random", || random(&rows));
"#
        );
        let helpers = format!("{VALUES}\n{}\n{STRIDE}", podbuf::HARNESS);
        rustc::run_program_capturing_stdout(
            "podbuf_conformance_table",
            &podbuf::program(&generated, &helpers, &main),
        )
    })
}

/// The line of the transcript for `phase`, asserted to be a success.
fn phase_line(phase: &str) -> &'static str {
    let line = table_transcript()
        .lines()
        .find(|line| line.starts_with(&format!("{phase} ")))
        .unwrap_or_else(|| panic!("no `{phase}` line in:\n{}", table_transcript()));
    assert!(
        line.starts_with(&format!("{phase} ok")),
        "the `{phase}` phase failed: {line}"
    );
    line
}

/// `key=N` from a phase line.
fn count(line: &str, key: &str) -> usize {
    line.split_whitespace()
        .find_map(|word| word.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("no `{key}` in {line}"))
        .parse()
        .expect("a number")
}

#[test]
fn the_value_table_encodes_to_the_expected_bytes() {
    // Each row's `row` call asserts the bytes the encoder writes, that they
    // verify, and that `decode` gives the value back.
    let line = phase_line("table");
    assert_eq!(count(line, "rows"), VALUE_TABLE.len());
}

#[test]
fn every_row_of_the_table_is_a_buffer_of_its_declared_head_size() {
    // A hand-written row is easy to get wrong in its preamble; the head size
    // it names is the number the law gives, and the first tail byte follows.
    for row in VALUE_TABLE {
        let bytes = podbuf::hex_bytes(row.bytes);
        let head_size = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(
            head_size % 8,
            0,
            "{}: head_size is a multiple of 8",
            row.name
        );
        assert!(bytes.len() >= 8 + head_size, "{}: the head fits", row.name);
        assert_eq!(&bytes[4..8], &[0; 4], "{}: reserved is zero", row.name);
    }
}

/// Stride refusals (§5.1, §7 rule 5): the element size of a struct array is a
/// multiple of 8, and the element size of a scalar array is exact.
const STRIDE: &str = r#"
fn stride_refusals() -> String {
    // A one-event page whose `elem_size` (at buffer offset 14) is 20, which is
    // not a multiple of 8.
    let page = "08000000 00000000 | 10000000 0100 1400 \
                | 0700 0000 | 28000000 01000000 | 00000000 | 0000000000000000 | 6100";
    aligned_bytes!(page_bytes, page);
    assert_eq!(verdict::<FaultPage>(page_bytes), "Stride");
    // The same page with the right size is accepted.
    aligned_bytes!(good, &page.replace("0100 1400", "0100 1800"));
    assert_eq!(verdict::<FaultPage>(good), "OK");

    // An array of two-byte scalars: the element size has to be exactly 2.
    let readings = "08000000 00000000 | 10000000 0200 0200 | 0100 0200";
    aligned_bytes!(readings_bytes, readings);
    assert_eq!(verdict::<Readings>(readings_bytes), "OK");
    for wrong in ["0400", "0100", "0800"] {
        aligned_bytes!(patched, &readings.replace("0200 0200", &format!("0200 {wrong}")));
        assert_eq!(verdict::<Readings>(patched), "Stride", "elem_size {wrong}");
    }
    "refused=4".to_string()
}
"#;

#[test]
fn a_stride_that_is_not_the_element_size_is_refused() {
    let line = phase_line("stride");
    assert_eq!(count(line, "refused"), 4);
}

#[test]
fn every_truncation_and_byte_replacement_is_refused_or_read_in_bounds() {
    // The pass panics, and the phase fails, when `verify` or a read of an
    // accepted buffer panics, or when an accepted buffer does not decode and
    // encode again to bytes that verify.
    let line = phase_line("exhaustive");
    let cases = count(line, "cases");
    let accepted = count(line, "accepted");
    // A pass that refuses everything at the preamble tests nothing past it:
    // replacing a pad byte, or a value byte inside its range, is accepted.
    assert!(cases > 1_000, "the pass ran {cases} cases");
    assert!(
        accepted > cases / 20,
        "the pass accepted {accepted} of {cases} cases"
    );
    assert!(accepted < cases, "the pass refused no case");
}

#[test]
fn a_seeded_random_mutation_pass_finds_no_panic() {
    let line = phase_line("random");
    assert_eq!(count(line, "cases"), 10_000);
    assert!(
        count(line, "accepted") > 0,
        "the pass accepted no buffer: {line}"
    );
}

// ---------------------------------------------------------------------------
// Evolution
// ---------------------------------------------------------------------------

// The two versions of `FaultEvent`, `FaultPage`, `Entry` and `Entries`
// (`podbuf_evolution_v1.ridl` and `podbuf_evolution_v2.ridl`), written by
// hand from the law. Version 1 holds `code`, `message` and `timestamp`;
// version 2 appends `priority: [0..9]` and `extra: i64` to `FaultEvent`, and
// `note: string?` to `Entry`.
//
// Offsets of the appended fields. First-fit places `priority` (1 byte) at the
// lowest free offset, which is 2, inside version 1's 24-byte head, because
// `code` takes 0..2 and the descriptor of `message` takes 4..12. `extra`
// (8 bytes, aligned to 8) fits at none of 0..8, 8..16 or 16..24, so it goes
// at 24 and the head grows to 32. `note` is the first optional field, so the
// flags byte is placed first, at offset 1, and the descriptor goes at 4..12:
// version 2's `Entry` head is 16 bytes against version 1's 8.

/// A version 1 `FaultEvent { code: 1, message: "a", timestamp }` with the
/// timestamp 0x0102030405060708: head 24, the string at 32.
const V1_EVENT: &str = "18000000 00000000 | 0100 0000 | 20000000 01000000 | 00000000 \
                        | 0807060504030201 | 6100";

/// A version 2 `FaultEvent { code: 1, message: "a", timestamp, priority: 9,
/// extra: -2 }`: head 32, `priority` at 2, `extra` at 24, the string at 40.
const V2_EVENT: &str = "20000000 00000000 | 0100 09 00 | 28000000 01000000 | 00000000 \
                        | 0807060504030201 | feffffffffffffff | 6100";

/// A version 1 page of two events, `(1, "a")` and `(2, "bc")`, timestamps 0:
/// `elem_size` 24, the block at 16..64, the strings at 64 and 66.
const V1_PAGE: &str = "08000000 00000000 | 10000000 0200 1800 \
                       | 0100 0000 | 40000000 01000000 | 00000000 | 0000000000000000 \
                       | 0200 0000 | 42000000 02000000 | 00000000 | 0000000000000000 \
                       | 6100 626300";

/// A version 2 page of the same two strings, with priorities 5 and 6 and
/// extras 7 and 8: `elem_size` 32, the block at 16..80, the strings at 80
/// and 82.
const V2_PAGE: &str = "08000000 00000000 | 10000000 0200 2000 \
                       | 0100 05 00 | 50000000 01000000 | 00000000 | 0000000000000000 \
                       | 0700000000000000 \
                       | 0200 06 00 | 52000000 02000000 | 00000000 | 0000000000000000 \
                       | 0800000000000000 \
                       | 6100 626300";

/// A version 1 `Entries` of two entries, ids 3 and 4: `elem_size` 8.
const V1_ENTRIES: &str = "08000000 00000000 | 10000000 0200 0800 \
                          | 03 00000000000000 | 04 00000000000000";

/// A version 2 `Entries`: `elem_size` 16; the second entry has the note "x",
/// whose string is at 48, the end of the block.
const V2_ENTRIES: &str = "08000000 00000000 | 10000000 0200 1000 \
                          | 03 00 0000 | 0000000000000000 | 00000000 \
                          | 04 01 0000 | 30000000 01000000 | 00000000 \
                          | 7800";

/// `V1_ENTRIES` with a byte set in the first entry's pad at 4..8, which
/// version 2 reads as the first half of the descriptor of an absent `note`.
const V1_ENTRIES_DIRTY: &str = "08000000 00000000 | 10000000 0200 0800 \
                                | 03 000000 01000000 | 04 00000000000000";

/// `V1_EVENT` with the pad byte at head offset 2 set to 5, and to 255: the
/// byte that version 2 reads as `priority`.
const V1_EVENT_PAD_5: &str = "18000000 00000000 | 0100 05 00 | 20000000 01000000 | 00000000 \
                              | 0807060504030201 | 6100";
const V1_EVENT_PAD_255: &str = "18000000 00000000 | 0100 ff 00 | 20000000 01000000 | 00000000 \
                                | 0807060504030201 | 6100";

/// The constants every evolution program reads.
fn evolution_consts() -> String {
    [
        ("V1_EVENT", V1_EVENT),
        ("V2_EVENT", V2_EVENT),
        ("V1_PAGE", V1_PAGE),
        ("V2_PAGE", V2_PAGE),
        ("V1_ENTRIES", V1_ENTRIES),
        ("V2_ENTRIES", V2_ENTRIES),
        ("V1_ENTRIES_DIRTY", V1_ENTRIES_DIRTY),
        ("V1_EVENT_PAD_5", V1_EVENT_PAD_5),
        ("V1_EVENT_PAD_255", V1_EVENT_PAD_255),
    ]
    .iter()
    .map(|(name, spec)| format!("const {name}: &str = {spec:?};\n"))
    .collect()
}

/// Compiles and runs the evolution program of `fixture` with `main`.
fn run_evolution(fixture: &str, name: &str, main: &str) {
    let package = ir::compile_fixture(fixture);
    let generated = podbuf::generated(&package, &[]);
    rustc::run_program(
        name,
        &podbuf::program(&generated, &evolution_consts(), main),
    );
}

/// The version 1 codec: it writes the version 1 bytes, and it reads the
/// version 2 bytes by ignoring what it does not know (§8.1, "older reader,
/// newer head").
#[test]
fn an_older_reader_ignores_an_appended_field_in_a_longer_head() {
    run_evolution(
        "podbuf_evolution_v1.ridl",
        "podbuf_evolution_v1_reads_v2",
        r#"
    let event = FaultEvent { code: 1, message: "a".into(), timestamp: 0x0102030405060708 };
    assert_eq!(encode(&event), parse(V1_EVENT), "version 1 writes the hand-derived bytes");
    // Version 2's `priority` sits at offset 2, inside version 1's head; its
    // `extra` is past version 1's 24 bytes. Version 1 reads neither.
    aligned_bytes!(v2_event, V2_EVENT);
    let proof = Ref::<FaultEvent, Podbuf>::verify(v2_event).expect("v1 accepts a v2 buffer");
    assert_eq!(proof.view().code(), 1);
    assert_eq!(proof.view().message(), "a");
    assert_eq!(proof.view().timestamp(), 0x0102030405060708);
    assert_eq!(proof.decode(), event);

    // The pad at offset 2 is not checked: any byte there is accepted.
    for spec in [V1_EVENT_PAD_5, V1_EVENT_PAD_255] {
        aligned_bytes!(padded, spec);
        let proof = Ref::<FaultEvent, Podbuf>::verify(padded).expect("pad bytes are not checked");
        assert_eq!(proof.decode(), event);
    }
"#,
    );
}

/// Both stride directions, from the version 1 side: it steps by the
/// producer's `elem_size` of 32 and reads its 24-byte prefix, and an entry
/// with a longer head and an optional field reads by the same rule.
#[test]
fn stride_differs_from_the_reader_in_both_directions_seen_by_version_1() {
    run_evolution(
        "podbuf_evolution_v1.ridl",
        "podbuf_evolution_v1_stride",
        r#"
    let page = FaultPage {
        faults: vec![
            FaultEvent { code: 1, message: "a".into(), timestamp: 0 },
            FaultEvent { code: 2, message: "bc".into(), timestamp: 0 },
        ],
    };
    assert_eq!(encode(&page), parse(V1_PAGE));
    // `elem_size` 32 against a 24-byte head: the reader steps by 32.
    aligned_bytes!(v2_page, V2_PAGE);
    let proof = Ref::<FaultPage, Podbuf>::verify(v2_page).expect("v1 accepts a v2 page");
    let faults: Vec<_> = proof.view().faults().collect();
    assert_eq!(faults.len(), 2);
    assert_eq!((faults[0].code(), faults[0].message()), (1, "a"));
    assert_eq!((faults[1].code(), faults[1].message()), (2, "bc"));
    assert_eq!(proof.decode(), page);

    let entries = Entries { items: vec![Entry { id: 3 }, Entry { id: 4 }] };
    assert_eq!(encode(&entries), parse(V1_ENTRIES));
    aligned_bytes!(v2_entries, V2_ENTRIES);
    let proof = Ref::<Entries, Podbuf>::verify(v2_entries).expect("v1 accepts v2 entries");
    assert_eq!(proof.decode(), entries);
    // A byte in the pad of a version 1 entry is not checked by version 1.
    aligned_bytes!(dirty, V1_ENTRIES_DIRTY);
    assert_eq!(verdict::<Entries>(dirty), "OK");

    // `Payload::verify` refuses a buffer longer than the reader's own
    // `MAX_SIZE`, which the reader sized its receive buffer from, whatever the
    // content: 8 + 8 + 8 * 8 = 80 bytes for version 1. Unreferenced tail bytes
    // are not checked, so 40 of them take the 50 bytes of `V2_ENTRIES` past it.
    assert_eq!(<Entries as Payload<Podbuf>>::MAX_SIZE, 80);
    let long = format!("{V2_ENTRIES} | {}", "00".repeat(40));
    aligned_bytes!(too_long, &long);
    assert_eq!(too_long.len(), 90);
    assert_eq!(verdict::<Entries>(too_long), "TooLarge");
"#,
    );
}

/// The version 2 codec: it writes the version 2 bytes, and it reads the
/// version 1 bytes, where an appended field is 0 (§8.1, "newer reader, older
/// head"): `priority` is in a pad that the encoder wrote as zero, and
/// `extra` is past the head the producer wrote.
#[test]
fn a_newer_reader_reads_zero_for_a_field_the_producer_did_not_write() {
    run_evolution(
        "podbuf_evolution_v2.ridl",
        "podbuf_evolution_v2_reads_v1",
        r#"
    let event = FaultEvent {
        code: 1,
        message: "a".into(),
        timestamp: 0x0102030405060708,
        priority: 9,
        extra: -2,
    };
    assert_eq!(encode(&event), parse(V2_EVENT), "version 2 writes the hand-derived bytes");

    aligned_bytes!(v1_event, V1_EVENT);
    let proof = Ref::<FaultEvent, Podbuf>::verify(v1_event).expect("v2 accepts a v1 buffer");
    // `priority` is at offset 2, inside the 24 bytes the producer wrote.
    assert_eq!(proof.view().priority(), 0);
    // `extra` is at 24, past the producer's `head_size` of 24.
    assert_eq!(proof.view().extra(), 0);
    assert_eq!(proof.view().code(), 1);
    assert_eq!(proof.view().timestamp(), 0x0102030405060708);
    assert_eq!(
        proof.decode(),
        FaultEvent { priority: 0, extra: 0, ..event.clone() }
    );

    // The byte in the pad is the field once the reader knows the field. 5 is
    // a `priority`; 255 is outside `[0..9]` and the range check refuses it.
    aligned_bytes!(five, V1_EVENT_PAD_5);
    let proof = Ref::<FaultEvent, Podbuf>::verify(five).expect("5 is a priority");
    assert_eq!(proof.view().priority(), 5);
    aligned_bytes!(wild, V1_EVENT_PAD_255);
    assert_eq!(verdict::<FaultEvent>(wild), "Contract");
"#,
    );
}

/// Both stride directions, from the version 2 side: a producer of version 1
/// wrote 24-byte elements and `extra` reads 0 from each; the entry's flags
/// byte and descriptor lie past or inside the producer's 8 bytes, and the
/// check that an absent optional is zero still applies to them.
#[test]
fn stride_differs_from_the_reader_in_both_directions_seen_by_version_2() {
    run_evolution(
        "podbuf_evolution_v2.ridl",
        "podbuf_evolution_v2_stride",
        r#"
    let page = FaultPage {
        faults: vec![
            FaultEvent { code: 1, message: "a".into(), timestamp: 0, priority: 5, extra: 7 },
            FaultEvent { code: 2, message: "bc".into(), timestamp: 0, priority: 6, extra: 8 },
        ],
    };
    assert_eq!(encode(&page), parse(V2_PAGE));
    // Elements of 24 bytes read by a 32-byte head: `extra` and `priority`
    // read 0 in every element, and the strings read.
    aligned_bytes!(v1_page, V1_PAGE);
    let proof = Ref::<FaultPage, Podbuf>::verify(v1_page).expect("v2 accepts a v1 page");
    let faults: Vec<_> = proof.view().faults().collect();
    assert_eq!(faults.len(), 2);
    assert!(faults.iter().all(|fault| fault.extra() == 0 && fault.priority() == 0));
    assert_eq!((faults[0].message(), faults[1].message()), ("a", "bc"));

    let entries = Entries {
        items: vec![Entry { id: 3, note: None }, Entry { id: 4, note: Some("x".into()) }],
    };
    assert_eq!(encode(&entries), parse(V2_ENTRIES));
    // Entries of 8 bytes read by a 16-byte head: the flags byte at offset 1
    // is inside the 8 bytes and reads 0 (absent), and the descriptor at
    // 4..12 is cut at the producer's 8, so its second word reads 0.
    aligned_bytes!(v1_entries, V1_ENTRIES);
    let proof = Ref::<Entries, Podbuf>::verify(v1_entries).expect("v2 accepts v1 entries");
    assert!(proof.view().items().all(|entry| entry.note().is_none()));
    assert_eq!(
        proof.decode(),
        Entries { items: vec![Entry { id: 3, note: None }, Entry { id: 4, note: None }] }
    );
    // A byte set where the absent note's descriptor starts: version 1 does
    // not check its pad, version 2 checks that an absent optional is zero.
    aligned_bytes!(dirty, V1_ENTRIES_DIRTY);
    assert_ne!(
        verdict::<Entries>(dirty),
        "OK",
        "an absent optional with a nonzero descriptor is refused"
    );
"#,
    );
}

// ---------------------------------------------------------------------------
// Cross-package inline head
// ---------------------------------------------------------------------------

/// A head that holds a struct of another package inline and an array of it:
/// the generated head names the foreign head struct through a `crate::` path.
/// The bytes are derived from the law: `tag` at 0, the `Pair` head at 8..16,
/// the descriptor of `pairs` at 16..24 (`off` 32, `len` 2, `elem_size` 8);
/// each `Pair` is `a` at 0 and `b` at 2.
#[test]
fn a_head_inlines_a_struct_of_another_package() {
    let packages = podbuf::compile_fixture_workspace(&[
        ("parts", "pod.parts", "podbuf_cross_parts.ridl"),
        ("holder", "pod.holder", "podbuf_cross_holder.ridl"),
    ]);
    let parts = packages
        .iter()
        .find(|package| package.name == "pod.parts")
        .expect("the parts package");
    let holder = packages
        .iter()
        .find(|package| package.name == "pod.holder")
        .expect("the holder package");
    let parts_source = podbuf::generated(parts, &[holder]);
    let holder_source = podbuf::generated(holder, &[parts]);
    assert!(
        holder_source.contains("crate::pod::parts::PairPodbufHead"),
        "the holder's head names the foreign head struct:\n{holder_source}"
    );
    let modules = format!(
        "pub mod pod {{\npub mod parts {{\n{parts_source}\n}}\npub mod holder {{\n{holder_source}\n}}\n}}\n\
         use pod::holder::Holder;\nuse pod::parts::Pair;\n"
    );
    let main = r#"
    let value = Holder {
        tag: 7,
        pair: Pair { a: 1, b: 0x0302 },
        pairs: vec![Pair { a: 4, b: 5 }, Pair { a: 6, b: 7 }],
    };
    let expected = parse(
        "18000000 00000000 | 07 00000000000000 | 01 00 0203 00000000 \
         | 20000000 0200 0800 | 04 00 0500 00000000 | 06 00 0700 00000000",
    );
    assert_eq!(encode(&value), expected);
    aligned_bytes!(bytes, "18000000 00000000 | 07 00000000000000 | 01 00 0203 00000000 \
         | 20000000 0200 0800 | 04 00 0500 00000000 | 06 00 0700 00000000");
    let proof = Ref::<Holder, Podbuf>::verify(bytes).expect("the bytes verify");
    assert_eq!(proof.view().pair().b(), 0x0302);
    assert_eq!(proof.decode(), value);
"#;
    rustc::run_program("podbuf_cross_package", &podbuf::program(&modules, "", main));
}

// ---------------------------------------------------------------------------
// The layout drift test
// ---------------------------------------------------------------------------

/// The workspaces the drift test reads beside the fixture, relative to the
/// repository root. A package whose podbuf codec is refused is compared with
/// the layout law's own refusal.
const CORPUS: &[&str] = &[
    "crates/ridlc/tests/corpus/diag-showcase",
    "crates/ridlc/tests/corpus/ridl-diag-showcase",
    "crates/ridlc/tests/corpus/rsdl-appendix-a",
    "crates/ridlc/tests/corpus/rsdl-diag-showcase",
    "crates/ridlc/tests/corpus/services-workspace",
    "crates/ridlc/tests/corpus/veh-cluster",
    "crates/ridlc/tests/corpus/veh-common",
    "crates/ridlc/tests/corpus/workspace-two-members",
    "examples/cabin",
    "evals/corpus/mavlink",
    "evals/corpus/ros2",
    "evals/corpus/vss",
];

/// A head as `(offset, size)` pairs sorted by offset, the pads included: a
/// placement of size 0 (an inline head with no field) holds no byte and has
/// no member in the generated struct.
fn law_members(head: &ridl_ir::projection::podbuf::Head) -> Vec<(u32, u32)> {
    let mut members: Vec<(u32, u32)> = head
        .fields
        .iter()
        .filter(|placement| placement.size > 0)
        .map(|placement| (placement.offset, placement.size))
        .chain(head.pads.iter().map(|pad| (pad.offset, pad.size)))
        .collect();
    members.sort();
    members
}

/// What the number in the emitted source says: every `size_of` and
/// `offset_of!` assertion of the head struct `name`, read back from the text
/// of the generated module, as `(size of the head, [(field, offset)])`.
fn emitted_asserts(source: &str, name: &str) -> (Option<u32>, BTreeMap<String, u32>) {
    // The formatter may break an assertion over several lines, so the search
    // runs over the text with its whitespace removed.
    let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    let number = |rest: &str| -> u32 {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().expect("a number")
    };
    let size_prefix = format!("::core::mem::size_of::<{name}>()==");
    let size = dense
        .find(&size_prefix)
        .map(|at| number(&dense[at + size_prefix.len()..]));
    let offset_prefix = format!("::core::mem::offset_of!({name},");
    let mut offsets = BTreeMap::new();
    for (at, _) in dense.match_indices(&offset_prefix) {
        let rest = &dense[at + offset_prefix.len()..];
        let (field, tail) = rest.split_once(")==").expect("an offset assertion");
        offsets.insert(field.to_string(), number(tail));
    }
    (size, offsets)
}

/// Compares the head layouts the codec emitted for `package` with the layout
/// law computed from the IR. Returns the number of heads compared.
///
/// The codec's own numbers come from the model it read, so a comparison with
/// the model would compare the codec with its own input. The law is computed
/// here from the IR with `ridl_ir::projection::podbuf`, and the codec's
/// numbers are taken from the `const` assertions of the emitted source: the
/// text a consumer's compiler checks.
fn compare(package: &v2::Package, others: &[&v2::Package], mismatches: &mut Vec<String>) -> usize {
    let packages = Packages { package, others };
    let mut law: BTreeMap<String, Result<Vec<(u32, u32)>, String>> = BTreeMap::new();
    let mut sizes: BTreeMap<String, u32> = BTreeMap::new();
    for decl in &package.decls {
        let head = match decl.kind.as_ref() {
            Some(v2::decl::Kind::StructDef(def)) => struct_head(packages, &decl.name, def),
            Some(v2::decl::Kind::UnionDef(def)) => union_head(packages, &decl.name, def),
            _ => continue,
        };
        let name = format!("{}PodbufHead", decl.name);
        match head {
            Ok(head) => {
                sizes.insert(name.clone(), head.size);
                law.insert(name, Ok(law_members(&head)));
            }
            Err(error) => {
                law.insert(name, Err(format!("{error:?}")));
            }
        }
    }
    let generated = ridl_backend_rust::generate_pipeline(
        package,
        &[ridl_backend_rust::WireEncoding::Podbuf],
        false,
        others,
    );
    let layout = ridl_backend_rust::podbuf_layout(package, others);
    let (source, layout) = match (generated, layout) {
        (Ok(generated), Ok(layout)) => (generated.rust_source, layout),
        (generated, layout) => {
            let error = generated.err().or(layout.err()).expect("a refusal");
            // The codec refuses the package. That is right only when the law
            // refuses a head of it, or a head is past a descriptor limit.
            let refused = law.values().any(Result::is_err)
                || package.decls.iter().any(|decl| {
                    let head = match decl.kind.as_ref() {
                        Some(v2::decl::Kind::StructDef(def)) => {
                            struct_head(packages, &decl.name, def)
                        }
                        Some(v2::decl::Kind::UnionDef(def)) => {
                            union_head(packages, &decl.name, def)
                        }
                        _ => return false,
                    };
                    head.is_ok_and(|head| exceeds_descriptor_limits(&head))
                        || max_size(packages, decl).is_none()
                });
            if !refused {
                mismatches.push(format!(
                    "{}: the codec refuses a package the law accepts: {}",
                    package.name, error.message
                ));
            }
            return 0;
        }
    };
    let mut compared = 0;
    for head in &layout {
        // A named tuple an interaction induces has no declaration to read
        // the law from; its numbers are still checked against the source.
        let (emitted_size, emitted_offsets) = emitted_asserts(&source, &head.name);
        if emitted_size != Some(head.size) {
            mismatches.push(format!(
                "{}.{}: the emitted size assertion is {emitted_size:?}, the layout holds {}",
                package.name, head.name, head.size
            ));
        }
        for (field, offset, _) in &head.fields {
            if emitted_offsets.get(field) != Some(offset) {
                mismatches.push(format!(
                    "{}.{}.{field}: the emitted offset assertion is {:?}, the layout holds {offset}",
                    package.name,
                    head.name,
                    emitted_offsets.get(field)
                ));
            }
        }
        let Some(expected) = law.get(&head.name) else {
            continue;
        };
        compared += 1;
        let Ok(expected) = expected else {
            mismatches.push(format!(
                "{}.{}: the codec emits a head the law refuses: {}",
                package.name,
                head.name,
                expected.as_ref().unwrap_err()
            ));
            continue;
        };
        let mut members: Vec<(u32, u32)> = head
            .fields
            .iter()
            .map(|(_, offset, size)| (*offset, *size))
            .collect();
        members.sort();
        if &members != expected || Some(&head.size) != sizes.get(&head.name) {
            mismatches.push(format!(
                "{}.{}: the codec emits size {} with {members:?}, the law gives size {:?} with \
                 {expected:?}",
                package.name,
                head.name,
                head.size,
                sizes.get(&head.name)
            ));
        }
    }
    for (name, expected) in &law {
        if expected.is_ok() && !layout.iter().any(|head| &head.name == name) {
            mismatches.push(format!(
                "{}.{name}: the law lays out a head the codec does not emit",
                package.name
            ));
        }
    }
    compared
}

/// The numbers in the emitted `const` assertions equal the layout law
/// computed from the IR, over the fixture and over every corpus workspace
/// that compiles. A change to the law, the lowering or the emitter that moves
/// one field in one of them fails here without running any generated code.
#[test]
fn the_codec_layout_matches_the_model_projection() {
    let mut mismatches = Vec::new();
    let mut compared = 0;
    let fixture = ir::compile_fixture("podbuf_roundtrip.ridl");
    let fixture_compared = compare(&fixture, &[], &mut mismatches);
    assert!(fixture_compared >= 20, "the fixture holds 20 heads or more");
    compared += fixture_compared;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let std_ir = ridlc::std_ir();
    let mut skipped: Vec<(&str, String)> = Vec::new();
    for entry in CORPUS {
        let packages = match podbuf::compile_workspace(&root.join(entry)) {
            Ok(packages) => packages,
            Err(reason) => {
                skipped.push((*entry, reason));
                continue;
            }
        };
        for (at, package) in packages.iter().enumerate() {
            let mut others: Vec<&v2::Package> = packages
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != at)
                .map(|(_, other)| other)
                .collect();
            others.push(&std_ir);
            compared += compare(package, &others, &mut mismatches);
        }
    }
    // The workspaces that exist to draw diagnostics have no layout to read.
    // Any other workspace that stops compiling is a failure, not a skip.
    let expected_skipped = [
        "crates/ridlc/tests/corpus/diag-showcase",
        "crates/ridlc/tests/corpus/ridl-diag-showcase",
        "crates/ridlc/tests/corpus/rsdl-diag-showcase",
    ];
    let skipped_entries: Vec<&str> = skipped.iter().map(|(entry, _)| *entry).collect();
    assert_eq!(skipped_entries, expected_skipped, "skipped: {skipped:#?}");
    assert!(
        mismatches.is_empty(),
        "the emitted layout and the law disagree:\n{}",
        mismatches.join("\n")
    );
    assert!(
        compared > fixture_compared,
        "a corpus workspace was compared"
    );
}

// ---------------------------------------------------------------------------
// The target with no standard library
// ---------------------------------------------------------------------------

/// The generated codec of the fixture compiles as `no_std` for
/// `thumbv7em-none-eabihf`, against a `ridl-rt` built with only the `podbuf`
/// feature: the heads, the encoder, `verify`, the views and `decode`, with the
/// owned types from `alloc`. The target is installed by `rust-toolchain.toml`;
/// as for the wasm32 check, a missing target fails the test, after one attempt
/// to add it with `rustup`.
#[test]
fn the_generated_podbuf_module_checks_for_thumbv7em() {
    let package = ir::compile_fixture("podbuf_roundtrip.ridl");
    let generated = podbuf::generated(&package, &[]);
    let source =
        format!("#![no_std]\n#![allow(dead_code)]\nextern crate alloc as std;\n{generated}");
    assert!(
        rustc::check_for_thumbv7em("podbuf_thumbv7em", &source),
        "the thumbv7em-none-eabihf target must be installed for this check to \
         mean anything; `rust-toolchain.toml` lists it, and so does this test \
         when rustup is present"
    );
}
