//! Shared reading and writing for the FlatBuffers payload encoding.
//!
//! This module is gated by the `flatbuffers` cargo feature. It holds the parts
//! of a FlatBuffers codec that are the same for every payload type, so that a
//! generated `Payload<FlatBuffers>` implementation carries only what its own
//! shape decides: which fields it has, where the projection put them, and what
//! its typl constraints are.
//!
//! It takes no dependency. The FlatBuffers runtime crate that ADR-0020
//! decision 5 permits under this feature is not used here, and nothing in this
//! module allocates.
//!
//! # What is here, and what is not
//!
//! Reading: the byte-order reads, the vtable walk, and the string and vector
//! headers — everything a generated `verify` needs to walk a buffer it does
//! not trust. Writing: [`Builder`], which builds a buffer from the end of a
//! caller's slice downwards.
//!
//! **This module decides no layout.** Which slot a field takes, what its
//! offset inside the table is, and how large the table is are facts of the
//! projection, computed once and read by both the size bound and the encoder.
//! [`Builder::table`] is handed those facts and writes the bytes they
//! describe; it does not choose them. That is what keeps
//! `Payload::MAX_SIZE` and the encoder from disagreeing.
//!
//! Helpers for a vector of tables and for a union arrive with the emitter that
//! needs them, whose shape the projection settles.
//!
//! # Alignment, and why reads are safe at any alignment
//!
//! Every scalar is read by copying its bytes into an array and calling
//! `from_le_bytes`, so no read dereferences an unaligned pointer and a buffer
//! may arrive at any alignment, which is what a transport hands over. A
//! generated `verify` therefore never produces `Malformed::Unaligned`.
//!
//! The writer still aligns what it writes, because a reader that is not this
//! one — a `flatc`-generated accessor, or another language's runtime — does
//! read in place.

use core::mem::size_of;
use core::str;

use crate::payload::{EncodeError, Malformed};

/// The size of a `uoffset_t`, of an `soffset_t`, and of a vector's or a
/// string's length prefix.
pub const OFFSET_SIZE: usize = 4;

/// The size of a `voffset_t`, one vtable entry.
pub const VOFFSET_SIZE: usize = 2;

/// The bytes a vtable carries before its first entry: its own size, then the
/// size of the table it describes.
pub const VTABLE_HEADER: usize = 4;

fn at(buf: &[u8], from: usize, len: usize) -> Result<&[u8], Malformed> {
    buf.get(from..)
        .and_then(|rest| rest.get(..len))
        .ok_or(Malformed::OutOfBounds)
}

macro_rules! read_scalar {
    ($name:ident, $ty:ty, $what:literal) => {
        #[doc = concat!("Reads the little-endian `", $what, "` at `from`.")]
        ///
        /// The bytes are copied before they are read, so `from` needs no
        /// alignment.
        pub fn $name(buf: &[u8], from: usize) -> Result<$ty, Malformed> {
            let mut bytes = [0u8; size_of::<$ty>()];
            bytes.copy_from_slice(at(buf, from, size_of::<$ty>())?);
            Ok(<$ty>::from_le_bytes(bytes))
        }
    };
}

read_scalar!(read_u8, u8, "u8");
read_scalar!(read_i8, i8, "i8");
read_scalar!(read_u16, u16, "u16");
read_scalar!(read_i16, i16, "i16");
read_scalar!(read_u32, u32, "u32");
read_scalar!(read_i32, i32, "i32");
read_scalar!(read_u64, u64, "u64");
read_scalar!(read_i64, i64, "i64");
read_scalar!(read_f32, f32, "f32");
read_scalar!(read_f64, f64, "f64");

/// Reads the FlatBuffers boolean at `from`: zero is false, anything else is
/// true.
pub fn read_bool(buf: &[u8], from: usize) -> Result<bool, Malformed> {
    Ok(read_u8(buf, from)? != 0)
}

/// Follows the `uoffset_t` stored at `from` and returns the position it names.
///
/// A `uoffset_t` is unsigned and relative to its own position, so it always
/// points forward.
pub fn follow(buf: &[u8], from: usize) -> Result<usize, Malformed> {
    let relative = u64::from(read_u32(buf, from)?);
    let target = from as u64 + relative;
    if target >= buf.len() as u64 {
        return Err(Malformed::OutOfBounds);
    }
    Ok(target as usize)
}

/// The position of the root table: the `uoffset_t` at the start of the buffer.
pub fn root(buf: &[u8]) -> Result<usize, Malformed> {
    follow(buf, 0)
}

