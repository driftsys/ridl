//! The FlatBuffers reading and writing helpers.
//!
//! The whole file is gated on the feature, because `just test` builds the
//! workspace with default features and the module does not exist there.
#![cfg(feature = "flatbuffers")]

use ridl_rt::flatbuffers::{
    field, follow, read_bool, read_f64, read_i32, read_u16, root, string, vector, Builder, Field,
    Pos, TableField, Vector,
};
use ridl_rt::payload::{EncodeError, Malformed};

/// The table every test here encodes:
///
/// ```text
/// table Point { x: int32; y: int32; label: string; }
/// ```
///
/// The layout is the one a projection would hand the builder: the vtable
/// offset at 0, then the two scalars, then the string offset.
const SIZE: usize = 16;
const ALIGN: usize = 4;
const SLOTS: u16 = 3;
const X: u16 = 4;
const Y: u16 = 8;
const LABEL: u16 = 12;

fn encode_point<'o>(out: &'o mut [u8], x: i32, y: i32, label: Option<&str>) -> &'o [u8] {
    let mut b = Builder::new(out);
    let text = label.map(|l| b.push_string(l).expect("the buffer fits the string"));
    let mut fields = [
        TableField {
            slot: 0,
            offset: X,
            value: Field::I32(x),
        },
        TableField {
            slot: 1,
            offset: Y,
            value: Field::I32(y),
        },
        TableField {
            slot: 2,
            offset: LABEL,
            value: Field::U32(0),
        },
    ];
    let count = match text {
        Some(t) => {
            fields[2].value = Field::Offset(t);
            3
        }
        None => 2,
    };
    let table = b
        .push_table(SIZE, ALIGN, SLOTS, &fields[..count])
        .expect("the buffer fits the table");
    b.finish(table, ALIGN).expect("the buffer fits the root")
}

/// The bytes of `Point { x: 7, y: -2, label: "hi" }`, written out by hand from
/// the FlatBuffers format rather than from this encoder's behaviour.
///
/// This is what makes the test worth having: a round trip through one
/// implementation proves that implementation self-consistent, and says nothing
/// about whether the bytes are FlatBuffers. Every offset below is checked
/// against the format's own rules — the root `uoffset_t` points forward to the
/// table, the table's `soffset_t` points backwards to its vtable, and each
/// object sits at its own alignment.
///
/// Conformance against an independent implementation is K8's obligation
/// (design note D-8 and K-12); this is the byte-level anchor until then.
#[rustfmt::skip]
const POINT_7_M2_HI: [u8; 40] = [
    // 0: the root uoffset, pointing forward 16 bytes to the table.
    0x10, 0x00, 0x00, 0x00,
    // 4: padding, so that the root's 4-byte alignment leaves the vtable at 6.
    0x00, 0x00,
    // 6: the vtable — its own size, the table's size, then one entry per slot.
    0x0a, 0x00,
    0x10, 0x00,
    0x04, 0x00,
    0x08, 0x00,
    0x0c, 0x00,
    // 16: the table. Its soffset points backwards 10 bytes, to 6.
    0x0a, 0x00, 0x00, 0x00,
    // 20: x = 7.
    0x07, 0x00, 0x00, 0x00,
    // 24: y = -2.
    0xfe, 0xff, 0xff, 0xff,
    // 28: the label uoffset, pointing forward 4 bytes to 32.
    0x04, 0x00, 0x00, 0x00,
    // 32: the string — its length, its bytes, its terminating zero.
    0x02, 0x00, 0x00, 0x00,
    0x68, 0x69,
    0x00,
    // 39: the string's alignment padding.
    0x00,
];

#[test]
fn a_table_encodes_to_the_bytes_the_format_defines() {
    let mut out = [0u8; 64];
    let bytes = encode_point(&mut out, 7, -2, Some("hi"));
    assert_eq!(bytes, &POINT_7_M2_HI[..]);
}

/// `Encoded.bytes` is a subslice, not a prefix: the builder fills the slice
/// from its end. This is the property ADR-0021 decision 7's wording changed
/// for, so it is pinned by a test rather than only by prose.
#[test]
fn the_finished_buffer_ends_at_the_end_of_the_slice() {
    let mut out = [0u8; 64];
    let len = {
        let bytes = encode_point(&mut out, 7, -2, Some("hi"));
        assert_eq!(bytes.len(), 40);
        bytes.len()
    };
    // The bytes sit at the end of `out`, and the front of `out` is untouched.
    assert_eq!(&out[out.len() - len..], &POINT_7_M2_HI[..]);
    assert!(out[..out.len() - len].iter().all(|&b| b == 0));
}

