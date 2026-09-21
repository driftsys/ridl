//! Conformance against an independent FlatBuffers implementation, and the
//! `wasm32` check over generated code (E11.7 stage K8, plan Task 7 of
//! `docs/archive/2026-09-20-flatbuffers-codec-plan.md`).
//!
//! Design note D-8 chose a round trip through a second implementation over
//! byte equality with one, because FlatBuffers fixes no canonical encoding:
//! vtable sharing, field write order and alignment slack are all writer
//! choices, and two conforming writers differ. So the claim under test is
//! that the bytes this codec writes are FlatBuffers, and that FlatBuffers
//! this codec did not write are read back to the same value.
//!
//! The second implementation is `planus`, which is already this repository's
//! FlatBuffers oracle (`ridl-backend-flatbuffers` compiles every emitted
//! schema with `planus-translation`). Here the oracle reads and writes
//! buffers as well as schemas: `planus-codegen` turns the `.fbs` the schema
//! backend emits for the round-trip fixture into a Rust reader and writer,
//! and that reader is **checked in** beside this file. There is no
//! `build.rs` and no `flatc`: nothing outside this test binary depends on
//! planus, and nothing a generated package links does
//! (design note D-12, which leaves unused the one FlatBuffers runtime
//! ADR-0020 decision 5 permits, so ADR-0020's RA-01 dependency ceiling
//! stays unspent).
//!
//! [`the_checked_in_planus_reader_is_what_planus_codegen_writes`] is what
//! keeps the checked-in reader honest: it regenerates it from the fixture's
//! own emitted schema and asserts byte equality.
//!
//! **The two implementations do not agree about a default-valued field, by
//! design, and the disagreement has two sides.** planus omits a table field
//! whose value equals its FlatBuffers default, which is what a FlatBuffers
//! writer does. Design note D-9 makes this codec always write such a field
//! and read an absent non-optional one as `Malformed::MissingRequired`. So:
//!
//! - a buffer planus writes from a value carrying a default-valued
//!   **non-optional** scalar is a buffer this codec refuses
//!   ([`a_buffer_planus_wrote_omitting_a_default_is_refused`]);
//! - an **optional** scalar present at its default survives no foreign round
//!   trip, because it projects to a plain scalar with no `= null` and the
//!   schema gives another reader nothing to read presence from
//!   ([`an_optional_scalar_at_its_default_is_lost_by_a_foreign_round_trip`]).
//!
//! **A scalar root is a root like any other** (ADR-0019 decision 8). A named
//! scalar, an enum and an enum set are rooted in a box table, and
//! [`a_scalar_root_round_trips_through_planus_both_ways`] measures both
//! directions over one — the same obligation the composite cases carry, on the
//! shape that had no root at all until decision 8.
//!
//! Both are decided divergences rather than defects, both are measured here
//! rather than described, and driftsys/ridl#472 carries the pair. The
//! optional half also bounds what D-9 claims: a present default is
//! distinguishable from an absent optional for this codec reading its own
//! bytes, and for no reader following the emitted schema.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

/// The Rust reader and writer `planus-codegen` produces from the `.fbs` the
/// schema backend emits for `tests/fixtures/flatbuffers_roundtrip.ridl`.
///
/// Checked in rather than generated at build time: a `build.rs` would make
/// planus's code generator run for every consumer of this crate, and this
/// crate emits text rather than linking what planus writes.
/// [`the_checked_in_planus_reader_is_what_planus_codegen_writes`] is the
/// drift guard.
#[allow(
    warnings,
    reason = "third-party generated code, checked in verbatim so that a \
              regeneration can be compared byte for byte; editing it to \
              satisfy a lint would make that comparison impossible"
)]
mod planus_reader {
    include!("generated/flatbuffers_conformance_planus.rs");
}

use planus_reader::fb::demo as fb;

