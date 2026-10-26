//! The podbuf layout law: where every byte of a type's head goes, computed
//! once over the IR for every emitter and check that reads the layout.
//!
//! A podbuf buffer is `[preamble][head][tail]`. The head of a struct is one
//! `repr(C, align(8))` struct whose fields are placed by the first-fit law
//! ([`place`]): each field in declaration order at the lowest offset where it
//! fits at its natural alignment. Every variable-length field — a bounded
//! `string`, `bytes` or array, a map, the active arm of a union — is an 8-byte
//! descriptor in the head at alignment 4, and its data is in the tail. A
//! nested struct, a tuple, an exact-length array and exact-length `bytes` are
//! inline in the head.
//!
//! The Rust codec, the C header, the sizer, `ridl diff` and the padding lint
//! all read the layout from here, so the one place that states where a field
//! goes is this module. Nothing here spells a Rust or C name.
//!
//! The facts:
//!
//! - **the head of a struct, a tuple and a union** ([`struct_head`],
//!   [`tuple_head`], [`union_head`]): every placement with its offset, the
//!   flags fields of the optional fields, and the pad runs;
//! - **the form of a field** ([`field_form`]) and **the size of one element**
//!   ([`elem_size`]), the stride an array or map descriptor records;
//! - **the width of an enum** ([`enum_width`]);
//! - **the size bound** ([`max_size`]): the largest buffer any legal value of
//!   a type encodes to.

use super::flatbuffers::{Packages, ProjectionError};
use crate::v2;

/// The preamble: `{ head_size: u32, reserved: u32 }`, before every head.
pub const PREAMBLE_SIZE: u32 = 8;
/// The alignment of every head, at the buffer start and in the tail.
pub const HEAD_ALIGN: u32 = 8;
/// A descriptor: `{ off: u32, len: u32 }` or `{ off: u32, len: u16, elem_size: u16 }`.
pub const DESCRIPTOR_SIZE: u32 = 8;
/// A descriptor has the alignment of its `u32` words.
pub const DESCRIPTOR_ALIGN: u32 = 4;
/// The largest element or entry count an array or map descriptor can record:
/// its `len` is a `u16`.
pub const MAX_COUNT: u64 = 65_535;
/// The largest element or entry size an array or map descriptor can record:
/// its `elem_size` is a `u16`.
pub const MAX_ELEM_SIZE: u64 = 65_535;

/// One head: a struct's, a tuple's, a union's, a map entry's, or the one-field
/// head of a union arm whose type is a scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The head size, a multiple of [`HEAD_ALIGN`]; 0 for a head with no
    /// field.
    pub size: u32,
    /// Every placement, the flags fields included, sorted by offset.
    pub fields: Vec<Placement>,
    /// The unused byte runs, the `_padN` fields of the generated struct, `N`
    /// the offset. Sorted by offset.
    pub pads: Vec<Pad>,
}

/// One field of a head and where the law put it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub source: Source,
    pub offset: u32,
    pub size: u32,
    pub align: u32,
    pub form: Form,
    /// The flag bit of an optional field; `None` for a field that is always
    /// present.
    pub optional: Option<FlagBit>,
}

/// What a placement holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A struct field, by its typl name and 1-based ordinal.
    Field { name: String, ordinal: u32 },
    /// Flags field `index`: a `u8` whose bits record the presence of optional
    /// fields `8 * index + 1` to `8 * index + 8`, counting optional fields
    /// from 1 in declaration order.
    Flags { index: u32 },
    /// A union's tag: the ordinal of the active arm.
    Tag,
    /// In a union head, the descriptor to the active arm. In the head of an
    /// arm whose type is a named scalar, an enum or an enum set, the one
    /// value field.
    Arm,
    /// A tuple field, by its 1-based position.
    Position(u32),
    /// A map entry's key.
    Key,
    /// A map entry's value.
    Value,
}

/// The bit of a flags field that records an optional field's presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagBit {
    /// The index of the flags field ([`Source::Flags`]).
    pub flags: u32,
    /// The bit, 0 the least significant.
    pub bit: u8,
}

/// The bytes a field takes in a head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    /// A `u8`, 0 or 1.
    Bool,
    Int {
        width: v2::IntWidth,
        signed: bool,
    },
    Float(v2::FloatWidth),
    /// An enum at the width and signedness [`enum_width`] gives it.
    /// `reference` is the name the field wrote.
    Enum {
        width: v2::IntWidth,
        signed: bool,
        reference: String,
    },
    /// An enum set at the width typl infers.
    EnumSet {
        width: v2::IntWidth,
        reference: String,
    },
    /// A nested struct, a tuple, or a union (its tag and arm descriptor),
    /// inline.
    Inline(Box<Head>),
    /// An exact-length array `[T; N]` or exact-length `bytes`, inline with
    /// no length.
    InlineArray {
        element: Box<Form>,
        count: u32,
    },
    /// A descriptor to data in the tail.
    Descriptor(DescriptorKind),
}

/// What a descriptor points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorKind {
    /// A bounded `string`: `min` to `max` scalar values; the tail holds the
    /// UTF-8 bytes and one NUL that `len` does not count.
    Text { min: u64, max: u64 },
    /// Bounded `bytes` whose bounds differ: `min` to `max` bytes.
    Bytes { min: u64, max: u64 },
    /// A bounded array: `min` to `max` elements at stride `elem_size`.
    Array {
        element: Box<Form>,
        elem_size: u32,
        min: u64,
        max: u64,
    },
    /// A map: `min` to `max` entries at stride `elem_size`, each entry the
    /// head `{ key, value }`, sorted by key.
    Map {
        entry: Box<Head>,
        elem_size: u32,
        min: u64,
        max: u64,
    },
    /// The active arm of a union: one head per declared arm, with its
    /// ordinal, in declaration order.
    Arm { arms: Vec<(u32, Head)> },
}

/// An unused byte run of a head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pad {
    pub offset: u32,
    pub size: u32,
}

/// The first-fit primitive: for each `(size, align)` in declaration order,
/// the lowest offset where it fits at `align` without overlapping an item
/// already placed. The offset of an item depends only on the items before
/// it, so an appended item never moves an existing one.
///
/// Every `align` is at least 1. The offsets are `u64` because the alignment
/// gaps can push an end past `u32::MAX` even when the sizes sum to less;
/// [`struct_head`] and its siblings refuse such a head instead of wrapping.
#[must_use]
pub fn place(items: &[(u32, u32)]) -> Vec<u64> {
    // (start, end) of each item placed.
    let mut placed: Vec<(u64, u64)> = Vec::with_capacity(items.len());
    let mut offsets = Vec::with_capacity(items.len());
    for &(size, align) in items {
        let (size, align) = (u64::from(size), u64::from(align));
        let mut off = 0u64;
        loop {
            off = off.div_ceil(align) * align;
            match placed.iter().find(|&&(s, e)| off < e && s < off + size) {
                None => break,
                Some(&(_, end)) => off = end,
            }
        }
        placed.push((off, off + size));
        offsets.push(off);
    }
    offsets
}

