//! The podbuf helpers shared by a generated `Payload<Podbuf>` implementation.
//!
//! The whole file is gated on the feature, because `just test` builds the
//! workspace with default features and the module does not exist there.
#![cfg(feature = "podbuf")]

use ridl_rt::payload::{EncodeError, Malformed, VerifyError};
use ridl_rt::podbuf::{
    tail_range, text, Aligned, ArrayDesc, Desc, Preamble, Region, Writer, PREAMBLE_SIZE,
};

#[test]
fn the_preamble_check_is_rule_1() {
    let buf = Aligned([0u8; 16]);
    assert!(matches!(
        Preamble::read(&buf.0[1..]),
        Err(VerifyError::Structure(Malformed::Unaligned))
    ));
    assert!(matches!(
        Preamble::read(&buf.0[..7]),
        Err(VerifyError::Structure(Malformed::Preamble))
    ));
    let mut b = Aligned([0u8; 16]);
    b.0[0] = 12; // head_size 12, not a multiple of 8
    assert!(matches!(
        Preamble::read(&b.0),
        Err(VerifyError::Structure(Malformed::Preamble))
    ));
    let mut b = Aligned([0u8; 16]);
    b.0[0] = 16; // 8 + 16 > 16
    assert!(matches!(
        Preamble::read(&b.0),
        Err(VerifyError::Structure(Malformed::Preamble))
    ));
    let mut b = Aligned([0u8; 16]);
    b.0[0] = 8;
    b.0[4..8].copy_from_slice(&[0xff; 4]);
    assert_eq!(Preamble::read(&b.0).unwrap().head_size, 8); // reserved ignored
    let mut b = Aligned([0u8; 16]);
    b.0[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Preamble::read(&b.0),
        Err(VerifyError::Structure(Malformed::Preamble))
    ));
}

#[test]
fn a_region_reads_zero_past_the_producer_size() {
    let buf = Aligned([0xffu8; 32]);
    let r = Region::new(&buf.0, 8, 8);
    assert_eq!(r.u8_at(7), 0xff);
    assert_eq!(r.u8_at(8), 0);
    assert_eq!(r.u64_at(4), 0x0000_0000_ffff_ffff); // straddles the size
    assert_eq!(r.i8_at(0), -1);
    assert_eq!(r.bytes_at(6, 4).collect::<Vec<u8>>(), [0xff, 0xff, 0, 0]);
}

#[test]
fn a_region_reads_zero_past_the_buffer() {
    let buf = Aligned([0xffu8; 16]);
    let r = Region::new(&buf.0, 8, 64); // size claims more than the buffer holds
    assert_eq!(r.u32_at(4), 0xffff_ffff);
    assert_eq!(r.u32_at(8), 0);
    assert_eq!(r.u64_at(6), 0x0000_0000_0000_ffff);
}

#[test]
fn descriptors_read_both_layouts() {
    let mut buf = Aligned([0u8; 24]);
    buf.0[8..12].copy_from_slice(&40u32.to_le_bytes());
    buf.0[12..16].copy_from_slice(&7u32.to_le_bytes());
    buf.0[16..20].copy_from_slice(&48u32.to_le_bytes());
    buf.0[20..22].copy_from_slice(&3u16.to_le_bytes());
    buf.0[22..24].copy_from_slice(&16u16.to_le_bytes());
    let r = Region::new(&buf.0, 0, 24);
    let d = Desc::read(&r, 8);
    assert_eq!((d.off, d.len), (40, 7));
    assert!(!d.is_zero());
    let a = ArrayDesc::read(&r, 16);
    assert_eq!((a.off, a.len, a.elem_size), (48, 3, 16));
    assert!(!a.is_zero());
    assert!(Desc::read(&r, 0).is_zero());
    assert!(ArrayDesc::read(&r, 0).is_zero());
}

#[test]
fn tail_range_uses_checked_arithmetic() {
    assert!(matches!(
        tail_range(u32::MAX - 3, 8, 64, 16, 1),
        Err(VerifyError::Structure(Malformed::OutOfBounds))
    ));
    assert!(matches!(
        tail_range(12, 4, 64, 16, 1),
        Err(VerifyError::Structure(Malformed::OutOfBounds))
    )); // inside the head
    assert!(matches!(
        tail_range(18, 8, 64, 16, 8),
        Err(VerifyError::Structure(Malformed::Unaligned))
    ));
    assert_eq!(tail_range(16, 8, 64, 16, 8).unwrap(), 16..24);
}