/// Regenerates the checked-in planus reader and returns its source.
///
/// The `.fbs` comes from `ridl-backend-flatbuffers` over the same IR the
/// codec under test is generated from, so the schema and the codec cannot
/// drift apart: one fixture, one compile, two emitters.
///
/// planus writes the schema's own path into a doc comment on every item, so
/// the temporary directory it was written in is normalized away. Without
/// that the output would differ on every run and byte equality would be
/// meaningless.
fn planus_reader_source() -> String {
    let package = ir::compile_fixture("flatbuffers_roundtrip.ridl");
    let schema = ridl_backend_flatbuffers::generate(&package)
        .expect("the fixture projects to a FlatBuffers schema")
        .fbs_source;

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let path = dir.path().join("fb_demo.fbs");
    std::fs::write(&path, &schema).expect("the schema is written");
    let declarations = planus_translation::translate_files(std::slice::from_ref(&path))
        .expect("the emitted schema is a valid FlatBuffers schema");
    let source = planus_codegen::generate_rust(&declarations, true)
        .expect("planus generates Rust from the emitted schema");

    let prefix = format!("{}/", dir.path().display());
    source.replace(&prefix, "")
}

/// The checked-in reader is what `planus-codegen` writes from the emitted
/// schema, byte for byte.
///
/// Without this the conformance tests below would prove only that this codec
/// agrees with a file someone once pasted into the tree. `planus` and
/// `planus-codegen` are pinned at one version in the workspace manifest, so
/// a version bump that changes the generated reader fails here rather than
/// silently changing what conformance means. `RIDL_UPDATE_GENERATED=1`
/// writes the fresh output instead of comparing, the same switch
/// `tests/interaction_face.rs` uses.
#[test]
fn the_checked_in_planus_reader_is_what_planus_codegen_writes() {
    let fresh = planus_reader_source();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/flatbuffers_conformance_planus.rs");

    if std::env::var_os("RIDL_UPDATE_GENERATED").is_some() {
        std::fs::write(&path, &fresh)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        return;
    }

    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        fresh, checked_in,
        "the checked-in planus reader is stale; regenerate it with \
         RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test \
         flatbuffers_conformance"
    );
}

/// The one value both implementations build, on both sides of the round
/// trip, written in the generated package's own vocabulary.
///
/// **Every scalar in it differs from its FlatBuffers default**, deliberately:
/// planus omits a default-valued field, and this codec reads an omitted
/// non-optional field as `Malformed::MissingRequired` (D-9). The divergence
/// has its own case below; this value is chosen so that the two directions
/// of the round trip test the encoding rather than that one disagreement.
const CONFORMANCE_VALUE: &str = r#"
fn inner(speed: i64, label: &str) -> Inner {
    Inner {
        speed: Speed::new_unchecked(speed),
        label: Label::new_unchecked(String::from(label)),
    }
}

fn conformance() -> Report {
    Report {
        id: Count::new_unchecked(7),
        name: Label::new_unchecked(String::from("cabin")),
        blob: Blob::new_unchecked(vec![1, 2, 3]),
        ratio: Ratio::new_unchecked(0.25),
        engaged: Engaged::new(true),
        health: Health::WARN,
        flags: WarningFlags::HIGH,
        inner: inner(120, "inner"),
        outcome: Outcome::Bad(Health::FAIL),
        range: ReportRange {
            min: Speed::new_unchecked(10),
            max: Speed::new_unchecked(300),
        },
        readings: [
            Speed::new_unchecked(1),
            Speed::new_unchecked(2),
            Speed::new_unchecked(3),
        ],
        faults: vec![Health::OK, Health::FAIL],
        meta: vec![
            (Label::new_unchecked(String::from("a")), Count::new_unchecked(1)),
            (Label::new_unchecked(String::from("bb")), Count::new_unchecked(2)),
        ],
        names: vec![
            Label::new_unchecked(String::from("first")),
            Label::new_unchecked(String::from("second")),
        ],
        inners: vec![inner(1, "one"), inner(2, "two")],
        outcomes: vec![Outcome::Ok(inner(3, "three")), Outcome::Bad(Health::WARN)],
        points: vec![
            ReportPointsElement {
                x: Speed::new_unchecked(10),
                y: Speed::new_unchecked(20),
            },
            ReportPointsElement {
                x: Speed::new_unchecked(30),
                y: Speed::new_unchecked(40),
            },
        ],
        pair: Some(ReportPair {
            lo: Speed::new_unchecked(5),
            inner: inner(6, "six"),
        }),
        note: Some(Label::new_unchecked(String::from("note"))),
        spare: Some(Speed::new_unchecked(9)),
    }
}
"#;