#[test]
fn a_buffer_reads_back_through_the_walk() {
    let mut out = [0u8; 64];
    let bytes = encode_point(&mut out, 7, -2, Some("hi"));

    let table = root(bytes).expect("the root resolves");
    assert_eq!(table, 16);

    let x = field(bytes, table, 0)
        .expect("the slot is readable")
        .expect("x is present");
    assert_eq!(read_i32(bytes, x).expect("x reads"), 7);

    let y = field(bytes, table, 1)
        .expect("the slot is readable")
        .expect("y is present");
    assert_eq!(read_i32(bytes, y).expect("y reads"), -2);

    let label = field(bytes, table, 2)
        .expect("the slot is readable")
        .expect("label is present");
    assert_eq!(string(bytes, label).expect("the label reads"), "hi");
}

/// A field the encoder left out is absent from the buffer, and the walk says
/// so rather than failing. That is how a FlatBuffers table carries a field
/// equal to its default (design note D-9).
#[test]
fn an_omitted_field_is_absent_rather_than_an_error() {
    let mut out = [0u8; 64];
    let bytes = encode_point(&mut out, 1, 2, None);
    let table = root(bytes).expect("the root resolves");

    assert!(field(bytes, table, 0)
        .expect("the slot is readable")
        .is_some());
    assert_eq!(field(bytes, table, 2).expect("the slot is readable"), None);
}

/// A slot past the end of the vtable is absent, not out of bounds: a writer
/// that knew fewer fields than this reader produces a short vtable.
#[test]
fn a_slot_past_the_vtable_is_absent() {
    let mut out = [0u8; 64];
    let bytes = encode_point(&mut out, 1, 2, Some("hi"));
    let table = root(bytes).expect("the root resolves");
    assert_eq!(field(bytes, table, 9).expect("the slot is readable"), None);
}

#[test]
fn every_read_is_bounded_by_the_buffer() {
    let mut out = [0u8; 64];
    let len = encode_point(&mut out, 7, -2, Some("hi")).len();
    let full = &out[out.len() - len..];
    let table = root(full).expect("the root resolves");

    for cut in 1..len {
        let short = &full[..cut];
        // Whatever the truncation, no read panics and none reports a value
        // from outside the bytes it was given.
        let _ = root(short);
        let _ = field(short, table.min(cut), 0);
        let _ = read_i32(short, cut.saturating_sub(2));
        let _ = string(short, 28.min(cut));
        let _ = vector(short, 28.min(cut), 4);
    }
}

#[test]
fn a_read_outside_the_buffer_is_out_of_bounds() {
    let buf = [0u8; 4];
    assert_eq!(read_i32(&buf, 1), Err(Malformed::OutOfBounds));
    assert_eq!(read_u16(&buf, 3), Err(Malformed::OutOfBounds));
    assert_eq!(read_f64(&buf, 0), Err(Malformed::OutOfBounds));
    assert_eq!(follow(&buf, 2), Err(Malformed::OutOfBounds));
}

/// A `uoffset_t` that points at or past the end of the buffer names no object.
#[test]
fn an_offset_off_the_end_is_out_of_bounds() {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(follow(&buf, 0), Err(Malformed::OutOfBounds));
    buf[..4].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(follow(&buf, 0), Ok(7));
}

#[test]
fn a_string_that_is_not_utf8_is_rejected() {
    let mut out = [0u8; 64];
    let len = encode_point(&mut out, 1, 2, Some("hi")).len();
    let start = out.len() - len;
    // The string's bytes sit at offsets 36 and 37 of the finished buffer.
    out[start + 36] = 0xff;
    let bytes = &out[start..];
    let table = root(bytes).expect("the root resolves");
    let label = field(bytes, table, 2)
        .expect("the slot is readable")
        .expect("label is present");
    assert_eq!(string(bytes, label), Err(Malformed::Utf8));
}

/// The scalar reads never dereference an unaligned pointer, so a buffer that
/// arrives at an odd address reads the same as one that does not.
#[test]
fn a_buffer_reads_the_same_at_any_alignment() {
    let mut out = [0u8; 64];
    let len = encode_point(&mut out, 7, -2, Some("hi")).len();
    let aligned = out[out.len() - len..].to_vec();

    for shift in 0..4 {
        let mut moved = vec![0u8; shift];
        moved.extend_from_slice(&aligned);
        let bytes = &moved[shift..];
        let table = root(bytes).expect("the root resolves");
        let x = field(bytes, table, 0).expect("readable").expect("present");
        assert_eq!(read_i32(bytes, x).expect("x reads"), 7);
    }
}