/// The head of a struct: its live fields in declaration order, each placed by
/// [`place`], with flags field `k` placed as a `u8` just before optional
/// field number `8k + 1`. A tombstone holds no bytes: removing a field is a
/// podbuf break. The size is the largest end rounded up to [`HEAD_ALIGN`];
/// the pads are the gaps.
///
/// Bare names inside `def` resolve in `packages.package`, the package that
/// declares `owner`; a name from another package is followed into the
/// package that declares it. An error is malformed IR: a field with no type,
/// a name that does not resolve or names no type, a string or bytes with no
/// length bound, a width the IR left unspecified, a stream, a type that
/// reaches itself, or a head larger than `u32` can offset.
pub fn struct_head(
    packages: Packages<'_>,
    owner: &str,
    def: &v2::StructDef,
) -> Result<Head, ProjectionError> {
    Layout::new(packages).struct_head(packages.package, owner, def)
}

/// The head of a tuple: its fields by position, laid out as a struct's. `name`
/// names the tuple in an error.
pub fn tuple_head(
    packages: Packages<'_>,
    name: &str,
    tuple: &v2::TupleType,
) -> Result<Head, ProjectionError> {
    Layout::new(packages).tuple_head(packages.package, name, tuple)
}

/// The head of a union: the tag at offset 0, the narrowest unsigned integer
/// that holds the largest arm ordinal, then the descriptor to the active arm
/// placed by the law, so a union head is 16 bytes. The descriptor carries one
/// head per arm: a struct's or a union's own head, or a one-field head whose
/// value is at offset 0 for an arm whose type is a named scalar, an enum or
/// an enum set.
pub fn union_head(
    packages: Packages<'_>,
    owner: &str,
    def: &v2::UnionDef,
) -> Result<Head, ProjectionError> {
    Layout::new(packages).union_head(packages.package, owner, def)
}

/// The integer of an enum: the narrowest width that holds every declared
/// value, signed when any value is negative. An enum with no value is a `u8`.
#[must_use]
pub fn enum_width(def: &v2::EnumDef) -> (v2::IntWidth, bool) {
    let min = def
        .values
        .iter()
        .map(|value| value.value)
        .min()
        .unwrap_or(0);
    let max = def
        .values
        .iter()
        .map(|value| value.value)
        .max()
        .unwrap_or(0);
    if min < 0 {
        let width = if i8::try_from(min).is_ok() && i8::try_from(max).is_ok() {
            v2::IntWidth::I8
        } else if i16::try_from(min).is_ok() && i16::try_from(max).is_ok() {
            v2::IntWidth::I16
        } else if i32::try_from(min).is_ok() && i32::try_from(max).is_ok() {
            v2::IntWidth::I32
        } else {
            v2::IntWidth::I64
        };
        (width, true)
    } else {
        let width = if u8::try_from(max).is_ok() {
            v2::IntWidth::U8
        } else if u16::try_from(max).is_ok() {
            v2::IntWidth::U16
        } else if u32::try_from(max).is_ok() {
            v2::IntWidth::U32
        } else {
            v2::IntWidth::U64
        };
        (width, false)
    }
}

/// The form of one field type, its bare names resolved in `packages.package`.
/// The field's own `optional` flag is not part of its form: the head that
/// holds the field records it as a [`FlagBit`].
pub fn field_form(packages: Packages<'_>, ty: &v2::FieldType) -> Result<Form, ProjectionError> {
    Layout::new(packages).form(packages.package, ty)
}

/// The bytes one value of `form` takes in a head, and the stride of an array
/// or map whose element it is. A head's size is a multiple of 8, so an
/// [`Form::Inline`] element's stride is too, and the element can grow by an
/// appended field; every other form is its exact size, because such an
/// element cannot grow compatibly.
///
/// An inline array larger than `u32::MAX` bytes saturates; no head can hold
/// one, and [`struct_head`] refuses it.
#[must_use]
pub fn elem_size(form: &Form) -> u32 {
    match form {
        Form::Bool => 1,
        Form::Int { width, .. } | Form::Enum { width, .. } | Form::EnumSet { width, .. } => {
            int_bytes(*width)
        }
        Form::Float(width) => float_bytes(*width),
        Form::Inline(head) => head.size,
        Form::InlineArray { element, count } => elem_size(element).saturating_mul(*count),
        Form::Descriptor(_) => DESCRIPTOR_SIZE,
    }
}

/// The largest podbuf buffer any legal value of `decl` encodes to: the
/// preamble, the head, and the largest tail. `None` when the sum exceeds
/// `u32::MAX`, which a `u32` offset cannot address; when an array or map
/// declares more than [`MAX_COUNT`] elements or an element larger than
/// [`MAX_ELEM_SIZE`], which its descriptor cannot record; when the layout
/// itself is an error; and for a declaration that is not a type.
///
/// The tail is charged at its worst case, every optional field present and
/// every bound at its maximum, in the order the encoder writes it: the
/// `fields` of each head in turn, depth first, so the tail of an array
/// element follows the array's block. Tail data is placed at the alignment of
/// its element type and the encoder zero-fills the gap before it, so the gap
/// is charged too: the bound tracks the alignment the tail cursor is
/// guaranteed after each block (8 where the tail starts, 1 after a string),
/// and charges `align - guaranteed` before a block that needs more. The
/// elements of one array each pay the tail of the element at the worst
/// alignment, 1.
///
/// A named scalar, an enum or an enum set as a root is the one-field head
/// a union arm of that type gets.
#[must_use]
pub fn max_size(packages: Packages<'_>, decl: &v2::Decl) -> Option<u64> {
    let mut layout = Layout::new(packages);
    let home = packages.package;
    let head = match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => layout.struct_head(home, &decl.name, def).ok()?,
        Some(v2::decl::Kind::UnionDef(def)) => layout.union_head(home, &decl.name, def).ok()?,
        Some(v2::decl::Kind::TypeDef(_))
        | Some(v2::decl::Kind::EnumDef(_))
        | Some(v2::decl::Kind::EnumSetDef(_)) => {
            boxed_head(layout.named_form(home, &decl.name).ok()?).ok()?
        }
        _ => return None,
    };
    head_bound(&head)
}

/// The largest podbuf buffer any legal value of `tuple` encodes to, as
/// [`max_size`] bounds a struct: the induced argument or reply struct of an
/// interaction. `None` for the same causes.
#[must_use]
pub fn tuple_max_size(packages: Packages<'_>, tuple: &v2::TupleType) -> Option<u64> {
    let head = Layout::new(packages)
        .tuple_head(packages.package, "a tuple", tuple)
        .ok()?;
    head_bound(&head)
}

/// The preamble, the head and the largest tail of `head`, or `None` above
/// `u32::MAX` or past a descriptor limit.
fn head_bound(head: &Head) -> Option<u64> {
    let (tail, _) = head_tail(head, HEAD_ALIGN)?;
    let bound = u64::from(PREAMBLE_SIZE)
        .checked_add(u64::from(head.size))?
        .checked_add(tail)?;
    (bound <= u64::from(u32::MAX)).then_some(bound)
}

/// Whether an array or a map anywhere in `head` declares more than
/// [`MAX_COUNT`] elements or an element or entry larger than
/// [`MAX_ELEM_SIZE`]. This is the one cause, besides a sum above `u32::MAX`,
/// for which [`max_size`] answers `None` on a head that was laid out, so a
/// caller can tell the two apart.
#[must_use]
pub fn exceeds_descriptor_limits(head: &Head) -> bool {
    head.fields
        .iter()
        .any(|placement| form_exceeds_descriptor_limits(&placement.form))
}