/// The same value, in planus's own vocabulary.
///
/// Written out field by field rather than derived from the codec's output:
/// a value derived from what this codec wrote could not witness a
/// disagreement about what the bytes mean.
fn planus_value() -> fb::Report {
    fb::Report {
        id: 7,
        name: Some(String::from("cabin")),
        blob: Some(vec![1, 2, 3]),
        ratio: 0.25,
        engaged: true,
        health: fb::Health::Warn,
        flags: 0b10,
        inner: Some(Box::new(planus_inner(120, "inner"))),
        outcome: Some(Box::new(fb::Outcome {
            value: Some(fb::OutcomeUnion::Bad(Box::new(fb::OutcomeBadBox {
                value: fb::Health::Fail,
            }))),
        })),
        range: Some(Box::new(fb::ReportRange {
            field_1: 10,
            field_2: 300,
        })),
        readings: Some(vec![1, 2, 3]),
        faults: Some(vec![fb::Health::Ok, fb::Health::Fail]),
        meta: Some(vec![
            fb::ReportMetaEntry {
                key: Some(String::from("a")),
                value: 1,
            },
            fb::ReportMetaEntry {
                key: Some(String::from("bb")),
                value: 2,
            },
        ]),
        names: Some(vec![String::from("first"), String::from("second")]),
        inners: Some(vec![planus_inner(1, "one"), planus_inner(2, "two")]),
        outcomes: Some(vec![
            fb::Outcome {
                value: Some(fb::OutcomeUnion::Ok(Box::new(planus_inner(3, "three")))),
            },
            fb::Outcome {
                value: Some(fb::OutcomeUnion::Bad(Box::new(fb::OutcomeBadBox {
                    value: fb::Health::Warn,
                }))),
            },
        ]),
        points: Some(vec![
            fb::ReportPointsElement {
                field_1: 10,
                field_2: 20,
            },
            fb::ReportPointsElement {
                field_1: 30,
                field_2: 40,
            },
        ]),
        pair: Some(Box::new(fb::ReportPair {
            field_1: 5,
            field_2: Some(Box::new(planus_inner(6, "six"))),
        })),
        note: Some(String::from("note")),
        spare: 9,
    }
}

fn planus_inner(speed: u16, label: &str) -> fb::Inner {
    fb::Inner {
        speed,
        label: Some(String::from(label)),
    }
}

/// The generated codec's source for the fixture, with `main` appended.
fn program(main: &str) -> String {
    let package = ir::compile_fixture("flatbuffers_roundtrip.ridl");
    let generated = ridl_backend_rust::generate(&package)
        .expect("the fixture generates")
        .rust_source;
    format!("#![allow(dead_code)]\n{generated}\n{CONFORMANCE_VALUE}\n{main}")
}

/// Reads the hexadecimal transcript a generated program writes.
fn from_hex(text: &str) -> Vec<u8> {
    let text = text.trim();
    assert!(
        !text.is_empty() && text.len().is_multiple_of(2),
        "the transcript is a whole number of bytes, got {} characters",
        text.len()
    );
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("a hexadecimal byte"))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// **Direction one.** Bytes this codec writes are read by planus, and every
/// field compares equal.
///
/// planus's reader is total over the buffer — `Report::try_from` walks every
/// offset and every vector — so a buffer this codec wrote that planus cannot
/// read at all fails before any field is compared.
#[test]
fn bytes_this_codec_writes_are_read_by_planus() {
    let transcript = rustc::run_program_capturing_stdout(
        "fb_conformance_encode",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;

fn main() {
    let value = conformance();
    let mut out = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes;
    let mut text = String::new();
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    println!("{text}");
}
"#,
        ),
    );

    let bytes = from_hex(&transcript);
    let read = <fb::ReportRef<'_> as planus::ReadAsRoot>::read_as_root(&bytes)
        .expect("planus reads the root table this codec wrote");
    let owned = fb::Report::try_from(read).expect("planus reads every field this codec wrote");
    assert_eq!(
        owned,
        planus_value(),
        "an independent FlatBuffers reader must see the value this codec encoded"
    );
}