/// The position of field `slot` of the table at `table`, or `None` when the
/// table does not carry it.
///
/// An absent field is not an error: a FlatBuffers table omits a field whose
/// value equals its declared default, and a vtable shorter than `slot` omits
/// every field from there on. Whether an absent field is legal for the type is
/// the generated `verify`'s question, not this one's.
pub fn field(buf: &[u8], table: usize, slot: u16) -> Result<Option<usize>, Malformed> {
    // The table's first field is a signed offset backwards to its vtable.
    // The arithmetic is done in i64 so that it cannot wrap on any target.
    let soffset = read_i32(buf, table)?;
    let vtable = table as i64 - i64::from(soffset);
    if vtable < 0 || vtable as u64 >= buf.len() as u64 {
        return Err(Malformed::OutOfBounds);
    }
    let vtable = vtable as usize;

    let vtable_bytes = usize::from(read_u16(buf, vtable)?);
    if vtable_bytes < VTABLE_HEADER {
        return Err(Malformed::OutOfBounds);
    }
    // The whole vtable must lie inside the buffer, so that a walk of it is
    // bounded by the buffer rather than by its own declared size.
    at(buf, vtable, vtable_bytes)?;
    let table_bytes = usize::from(read_u16(buf, vtable + VOFFSET_SIZE)?);

    let entry = vtable + VTABLE_HEADER + usize::from(slot) * VOFFSET_SIZE;
    if entry + VOFFSET_SIZE > vtable + vtable_bytes {
        return Ok(None);
    }
    let offset = usize::from(read_u16(buf, entry)?);
    if offset == 0 {
        return Ok(None);
    }
    // The field must lie inside the table the vtable describes.
    if offset >= table_bytes {
        return Err(Malformed::OutOfBounds);
    }
    let position = table as u64 + offset as u64;
    if position >= buf.len() as u64 {
        return Err(Malformed::OutOfBounds);
    }
    Ok(Some(position as usize))
}

/// The string the `uoffset_t` at `from` names, checked for UTF-8.
pub fn string(buf: &[u8], from: usize) -> Result<&str, Malformed> {
    let start = follow(buf, from)?;
    let len = read_u32(buf, start)? as usize;
    let bytes = at(buf, start + OFFSET_SIZE, len)?;
    str::from_utf8(bytes).map_err(|_| Malformed::Utf8)
}

/// A vector's length and the position of its first element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vector {
    /// The number of elements.
    pub len: usize,
    /// The position of element zero.
    pub first: usize,
}

impl Vector {
    /// The position of element `index`, which the caller has checked against
    /// [`Vector::len`].
    pub fn element(&self, index: usize, stride: usize) -> usize {
        self.first + index * stride
    }
}

/// The vector the `uoffset_t` at `from` names, whose elements are `stride`
/// bytes each.
///
/// The whole element span is checked against the buffer here, so a walk over
/// the elements is bounded by one check rather than by one per element.
pub fn vector(buf: &[u8], from: usize, stride: usize) -> Result<Vector, Malformed> {
    let start = follow(buf, from)?;
    let len = read_u32(buf, start)? as usize;
    let first = start + OFFSET_SIZE;
    let span = len.checked_mul(stride).ok_or(Malformed::OutOfBounds)?;
    at(buf, first, span)?;
    Ok(Vector { len, first })
}

/// The position of an object in a buffer under construction, measured
/// backwards from the end of the builder's slice.
///
/// It is not an offset into the finished buffer. A `Pos` is only meaningful to
/// the [`Builder`] that produced it, and passing one to a different builder is
/// a programming error that `debug_assert` catches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos(usize);

/// A value written into a table field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    /// A boolean, one byte.
    Bool(bool),
    /// An unsigned byte.
    U8(u8),
    /// A signed byte.
    I8(i8),
    /// An unsigned 16-bit integer.
    U16(u16),
    /// A signed 16-bit integer.
    I16(i16),
    /// An unsigned 32-bit integer.
    U32(u32),
    /// A signed 32-bit integer.
    I32(i32),
    /// An unsigned 64-bit integer.
    U64(u64),
    /// A signed 64-bit integer.
    I64(i64),
    /// A 32-bit float.
    F32(f32),
    /// A 64-bit float.
    F64(f64),
    /// A `uoffset_t` to an object already written: a string, a vector or a
    /// table.
    Offset(Pos),
}

impl Field {
    /// The bytes this value occupies in a table.
    pub fn size(&self) -> usize {
        match self {
            Field::Bool(_) | Field::U8(_) | Field::I8(_) => 1,
            Field::U16(_) | Field::I16(_) => 2,
            Field::U32(_) | Field::I32(_) | Field::F32(_) | Field::Offset(_) => 4,
            Field::U64(_) | Field::I64(_) | Field::F64(_) => 8,
        }
    }
}

/// One field of a table, at the slot and offset the projection assigned it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableField {
    /// The vtable slot, which is the field's position in declaration order.
    pub slot: u16,
    /// The field's offset from the start of the table.
    pub offset: u16,
    /// The value.
    pub value: Field,
}