#[test]
fn a_vector_of_scalars_round_trips() {
    let mut out = [0u8; 64];
    let elements: [u8; 12] = {
        let mut e = [0u8; 12];
        for (i, v) in [10i32, -20, 30].iter().enumerate() {
            e[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        e
    };
    let bytes = {
        let mut b = Builder::new(&mut out);
        let v = b
            .push_vector(&elements, 4)
            .expect("the buffer fits the vector");
        let table = b
            .push_table(
                8,
                ALIGN,
                1,
                &[TableField {
                    slot: 0,
                    offset: 4,
                    value: Field::Offset(v),
                }],
            )
            .expect("the buffer fits the table");
        b.finish(table, ALIGN).expect("the buffer fits the root")
    };

    let table = root(bytes).expect("the root resolves");
    let at = field(bytes, table, 0).expect("readable").expect("present");
    let v: Vector = vector(bytes, at, 4).expect("the vector resolves");
    assert_eq!(v.len, 3);
    let read: Vec<i32> = (0..v.len)
        .map(|i| read_i32(bytes, v.element(i, 4)).expect("the element reads"))
        .collect();
    assert_eq!(read, vec![10, -20, 30]);
}

/// A vector whose declared length runs past the buffer is rejected once, by
/// the span check, rather than element by element.
#[test]
fn a_vector_longer_than_the_buffer_is_out_of_bounds() {
    let mut buf = [0u8; 16];
    buf[..4].copy_from_slice(&4u32.to_le_bytes()); // a uoffset at 0 naming 4
    buf[4..8].copy_from_slice(&1000u32.to_le_bytes()); // a length of 1000
    assert_eq!(vector(&buf, 0, 4), Err(Malformed::OutOfBounds));
}

#[test]
fn an_output_buffer_that_is_too_small_reports_what_it_needed() {
    let mut out = [0u8; 8];
    let mut b = Builder::new(&mut out);
    let text = b.push_string("hi").expect("a 7-byte string fits 8 bytes");
    let err = b
        .push_table(
            SIZE,
            ALIGN,
            SLOTS,
            &[TableField {
                slot: 2,
                offset: LABEL,
                value: Field::Offset(text),
            }],
        )
        .expect_err("a 16-byte table does not fit the remaining 0 bytes");
    assert_eq!(
        err,
        EncodeError::Capacity {
            needed: 24,
            available: 8
        }
    );
}

/// Two encodes of one value produce the same bytes, whatever was in the
/// buffer before (design note D-8).
#[test]
fn encoding_is_deterministic_over_a_dirty_buffer() {
    let mut clean = [0u8; 64];
    let first = encode_point(&mut clean, 7, -2, Some("hi")).to_vec();

    let mut dirty = [0xabu8; 64];
    let second = encode_point(&mut dirty, 7, -2, Some("hi")).to_vec();

    assert_eq!(first, second);
}

/// The builder writes children before parents, so a `Pos` it hands out is
/// always below the one that names it. The ordering is what makes a
/// `uoffset_t` positive, and it is worth pinning because the whole tail
/// builder rests on it.
#[test]
fn a_child_is_written_before_the_parent_that_names_it() {
    let mut out = [0u8; 64];
    let mut b = Builder::new(&mut out);
    let child: Pos = b.push_string("hi").expect("the string fits");
    let parent = b
        .push_table(
            8,
            ALIGN,
            1,
            &[TableField {
                slot: 0,
                offset: 4,
                value: Field::Offset(child),
            }],
        )
        .expect("the table fits");
    assert!(child < parent);
}

#[test]
fn a_boolean_reads_as_zero_or_not() {
    let mut out = [0u8; 32];
    let mut b = Builder::new(&mut out);
    let table = b
        .push_table(
            8,
            ALIGN,
            2,
            &[
                TableField {
                    slot: 0,
                    offset: 4,
                    value: Field::Bool(true),
                },
                TableField {
                    slot: 1,
                    offset: 5,
                    value: Field::Bool(false),
                },
            ],
        )
        .expect("the table fits");
    let bytes = b.finish(table, ALIGN).expect("the root fits");

    let t = root(bytes).expect("the root resolves");
    let yes = field(bytes, t, 0).expect("readable").expect("present");
    let no = field(bytes, t, 1).expect("readable").expect("present");
    assert!(read_bool(bytes, yes).expect("reads"));
    assert!(!read_bool(bytes, no).expect("reads"));
}