/// **Direction two.** Bytes planus writes are accepted by this codec's
/// `verify` and decode to the value planus encoded.
///
/// planus's writer is not this one: it orders fields its own way, shares
/// vtables its own way, and writes the union's wrapper table itself. If this
/// codec's `verify` or `decode` depended on the layout its own encoder
/// happens to produce, this is the case that fails.
#[test]
fn bytes_planus_writes_are_decoded_by_this_codec() {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(planus_value(), None).to_vec();

    let hex = to_hex(&bytes);
    let main = format!(
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Ref;

const FOREIGN: &str = "{hex}";

fn main() {{
    let bytes: Vec<u8> = (0..FOREIGN.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&FOREIGN[at..at + 2], 16).unwrap())
        .collect();
    let proof: Ref<'_, Report, FlatBuffers> =
        Ref::verify(&bytes).expect("a buffer written by another implementation verifies");
    assert_eq!(
        proof.decode(),
        conformance(),
        "a buffer written by another implementation decodes to the same value"
    );
}}
"#
    );
    rustc::run_program("fb_conformance_decode", &program(&main));
}

/// **Direction two, over a vtable this codec's own writer never produces.**
///
/// [`bytes_planus_writes_are_decoded_by_this_codec`] carries every field, so
/// planus's vtable for it is as long as this codec's own. This codec always
/// writes a slot for every field — an absent optional gets a zero entry —
/// so a vtable **shorter than the field count** is a shape its own encoder
/// never emits, and reading one is a path only a foreign buffer reaches.
/// planus truncates the trailing entries of a vtable whose last fields are
/// absent or default, so dropping the three trailing optionals produces
/// exactly that.
///
/// The case asserts the truncation happened, rather than assuming it, and
/// then asserts the three fields decode as absent.
#[test]
fn a_planus_buffer_with_a_truncated_vtable_is_decoded_by_this_codec() {
    let mut value = planus_value();
    value.pair = None;
    value.note = None;
    value.spare = 0;
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(value, None).to_vec();

    assert!(
        vtable_entries(&bytes) < 21,
        "planus must truncate the vtable for this case, or the case proves \
         nothing about a short vtable: `Report` has 21 slots and the buffer's \
         root vtable carries {}",
        vtable_entries(&bytes)
    );

    let hex = to_hex(&bytes);
    let main = format!(
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Ref;

const FOREIGN: &str = "{hex}";

fn main() {{
    let bytes: Vec<u8> = (0..FOREIGN.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&FOREIGN[at..at + 2], 16).unwrap())
        .collect();
    let proof: Ref<'_, Report, FlatBuffers> =
        Ref::verify(&bytes).expect("a buffer with a truncated vtable verifies");
    let mut expected = conformance();
    expected.pair = None;
    expected.note = None;
    expected.spare = None;
    assert_eq!(
        proof.decode(),
        expected,
        "a slot the vtable does not carry is an absent field, not a failure"
    );
}}
"#
    );
    rustc::run_program("fb_conformance_short_vtable", &program(&main));
}

/// The number of entries in the root table's vtable.
///
/// Read here rather than taken on trust: the case above is only about a
/// short vtable if the buffer actually carries one.
fn vtable_entries(bytes: &[u8]) -> usize {
    let root = u32::from_le_bytes(bytes[0..4].try_into().expect("a root offset")) as usize;
    let soffset = i32::from_le_bytes(bytes[root..root + 4].try_into().expect("a vtable offset"));
    let vtable = (root as i64 - i64::from(soffset)) as usize;
    let vtable_bytes =
        u16::from_le_bytes(bytes[vtable..vtable + 2].try_into().expect("a vtable size")) as usize;
    // Four header bytes (the vtable's own size, then the table's), then one
    // `voffset_t` per slot.
    (vtable_bytes - 4) / 2
}

