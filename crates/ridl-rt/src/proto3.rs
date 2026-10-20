//! Shared reading and writing for the proto3 payload encoding.
//!
//! This module is gated by the `proto3` cargo feature. It holds the parts of
//! a proto3 codec that are the same for every payload type, so that a
//! generated `Payload<Proto3>` implementation carries only what its own shape
//! decides: which fields it has, their numbers and wire types, and what its
//! typl constraints are.
//!
//! It takes no dependency, allocates nothing, and contains no `unsafe` code.
//!
//! # What is here, and what is not
//!
//! The varint and zigzag encodings and their lengths; [`Writer`], which fills
//! a caller's slice forwards; [`Reader`], which walks a message's fields in
//! order and hands each one back as a [`Field`]; the narrowing of a varint to
//! the scalar its field declares; and [`Verified`], the view a generated
//! `verify` returns.
//!
//! **This module decides no layout.** It is handed field numbers, wire types
//! and scalars, and it does not know which field number a declaration's
//! member has, nor whether a run is packed. Both are facts of the emitted
//! `.proto` schema, which the generated code and the size bound read.
//!
//! # What the reader refuses
//!
//! The reader is stricter than a protobuf parser where the bytes cannot hold
//! a value of the field's type. A varint longer than ten bytes, or whose tenth
//! byte carries more than bit 0, is [`Malformed::Varint`]; a protobuf parser
//! truncates it. Field number 0 and the wire types 3, 4, 6 and 7 are
//! [`Malformed::WireType`]; proto3 has no groups, and no schema this toolchain
//! emits can produce one. A non-minimal varint (`0x80 0x00` for 0) reads as
//! its value, as every parser accepts it.
//!
//! # Writing
//!
//! A generated `encode` computes the exact encoded length first, checks it
//! against the output buffer with [`check_capacity`], and only then writes.
//! The [`Writer`]'s methods therefore return nothing: a write that would pass
//! the end of the slice writes what fits and records the overflow, and
//! [`Writer::finish`] reports it as [`EncodeError::Capacity`]. Reaching that
//! error means a length function undercounted; no method panics.

use core::str;

use crate::payload::{EncodeError, Malformed, View};

/// The wire type a tag carries in its low three bits.
///
/// proto3 has four: the groups of proto2 (3 and 4) are not among them, and 6
/// and 7 are unassigned. A tag that names any of those is
/// [`Malformed::WireType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WireType {
    /// A varint: `int32`, `int64`, `uint32`, `uint64`, `sint32`, `sint64`,
    /// `bool` and an enum.
    Varint = 0,
    /// Eight bytes, little-endian: `fixed64`, `sfixed64` and `double`.
    I64 = 1,
    /// A varint length, then that many bytes: a string, `bytes`, a nested
    /// message and a packed repeated field.
    Len = 2,
    /// Four bytes, little-endian: `fixed32`, `sfixed32` and `float`.
    I32 = 5,
}

impl WireType {
    /// The wire type the low three bits of `tag` name, or `None` for a value
    /// proto3 does not have.
    const fn from_tag(tag: u64) -> Option<WireType> {
        match tag & 0x7 {
            0 => Some(WireType::Varint),
            1 => Some(WireType::I64),
            2 => Some(WireType::Len),
            5 => Some(WireType::I32),
            _ => None,
        }
    }
}

/// The longest varint: ten bytes of seven bits each hold 70 bits, of which a
/// `u64` uses 64, so the tenth byte may carry bit 0 alone.
const MAX_VARINT_LEN: usize = 10;

/// The number of bytes the varint encoding of `v` takes: one for every seven
/// bits, and one for zero.
pub const fn varint_len(v: u64) -> usize {
    // `v | 1` makes zero one bit wide, so that it takes one byte.
    let bits = 64 - (v | 1).leading_zeros() as usize;
    bits.div_ceil(7)
}