#[test]
fn text_requires_a_terminator_and_no_interior_nul() {
    assert_eq!(text(b"ab\0").unwrap(), "ab");
    assert!(matches!(
        text(b"ab"),
        Err(VerifyError::Structure(Malformed::Terminator))
    ));
    assert!(matches!(
        text(b"a\0b\0"),
        Err(VerifyError::Structure(Malformed::Terminator))
    ));
    assert!(matches!(
        text(b"\xff\0"),
        Err(VerifyError::Structure(Malformed::Utf8))
    ));
}

#[test]
fn the_writer_zeroes_the_head_and_the_alignment_gaps() {
    let mut out = Aligned([0xaau8; 64]);
    let mut w = Writer::new(&mut out.0);
    let head = w.reserve_head(16).unwrap();
    assert_eq!(head, PREAMBLE_SIZE);
    let s = w.tail(5, 1).unwrap(); // 24..29
    let e = w.tail(24, 8).unwrap(); // 32..56, gap 29..32 zeroed
    let bytes = w.finish().unwrap();
    assert_eq!(&bytes[..8], &[16, 0, 0, 0, 0, 0, 0, 0]);
    assert!(bytes[8..24].iter().all(|b| *b == 0));
    assert!(bytes[s + 5..e].iter().all(|b| *b == 0));
    assert_eq!(bytes.len(), 56);
}

#[test]
fn the_writer_puts_little_endian_values_and_descriptors() {
    let mut out = Aligned::<[u8; 48]>::zeroed();
    let mut w = Writer::new(out.as_mut());
    let h = w.reserve_head(32).unwrap();
    w.put_u8(h, 0x11);
    w.put_u16(h + 2, 0x2233);
    w.put_u32(h + 4, 0x4455_6677);
    w.put_u64(h + 8, 0x0102_0304_0506_0708);
    w.put_desc(h + 16, Desc { off: 40, len: 5 });
    w.put_array_desc(
        h + 24,
        ArrayDesc {
            off: 48,
            len: 2,
            elem_size: 8,
        },
    );
    w.put_f32(h + 16, 1.0); // overwrites the descriptor, a plain store
    w.put_desc(h + 16, Desc { off: 40, len: 5 });
    let bytes = w.finish().unwrap();
    let r = Region::new(bytes, h, 32);
    assert_eq!(r.u8_at(0), 0x11);
    assert_eq!(r.u16_at(2), 0x2233);
    assert_eq!(r.u32_at(4), 0x4455_6677);
    assert_eq!(r.u64_at(8), 0x0102_0304_0506_0708);
    assert_eq!(Desc::read(&r, 16), Desc { off: 40, len: 5 });
    assert_eq!(
        ArrayDesc::read(&r, 24),
        ArrayDesc {
            off: 48,
            len: 2,
            elem_size: 8
        }
    );
}

#[test]
fn the_writer_reports_capacity_and_never_panics() {
    let mut out = Aligned([0u8; 16]);
    let mut w = Writer::new(&mut out.0);
    assert!(matches!(
        w.reserve_head(16),
        Err(EncodeError::Capacity {
            needed: 24,
            available: 16
        })
    ));
    assert!(matches!(
        w.reserve_head(u32::MAX),
        Err(EncodeError::Capacity { .. })
    ));
    let mut w = Writer::new(&mut out.0);
    w.reserve_head(0).unwrap();
    assert!(matches!(
        w.tail(u64::MAX, 8),
        Err(EncodeError::Capacity { .. })
    ));
    assert!(matches!(
        w.tail(9, 1),
        Err(EncodeError::Capacity {
            needed: 17,
            available: 16
        })
    ));
    assert_eq!(w.tail(8, 8).unwrap(), 8);
    w.put_u64(usize::MAX, 1); // out of range: ignored, no panic
    assert_eq!(w.finish().unwrap().len(), 16);
}

#[test]
fn a_writer_with_no_head_cannot_finish() {
    let mut out = Aligned([0u8; 16]);
    let w = Writer::new(&mut out.0);
    assert!(matches!(w.finish(), Err(EncodeError::Capacity { .. })));
}