/// [`exceeds_descriptor_limits`] for one form.
#[must_use]
pub fn form_exceeds_descriptor_limits(form: &Form) -> bool {
    match form {
        Form::Bool
        | Form::Int { .. }
        | Form::Float(_)
        | Form::Enum { .. }
        | Form::EnumSet { .. } => false,
        Form::Inline(head) => exceeds_descriptor_limits(head),
        Form::InlineArray { element, .. } => form_exceeds_descriptor_limits(element),
        Form::Descriptor(kind) => match kind {
            DescriptorKind::Text { .. } | DescriptorKind::Bytes { .. } => false,
            DescriptorKind::Array {
                element,
                elem_size,
                max,
                ..
            } => {
                *max > MAX_COUNT
                    || u64::from(*elem_size) > MAX_ELEM_SIZE
                    || form_exceeds_descriptor_limits(element)
            }
            DescriptorKind::Map {
                entry,
                elem_size,
                max,
                ..
            } => {
                *max > MAX_COUNT
                    || u64::from(*elem_size) > MAX_ELEM_SIZE
                    || exceeds_descriptor_limits(entry)
            }
            DescriptorKind::Arm { arms } => {
                arms.iter().any(|(_, head)| exceeds_descriptor_limits(head))
            }
        },
    }
}

// --- the walk --------------------------------------------------------------

fn error(message: String) -> ProjectionError {
    ProjectionError { message }
}

/// The walk over the IR, carrying the packages a reference resolves in and
/// the named composites on the stack, so a type that reaches itself is an
/// error instead of a stack overflow.
struct Layout<'a> {
    packages: Packages<'a>,
    visiting: Vec<String>,
}

/// One field before placement.
struct Item {
    source: Source,
    form: Form,
    optional: Option<FlagBit>,
}

/// The members of one head as they are collected, with the flags fields the
/// optional fields need inserted where the law places them.
#[derive(Default)]
struct Members {
    items: Vec<Item>,
    /// How many optional fields have been collected.
    optionals: u32,
}

impl Members {
    fn push(&mut self, source: Source, form: Form, optional: bool) {
        let optional = optional.then(|| {
            // This is optional field number `n + 1`; flags field `k` is placed
            // just before optional field `8k + 1`.
            let n = self.optionals;
            if n.is_multiple_of(8) {
                self.items.push(Item {
                    source: Source::Flags { index: n / 8 },
                    form: Form::Int {
                        width: v2::IntWidth::U8,
                        signed: false,
                    },
                    optional: None,
                });
            }
            self.optionals = n + 1;
            FlagBit {
                flags: n / 8,
                bit: (n % 8) as u8,
            }
        });
        self.items.push(Item {
            source,
            form,
            optional,
        });
    }
}

/// The one-field head of a scalar, an enum or an enum set: the value at
/// offset 0, the size rounded to 8.
fn boxed_head(form: Form) -> Result<Head, ProjectionError> {
    lay_out(vec![Item {
        source: Source::Arm,
        form,
        optional: None,
    }])
}

/// Rules 2 to 4 of the law over collected items: place them, sort by offset,
/// round the size up to 8, and list the gaps as pads.
fn lay_out(items: Vec<Item>) -> Result<Head, ProjectionError> {
    let sizes: Vec<(u32, u32)> = items
        .iter()
        .map(|item| (elem_size(&item.form), align_of(&item.form)))
        .collect();
    let offsets = place(&sizes);
    // Ends and the rounded size in u64: a head a u32 offset cannot address
    // is refused here, so every offset below fits a u32.
    let end = offsets
        .iter()
        .zip(&sizes)
        .map(|(offset, &(size, _))| offset + u64::from(size))
        .max()
        .unwrap_or(0);
    let size = end.div_ceil(u64::from(HEAD_ALIGN)) * u64::from(HEAD_ALIGN);
    let size = u32::try_from(size).map_err(|_| {
        error(format!(
            "a head of {size} bytes is larger than a u32 offset can address"
        ))
    })?;
    let mut fields: Vec<Placement> = items
        .into_iter()
        .zip(sizes)
        .zip(offsets)
        .map(|((item, (size, align)), offset)| {
            // Every offset is below the head size, which is a u32.
            let offset = u32::try_from(offset)
                .map_err(|_| error(format!("offset {offset} does not fit a u32")))?;
            Ok(Placement {
                source: item.source,
                offset,
                size,
                align,
                form: item.form,
                optional: item.optional,
            })
        })
        .collect::<Result<_, ProjectionError>>()?;
    fields.sort_by_key(|placement| placement.offset);
    let mut pads = Vec::new();
    let mut cursor = 0u32;
    for placement in &fields {
        if placement.offset > cursor {
            pads.push(Pad {
                offset: cursor,
                size: placement.offset - cursor,
            });
        }
        cursor = cursor.max(placement.offset + placement.size);
    }
    if size > cursor {
        pads.push(Pad {
            offset: cursor,
            size: size - cursor,
        });
    }
    Ok(Head { size, fields, pads })
}

impl<'a> Layout<'a> {
    fn new(packages: Packages<'a>) -> Self {
        Self {
            packages,
            visiting: Vec::new(),
        }
    }

    /// `body` over the named composite `key`, an error when `key` is already
    /// on the stack.
    fn guarded(
        &mut self,
        key: String,
        body: impl FnOnce(&mut Self) -> Result<Head, ProjectionError>,
    ) -> Result<Head, ProjectionError> {
        if self.visiting.contains(&key) {
            return Err(error(format!("{key} reaches itself, so it has no layout")));
        }
        self.visiting.push(key);
        let out = body(self);
        self.visiting.pop();
        out
    }

    fn resolve(
        &self,
        home: &'a v2::Package,
        reference: &str,
    ) -> Result<(&'a v2::Decl, &'a v2::Package), ProjectionError> {
        self.packages
            .resolve(home, reference)
            .ok_or_else(|| error(format!("{reference} does not resolve")))
    }

    fn struct_head(
        &mut self,
        home: &'a v2::Package,
        owner: &str,
        def: &v2::StructDef,
    ) -> Result<Head, ProjectionError> {
        self.guarded(format!("{}.{owner}", home.name), |layout| {
            let mut members = Members::default();
            for member in &def.members {
                match &member.member {
                    Some(v2::struct_member::Member::Field(field)) => {
                        let ty = field
                            .r#type
                            .as_ref()
                            .ok_or_else(|| error(format!("{owner}.{} has no type", field.name)))?;
                        let form = layout.form(home, ty)?;
                        members.push(
                            Source::Field {
                                name: field.name.clone(),
                                ordinal: field.ordinal,
                            },
                            form,
                            ty.optional,
                        );
                    }
                    // A tombstone holds no bytes: the type of the removed
                    // field is gone from the source, so its bytes cannot be
                    // kept, which is why the removal is a break.
                    Some(v2::struct_member::Member::Reserved(_)) | None => {}
                }
            }
            lay_out(members.items)
        })
    }