/// The number of bytes the tag of field `number` takes, whichever wire type it
/// carries: the wire type occupies the low three bits, below the number.
pub const fn tag_len(number: u32) -> usize {
    varint_len((number as u64) << 3)
}

/// The zigzag encoding of a `sint32`: the sign is moved into bit 0 so that a
/// value near zero takes few bytes.
pub const fn zigzag32(v: i32) -> u32 {
    ((v << 1) ^ (v >> 31)) as u32
}

/// The zigzag encoding of a `sint64`.
pub const fn zigzag64(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

/// The `sint32` a zigzag-encoded `u32` holds.
pub const fn unzigzag32(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

/// The `sint64` a zigzag-encoded `u64` holds.
pub const fn unzigzag64(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// `Ok` when `out` can hold `needed` bytes, otherwise the
/// [`EncodeError::Capacity`] a generated `encode` returns.
pub fn check_capacity(needed: usize, out: &[u8]) -> Result<(), EncodeError> {
    let available = out.len();
    if needed > available {
        return Err(EncodeError::Capacity { needed, available });
    }
    Ok(())
}

/// Reads the varint at `*at` in `buf` and moves `*at` past it.
///
/// `*at` moves only on success. A varint that runs past the end of `buf` is
/// [`Malformed::OutOfBounds`]; one longer than ten bytes, or whose tenth byte
/// carries more than bit 0, is [`Malformed::Varint`]. A non-minimal encoding
/// reads as its value.
///
/// A packed run is read by calling this in a loop over the run's bytes; the
/// [`Reader`] calls it for a tag, a varint value and a length.
pub fn read_varint(buf: &[u8], at: &mut usize) -> Result<u64, Malformed> {
    let mut value: u64 = 0;
    let mut index = 0;
    loop {
        let byte = *at
            .checked_add(index)
            .and_then(|i| buf.get(i))
            .ok_or(Malformed::OutOfBounds)?;
        if index == MAX_VARINT_LEN - 1 {
            // The tenth byte may carry bit 0 alone: a continuation bit would
            // ask for an eleventh byte, and any higher bit would not fit a
            // `u64`.
            if byte > 1 {
                return Err(Malformed::Varint);
            }
            value |= u64::from(byte) << (7 * index);
            *at += MAX_VARINT_LEN;
            return Ok(value);
        }
        value |= u64::from(byte & 0x7f) << (7 * index);
        index += 1;
        if byte & 0x80 == 0 {
            *at += index;
            return Ok(value);
        }
    }
}

/// The `uint32` a varint holds, or [`Malformed::Varint`] when the value does
/// not fit.
pub fn narrow_u32(v: u64) -> Result<u32, Malformed> {
    u32::try_from(v).map_err(|_| Malformed::Varint)
}

/// The `int32` a varint holds, or [`Malformed::Varint`] when the value does
/// not fit.
///
/// A writer encodes a negative `int32` — an enum value, in a schema this
/// toolchain emits — as its sign extension to 64 bits, the ten-byte form, so
/// a value whose high 32 bits are all set and whose low 32 bits are a negative
/// `i32` is accepted; one whose high bits are anything else is refused.
pub fn narrow_i32(v: u64) -> Result<i32, Malformed> {
    i32::try_from(v as i64).map_err(|_| Malformed::Varint)
}

/// The `sint32` a zigzag varint holds, or [`Malformed::Varint`] when the
/// encoded value does not fit a `u32`.
pub fn unzigzag32_checked(v: u64) -> Result<i32, Malformed> {
    narrow_u32(v).map(unzigzag32)
}

/// The string `b` holds, checked for UTF-8.
pub fn utf8(b: &[u8]) -> Result<&str, Malformed> {
    str::from_utf8(b).map_err(|_| Malformed::Utf8)
}

/// A value read from a message, by wire type.
///
/// A varint is handed back as its `u64` and narrowed by the generated code to
/// the scalar the field declares, with [`narrow_u32`], [`narrow_i32`] or
/// [`unzigzag32_checked`], because only that code knows the declared type. A
/// fixed-width value is handed back as its bits, for the same reason: an
/// `I32` is a `fixed32`, an `sfixed32` or a `float`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'a> {
    /// A varint, as the `u64` its bytes hold.
    Varint(u64),
    /// Eight bytes, as the `u64` they hold little-endian.
    I64(u64),
    /// The bytes behind a varint length.
    Len(&'a [u8]),
    /// Four bytes, as the `u32` they hold little-endian.
    I32(u32),
}

/// One field of a message: its number and its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field<'a> {
    /// The field number the tag names, never 0.
    pub number: u32,
    /// The value, by wire type.
    pub value: Value<'a>,
}

/// Walks the fields of one message in the order the writer put them.
///
/// Each call of [`Reader::next`] reads one tag and the value it announces, so
/// a generated `verify` checks a field by number and the generated `decode`
/// reads it; an unknown field is one the generated code does not match and
/// the reader has already moved past. After an error the reader is finished:
/// every later call returns `None`.
///
/// A nested message or a packed run is handed back as [`Value::Len`], and the
/// generated code opens a `Reader` over those bytes, or reads the run with
/// [`read_varint`].
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// A reader over the message `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, at: 0 }
    }

    /// The next field, `None` at the end of the message, or the first fault
    /// found.
    ///
    /// A tag that does not fit a `u32` is [`Malformed::Varint`]; field number
    /// 0 and the wire types 3, 4, 6 and 7 are [`Malformed::WireType`]; a
    /// length or a fixed-width value that runs past the end is
    /// [`Malformed::OutOfBounds`].
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Result<Field<'a>, Malformed>> {
        if self.at >= self.buf.len() {
            return None;
        }
        let result = self.read_field();
        if result.is_err() {
            self.at = self.buf.len();
        }
        Some(result)
    }

    fn read_field(&mut self) -> Result<Field<'a>, Malformed> {
        // A tag is a `uint32` varint, so the whole tag is narrowed before it
        // is split, as a protobuf parser refuses it.
        let tag = narrow_u32(read_varint(self.buf, &mut self.at)?)?;
        let number = tag >> 3;
        let wire = WireType::from_tag(u64::from(tag)).ok_or(Malformed::WireType)?;
        if number == 0 {
            return Err(Malformed::WireType);
        }
        let value = match wire {
            WireType::Varint => Value::Varint(read_varint(self.buf, &mut self.at)?),
            WireType::I64 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(self.take(8)?);
                Value::I64(u64::from_le_bytes(bytes))
            }
            WireType::Len => {
                let len = read_varint(self.buf, &mut self.at)?;
                let len = usize::try_from(len).map_err(|_| Malformed::OutOfBounds)?;
                Value::Len(self.take(len)?)
            }
            WireType::I32 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(self.take(4)?);
                Value::I32(u32::from_le_bytes(bytes))
            }
        };
        Ok(Field { number, value })
    }

    /// The next `len` bytes, moved past, or [`Malformed::OutOfBounds`].
    fn take(&mut self, len: usize) -> Result<&'a [u8], Malformed> {
        let bytes = self
            .buf
            .get(self.at..)
            .and_then(|rest| rest.get(..len))
            .ok_or(Malformed::OutOfBounds)?;
        self.at += len;
        Ok(bytes)
    }
}