/// Builds a FlatBuffers buffer from the end of a caller's slice downwards.
///
/// A FlatBuffers buffer is built back to front: a child is written before the
/// parent that names it, and the root offset is written last and sits at the
/// start of the finished buffer. The builder therefore fills the slice from
/// its end, and [`Builder::finish`] returns the finished buffer as a subslice
/// of the slice ending at its end — which is why `Encoded.bytes` is a subslice
/// rather than a prefix.
///
/// Nothing here allocates. The caller's slice is the only storage, and a write
/// that does not fit returns [`EncodeError::Capacity`] with the bytes the
/// encoding needed so far.
pub struct Builder<'a> {
    out: &'a mut [u8],
    used: usize,
}

impl<'a> Builder<'a> {
    /// A builder over `out`.
    pub fn new(out: &'a mut [u8]) -> Self {
        Builder { out, used: 0 }
    }

    /// The bytes written so far, including alignment padding.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Reserves `size` bytes for an object aligned to `align`, and returns its
    /// position and the bytes to fill, in address order.
    ///
    /// The reserved bytes and any padding are zeroed, so a buffer the caller
    /// handed over dirty does not leak its old contents into the encoding and
    /// two encodes of one value produce the same bytes.
    pub fn reserve(&mut self, size: usize, align: usize) -> Result<(Pos, &mut [u8]), EncodeError> {
        debug_assert!(align.is_power_of_two(), "an alignment is a power of two");
        let capacity = self.out.len();
        let unpadded = match self.used.checked_add(size) {
            Some(n) => n,
            None => {
                return Err(EncodeError::Capacity {
                    needed: usize::MAX,
                    available: capacity,
                })
            }
        };
        // An object at position `p` starts at offset `total - p` of the
        // finished buffer, and `finish` makes `total` a multiple of the
        // buffer's alignment. Padding so that `p` is a multiple of `align`
        // therefore aligns the object in the finished buffer.
        let padding = (align - (unpadded % align)) % align;
        let needed = match unpadded.checked_add(padding) {
            Some(n) => n,
            None => {
                return Err(EncodeError::Capacity {
                    needed: usize::MAX,
                    available: capacity,
                })
            }
        };
        if needed > capacity {
            return Err(EncodeError::Capacity {
                needed,
                available: capacity,
            });
        }
        self.used = needed;
        let start = capacity - needed;
        self.out[start..start + size + padding].fill(0);
        Ok((Pos(needed), &mut self.out[start..start + size]))
    }

    fn delta(from: Pos, to: Pos) -> usize {
        debug_assert!(
            to < from,
            "a uoffset points forward, so its target is written before it and by this builder"
        );
        from.0.saturating_sub(to.0)
    }
}

macro_rules! push_scalar {
    ($name:ident, $ty:ty, $what:literal) => {
        #[doc = concat!("Writes a little-endian `", $what, "`, aligned to its own size.")]
        pub fn $name(&mut self, value: $ty) -> Result<Pos, EncodeError> {
            let (position, bytes) = self.reserve(size_of::<$ty>(), size_of::<$ty>())?;
            bytes.copy_from_slice(&value.to_le_bytes());
            Ok(position)
        }
    };
}

impl<'a> Builder<'a> {
    push_scalar!(push_u8, u8, "u8");
    push_scalar!(push_i8, i8, "i8");
    push_scalar!(push_u16, u16, "u16");
    push_scalar!(push_i16, i16, "i16");
    push_scalar!(push_u32, u32, "u32");
    push_scalar!(push_i32, i32, "i32");
    push_scalar!(push_u64, u64, "u64");
    push_scalar!(push_i64, i64, "i64");
    push_scalar!(push_f32, f32, "f32");
    push_scalar!(push_f64, f64, "f64");

    /// Writes a `uoffset_t` naming `target`.
    pub fn push_offset(&mut self, target: Pos) -> Result<Pos, EncodeError> {
        let (position, bytes) = self.reserve(OFFSET_SIZE, OFFSET_SIZE)?;
        let delta = Builder::delta(position, target) as u32;
        bytes.copy_from_slice(&delta.to_le_bytes());
        Ok(position)
    }

    /// Writes a string: its length, its bytes, and the terminating zero a
    /// FlatBuffers string carries so that a reader can hand it to C.
    pub fn push_string(&mut self, value: &str) -> Result<Pos, EncodeError> {
        let len = value.len();
        let size = OFFSET_SIZE + len + 1;
        let (position, bytes) = self.reserve(size, OFFSET_SIZE)?;
        bytes[..OFFSET_SIZE].copy_from_slice(&(len as u32).to_le_bytes());
        bytes[OFFSET_SIZE..OFFSET_SIZE + len].copy_from_slice(value.as_bytes());
        // The terminator is already zero: `reserve` zeroes what it hands back.
        Ok(position)
    }

