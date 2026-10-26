//! Shared reading and writing for the podbuf payload encoding.
//!
//! This module is gated by the `podbuf` cargo feature. It holds the parts of a
//! podbuf codec that are the same for every payload type, so that a generated
//! `Payload<Podbuf>` implementation carries only what its own shape decides:
//! the size and the offsets of its head, which fields it has, and what its
//! typl constraints are.
//!
//! It takes no dependency, allocates nothing, and contains no `unsafe` code.
//! Every read is a little-endian load over bytes that were fetched with a
//! bounds check, so no input makes a function of this module panic.
//!
//! # What is here, and what is not
//!
//! [`Preamble`], the 8-byte start of every buffer; [`Region`], a window over
//! the head, an element or a union arm that reads 0 past the size the producer
//! wrote; [`Desc`] and [`ArrayDesc`], the two descriptor layouts;
//! [`tail_range`], the bounds and alignment check of a descriptor's data;
//! [`text`] and [`scalar_count`], the string checks; [`Writer`], which fills
//! the caller's slice and zeroes every byte it reserves; and [`Aligned`], a
//! wrapper that gives a byte array the 8-byte alignment of a buffer.
//!
//! **This module decides no layout.** It is handed sizes, offsets and
//! alignments.
//!
//! # Reading past the producer's size
//!
//! A [`Region`] stores the size in bytes that the producer wrote for its head,
//! element or arm. A byte at or past that size reads as 0, so a field that a
//! later release appended reads as 0 when an older producer wrote the buffer.

#[cfg(not(target_endian = "little"))]
compile_error!("podbuf requires a little-endian target");

use crate::payload::{EncodeError, Malformed, VerifyError};

/// The size in bytes of the preamble that starts every buffer.
pub const PREAMBLE_SIZE: usize = 8;

/// The alignment in bytes of the start of a buffer and of a head.
const BUFFER_ALIGN: usize = 8;

/// A value stored at the alignment of a buffer start.
///
/// podbuf requires a buffer to start on an 8-byte boundary. `Aligned([0u8; N])`
/// gives a byte array that alignment, for an output buffer or for a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct Aligned<T>(pub T);

impl<const N: usize> Aligned<[u8; N]> {
    /// An array of `N` zero bytes.
    pub const fn zeroed() -> Self {
        Aligned([0; N])
    }
}

impl<const N: usize> AsRef<[u8]> for Aligned<[u8; N]> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> AsMut<[u8]> for Aligned<[u8; N]> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

/// The preamble of a buffer: `{ head_size: u32, reserved: u32 }`, both
/// little-endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preamble {
    /// The size in bytes of the head the producer wrote. It is a multiple of 8.
    pub head_size: u32,
}

impl Preamble {
    /// Reads and checks the preamble of `buf`.
    ///
    /// The buffer start must be 8-aligned ([`Malformed::Unaligned`]), the
    /// buffer must hold at least 8 bytes, `head_size` must be a multiple of 8,
    /// and `8 + head_size` must not exceed the buffer length
    /// ([`Malformed::Preamble`]). The `reserved` word is ignored.
    pub fn read(buf: &[u8]) -> Result<Preamble, VerifyError> {
        if !(buf.as_ptr() as usize).is_multiple_of(BUFFER_ALIGN) {
            return Err(VerifyError::Structure(Malformed::Unaligned));
        }
        let head_size = Region::new(buf, 0, buf.len()).u32_at(0);
        let bad = VerifyError::Structure(Malformed::Preamble);
        if buf.len() < PREAMBLE_SIZE || !head_size.is_multiple_of(8) {
            return Err(bad);
        }
        match PREAMBLE_SIZE.checked_add(head_size as usize) {
            Some(end) if end <= buf.len() => Ok(Preamble { head_size }),
            _ => Err(bad),
        }
    }
}

/// A window over a buffer that reads 0 past the size the producer wrote.
///
/// `start` is the offset of the window in the buffer and `size` is the number
/// of bytes the producer wrote for it. A load at `off` reads each of its bytes
/// from `buf[start + off]` when `off` is below `size` and the byte exists, and
/// reads 0 otherwise.
#[derive(Clone, Copy, Debug)]
pub struct Region<'a> {
    buf: &'a [u8],
    start: usize,
    size: usize,
}

