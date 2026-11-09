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
//! [`text`], [`has_nul`] and [`scalar_count`], the string checks; [`Writer`], which fills
//! the caller's slice and leaves no reserved byte unwritten; and [`Aligned`], a
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
        #[inline]
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

    #[inline]
    fn array<const N: usize>(&self, off: usize) -> [u8; N] {
        // One range check when all N bytes are inside the window and the buffer;
        // a load that crosses either end reads each byte like `u8_at`.
        let whole = off
            .checked_add(N)
            .filter(|end| *end <= self.size)
            .and_then(|end| Some((self.start.checked_add(off)?, self.start.checked_add(end)?)))
            .and_then(|(from, to)| self.buf.get(from..to));
        if let Some(bytes) = whole {
            if let Ok(out) = <[u8; N]>::try_from(bytes) {
                return out;
            }
        }
        let mut out = [0u8; N];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.u8_at(off.saturating_add(i));
        }
        out
    }

    /// Loads the byte at `off`.
    #[inline]
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
    #[inline]
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
///
/// `repr(C)`, so that a generated head struct can hold one as a field and
/// state its offset with the layout the law gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
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
///
/// `repr(C)`, for the reason [`Desc`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
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
        Some((0, data)) if !has_nul(data) => {
            core::str::from_utf8(data).map_err(|_| VerifyError::Structure(Malformed::Utf8))
        }
        _ => Err(VerifyError::Structure(Malformed::Terminator)),
    }
}

/// Whether `bytes` holds a NUL byte.
///
/// The scan takes the smallest byte of each 64-byte block, a reduction that a
/// compiler can turn into vector minimum instructions on a target that has
/// them; `<[u8]>::contains` tests one machine word per step.
#[inline]
pub fn has_nul(bytes: &[u8]) -> bool {
    let (blocks, rest) = bytes.as_chunks::<64>();
    for block in blocks {
        if block.iter().fold(u8::MAX, |least, b| least.min(*b)) == 0 {
            return true;
        }
    }
    rest.contains(&0)
}

/// The number of Unicode scalar values of `s`, the unit of a typl string bound.
pub fn scalar_count(s: &str) -> usize {
    s.chars().count()
}

/// Writes a buffer into the caller's slice. Every byte it reserves is either
/// zeroed or written by the encoder, so no byte of the caller's slice before
/// the end of the buffer keeps an older value.
///
/// The encoder reserves the head with [`Writer::reserve_head`], stores the head
/// fields with the `put_*` methods, and appends the tail blocks in the order of
/// the head's fields, depth first: [`Writer::tail_data`] for a string or a bytes
/// field, [`Writer::tail`] for any other block. A tail block starts at its
/// alignment; the gap before it is zero-filled.
///
/// A store outside the written bytes is a defect of the encoder that issued
/// it. The writer drops the store and remembers the first such offset, and
/// [`Writer::finish`] then returns [`EncodeError::Layout`] instead of a buffer
/// that would verify with wrong data. A debug build panics at the store.
#[derive(Debug)]
pub struct Writer<'o> {
    out: &'o mut [u8],
    len: usize,
    defect: Option<usize>,
}

macro_rules! puts {
    ($($name:ident => $ty:ty),* $(,)?) => {$(
        #[doc = concat!("Stores a little-endian `", stringify!($ty), "` at `at`.")]
        #[inline]
        pub fn $name(&mut self, at: usize, v: $ty) {
            self.store(at, &v.to_le_bytes());
        }
    )*};
}