/// The `voffset` the root table's vtable carries for `slot`, or zero when
/// the field is absent.
///
/// **This, not [`vtable_entries`], is what says a field was written.** A
/// vtable's declared width is a property of the writer, not of the value:
/// `ridl-rt`'s `push_table` always emits
/// `VTABLE_HEADER + slots * VOFFSET_SIZE` bytes whatever it put in them, so
/// a codec that skipped a field still produces a 21-entry vtable — with a
/// zero in that entry. FlatBuffers spells absence as a zero `voffset`, and
/// a slot past the vtable's end is absent too, which is the other half of
/// this function.
fn voffset(bytes: &[u8], slot: usize) -> u16 {
    let root = u32::from_le_bytes(bytes[0..4].try_into().expect("a root offset")) as usize;
    let soffset = i32::from_le_bytes(bytes[root..root + 4].try_into().expect("a vtable offset"));
    let vtable = (root as i64 - i64::from(soffset)) as usize;
    let vtable_bytes =
        u16::from_le_bytes(bytes[vtable..vtable + 2].try_into().expect("a vtable size")) as usize;
    let entry = vtable + 4 + slot * 2;
    if entry + 2 > vtable + vtable_bytes {
        return 0;
    }
    u16::from_le_bytes(bytes[entry..entry + 2].try_into().expect("a voffset"))
}

/// `Report.spare`'s vtable slot. The `.fbs` gives it `(id: 20)`, and a
/// FlatBuffers slot is its id.
const SPARE_SLOT: usize = 20;

/// **The one disagreement, measured.** planus omits a non-optional scalar
/// whose value equals its FlatBuffers default; this codec reads an omitted
/// non-optional field as `Malformed::MissingRequired` (D-9), so it refuses
/// that buffer.
///
/// The value below differs from [`planus_value`] in one field: `Report.id`
/// is 0, which is `ubyte`'s default, so planus writes no slot for it. Every
/// other field is unchanged, so what this case measures is that one
/// omission and nothing else.
///
/// This is a decided divergence rather than a defect — D-9 rejected omitting
/// a default-valued field because it makes a present default
/// indistinguishable from an absent optional, which typl distinguishes — but
/// it means this codec cannot read every buffer a conforming FlatBuffers
/// writer produces. driftsys/ridl#472 carries it.
#[test]
fn a_buffer_planus_wrote_omitting_a_default_is_refused() {
    let mut value = planus_value();
    value.id = 0;
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(value, None).to_vec();

    // The omission is real: planus wrote a buffer this codec's own encoder
    // would have written one slot longer. Without this the case could pass
    // for a reason unrelated to the default.
    let read = <fb::ReportRef<'_> as planus::ReadAsRoot>::read_as_root(&bytes)
        .expect("planus reads its own buffer");
    assert_eq!(
        read.id().expect("id reads"),
        0,
        "planus reads back the default"
    );

    let hex = to_hex(&bytes);
    let main = format!(
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{{Malformed, Ref, VerifyError}};

const FOREIGN: &str = "{hex}";

fn main() {{
    let bytes: Vec<u8> = (0..FOREIGN.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&FOREIGN[at..at + 2], 16).unwrap())
        .collect();
    match Ref::<'_, Report, FlatBuffers>::verify(&bytes) {{
        Err(VerifyError::Structure(Malformed::MissingRequired)) => {{}}
        Err(other) => panic!("expected MissingRequired, got {{other:?}}"),
        Ok(_) => panic!("an omitted non-optional field must not verify (D-9)"),
    }}
}}
"#
    );
    rustc::run_program("fb_conformance_omitted_default", &program(&main));
}