macro_rules! loads {
    ($($name:ident => $ty:ty),* $(,)?) => {$(
        #[doc = concat!("Loads a little-endian `", stringify!($ty), "` at `off`.")]
        pub fn $name(&self, off: usize) -> $ty {
            <$ty>::from_le_bytes(self.array(off))
        }
    )*};
}

impl<'a> Region<'a> {
    /// A window of `size` bytes that starts at `start` in `buf`.
    pub fn new(buf: &'a [u8], start: usize, size: usize) -> Region<'a> {
        Region { buf, start, size }
    }

    fn array<const N: usize>(&self, off: usize) -> [u8; N] {
        let mut out = [0u8; N];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.u8_at(off.saturating_add(i));
        }
        out
    }

    /// Loads the byte at `off`.
    pub fn u8_at(&self, off: usize) -> u8 {
        if off >= self.size {
            return 0;
        }
        self.start
            .checked_add(off)
            .and_then(|at| self.buf.get(at))
            .copied()
            .unwrap_or(0)
    }

    /// Loads the signed byte at `off`.
    pub fn i8_at(&self, off: usize) -> i8 {
        i8::from_le_bytes([self.u8_at(off)])
    }

    loads! {
        u16_at => u16,
        u32_at => u32,
        u64_at => u64,
        i16_at => i16,
        i32_at => i32,
        i64_at => i64,
        f32_at => f32,
        f64_at => f64,
    }

    /// The `n` bytes from `off`, each read like [`Region::u8_at`].
    pub fn bytes_at(&self, off: usize, n: usize) -> impl Iterator<Item = u8> + '_ {
        (0..n).map(move |i| self.u8_at(off.saturating_add(i)))
    }
}

/// The descriptor of a string, bytes or union field:
/// `{ off: u32, len: u32 }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Desc {
    /// The offset of the data from the start of the buffer.
    pub off: u32,
    /// The length of the data: bytes for a string or bytes, the arm head size
    /// in bytes for a union.
    pub len: u32,
}

impl Desc {
    /// Reads the descriptor at offset `at` of `region`.
    pub fn read(region: &Region<'_>, at: usize) -> Desc {
        Desc {
            off: region.u32_at(at),
            len: region.u32_at(at.saturating_add(4)),
        }
    }

    /// Whether both words are 0: an empty field or an absent optional.
    pub fn is_zero(&self) -> bool {
        self.off == 0 && self.len == 0
    }
}

/// The descriptor of an array or map field:
/// `{ off: u32, len: u16, elem_size: u16 }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArrayDesc {
    /// The offset of the first element from the start of the buffer.
    pub off: u32,
    /// The number of elements or entries.
    pub len: u16,
    /// The size in bytes of one element or entry as the producer laid it out.
    pub elem_size: u16,
}

impl ArrayDesc {
    /// Reads the descriptor at offset `at` of `region`.
    pub fn read(region: &Region<'_>, at: usize) -> ArrayDesc {
        ArrayDesc {
            off: region.u32_at(at),
            len: region.u16_at(at.saturating_add(4)),
            elem_size: region.u16_at(at.saturating_add(6)),
        }
    }

    /// Whether all three words are 0: an empty field or an absent optional.
    pub fn is_zero(&self) -> bool {
        self.off == 0 && self.len == 0 && self.elem_size == 0
    }
}

/// Checks where a descriptor's data lies and returns its byte range.
///
/// The data starts at `off` and is `bytes` long. `off` must be at least
/// `min_off`, the first byte of the tail, and the end must not pass `buf_len`;
/// both failures, and an overflow of the end, are [`Malformed::OutOfBounds`].
/// `off` must be a multiple of `align` ([`Malformed::Unaligned`]); an `align`
/// of 0 is treated as 1.
pub fn tail_range(
    off: u32,
    bytes: u64,
    buf_len: usize,
    min_off: usize,
    align: usize,
) -> Result<core::ops::Range<usize>, VerifyError> {
    let out_of_bounds = VerifyError::Structure(Malformed::OutOfBounds);
    let start = off as usize;
    if start < min_off {
        return Err(out_of_bounds);
    }
    let end = (start as u64)
        .checked_add(bytes)
        .and_then(|end| usize::try_from(end).ok())
        .filter(|end| *end <= buf_len)
        .ok_or(out_of_bounds)?;
    if !start.is_multiple_of(align.max(1)) {
        return Err(VerifyError::Structure(Malformed::Unaligned));
    }
    Ok(start..end)
}