    fn tuple_head(
        &mut self,
        home: &'a v2::Package,
        name: &str,
        tuple: &v2::TupleType,
    ) -> Result<Head, ProjectionError> {
        let mut members = Members::default();
        for (index, field) in tuple.fields.iter().enumerate() {
            let position = u32::try_from(index + 1)
                .map_err(|_| error(format!("{name} has more fields than u32 positions")))?;
            let ty = field
                .r#type
                .as_ref()
                .ok_or_else(|| error(format!("{name}.{} has no type", field.name)))?;
            let form = self.form(home, ty)?;
            members.push(Source::Position(position), form, ty.optional);
        }
        lay_out(members.items)
    }

    fn union_head(
        &mut self,
        home: &'a v2::Package,
        owner: &str,
        def: &v2::UnionDef,
    ) -> Result<Head, ProjectionError> {
        self.guarded(format!("{}.{owner}", home.name), |layout| {
            let mut arms = Vec::with_capacity(def.arms.len());
            for arm in &def.arms {
                arms.push((arm.ordinal, layout.arm_head(home, arm)?));
            }
            let largest = def.arms.iter().map(|arm| arm.ordinal).max().unwrap_or(0);
            let width = if u8::try_from(largest).is_ok() {
                v2::IntWidth::U8
            } else if u16::try_from(largest).is_ok() {
                v2::IntWidth::U16
            } else {
                v2::IntWidth::U32
            };
            lay_out(vec![
                Item {
                    source: Source::Tag,
                    form: Form::Int {
                        width,
                        signed: false,
                    },
                    optional: None,
                },
                Item {
                    source: Source::Arm,
                    form: Form::Descriptor(DescriptorKind::Arm { arms }),
                    optional: None,
                },
            ])
        })
    }

    fn arm_head(
        &mut self,
        home: &'a v2::Package,
        arm: &v2::UnionArm,
    ) -> Result<Head, ProjectionError> {
        let (decl, declaring) = self.resolve(home, &arm.type_ref)?;
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => self.struct_head(declaring, &decl.name, def),
            Some(v2::decl::Kind::UnionDef(def)) => self.union_head(declaring, &decl.name, def),
            Some(v2::decl::Kind::TypeDef(_))
            | Some(v2::decl::Kind::EnumDef(_))
            | Some(v2::decl::Kind::EnumSetDef(_)) => {
                boxed_head(self.named_form(home, &arm.type_ref)?)
            }
            _ => Err(error(format!(
                "arm {} names {}, which is not a type",
                arm.name, arm.type_ref
            ))),
        }
    }

    fn form(&mut self, home: &'a v2::Package, ty: &v2::FieldType) -> Result<Form, ProjectionError> {
        match ty.kind.as_ref() {
            None => Err(error("a field with no type has no form".to_string())),
            Some(v2::field_type::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive) {
                    Ok(v2::PrimitiveType::Boolean) => Ok(Form::Bool),
                    // A bare `integer` or `float` carries no derived width, so
                    // the whole typl domain is on the wire (typl §4).
                    Ok(v2::PrimitiveType::Integer) => Ok(Form::Int {
                        width: v2::IntWidth::I64,
                        signed: true,
                    }),
                    Ok(v2::PrimitiveType::Float) => Ok(Form::Float(v2::FloatWidth::F64)),
                    // A bare `string` or `bytes` carries no length bound; the
                    // compiler never hands one over (typl §15.3, TYPL-208).
                    _ => Err(error(
                        "a bare string or bytes, or an unspecified primitive, has no form"
                            .to_string(),
                    )),
                }
            }
            Some(v2::field_type::Kind::InlineScalar(def)) => scalar_form(def),
            Some(v2::field_type::Kind::Named(reference)) => self.named_form(home, reference),
            Some(v2::field_type::Kind::Tuple(tuple)) => Ok(Form::Inline(Box::new(
                self.tuple_head(home, "a tuple", tuple)?,
            ))),
            Some(v2::field_type::Kind::Array(array)) => {
                let element_ty = array.element.as_ref().ok_or_else(|| {
                    error("an array with no element type has no form".to_string())
                })?;
                let element = self.form(home, element_ty)?;
                if array.min == array.max {
                    let count = u32::try_from(array.max).map_err(|_| {
                        error(format!(
                            "an array of {} elements is larger than a head",
                            array.max
                        ))
                    })?;
                    Ok(Form::InlineArray {
                        element: Box::new(element),
                        count,
                    })
                } else {
                    let elem_size = elem_size(&element);
                    Ok(Form::Descriptor(DescriptorKind::Array {
                        element: Box::new(element),
                        elem_size,
                        min: array.min,
                        max: array.max,
                    }))
                }
            }
            Some(v2::field_type::Kind::Map(map)) => {
                let key_ty = map
                    .key
                    .as_ref()
                    .ok_or_else(|| error("a map with no key type has no form".to_string()))?;
                let value_ty = map
                    .value
                    .as_ref()
                    .ok_or_else(|| error("a map with no value type has no form".to_string()))?;
                let mut members = Members::default();
                let key = self.form(home, key_ty)?;
                members.push(Source::Key, key, key_ty.optional);
                let value = self.form(home, value_ty)?;
                members.push(Source::Value, value, value_ty.optional);
                let entry = lay_out(members.items)?;
                let elem_size = entry.size;
                Ok(Form::Descriptor(DescriptorKind::Map {
                    entry: Box::new(entry),
                    elem_size,
                    min: map.min,
                    max: map.max,
                }))
            }
            Some(v2::field_type::Kind::Stream(_)) => Err(error("a stream has no form".to_string())),
        }
    }

    fn named_form(
        &mut self,
        home: &'a v2::Package,
        reference: &str,
    ) -> Result<Form, ProjectionError> {
        let (decl, declaring) = self.resolve(home, reference)?;
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(def)) => scalar_form(def),
            Some(v2::decl::Kind::StructDef(def)) => Ok(Form::Inline(Box::new(
                self.struct_head(declaring, &decl.name, def)?,
            ))),
            Some(v2::decl::Kind::UnionDef(def)) => Ok(Form::Inline(Box::new(
                self.union_head(declaring, &decl.name, def)?,
            ))),
            Some(v2::decl::Kind::EnumDef(def)) => {
                let (width, signed) = enum_width(def);
                Ok(Form::Enum {
                    width,
                    signed,
                    reference: reference.to_string(),
                })
            }
            Some(v2::decl::Kind::EnumSetDef(def)) => Ok(Form::EnumSet {
                width: int_width(def.width)
                    .ok_or_else(|| error(format!("{reference} has no width")))?,
                reference: reference.to_string(),
            }),
            _ => Err(error(format!("{reference} is not a type"))),
        }
    }
}