/// Writes a proto3 message forwards into a caller's slice.
///
/// Nothing here allocates and no method panics. The caller has already sized
/// the encoding and checked it with [`check_capacity`], so each method returns
/// nothing: a write that would pass the end of the slice writes what fits and
/// records the overflow, and [`Writer::finish`] reports it. The bytes written
/// are a prefix of the slice, which is why a proto3 `Encoded.bytes` is a
/// prefix of its output buffer.
pub struct Writer<'a> {
    out: &'a mut [u8],
    /// The bytes written, at most `out.len()`.
    used: usize,
    /// The bytes every write asked for, whether or not they fit.
    needed: usize,
}

impl<'a> Writer<'a> {
    /// A writer over `out`.
    pub fn new(out: &'a mut [u8]) -> Self {
        Writer {
            out,
            used: 0,
            needed: 0,
        }
    }

    /// Writes the tag of field `number` with wire type `wire`.
    pub fn tag(&mut self, number: u32, wire: WireType) {
        self.varint((u64::from(number) << 3) | u64::from(wire as u8));
    }

    /// Writes `v` as a varint: seven bits per byte, low bits first, a
    /// continuation bit on every byte but the last.
    pub fn varint(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.push(&[byte]);
                return;
            }
            self.push(&[byte | 0x80]);
        }
    }

    /// Writes `v` as four bytes, little-endian.
    pub fn fixed32(&mut self, v: u32) {
        self.push(&v.to_le_bytes());
    }

    /// Writes `v` as eight bytes, little-endian.
    pub fn fixed64(&mut self, v: u64) {
        self.push(&v.to_le_bytes());
    }

    /// Writes `b` as it is. A length-delimited field's length is written
    /// first, with [`Writer::varint`].
    pub fn bytes(&mut self, b: &[u8]) {
        self.push(b);
    }

    /// The bytes written, or [`EncodeError::Capacity`] when a write did not
    /// fit, with `needed` the bytes every write asked for in total.
    pub fn finish(self) -> Result<&'a [u8], EncodeError> {
        if self.needed > self.out.len() {
            return Err(EncodeError::Capacity {
                needed: self.needed,
                available: self.out.len(),
            });
        }
        let out: &'a [u8] = self.out;
        Ok(&out[..self.used])
    }

    fn push(&mut self, bytes: &[u8]) {
        self.needed = self.needed.saturating_add(bytes.len());
        let room = self.out.len() - self.used;
        let fits = if bytes.len() < room {
            bytes.len()
        } else {
            room
        };
        self.out[self.used..self.used + fits].copy_from_slice(&bytes[..fits]);
        self.used += fits;
    }
}