impl<'o> Writer<'o> {
    /// A writer over `out`, which should start on an 8-byte boundary.
    pub fn new(out: &'o mut [u8]) -> Writer<'o> {
        Writer {
            out,
            len: 0,
            defect: None,
        }
    }

    /// Writes the preamble, zeroes a head of `size` bytes, and returns the
    /// offset of the head, which is [`PREAMBLE_SIZE`]. Calling it again starts
    /// the buffer over.
    ///
    /// An output that does not start on an 8-byte boundary is
    /// [`EncodeError::Unaligned`], and a `size` that is not a multiple of 8 is
    /// [`EncodeError::Layout`].
    pub fn reserve_head(&mut self, size: u32) -> Result<usize, EncodeError> {
        if !(self.out.as_ptr() as usize).is_multiple_of(BUFFER_ALIGN) {
            return Err(EncodeError::Unaligned);
        }
        if !size.is_multiple_of(8) {
            return Err(EncodeError::Layout { at: size as usize });
        }
        let end = PREAMBLE_SIZE.saturating_add(size as usize);
        let available = self.out.len();
        let region = self.out.get_mut(..end).ok_or(EncodeError::Capacity {
            needed: end,
            available,
        })?;
        region.fill(0);
        region[..4].copy_from_slice(&size.to_le_bytes());
        self.len = end;
        self.defect = None;
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

    /// Appends `data` at the next byte, followed by one NUL byte when
    /// `terminated` is true, and returns the offset of `data`.
    ///
    /// The block has alignment 1, so no gap precedes it. Every reserved byte
    /// is written once: unlike [`Writer::tail`] followed by
    /// [`Writer::put_bytes`], no byte is zeroed first and then overwritten.
    pub fn tail_data(&mut self, data: &[u8], terminated: bool) -> Result<usize, EncodeError> {
        let start = self.len;
        let end = start
            .saturating_add(data.len())
            .saturating_add(usize::from(terminated));
        let available = self.out.len();
        match self.out.get_mut(start..end) {
            Some(region) => {
                let (head, nul) = region.split_at_mut(data.len());
                head.copy_from_slice(data);
                nul.fill(0);
                self.len = end;
                Ok(start)
            }
            None => Err(EncodeError::Capacity {
                needed: end,
                available,
            }),
        }
    }

    #[inline]
    fn store(&mut self, at: usize, bytes: &[u8]) {
        let range = at
            .checked_add(bytes.len())
            .filter(|end| *end <= self.len)
            .map(|end| at..end);
        match range.and_then(|r| self.out.get_mut(r)) {
            Some(dst) => dst.copy_from_slice(bytes),
            None => {
                debug_assert!(false, "podbuf store outside the written bytes at {at}");
                self.defect.get_or_insert(at);
            }
        }
    }

    /// Stores the byte `v` at `at`.
    #[inline]
    pub fn put_u8(&mut self, at: usize, v: u8) {
        self.store(at, &[v]);
    }

    /// Stores `bytes` at `at`: the data of a fixed-size bytes array in the head,
    /// which [`Writer::reserve_head`] has reserved. A string or a bytes field
    /// in the tail is appended with [`Writer::tail_data`] instead.
    #[inline]
    pub fn put_bytes(&mut self, at: usize, bytes: &[u8]) {
        self.store(at, bytes);
    }

    /// Stores the signed byte `v` at `at`.
    #[inline]
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
    /// A writer that dropped a store outside the written bytes returns
    /// [`EncodeError::Layout`] with the offset of the first one. A writer
    /// whose head was never reserved has no preamble, and finishing it is
    /// [`EncodeError::Capacity`] with the preamble size as `needed`.
    pub fn finish(self) -> Result<&'o [u8], EncodeError> {
        if let Some(at) = self.defect {
            return Err(EncodeError::Layout { at });
        }
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
    fn a_load_that_crosses_the_window_or_the_buffer_reads_zero_past_the_end() {
        let buf = Aligned([
            1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ]);
        // The window is bytes 4..12 of the buffer.
        let r = Region::new(&buf.0, 4, 8);
        assert_eq!(r.u32_at(0), u32::from_le_bytes([5, 6, 7, 8]));
        assert_eq!(r.u64_at(0), u64::from_le_bytes([5, 6, 7, 8, 9, 10, 11, 12]));
        // Inside the window, ending at its last byte.
        assert_eq!(r.u16_at(6), u16::from_le_bytes([11, 12]));
        // Crossing the window end: the bytes past it read zero, not the
        // buffer's bytes 12 and 13.
        assert_eq!(r.u32_at(6), u32::from_le_bytes([11, 12, 0, 0]));
        assert_eq!(r.u16_at(7), u16::from_le_bytes([12, 0]));
        // At and past the window end.
        assert_eq!(r.u16_at(8), 0);
        assert_eq!(r.u64_at(100), 0);
        // A window that extends past the buffer: bytes past the buffer read zero.
        let r = Region::new(&buf.0[..8], 4, 100);
        assert_eq!(r.u64_at(0), u64::from_le_bytes([5, 6, 7, 8, 0, 0, 0, 0]));
        assert_eq!(r.u32_at(2), u32::from_le_bytes([7, 8, 0, 0]));
        assert_eq!(r.i16_at(4), 0);
        // A start so large that start + off overflows reads zero; the buffer's
        // first bytes are not read through a wrapped address.
        for start in [usize::MAX, usize::MAX - 3] {
            let r = Region::new(&buf.0, start, 8);
            assert_eq!(r.u32_at(1), 0);
            assert_eq!(r.u64_at(0), 0);
        }
        // Every load shares the path: the other widths and the float loads.
        let r = Region::new(&buf.0, 0, 16);
        assert_eq!(r.i32_at(4), i32::from_le_bytes([5, 6, 7, 8]));
        assert_eq!(
            r.i64_at(8),
            i64::from_le_bytes([9, 10, 11, 12, 13, 14, 15, 16])
        );
        assert_eq!(r.f32_at(0), f32::from_le_bytes([1, 2, 3, 4]));
        assert_eq!(
            r.f64_at(8),
            f64::from_le_bytes([9, 10, 11, 12, 13, 14, 15, 16])
        );
        assert_eq!(r.i32_at(14), i32::from_le_bytes([15, 16, 0, 0]));
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
        assert!(matches!(
            w.reserve_head(u32::MAX - 7),
            Err(EncodeError::Capacity { .. })
        ));
        w.reserve_head(8).unwrap();
        assert!(w.tail(u64::MAX, usize::MAX).is_err());
        assert!(w.tail(u64::MAX, 8).is_err());
        assert!(w.tail(1, usize::MAX).is_err());
        // The refused calls leave the written length alone.
        assert_eq!(w.finish().unwrap().len(), 16);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn a_store_past_the_written_bytes_makes_finish_fail() {
        let mut out = Aligned([0u8; 32]);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        w.put_u32(14, 0xffff_ffff); // 14..18 passes the written 16 bytes
        w.put_u8(usize::MAX, 1); // a later defect does not replace the first
        assert!(matches!(w.finish(), Err(EncodeError::Layout { at: 14 })));
        assert!(out.0.iter().skip(8).all(|b| *b == 0));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "outside the written bytes")]
    fn a_store_past_the_written_bytes_panics_in_a_debug_build() {
        let mut out = Aligned([0u8; 32]);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        w.put_u32(14, 0xffff_ffff);
    }

    #[test]
    fn reserve_head_refuses_an_unaligned_output_and_a_size_that_is_not_a_multiple_of_8() {
        let mut out = Aligned([0u8; 64]);
        let mut w = Writer::new(&mut out.0[1..]);
        assert!(matches!(w.reserve_head(8), Err(EncodeError::Unaligned)));
        let mut w = Writer::new(&mut out.0);
        assert!(matches!(
            w.reserve_head(12),
            Err(EncodeError::Layout { at: 12 })
        ));
        assert!(w.reserve_head(16).is_ok());
    }

    #[test]
    fn put_bytes_stores_a_slice_inside_the_reserved_tail() {
        let mut out = Aligned([0xaau8; 32]);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        let at = w.tail(3, 1).unwrap();
        w.put_bytes(at, b"ab");
        let bytes = w.finish().unwrap();
        assert_eq!(&bytes[16..19], b"ab\0");
        assert_eq!(bytes.len(), 19);
    }

    #[test]
    fn tail_data_writes_the_data_and_a_zero_terminator_over_dirty_output() {
        let mut out = Aligned::<[u8; 64]>::zeroed();
        out.0.fill(0xAA);
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        let a = w.tail_data(b"abc", true).unwrap();
        let b = w.tail_data(b"xy", false).unwrap();
        let c = w.tail_data(b"", true).unwrap();
        assert_eq!((a, b, c), (16, 20, 22));
        let bytes = w.finish().unwrap();
        assert_eq!(&bytes[16..23], b"abc\0xy\0");
    }

    #[test]
    fn tail_data_refuses_a_block_that_does_not_fit_and_leaves_the_writer_usable() {
        let mut out = Aligned::<[u8; 24]>::zeroed();
        let mut w = Writer::new(&mut out.0);
        w.reserve_head(8).unwrap();
        assert_eq!(
            w.tail_data(b"abcdefgh", true),
            Err(EncodeError::Capacity {
                needed: 25,
                available: 24
            })
        );
        assert_eq!(w.tail_data(b"abcdefg", true), Ok(16));
        assert!(w.tail_data(b"", true).is_err());
        assert!(w.tail_data(&[], false).is_ok());
    }

    #[test]
    fn has_nul_finds_a_nul_at_every_position_and_length() {
        for len in 0..200usize {
            // Non-zero fillers that differ, so that a reduction that is right
            // only for 0 and 1 (an AND, for one) fails.
            let mut data = [0u8; 200];
            for (i, b) in data.iter_mut().enumerate() {
                *b = [1u8, 2, 0x7f, 0x80, 0xff][i % 5];
            }
            assert!(!has_nul(&data[..len]), "len {len}");
            for at in 0..len {
                let kept = data[at];
                data[at] = 0;
                assert!(has_nul(&data[..len]), "len {len} at {at}");
                data[at] = kept;
            }
        }
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