/// A named or inline scalar at its declared width; a string is a descriptor,
/// and bytes are a descriptor unless the bounds are equal, which is an inline
/// `u8[N]`.
fn scalar_form(def: &v2::TypeDef) -> Result<Form, ProjectionError> {
    match &def.width {
        Some(v2::type_def::Width::IntWidth(width)) => {
            let width = int_width(*width)
                .ok_or_else(|| error("an integer with no width has no form".to_string()))?;
            Ok(Form::Int {
                width,
                signed: matches!(
                    width,
                    v2::IntWidth::I8 | v2::IntWidth::I16 | v2::IntWidth::I32 | v2::IntWidth::I64
                ),
            })
        }
        Some(v2::type_def::Width::FloatWidth(width)) => match v2::FloatWidth::try_from(*width) {
            Ok(width @ (v2::FloatWidth::F32 | v2::FloatWidth::F64)) => Ok(Form::Float(width)),
            _ => Err(error("a float with no width has no form".to_string())),
        },
        None => {
            let primitive = match def
                .backing
                .as_ref()
                .and_then(|backing| backing.kind.as_ref())
            {
                Some(v2::backing::Kind::Primitive(primitive)) => {
                    v2::PrimitiveType::try_from(*primitive).ok()
                }
                _ => None,
            };
            match primitive {
                Some(v2::PrimitiveType::Boolean) => Ok(Form::Bool),
                Some(v2::PrimitiveType::String) => {
                    let (min, max) = length_bounds(def, "string")?;
                    Ok(Form::Descriptor(DescriptorKind::Text { min, max }))
                }
                Some(v2::PrimitiveType::Bytes) => {
                    let (min, max) = length_bounds(def, "bytes")?;
                    if min == max {
                        let count = u32::try_from(max).map_err(|_| {
                            error(format!("bytes of {max} bytes are larger than a head"))
                        })?;
                        Ok(Form::InlineArray {
                            element: Box::new(Form::Int {
                                width: v2::IntWidth::U8,
                                signed: false,
                            }),
                            count,
                        })
                    } else {
                        Ok(Form::Descriptor(DescriptorKind::Bytes { min, max }))
                    }
                }
                _ => Err(error("a scalar with no width has no form".to_string())),
            }
        }
    }
}

/// The length bounds of a string or bytes scalar. The compiler always writes
/// `len_max` (typl §4.4–§4.5 default it to `[0..256]`), so a missing one is
/// IR handed in directly.
fn length_bounds(def: &v2::TypeDef, what: &str) -> Result<(u64, u64), ProjectionError> {
    let constraint = def.constraint.as_ref();
    let max = constraint
        .and_then(|constraint| constraint.len_max)
        .ok_or_else(|| error(format!("a {what} with no length bound has no form")))?;
    let min = constraint
        .and_then(|constraint| constraint.len_min)
        .unwrap_or(0);
    Ok((min, max))
}

fn int_width(width: i32) -> Option<v2::IntWidth> {
    match v2::IntWidth::try_from(width) {
        Ok(v2::IntWidth::Unspecified) | Err(_) => None,
        Ok(width) => Some(width),
    }
}

/// The bytes of an integer width. [`field_form`] never builds an unspecified
/// width; one built by hand is given the widest, so the arithmetic over it
/// stays valid.
fn int_bytes(width: v2::IntWidth) -> u32 {
    match width {
        v2::IntWidth::U8 | v2::IntWidth::I8 => 1,
        v2::IntWidth::U16 | v2::IntWidth::I16 => 2,
        v2::IntWidth::U32 | v2::IntWidth::I32 => 4,
        v2::IntWidth::U64 | v2::IntWidth::I64 | v2::IntWidth::Unspecified => 8,
    }
}

fn float_bytes(width: v2::FloatWidth) -> u32 {
    match width {
        v2::FloatWidth::F32 => 4,
        v2::FloatWidth::F64 | v2::FloatWidth::Unspecified => 8,
    }
}

/// The natural alignment of one value of `form` in a head.
fn align_of(form: &Form) -> u32 {
    match form {
        Form::Bool
        | Form::Int { .. }
        | Form::Enum { .. }
        | Form::EnumSet { .. }
        | Form::Float(_) => elem_size(form),
        Form::Inline(_) => HEAD_ALIGN,
        Form::InlineArray { element, .. } => align_of(element),
        Form::Descriptor(_) => DESCRIPTOR_ALIGN,
    }
}

// --- the tail bound --------------------------------------------------------
//
// Each function takes the alignment the tail cursor is guaranteed to have (a
// power of two from 1 to 8) and answers the bytes the tail takes and the
// alignment guaranteed afterwards. Every step is monotone in that alignment,
// so an answer computed from a coarser guarantee is sound for a finer one.

/// The tail of every field of `head`, in `fields` order, depth first.
fn head_tail(head: &Head, aligned: u32) -> Option<(u64, u32)> {
    let mut bytes = 0u64;
    let mut aligned = aligned;
    for placement in &head.fields {
        let (more, after) = form_tail(&placement.form, aligned)?;
        bytes = bytes.checked_add(more)?;
        aligned = after;
    }
    Some((bytes, aligned))
}

fn form_tail(form: &Form, aligned: u32) -> Option<(u64, u32)> {
    match form {
        Form::Bool
        | Form::Int { .. }
        | Form::Float(_)
        | Form::Enum { .. }
        | Form::EnumSet { .. } => Some((0, aligned)),
        Form::Inline(head) => head_tail(head, aligned),
        Form::InlineArray { element, count } => repeated(u64::from(*count), aligned, |aligned| {
            form_tail(element, aligned)
        }),
        Form::Descriptor(kind) => match kind {
            // The UTF-8 bytes, up to four per scalar value, and one NUL; an
            // empty string has no NUL.
            DescriptorKind::Text { max, .. } => {
                let bytes = if *max == 0 {
                    0
                } else {
                    max.checked_mul(4)?.checked_add(1)?
                };
                block(aligned, 1, bytes, 1)
            }
            DescriptorKind::Bytes { max, .. } => block(aligned, 1, *max, 1),
            DescriptorKind::Array {
                element,
                elem_size,
                max,
                ..
            } => {
                if *max > MAX_COUNT || u64::from(*elem_size) > MAX_ELEM_SIZE {
                    return None;
                }
                let (bytes, aligned) = block(
                    aligned,
                    align_of(element),
                    max.checked_mul(u64::from(*elem_size))?,
                    *elem_size,
                )?;
                let (more, aligned) =
                    repeated(*max, aligned, |aligned| form_tail(element, aligned))?;
                Some((bytes.checked_add(more)?, aligned))
            }
            DescriptorKind::Map {
                entry,
                elem_size,
                max,
                ..
            } => {
                if *max > MAX_COUNT || u64::from(*elem_size) > MAX_ELEM_SIZE {
                    return None;
                }
                let (bytes, aligned) = block(
                    aligned,
                    HEAD_ALIGN,
                    max.checked_mul(u64::from(*elem_size))?,
                    *elem_size,
                )?;
                let (more, aligned) = repeated(*max, aligned, |aligned| head_tail(entry, aligned))?;
                Some((bytes.checked_add(more)?, aligned))
            }
            // The largest arm, head and tail; the alignment afterwards is what
            // every arm guarantees.
            DescriptorKind::Arm { arms } => {
                if arms.is_empty() {
                    return Some((0, aligned));
                }
                let mut worst = 0u64;
                let mut after = 0u32;
                for (_, head) in arms {
                    let (bytes, arm_aligned) =
                        block(aligned, HEAD_ALIGN, u64::from(head.size), head.size)?;
                    let (more, arm_aligned) = head_tail(head, arm_aligned)?;
                    worst = worst.max(bytes.checked_add(more)?);
                    after = gcd(after, arm_aligned);
                }
                Some((worst, after))
            }
        },
    }
}