    /// Writes a vector of scalars that the caller has already encoded, each
    /// `stride` bytes, in element order.
    pub fn push_vector(&mut self, elements: &[u8], stride: usize) -> Result<Pos, EncodeError> {
        debug_assert!(stride > 0, "an element occupies at least one byte");
        debug_assert!(
            elements.len() % stride == 0,
            "the element bytes are a whole number of elements"
        );
        let count = elements.len() / stride;
        let size = OFFSET_SIZE + elements.len();
        let align = if stride > OFFSET_SIZE {
            stride
        } else {
            OFFSET_SIZE
        };
        let (position, bytes) = self.reserve(size, align)?;
        bytes[..OFFSET_SIZE].copy_from_slice(&(count as u32).to_le_bytes());
        bytes[OFFSET_SIZE..].copy_from_slice(elements);
        Ok(position)
    }

    /// Writes a table and the vtable that describes it.
    ///
    /// `size`, `align`, `slots` and each field's `offset` are the projection's
    /// facts about the type, not this builder's: it writes the layout it is
    /// given. `slots` is the number of vtable entries, which is the number of
    /// fields the type declares, whether or not each one is present here.
    ///
    /// A field the caller leaves out of `fields` is absent from the buffer,
    /// which is how a FlatBuffers table carries a field equal to its default.
    pub fn push_table(
        &mut self,
        size: usize,
        align: usize,
        slots: u16,
        fields: &[TableField],
    ) -> Result<Pos, EncodeError> {
        debug_assert!(
            size >= OFFSET_SIZE,
            "a table begins with the offset to its vtable"
        );
        let table = {
            let (table, bytes) = self.reserve(size, align)?;
            for f in fields {
                let from = usize::from(f.offset);
                let width = f.value.size();
                debug_assert!(
                    from >= OFFSET_SIZE && from + width <= size,
                    "a field lies inside the table and after the vtable offset"
                );
                let slot = &mut bytes[from..from + width];
                match f.value {
                    Field::Bool(v) => slot.copy_from_slice(&u8::from(v).to_le_bytes()),
                    Field::U8(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::I8(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::U16(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::I16(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::U32(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::I32(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::U64(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::I64(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::F32(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::F64(v) => slot.copy_from_slice(&v.to_le_bytes()),
                    Field::Offset(target) => {
                        // The field's own position: the table starts at
                        // `table`, and a field `offset` bytes further along is
                        // that many bytes closer to the end of the buffer.
                        let here = Pos(table.0 - from);
                        let delta = Builder::delta(here, target) as u32;
                        slot.copy_from_slice(&delta.to_le_bytes());
                    }
                }
            }
            table
        };

        let vtable_bytes = VTABLE_HEADER + usize::from(slots) * VOFFSET_SIZE;
        let vtable = {
            let (vtable, bytes) = self.reserve(vtable_bytes, VOFFSET_SIZE)?;
            bytes[..VOFFSET_SIZE].copy_from_slice(&(vtable_bytes as u16).to_le_bytes());
            bytes[VOFFSET_SIZE..VTABLE_HEADER].copy_from_slice(&(size as u16).to_le_bytes());
            for f in fields {
                debug_assert!(f.slot < slots, "a field's slot is one the vtable carries");
                let entry = VTABLE_HEADER + usize::from(f.slot) * VOFFSET_SIZE;
                bytes[entry..entry + VOFFSET_SIZE].copy_from_slice(&f.offset.to_le_bytes());
            }
            vtable
        };

        // The table names its vtable with a signed offset backwards. The
        // vtable was written after the table, so it lies at a lower address
        // and the offset is positive.
        let soffset = (vtable.0 - table.0) as i32;
        let start = self.out.len() - table.0;
        self.out[start..start + OFFSET_SIZE].copy_from_slice(&soffset.to_le_bytes());
        Ok(table)
    }

    /// Writes the root offset and returns the finished buffer: a subslice of
    /// the caller's slice ending at its end.
    ///
    /// `align` is the buffer's alignment, the largest any object in it needs.
    /// The finished buffer's length is a multiple of it, which is what makes
    /// every object's own alignment hold once the root sits at offset zero.
    pub fn finish(mut self, root: Pos, align: usize) -> Result<&'a [u8], EncodeError> {
        let align = if align > OFFSET_SIZE {
            align
        } else {
            OFFSET_SIZE
        };
        let position = {
            let (position, bytes) = self.reserve(OFFSET_SIZE, align)?;
            let delta = Builder::delta(position, root) as u32;
            bytes.copy_from_slice(&delta.to_le_bytes());
            position
        };
        let out: &'a [u8] = self.out;
        Ok(&out[out.len() - position.0..])
    }
}