/// Checks the bytes of a string and returns its text.
///
/// `bytes` is the string's data followed by its terminator, so the caller
/// passes `len + 1` bytes. The last byte must be NUL and no other byte may be
/// ([`Malformed::Terminator`]), and the data must be valid UTF-8
/// ([`Malformed::Utf8`]).
pub fn text(bytes: &[u8]) -> Result<&str, VerifyError> {
    match bytes.split_last() {
        Some((0, data)) if !data.contains(&0) => {
            core::str::from_utf8(data).map_err(|_| VerifyError::Structure(Malformed::Utf8))
        }
        _ => Err(VerifyError::Structure(Malformed::Terminator)),
    }
}

/// The number of Unicode scalar values of `s`, the unit of a typl string bound.
pub fn scalar_count(s: &str) -> usize {
    s.chars().count()
}

/// Writes a buffer into the caller's slice and zeroes every byte it reserves.
///
/// The encoder reserves the head with [`Writer::reserve_head`], stores the head
/// fields with the `put_*` methods, and appends the tail blocks with
/// [`Writer::tail`] in the order of the head's fields, depth first. A tail
/// block starts at its alignment; the gap before it is zero-filled. A store
/// outside the written bytes is ignored.
#[derive(Debug)]
pub struct Writer<'o> {
    out: &'o mut [u8],
    len: usize,
}

macro_rules! puts {
    ($($name:ident => $ty:ty),* $(,)?) => {$(
        #[doc = concat!("Stores a little-endian `", stringify!($ty), "` at `at`.")]
        pub fn $name(&mut self, at: usize, v: $ty) {
            self.store(at, &v.to_le_bytes());
        }
    )*};
}