/// **The same disagreement from the other side: an optional scalar present at
/// its FlatBuffers default is lost by a foreign round trip.**
///
/// `spare: Speed?` projects to a plain `ushort` with no `= null`, so
/// presence for it is exactly "the slot is in the buffer". This codec writes
/// `Some(Speed(0))` as a present slot, which is what design note D-9 asks
/// for; planus reads `0`, which is all the schema lets it read; and a planus
/// re-encode of what it read omits the slot, because 0 is the field's
/// default. Round-tripped through another implementation,
/// `Some(Speed(0))` comes back `None`.
///
/// **This bounds D-9's claim.** "A present value is written even when it
/// equals the field's FlatBuffers default, so that the reader can tell the
/// two apart" holds for this codec reading its own bytes, and for no reader
/// following the emitted schema. It is the optional half of
/// driftsys/ridl#472, which the case above measures the non-optional half
/// of; a `= null` default on an optional scalar field is the projection
/// change that would close it, and that is a projection decision rather than
/// a codec one.
///
/// All three legs are asserted, so a pass cannot come from the wrong place:
/// this codec's own buffer carries a **non-zero `voffset`** for `spare`,
/// planus's re-encode carries none, and the decode that follows reads
/// `None` while every other field is unchanged.
///
/// Leg one reads the `voffset`, not the vtable's declared width. The width
/// says nothing about what was written: `ridl-rt`'s `push_table` emits one
/// entry per slot whatever it filled them with, so a codec that stopped
/// writing a present default-valued optional — the regression D-9 exists to
/// forbid — still produces a 21-entry vtable, with a zero in that entry. An
/// earlier version of this case asserted the width and passed under exactly
/// that mutation; the review of 2026-09-21 found it. The current assertion
/// fails under it.
#[test]
fn an_optional_scalar_at_its_default_is_lost_by_a_foreign_round_trip() {
    let transcript = rustc::run_program_capturing_stdout(
        "fb_conformance_spare_zero",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;

fn main() {
    let mut value = conformance();
    value.spare = Some(Speed::new_unchecked(0));
    let mut out = vec![0u8; <Report as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes;
    let mut text = String::new();
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    println!("{text}");
}
"#,
        ),
    );

    // Leg one: this codec wrote the field. A non-zero `voffset` in the
    // slot is what "written" means in the format; the vtable's declared
    // width is the writer's habit and says nothing about this value.
    let ours = from_hex(&transcript);
    assert_ne!(
        voffset(&ours, SPARE_SLOT),
        0,
        "this codec must write a present optional at its FlatBuffers default \
         (D-9); the buffer carries a zero voffset for `spare`, which is how \
         the format spells an absent field"
    );
    let read = <fb::ReportRef<'_> as planus::ReadAsRoot>::read_as_root(&ours)
        .expect("planus reads the buffer this codec wrote");
    let owned = fb::Report::try_from(read).expect("planus reads every field");
    assert_eq!(
        owned.spare, 0,
        "planus reads the slot as 0, which is all the schema lets it read"
    );

    // Leg two: planus re-encodes what it read, and omits the slot.
    let mut builder = planus::Builder::new();
    let theirs = builder.finish(owned, None).to_vec();
    assert_eq!(
        voffset(&theirs, SPARE_SLOT),
        0,
        "planus must omit the default-valued slot, or this case proves \
         nothing about what a conforming writer does"
    );

    // Leg three: this codec decodes planus's buffer, and the optional is
    // gone.
    let hex = to_hex(&theirs);
    let main = format!(
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Ref;

const FOREIGN: &str = "{hex}";

fn main() {{
    let bytes: Vec<u8> = (0..FOREIGN.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&FOREIGN[at..at + 2], 16).unwrap())
        .collect();
    let proof: Ref<'_, Report, FlatBuffers> =
        Ref::verify(&bytes).expect("the re-encoded buffer still verifies");
    let back = proof.decode();
    assert_eq!(
        back.spare, None,
        "the round trip through another implementation turns Some(0) into None",
    );
    let mut expected = conformance();
    expected.spare = None;
    assert_eq!(
        back, expected,
        "and nothing else about the value changed, so the loss is the optional's",
    );
}}
"#
    );
    rustc::run_program("fb_conformance_spare_lost", &program(&main));
}