/// The view a generated proto3 `verify` returns: the verified bytes, with no
/// field accessors.
///
/// proto3 is not a random-access format — a field can be anywhere in the
/// message, so an accessor would be a scan — and the generated face reads
/// every payload through `decode`, not through a view. The bytes are reached
/// through [`View::bytes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verified<'a> {
    bytes: &'a [u8],
}

impl<'a> Verified<'a> {
    /// A view over `bytes`, which the caller has verified.
    pub fn new(bytes: &'a [u8]) -> Self {
        Verified { bytes }
    }
}

impl<'a> View<'a> for Verified<'a> {
    fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{
        narrow_i32, narrow_u32, read_varint, unzigzag32, unzigzag32_checked, unzigzag64,
        varint_len, zigzag32, zigzag64, Reader, Value, WireType, Writer,
    };
    use crate::payload::Malformed;

    #[test]
    fn varint_round_trips_at_every_length() {
        let cases: [(u64, usize); 8] = [
            (0, 1),
            (1, 1),
            (127, 1),
            (128, 2),
            (16383, 2),
            (16384, 3),
            (u64::from(u32::MAX), 5),
            (u64::MAX, 10),
        ];
        for (value, len) in cases {
            assert_eq!(varint_len(value), len, "length of {value}");
            let mut out = [0u8; 16];
            let mut w = Writer::new(&mut out);
            w.varint(value);
            let bytes = w.finish().unwrap();
            assert_eq!(bytes.len(), len, "written length of {value}");
            let mut at = 0;
            assert_eq!(read_varint(bytes, &mut at), Ok(value));
            assert_eq!(at, len);
        }
    }

    #[test]
    fn zigzag_matches_the_protobuf_table() {
        let table: [(i32, u32); 5] = [(0, 0), (-1, 1), (1, 2), (-2, 3), (i32::MIN, u32::MAX)];
        for (signed, encoded) in table {
            assert_eq!(zigzag32(signed), encoded);
            assert_eq!(unzigzag32(encoded), signed);
            assert_eq!(zigzag64(i64::from(signed)), u64::from(encoded));
            assert_eq!(unzigzag64(u64::from(encoded)), i64::from(signed));
        }
        assert_eq!(zigzag64(i64::MIN), u64::MAX);
        assert_eq!(unzigzag64(u64::MAX), i64::MIN);
    }

    #[test]
    fn an_eleven_byte_varint_is_refused() {
        let mut bytes = [0x80u8; 11];
        bytes[10] = 0x00;
        let mut at = 0;
        assert_eq!(read_varint(&bytes, &mut at), Err(Malformed::Varint));
    }

    #[test]
    fn a_tenth_byte_above_one_is_refused() {
        let mut bytes = [0xffu8; 10];
        bytes[9] = 0x02;
        let mut at = 0;
        assert_eq!(read_varint(&bytes, &mut at), Err(Malformed::Varint));
        // The tenth byte may carry bit 0 alone: this is `u64::MAX`.
        bytes[9] = 0x01;
        let mut at = 0;
        assert_eq!(read_varint(&bytes, &mut at), Ok(u64::MAX));
    }

    #[test]
    fn a_non_minimal_varint_reads() {
        let mut at = 0;
        assert_eq!(read_varint(&[0x80, 0x00], &mut at), Ok(0));
        assert_eq!(at, 2);
    }

    #[test]
    fn a_truncated_varint_is_out_of_bounds() {
        let mut at = 0;
        assert_eq!(read_varint(&[0x80], &mut at), Err(Malformed::OutOfBounds));
        let mut at = 0;
        assert_eq!(read_varint(&[], &mut at), Err(Malformed::OutOfBounds));
    }

    #[test]
    fn the_reader_refuses_field_zero_and_groups_and_wire_types_six_and_seven() {
        for tag in [0x00u8, 0x0b, 0x0c, 0x0e, 0x0f] {
            let buf = [tag, 0x01];
            let mut r = Reader::new(&buf);
            assert_eq!(r.next(), Some(Err(Malformed::WireType)), "tag {tag:#04x}");
            // After an error the reader is finished: the byte that follows the
            // bad tag is not read as a field.
            assert_eq!(r.next(), None, "tag {tag:#04x}");
        }
    }

    #[test]
    fn the_reader_reports_a_truncated_length_as_out_of_bounds() {
        let mut r = Reader::new(&[0x12, 0x05, 0x61]);
        assert_eq!(r.next(), Some(Err(Malformed::OutOfBounds)));
    }

    #[test]
    fn the_reader_yields_each_wire_type() {
        let buf = [
            0x08, 0x96, 0x01, // field 1, varint 150
            0x11, 1, 0, 0, 0, 0, 0, 0, 0, // field 2, i64
            0x1a, 0x02, 0x68, 0x69, // field 3, len "hi"
            0x25, 2, 0, 0, 0, // field 4, i32
        ];
        let mut r = Reader::new(&buf);
        let f = r.next().unwrap().unwrap();
        assert_eq!((f.number, f.value), (1, Value::Varint(150)));
        let f = r.next().unwrap().unwrap();
        assert_eq!((f.number, f.value), (2, Value::I64(1)));
        let f = r.next().unwrap().unwrap();
        assert_eq!((f.number, f.value), (3, Value::Len(b"hi")));
        let f = r.next().unwrap().unwrap();
        assert_eq!((f.number, f.value), (4, Value::I32(2)));
        assert_eq!(r.next(), None);
        assert_eq!(r.next(), None);
    }

    #[test]
    fn the_reader_refuses_a_tag_wider_than_u32() {
        // Ten bytes: a tag of `u64::MAX`, whose field number does not fit.
        let mut buf = [0xffu8; 10];
        buf[9] = 0x01;
        let mut r = Reader::new(&buf);
        assert_eq!(r.next(), Some(Err(Malformed::Varint)));
        // A tag of `1 << 32` with wire type 0: its field number `1 << 29`
        // would fit a `u32`, but the tag as a whole does not, and a protobuf
        // parser refuses it.
        let mut out = [0u8; 10];
        let mut w = Writer::new(&mut out);
        w.varint(1 << 32);
        let buf = w.finish().unwrap();
        let mut r = Reader::new(buf);
        assert_eq!(r.next(), Some(Err(Malformed::Varint)));
        assert_eq!(r.next(), None);
    }

    #[test]
    fn narrowing_refuses_what_does_not_fit() {
        assert_eq!(narrow_u32(1 << 32), Err(Malformed::Varint));
        assert_eq!(narrow_u32(u64::from(u32::MAX)), Ok(u32::MAX));
        assert_eq!(
            unzigzag32_checked(u64::from(u32::MAX) + 1),
            Err(Malformed::Varint)
        );
        assert_eq!(unzigzag32_checked(u64::from(u32::MAX)), Ok(i32::MIN));
        // The ten-byte form of -3: an `int32` sign-extended to 64 bits.
        assert_eq!(narrow_i32((-3i64) as u64), Ok(-3));
        assert_eq!(narrow_i32(1 << 40), Err(Malformed::Varint));
        assert_eq!(narrow_i32(u64::from(u32::MAX)), Err(Malformed::Varint));
        assert_eq!(narrow_i32(i32::MAX as u64), Ok(i32::MAX));
        assert_eq!(narrow_i32((i32::MIN as i64) as u64), Ok(i32::MIN));
        assert_eq!(
            narrow_i32((i32::MIN as i64 - 1) as u64),
            Err(Malformed::Varint)
        );
    }

    #[test]
    fn the_writer_writes_a_tag_and_a_packed_run() {
        let mut out = [0u8; 8];
        let mut w = Writer::new(&mut out);
        w.tag(15, WireType::Len);
        w.varint(2);
        w.varint(1);
        w.varint(2);
        assert_eq!(w.finish().unwrap(), &[0x7a, 0x02, 0x01, 0x02]);
    }

    #[test]
    fn the_writer_writes_fixed_widths_and_bytes() {
        let mut out = [0u8; 16];
        let mut w = Writer::new(&mut out);
        w.fixed32(0x0403_0201);
        w.fixed64(0x0807_0605_0403_0201);
        w.bytes(b"ab");
        assert_eq!(
            w.finish().unwrap(),
            &[1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8, b'a', b'b']
        );
    }

    #[test]
    fn the_writer_reports_an_overflow_at_finish() {
        use crate::payload::EncodeError;
        let mut out = [0u8; 3];
        let mut w = Writer::new(&mut out);
        w.tag(1, WireType::Varint);
        w.bytes(&[1, 2, 3, 4]);
        w.fixed32(7);
        assert_eq!(
            w.finish(),
            Err(EncodeError::Capacity {
                needed: 9,
                available: 3
            })
        );
    }

    #[test]
    fn check_capacity_reports_the_shortfall() {
        use crate::payload::EncodeError;
        assert_eq!(super::check_capacity(4, &[0; 4]), Ok(()));
        assert_eq!(
            super::check_capacity(5, &[0; 4]),
            Err(EncodeError::Capacity {
                needed: 5,
                available: 4
            })
        );
    }

    #[test]
    fn utf8_refuses_bad_bytes() {
        assert_eq!(super::utf8(b"hi"), Ok("hi"));
        assert_eq!(super::utf8(&[0xff]), Err(Malformed::Utf8));
    }

    #[test]
    fn verified_hands_back_its_bytes() {
        use crate::payload::View;
        let v = super::Verified::new(&[1, 2]);
        assert_eq!(View::bytes(&v), &[1, 2]);
    }

    #[test]
    fn tag_len_counts_the_shifted_number() {
        assert_eq!(super::tag_len(1), 1);
        assert_eq!(super::tag_len(15), 1);
        assert_eq!(super::tag_len(16), 2);
        assert_eq!(super::tag_len(2047), 2);
        assert_eq!(super::tag_len(2048), 3);
    }
}