impl<'o> Writer<'o> {
    /// A writer over `out`, which should start on an 8-byte boundary.
    pub fn new(out: &'o mut [u8]) -> Writer<'o> {
        Writer { out, len: 0 }
    }

    /// Writes the preamble, zeroes a head of `size` bytes, and returns the
    /// offset of the head, which is [`PREAMBLE_SIZE`]. `size` should be a
    /// multiple of 8. Calling it again starts the buffer over.
    pub fn reserve_head(&mut self, size: u32) -> Result<usize, EncodeError> {
        let end = PREAMBLE_SIZE.saturating_add(size as usize);
        let available = self.out.len();
        let region = self.out.get_mut(..end).ok_or(EncodeError::Capacity {
            needed: end,
            available,
        })?;
        region.fill(0);
        region[..4].copy_from_slice(&size.to_le_bytes());
        self.len = end;
        Ok(PREAMBLE_SIZE)
    }

    /// Appends `bytes` zeroed bytes at a multiple of `align`, zero-fills the
    /// gap before them, and returns their offset. An `align` of 0 is treated
    /// as 1.
    pub fn tail(&mut self, bytes: u64, align: usize) -> Result<usize, EncodeError> {
        let align = align.max(1);
        let start = self.len.div_ceil(align).saturating_mul(align);
        let end = (start as u64)
            .saturating_add(bytes)
            .try_into()
            .unwrap_or(usize::MAX);
        let available = self.out.len();
        match self.out.get_mut(self.len..end) {
            Some(region) => {
                region.fill(0);
                self.len = end;
                Ok(start)
            }
            None => Err(EncodeError::Capacity {
                needed: end,
                available,
            }),
        }
    }

    fn store(&mut self, at: usize, bytes: &[u8]) {
        let range = at
            .checked_add(bytes.len())
            .filter(|end| *end <= self.len)
            .map(|end| at..end);
        if let Some(dst) = range.and_then(|r| self.out.get_mut(r)) {
            dst.copy_from_slice(bytes);
        }
    }

    /// Stores the byte `v` at `at`.
    pub fn put_u8(&mut self, at: usize, v: u8) {
        self.store(at, &[v]);
    }

    /// Stores the signed byte `v` at `at`.
    pub fn put_i8(&mut self, at: usize, v: i8) {
        self.store(at, &v.to_le_bytes());
    }

    puts! {
        put_u16 => u16,
        put_u32 => u32,
        put_u64 => u64,
        put_i16 => i16,
        put_i32 => i32,
        put_i64 => i64,
        put_f32 => f32,
        put_f64 => f64,
    }

    /// Stores the descriptor `d` at `at`.
    pub fn put_desc(&mut self, at: usize, d: Desc) {
        self.put_u32(at, d.off);
        self.put_u32(at.saturating_add(4), d.len);
    }

    /// Stores the array descriptor `d` at `at`.
    pub fn put_array_desc(&mut self, at: usize, d: ArrayDesc) {
        self.put_u32(at, d.off);
        self.put_u16(at.saturating_add(4), d.len);
        self.put_u16(at.saturating_add(6), d.elem_size);
    }

    /// Ends the buffer and returns the bytes written.
    ///
    /// A writer whose head was never reserved has no preamble, and finishing
    /// it is [`EncodeError::Capacity`] with the preamble size as `needed`.
    pub fn finish(self) -> Result<&'o [u8], EncodeError> {
        if self.len < PREAMBLE_SIZE {
            return Err(EncodeError::Capacity {
                needed: PREAMBLE_SIZE,
                available: self.out.len(),
            });
        }
        let len = self.len;
        let out: &'o [u8] = self.out;
        out.get(..len).ok_or(EncodeError::Capacity {
            needed: len,
            available: out.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIG: [usize; 3] = [usize::MAX, usize::MAX - 3, u32::MAX as usize];

    #[test]
    fn region_loads_with_buffer_derived_numbers_do_not_panic() {
        let buf = Aligned([0xffu8; 32]);
        for start in BIG {
            for size in BIG {
                let r = Region::new(&buf.0, start, size);
                for off in BIG {
                    let _ = (r.u8_at(off), r.u16_at(off), r.u32_at(off), r.u64_at(off));
                    let _ = (r.i64_at(off), r.f32_at(off), r.f64_at(off));
                    let _ = Desc::read(&r, off);
                    let _ = ArrayDesc::read(&r, off);
                    assert_eq!(r.bytes_at(off, 3).count(), 3);
                }
            }
        }
        // An empty window at the maximum start reads zeros.
        let r = Region::new(&buf.0, usize::MAX, usize::MAX);
        assert_eq!(r.u64_at(usize::MAX), 0);
    }

    #[test]
    fn tail_range_with_maximum_numbers_is_an_error_not_a_panic() {
        for off in [0, 1, u32::MAX - 7, u32::MAX] {
            for bytes in [0, 1, u64::from(u32::MAX), u64::MAX - 1, u64::MAX] {
                for align in [0, 1, 8, usize::MAX] {
                    let _ = tail_range(off, bytes, usize::MAX, usize::MAX, align);
                    let _ = tail_range(off, bytes, 64, 16, align);
                    let _ = tail_range(off, bytes, 0, 0, align);
                }
            }
        }
        assert!(tail_range(8, u64::MAX, usize::MAX, 8, 8).is_err());
        assert_eq!(tail_range(8, 0, 8, 8, 0).unwrap(), 8..8);
    }

    #[test]
    fn the_writer_with_maximum_numbers_is_an_error_not_a_panic() {
        let mut out = Aligned([0u8; 32]);
        let mut w = Writer::new(&mut out.0);
        assert!(w.tail(u64::MAX, usize::MAX).is_err());
        assert!(w.reserve_head(u32::MAX).is_err());
        w.reserve_head(8).unwrap();
        assert!(w.tail(u64::MAX, usize::MAX).is_err());
        assert!(w.tail(u64::MAX, 8).is_err());
        assert!(w.tail(1, usize::MAX).is_err());
        w.put_desc(usize::MAX, Desc { off: 1, len: 1 });
        w.put_array_desc(
            usize::MAX - 2,
            ArrayDesc {
                off: 1,
                len: 1,
                elem_size: 1,
            },
        );
        // The refused calls leave the written length alone.
        assert_eq!(w.finish().unwrap().len(), 16);
    }

    #[test]
    fn a_store_past_the_written_bytes_is_ignored() {
        let mut out = Aligned([0u8; 32]);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        w.put_u32(14, 0xffff_ffff); // 14..18 passes the written 16 bytes
        let bytes = w.finish().unwrap();
        assert!(bytes.iter().skip(8).all(|b| *b == 0));
    }

    #[test]
    fn scalar_count_counts_scalar_values_not_bytes() {
        assert_eq!(scalar_count(""), 0);
        assert_eq!(scalar_count("ab"), 2);
        assert_eq!(scalar_count("é€😀"), 3);
    }

    #[test]
    fn a_second_reserve_starts_the_buffer_over() {
        let mut out = Aligned([0xaau8; 32]);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        w.tail(8, 8).unwrap();
        w.reserve_head(16).unwrap();
        assert_eq!(w.finish().unwrap().len(), 24);
    }
}