/// **A scalar root, both directions** (ADR-0019 decision 8).
///
/// `Speed` is a named scalar, so before decision 8 it had no root table and
/// no `Payload<FlatBuffers>` at all. Its root is now
/// `table SpeedBox { value: ushort (id: 0); }`, and what this case measures is
/// that the box is a FlatBuffers root like any other: planus reads the buffer
/// this codec writes for one, and this codec accepts the buffer planus writes
/// for one.
///
/// The value is 150, which is not `ushort`'s default, so the disagreement
/// [`a_buffer_planus_wrote_omitting_a_default_is_refused`] measures is not in
/// the way — it applies to a box's `value` field exactly as it does to any
/// other non-optional field, since decision 8 resolves that field as an
/// ordinary one.
#[test]
fn a_scalar_root_round_trips_through_planus_both_ways() {
    // Direction one: this codec writes the box, planus reads it.
    let transcript = rustc::run_program_capturing_stdout(
        "fb_conformance_scalar_root_encode",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;

fn main() {
    let value = Speed::new_unchecked(150);
    let mut out = vec![0u8; <Speed as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).expect("encode").bytes;
    let mut text = String::new();
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    println!("{text}");
}
"#,
        ),
    );

    let ours = from_hex(&transcript);
    let read = <fb::SpeedBoxRef<'_> as planus::ReadAsRoot>::read_as_root(&ours)
        .expect("planus reads the box table this codec rooted a named scalar in");
    assert_eq!(
        fb::SpeedBox::try_from(read).expect("planus reads the box's one field"),
        fb::SpeedBox { value: 150 },
        "an independent reader must see the value this codec boxed"
    );

    // Direction two: planus writes the box, this codec verifies and decodes
    // it. planus lays the one-field table out its own way, so a `verify` that
    // depended on this encoder's own placement fails here.
    let mut builder = planus::Builder::new();
    let theirs = builder.finish(fb::SpeedBox { value: 150 }, None).to_vec();
    let hex = to_hex(&theirs);
    let main = format!(
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Ref;

const FOREIGN: &str = "{hex}";

fn main() {{
    let bytes: Vec<u8> = (0..FOREIGN.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&FOREIGN[at..at + 2], 16).unwrap())
        .collect();
    let proof: Ref<'_, Speed, FlatBuffers> =
        Ref::verify(&bytes).expect("a box another implementation wrote verifies");
    assert_eq!(
        proof.decode(),
        Speed::new_unchecked(150),
        "a box another implementation wrote decodes to the same scalar"
    );
}}
"#
    );
    rustc::run_program("fb_conformance_scalar_root_decode", &program(&main));
}

/// The generated codec checks for `wasm32-unknown-unknown`.
///
/// ADR-0020 decision 2 makes the generated Rust compiled to `wasm32` the
/// codec a TypeScript consumer loads, so `just wasm-check`'s obligation
/// reaches generated code and not only this workspace's own packages. The
/// recipe cannot reach it: it runs `cargo check` over a `-p` list, and the
/// codec is text with no manifest. This runs the same check over the emitted
/// source through stage K3's proof mechanism, so it needs no new recipe and
/// no example crate — see `just wasm-check`'s own comment, which names this
/// test.
///
/// The check is `--emit=metadata`, which is the unit of work `cargo check`
/// performs, against a `ridl-rt` built for the same target with the three
/// encoding features on.
#[test]
fn the_generated_codec_checks_for_wasm32() {
    let package = ir::compile_fixture("flatbuffers_roundtrip.ridl");
    let generated = ridl_backend_rust::generate(&package)
        .expect("the fixture generates")
        .rust_source;
    let source = format!("#![allow(dead_code)]\n{generated}");
    assert!(
        rustc::check_for_wasm32("fb_wasm32", &source),
        "the wasm32-unknown-unknown target must be installed for this check to \
         mean anything; `just wasm-check` adds it with rustup, and so does this \
         test when rustup is present"
    );
}