/// One contiguous block of `bytes` placed at `align` when the cursor is a
/// multiple of `aligned`, every size the block can have a multiple of
/// `granularity`: the bytes with the worst-case gap before them, and the
/// alignment guaranteed after the block. An empty block moves nothing.
fn block(aligned: u32, align: u32, bytes: u64, granularity: u32) -> Option<(u64, u32)> {
    if bytes == 0 {
        return Some((0, aligned));
    }
    let gap = align.saturating_sub(aligned);
    let at = align.max(aligned);
    Some((u64::from(gap).checked_add(bytes)?, gcd(at, granularity)))
}

/// The tails of `count` elements, one after the other. The first follows the
/// cursor at `aligned`; each later one follows the tail before it, so every
/// element is charged at the worst alignment, 1, which costs the most and
/// guarantees afterwards what any alignment would.
fn repeated(
    count: u64,
    aligned: u32,
    tail: impl Fn(u32) -> Option<(u64, u32)>,
) -> Option<(u64, u32)> {
    if count == 0 {
        return Some((0, aligned));
    }
    let (bytes, after) = tail(1)?;
    if bytes == 0 {
        return Some((0, aligned));
    }
    Some((bytes.checked_mul(count)?, after))
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- IR fixtures --------------------------------------------------

    fn field(name: &str, ordinal: u32, ty: v2::FieldType) -> v2::StructMember {
        v2::StructMember {
            member: Some(v2::struct_member::Member::Field(Box::new(v2::Field {
                name: name.to_string(),
                ordinal,
                r#type: Some(ty),
                ..Default::default()
            }))),
        }
    }

    fn retired(ordinal: u32) -> v2::StructMember {
        v2::StructMember {
            member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
                ordinal,
                ..Default::default()
            })),
        }
    }

    fn optional(mut ty: v2::FieldType) -> v2::FieldType {
        ty.optional = true;
        ty
    }

    fn scalar(def: v2::TypeDef) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::InlineScalar(Box::new(def))),
        }
    }

    fn backing(primitive: v2::PrimitiveType) -> Option<v2::Backing> {
        Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(primitive as i32)),
        })
    }

    fn int(width: v2::IntWidth) -> v2::FieldType {
        scalar(v2::TypeDef {
            backing: backing(v2::PrimitiveType::Integer),
            width: Some(v2::type_def::Width::IntWidth(width as i32)),
            ..Default::default()
        })
    }

    fn byte() -> v2::FieldType {
        int(v2::IntWidth::U8)
    }

    fn boolean() -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Primitive(
                v2::PrimitiveType::Boolean as i32,
            )),
        }
    }

    fn string(max: u64) -> v2::FieldType {
        scalar(v2::TypeDef {
            backing: backing(v2::PrimitiveType::String),
            constraint: Some(v2::Constraint {
                len_min: Some(0),
                len_max: Some(max),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn bytes(min: u64, max: u64) -> v2::FieldType {
        scalar(v2::TypeDef {
            backing: backing(v2::PrimitiveType::Bytes),
            constraint: Some(v2::Constraint {
                len_min: Some(min),
                len_max: Some(max),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn named(reference: &str) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(reference.to_string())),
        }
    }

    fn array(element: v2::FieldType, min: u64, max: u64) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(element)),
                min,
                max,
            }))),
        }
    }

    fn map(key: v2::FieldType, value: v2::FieldType, min: u64, max: u64) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(key)),
                value: Some(Box::new(value)),
                min,
                max,
            }))),
        }
    }

    fn tuple(fields: Vec<(&str, v2::FieldType)>) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: fields
                    .into_iter()
                    .map(|(name, ty)| v2::TupleField {
                        name: name.to_string(),
                        r#type: Some(ty),
                    })
                    .collect(),
            })),
        }
    }

    fn struct_decl(name: &str, members: Vec<v2::StructMember>) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members,
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn arm(name: &str, ordinal: u32, type_ref: &str) -> v2::UnionArm {
        v2::UnionArm {
            name: name.to_string(),
            ordinal,
            type_ref: type_ref.to_string(),
            ..Default::default()
        }
    }

    fn union_decl(name: &str, arms: Vec<v2::UnionArm>) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                arms,
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn values(values: &[i64]) -> v2::EnumDef {
        v2::EnumDef {
            values: values
                .iter()
                .map(|value| v2::EnumValue {
                    name: format!("V{value}"),
                    value: *value,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// A package with the declarations of one worked example and the name of
    /// the declaration under test.
    struct Fixture {
        package: v2::Package,
        subject: &'static str,
    }

    fn fixture(subject: &'static str, decls: Vec<v2::Decl>) -> Fixture {
        Fixture {
            package: v2::Package {
                name: "veh".to_string(),
                decls,
                ..Default::default()
            },
            subject,
        }
    }

    fn subject(fixture: &Fixture) -> &v2::Decl {
        fixture
            .package
            .decls
            .iter()
            .find(|decl| decl.name == fixture.subject)
            .expect("the fixture declares its subject")
    }

    fn packages(fixture: &Fixture) -> Packages<'_> {
        Packages {
            package: &fixture.package,
            others: &[],
        }
    }

    fn head(fixture: Fixture) -> Head {
        let decl = subject(&fixture);
        let Some(v2::decl::Kind::StructDef(def)) = &decl.kind else {
            panic!("{} is a struct", fixture.subject);
        };
        struct_head(packages(&fixture), &decl.name, def).expect("layout")
    }

    fn head_of_union(fixture: Fixture) -> Head {
        let decl = subject(&fixture);
        let Some(v2::decl::Kind::UnionDef(def)) = &decl.kind else {
            panic!("{} is a union", fixture.subject);
        };
        union_head(packages(&fixture), &decl.name, def).expect("layout")
    }

    fn max_size_of(fixture: Fixture) -> Option<u64> {
        max_size(packages(&fixture), subject(&fixture))
    }

    fn offsets(head: &Head) -> Vec<u32> {
        head.fields
            .iter()
            .map(|placement| placement.offset)
            .collect()
    }

    fn pads(head: &Head) -> Vec<(u32, u32)> {
        head.pads.iter().map(|pad| (pad.offset, pad.size)).collect()
    }

    fn flag_offsets(head: &Head) -> Vec<u32> {
        head.fields
            .iter()
            .filter(|placement| matches!(placement.source, Source::Flags { .. }))
            .map(|placement| placement.offset)
            .collect()
    }

    fn is_field(placement: &Placement, field: &str) -> bool {
        matches!(&placement.source, Source::Field { name, .. } if name == field)
    }

    fn text(min: u64, max: u64) -> DescriptorKind {
        DescriptorKind::Text { min, max }
    }

    // --- the worked examples of the design ----------------------------

    fn door_payload() -> Fixture {
        fixture(
            "DoorPayload",
            vec![struct_decl(
                "DoorPayload",
                vec![
                    field("sensor_id", 1, byte()),
                    field("is_open", 2, boolean()),
                ],
            )],
        )
    }

    fn climate_report() -> Fixture {
        fixture(
            "ClimateReport",
            vec![struct_decl(
                "ClimateReport",
                vec![
                    field("cabin", 1, int(v2::IntWidth::I16)),
                    field("setpoint", 2, int(v2::IntWidth::I16)),
                    field("fan", 3, byte()),
                ],
            )],
        )
    }

    fn diag_filter() -> Fixture {
        fixture(
            "DiagFilter",
            vec![struct_decl(
                "DiagFilter",
                vec![
                    field("severity", 1, byte()),
                    field("category", 2, optional(string(256))),
                ],
            )],
        )
    }

    fn fault_event_decl() -> v2::Decl {
        struct_decl(
            "FaultEvent",
            vec![
                field("code", 1, int(v2::IntWidth::U16)),
                field("message", 2, string(256)),
                field("timestamp", 3, int(v2::IntWidth::I64)),
            ],
        )
    }

    fn fault_event() -> Fixture {
        fixture("FaultEvent", vec![fault_event_decl()])
    }

    fn fault_page() -> Fixture {
        fixture(
            "FaultPage",
            vec![
                fault_event_decl(),
                struct_decl(
                    "FaultPage",
                    vec![field("faults", 1, array(named("FaultEvent"), 0, 64))],
                ),
            ],
        )
    }

    fn nine_optionals() -> Fixture {
        let members = (1..=9)
            .map(|n| field(&format!("f{n}"), n, optional(byte())))
            .collect();
        fixture("Nine", vec![struct_decl("Nine", members)])
    }

    fn sensor_result() -> Fixture {
        fixture(
            "SensorResult",
            vec![
                struct_decl(
                    "SensorReading",
                    vec![field("value", 1, int(v2::IntWidth::I16))],
                ),
                struct_decl(
                    "SensorFault",
                    vec![field("code", 1, int(v2::IntWidth::U16))],
                ),
                union_decl(
                    "SensorResult",
                    vec![arm("ok", 1, "SensorReading"), arm("err", 2, "SensorFault")],
                ),
            ],
        )
    }

    /// `Holder { items: [element; 0..max] }`.
    fn array_of(max: u64, element: v2::FieldType) -> Fixture {
        fixture(
            "Holder",
            vec![struct_decl(
                "Holder",
                vec![field("items", 1, array(element, 0, max))],
            )],
        )
    }

    /// An inline struct of `size` bytes: a tuple holding `[u8; size]`.
    fn struct_of_size(size: u64) -> v2::FieldType {
        tuple(vec![("raw", array(byte(), size, size))])
    }

    // --- the law ------------------------------------------------------

    #[test]
    fn place_follows_the_first_fit_law() {
        // FaultEvent: code u16, message descriptor (8, align 4), timestamp i64.
        assert_eq!(place(&[(2, 2), (8, 4), (8, 8)]), vec![0, 4, 16]);
        // DiagFilter: severity u8, flags u8, category descriptor.
        assert_eq!(place(&[(1, 1), (1, 1), (8, 4)]), vec![0, 1, 4]);
        // A later small field fills the gap an earlier alignment left.
        assert_eq!(place(&[(1, 1), (8, 8), (2, 2)]), vec![0, 8, 2]);
    }

    #[test]
    fn the_worked_examples_of_the_design_lay_out_as_measured() {
        assert_eq!(head(door_payload()).size, 8);
        assert_eq!(head(climate_report()).size, 8);
        let filter = head(diag_filter());
        assert_eq!(filter.size, 16);
        // severity, flags 0, category
        assert_eq!(offsets(&filter), vec![0, 1, 4]);
        assert_eq!(pads(&filter), vec![(2, 2), (12, 4)]);
        let event = head(fault_event());
        assert_eq!(event.size, 24);
        assert_eq!(offsets(&event), vec![0, 4, 16]);
        assert_eq!(pads(&event), vec![(2, 2), (12, 4)]);
        let page = head(fault_page());
        assert_eq!(page.size, 8);
        assert!(matches!(
            page.fields[0].form,
            Form::Descriptor(DescriptorKind::Array {
                elem_size: 24,
                min: 0,
                max: 64,
                ..
            })
        ));
    }

    #[test]
    fn flags_are_placed_before_optional_field_8k_plus_1() {
        // Nine optional u8 fields: flags 0 at 0, fields 1..8 at 1..8, flags 1
        // at 9, field 9 at 10.
        let h = head(nine_optionals());
        assert_eq!(h.size, 16);
        assert_eq!(flag_offsets(&h), vec![0, 9]);
        let f9 = h.fields.iter().find(|p| is_field(p, "f9")).unwrap();
        assert_eq!(f9.offset, 10);
        assert_eq!(f9.optional, Some(FlagBit { flags: 1, bit: 0 }));
        let f8 = h.fields.iter().find(|p| is_field(p, "f8")).unwrap();
        assert_eq!(f8.optional, Some(FlagBit { flags: 0, bit: 7 }));
    }

    #[test]
    fn an_enum_takes_the_narrowest_width_and_the_signedness_of_its_values() {
        assert_eq!(
            enum_width(&values(&[0, 1, 2, 3])),
            (v2::IntWidth::U8, false)
        );
        assert_eq!(enum_width(&values(&[-1, 200])), (v2::IntWidth::I16, true));
        assert_eq!(enum_width(&values(&[70_000])), (v2::IntWidth::U32, false));
        assert_eq!(enum_width(&values(&[])), (v2::IntWidth::U8, false));
    }

    #[test]
    fn elem_size_is_a_multiple_of_8_for_a_struct_and_exact_otherwise() {
        assert_eq!(elem_size(&Form::Inline(Box::new(head(fault_event())))), 24);
        assert_eq!(
            elem_size(&Form::Int {
                width: v2::IntWidth::U16,
                signed: false
            }),
            2
        );
        assert_eq!(elem_size(&Form::Descriptor(text(0, 16))), 8);
    }

    #[test]
    fn a_union_head_is_a_tag_and_an_arm_descriptor() {
        // ok: SensorReading, err: SensorFault
        let h = head_of_union(sensor_result());
        assert_eq!(h.size, 16);
        // tag u8 at 0, descriptor at 4
        assert_eq!(offsets(&h), vec![0, 4]);
        assert_eq!(h.fields[0].source, Source::Tag);
        assert!(matches!(
            h.fields[0].form,
            Form::Int {
                width: v2::IntWidth::U8,
                signed: false
            }
        ));
        let Form::Descriptor(DescriptorKind::Arm { arms }) = &h.fields[1].form else {
            panic!("the second field is the arm descriptor");
        };
        assert_eq!(
            arms.iter().map(|(ordinal, _)| *ordinal).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(arms.iter().all(|(_, arm)| arm.size == 8));
    }

    #[test]
    fn max_size_sums_preamble_head_and_largest_tail() {
        // FaultPage: 8 + 8 + 64 * 24 + 64 * (4 * 256 + 1).
        assert_eq!(max_size_of(fault_page()), Some(8 + 8 + 64 * 24 + 64 * 1025));
        assert_eq!(max_size_of(array_of(65_536, byte())), None);
        assert_eq!(max_size_of(array_of(65_535, byte())), Some(8 + 8 + 65_535));
        assert_eq!(max_size_of(array_of(1, struct_of_size(65_536))), None);
        assert_eq!(
            max_size_of(array_of(1, struct_of_size(65_528))),
            Some(8 + 8 + 65_528)
        );
    }

    #[test]
    fn max_size_counts_the_alignment_gap_before_an_aligned_tail_block() {
        // The string's bytes leave the tail cursor at any alignment, so the
        // array of 8-aligned elements may need up to 7 bytes of zero fill.
        let gapped = fixture(
            "Gapped",
            vec![struct_decl(
                "Gapped",
                vec![
                    field("name", 1, string(16)),
                    field("stamps", 2, array(int(v2::IntWidth::I64), 0, 4)),
                ],
            )],
        );
        assert_eq!(max_size_of(gapped), Some(8 + 16 + (4 * 16 + 1) + 7 + 4 * 8));
        // In the other order the array starts where the head ends, which is
        // 8-aligned, so no gap is charged.
        let ordered = fixture(
            "Ordered",
            vec![struct_decl(
                "Ordered",
                vec![
                    field("stamps", 1, array(int(v2::IntWidth::I64), 0, 4)),
                    field("name", 2, string(16)),
                ],
            )],
        );
        assert_eq!(max_size_of(ordered), Some(8 + 16 + 4 * 8 + (4 * 16 + 1)));
    }

    #[test]
    fn every_array_element_pays_its_tail_at_the_worst_alignment() {
        // Rec head: two descriptors, 16 bytes. Recs head: one descriptor.
        // Tail: the block of 2 Rec heads (32 bytes, 8-aligned), then each
        // element's tail: its stamps block (8 bytes at alignment 8) and its
        // string (4 + 1 bytes). The real worst case is 81: the first element
        // follows the block at alignment 8 (0 + 8 + 5), the second follows
        // the first string at alignment 1 (7 + 8 + 5). The bound charges
        // every element at alignment 1 (7 + 8 + 5 = 20), so 8 + 8 + 32 + 40.
        let recs = fixture(
            "Recs",
            vec![
                struct_decl(
                    "Rec",
                    vec![
                        field("stamps", 1, array(int(v2::IntWidth::I64), 0, 1)),
                        field("name", 2, string(1)),
                    ],
                ),
                struct_decl("Recs", vec![field("items", 1, array(named("Rec"), 0, 2))]),
            ],
        );
        assert_eq!(max_size_of(recs), Some(88));
    }

    #[test]
    fn a_union_pays_its_largest_arm_whatever_the_arm_order() {
        // Each arm head is one descriptor, 8 bytes; the union head is 16.
        // Big's string is 4 * 100 + 1 bytes, Small's 4 + 1: the union pays
        // 8 + 16 + 8 + 401.
        let either = |first: (&str, u32, &str), second: (&str, u32, &str)| {
            fixture(
                "Either",
                vec![
                    struct_decl("Big", vec![field("s", 1, string(100))]),
                    struct_decl("Small", vec![field("s", 1, string(1))]),
                    union_decl(
                        "Either",
                        vec![
                            arm(first.0, first.1, first.2),
                            arm(second.0, second.1, second.2),
                        ],
                    ),
                ],
            )
        };
        assert_eq!(
            max_size_of(either(("big", 1, "Big"), ("small", 2, "Small"))),
            Some(433)
        );
        assert_eq!(
            max_size_of(either(("small", 1, "Small"), ("big", 2, "Big"))),
            Some(433)
        );
    }

    #[test]
    fn a_head_whose_padded_end_passes_u32_is_refused() {
        // The sizes sum to 4_294_967_277, under u32::MAX, but the alignment
        // gaps the law leaves push the last field's end to 4_294_967_296.
        let huge = fixture(
            "Huge",
            vec![struct_decl(
                "Huge",
                vec![
                    field("a", 1, array(byte(), 4_294_967_235, 4_294_967_235)),
                    field("b", 2, int(v2::IntWidth::I64)),
                    field("c", 3, array(byte(), 9, 9)),
                    field("d", 4, int(v2::IntWidth::I64)),
                    field("e", 5, array(byte(), 9, 9)),
                    field("f", 6, int(v2::IntWidth::I64)),
                ],
            )],
        );
        let decl = subject(&huge);
        let Some(v2::decl::Kind::StructDef(def)) = &decl.kind else {
            unreachable!()
        };
        let error = struct_head(packages(&huge), "Huge", def).expect_err("too large");
        assert!(error.message.contains("u32"), "{}", error.message);
        assert_eq!(max_size_of(huge), None);
    }

    #[test]
    fn a_tombstone_holds_no_bytes() {
        let h = head(fixture(
            "Trimmed",
            vec![struct_decl(
                "Trimmed",
                vec![field("a", 1, byte()), retired(2), field("c", 3, byte())],
            )],
        ));
        assert_eq!(offsets(&h), vec![0, 1]);
        assert_eq!(h.size, 8);
    }

    #[test]
    fn an_exact_length_array_and_exact_bytes_are_inline() {
        let h = head(fixture(
            "Fixed",
            vec![struct_decl(
                "Fixed",
                vec![
                    field("samples", 1, array(int(v2::IntWidth::U16), 3, 3)),
                    field("mac", 2, bytes(6, 6)),
                    field("stamp", 3, int(v2::IntWidth::I64)),
                ],
            )],
        ));
        // samples 0..6 (align 2), mac 6..12 (align 1), stamp 16..24.
        assert_eq!(offsets(&h), vec![0, 6, 16]);
        assert_eq!(pads(&h), vec![(12, 4)]);
        assert_eq!(h.size, 24);
        assert!(matches!(
            h.fields[0].form,
            Form::InlineArray { count: 3, .. }
        ));
        assert!(matches!(
            h.fields[1].form,
            Form::InlineArray { count: 6, .. }
        ));
    }

    #[test]
    fn a_map_entry_is_a_head_of_key_and_value() {
        let h = head(fixture(
            "Table",
            vec![struct_decl(
                "Table",
                vec![field("rows", 1, map(byte(), int(v2::IntWidth::I32), 0, 10))],
            )],
        ));
        let Form::Descriptor(DescriptorKind::Map {
            entry,
            elem_size,
            max: 10,
            ..
        }) = &h.fields[0].form
        else {
            panic!("a map descriptor");
        };
        assert_eq!(*elem_size, 8);
        assert_eq!(offsets(entry), vec![0, 4]);
        assert_eq!(entry.fields[0].source, Source::Key);
        assert_eq!(entry.fields[1].source, Source::Value);
    }

    #[test]
    fn a_scalar_arm_is_a_one_field_head() {
        let h = head_of_union(fixture(
            "Reading",
            vec![
                v2::Decl {
                    name: "Percent".to_string(),
                    kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                        backing: backing(v2::PrimitiveType::Integer),
                        width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                struct_decl("Empty", vec![]),
                union_decl(
                    "Reading",
                    vec![arm("level", 1, "Percent"), arm("none", 2, "Empty")],
                ),
            ],
        ));
        let Form::Descriptor(DescriptorKind::Arm { arms }) = &h.fields[1].form else {
            panic!("the arm descriptor");
        };
        assert_eq!(arms[0].1.size, 8);
        assert_eq!(arms[0].1.fields[0].source, Source::Arm);
        assert_eq!(arms[0].1.fields[0].offset, 0);
        assert_eq!(arms[1].1.size, 0);
    }

    #[test]
    fn a_type_that_reaches_itself_has_no_layout() {
        let loop_back = fixture(
            "Loop",
            vec![struct_decl("Loop", vec![field("again", 1, named("Loop"))])],
        );
        let decl = subject(&loop_back);
        let Some(v2::decl::Kind::StructDef(def)) = &decl.kind else {
            unreachable!()
        };
        let error = struct_head(packages(&loop_back), "Loop", def).expect_err("a cycle");
        assert!(error.message.contains("Loop"), "{}", error.message);
        assert_eq!(max_size_of(loop_back), None);
    }
}
