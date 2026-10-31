//! The proto3 payload codec, emitted beside the FlatBuffers codec
//! (`crate::codec`) when a package's `encodings` names `proto3`.
//!
//! For every declaration that projects a type — a struct over its own
//! message, a union over its message with `oneof value`, and a named scalar,
//! an enum and an enum set over the box message
//! `<Name>Box { <type> value = 1; }` the proto backend emits for them
//! (ADR-0017 decision 6) — and for every induced tuple the package's
//! declarations and interactions reach, this module emits, at the generated
//! package's module scope and `pub(crate)` for the reasons the FlatBuffers
//! codec gives for its own functions:
//!
//! - `__ridl_pb_len_<Name>(&T) -> usize`, the exact encoded length;
//! - `__ridl_pb_encode_<Name>(&T, &mut Writer)`, which writes forwards;
//! - `__ridl_pb_verify_<Name>(&[u8])`, which walks the message's fields and
//!   checks every typl constraint the FlatBuffers `verify` checks, through
//!   the same generated `check` functions and the same inline checks;
//! - `__ridl_pb_decode_<Name>(&[u8]) -> T`, which is total;
//! - the `Payload<::ridl_rt::encoding::Proto3>` implementation, whose view
//!   is `::ridl_rt::proto3::Verified`, the verified bytes with no accessor,
//!   and whose `MAX_SIZE` is the finite bound the model carries for the
//!   declaration's root message.
//!
//! An induced tuple gets the four functions, and the `Payload`
//! implementation only when it crosses the wire on its own: the argument
//! message of a call with zero or several parameters, and the reply message
//! of a query whose reply is a tuple (driftsys/ridl#713). Its `MAX_SIZE` is
//! the bound the model carries for that interaction's payload. A tuple
//! reached only from a field is encoded inside its owner.
//!
//! A declaration whose proto3 size state is not bounded is refused with a
//! [`GenerateError`] naming it and the cause: a member with no finite bound
//! — a string or bytes with no length bound — a bound or a sum past
//! `u32::MAX`, a member proto3 cannot represent, a declaration the sizer
//! did not resolve, or no message at all. An induced root whose state is
//! not bounded gets no `Payload` implementation, and the descriptor emitter
//! skips its interface, as it skips an interface whose induced root has no
//! finite FlatBuffers bound (`crate::descriptors::induced_name`). The size
//! limit lives in the type: a source that needs a proto3 codec declares a
//! finite bound on every string and bytes its payloads reach, and a
//! collection count that fits. The codec's own walk refuses the
//! same members the proto backend refuses, naming the field: an optional
//! array or map field of a struct or of a tuple a declaration holds
//! (ADR-0013 decision 7), an optional array element and an optional map
//! value, a nested collection, a map key outside the integral and string
//! scalars, an enum value outside int32, and a field number proto3 reserves
//! or exceeds. A tuple only an interaction reaches is never walked when its
//! root is not bounded, so such a member in it is the sizer's
//! `RefusedMember`, named by the interface's note without the field.
//!
//! # The wire mapping
//!
//! A field's number is its ordinal, a union arm's number is its ordinal, a
//! tuple's fields are numbered from 1 in position order, a map entry's key
//! and value are 1 and 2, and the scalar a width projects to is
//! `ridl_ir::projection::proto3`'s table, the one the proto backend and the
//! proto3 sizer read, so the codec cannot disagree with the emitted `.proto`
//! on any of them. The writer writes the canonical form: fields in number
//! order, an implicit-presence field omitted at its proto3 default — 0,
//! `false`, the empty string or bytes, enum value 0, the empty set, the
//! empty array or map — a `?` field written whenever present, a
//! non-optional nested struct or tuple always written, as a record that may
//! be empty, a union written as exactly its one arm, even at the arm's
//! default, an array of numerics, booleans, enums or enum sets as one packed
//! record, any other array as one record per element, a map as one entry
//! message per pair in the order of the domain `Vec`, where a key or a value
//! at its default is omitted and a message value is omitted when its own
//! encoding is empty, and a float compared by its bits, so `-0.0` is
//! written. The reader accepts fields in any order, a repeated numeric field
//! packed, unpacked or both mixed, skips an unknown field number, and
//! refuses a singular field written twice and a union carrying two arms
//! (`Malformed::Duplicate`), a known field under a wire type its type does
//! not take (`Malformed::WireType`), a varint wider than its field's scalar
//! (`Malformed::Varint`), and a packed run of fixed-width values whose
//! length is not a multiple of the width (`Malformed::OutOfBounds`).
//!
//! An absent implicit-presence field reads as its default when that default
//! is a legal value of its type, decided at generation time by the predicate
//! the FlatBuffers codec reads (`crate::zero_is_legal`, a string's or bytes'
//! length lower bound, a collection's count lower bound, an enum's zero
//! member), and is `Malformed::MissingRequired` otherwise. An absent
//! non-optional nested struct or tuple is the message decoded from zero
//! bytes, so `verify` verifies the empty message in its place, which refuses
//! it exactly when one of its own fields is refused when absent; an absent
//! non-optional union is refused the same way, because its empty message
//! holds no arm. A field whose default is not legal is never at its default,
//! so the writer writes it without testing it. The rule holds inside a map
//! entry too: an absent key or value reads as its default when that is
//! legal.
//!
//! `verify` checks a collection's count against its declared bound
//! (`Rule::Length`) and a map's keys for uniqueness (`Rule::Unique`), the
//! latter by re-reading the entries before the one in hand, so no index is
//! allocated; `encode` checks the same count and the same uniqueness before
//! it writes the collection, as the FlatBuffers `encode` does.
//!
//! `encode` sizes the value first, with the length function, and returns
//! `EncodeError::Capacity` carrying that exact `needed` before it writes a
//! byte; what it then writes is a prefix of the output buffer. The length
//! arithmetic saturates, so a value past its declared bound that is too
//! large for a `usize` asks for `usize::MAX` rather than wrapping. `verify`
//! applies no size ceiling: a buffer from a newer writer may carry fields
//! this version skips.

use std::collections::{HashMap, HashSet};

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::codegen::v1;
use ridl_ir::projection::proto3 as pb;

use crate::codec::{
    NamedScalar, Operand, count_check_of, encode_count_check, inline_length_check,
    missing_required, named_scalar_check, scalar_constraint_check,
};
use crate::descriptors::interactions;
use crate::{
    Ctx, GenerateError, ScalarBacking, allow_dead_code_if_internal, class_backing, declared, ident,
    pascal_of, snake_of, type_path, zero_is_legal,
};

/// Emits the proto3 codec for the package `ctx` was built over.
///
/// Also returns the types it emits a `Payload<Proto3>` implementation for,
/// recorded during the same walk, so the transcode functions follow the
/// codec's own rule.
pub(crate) fn package_items(
    ctx: &Ctx,
) -> Result<(Vec<TokenStream>, Vec<crate::transcode::PayloadOwner>), GenerateError> {
    Codec::new(ctx)?.items()
}

/// The layout the codec emits from, per emitted message, for the test that
/// compares it with the emitted `.proto` schema field by field. A
/// classification the codec refuses is the test's error, not an empty
/// layout.
pub(crate) fn layout(ctx: &Ctx) -> Result<Vec<MessageLayout>, GenerateError> {
    Ok(Codec::new(ctx)?
        .messages()?
        .into_iter()
        .map(|message| message.layout())
        .collect())
}

/// The presence label of one field, as the `.proto` states it.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// A field with implicit presence: no `?`, not a collection.
    Implicit,
    /// A `?` field, `optional` in the schema.
    Optional,
    /// An array or a map, `repeated` or `map<K, V>` in the schema.
    Repeated,
}

/// One emitted message: its `.proto` name and, per field, the field number,
/// the scalar's spelling or the referenced message's name (a map as
/// `map<K, V>`), and the label.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageLayout {
    pub name: String,
    pub fields: Vec<(u32, String, Label)>,
}

struct Codec<'a> {
    ctx: &'a Ctx<'a>,
    /// The name of the package the model was lowered for, which a type
    /// reference of this package names.
    scope: &'a str,
}

// ---------------------------------------------------------------------------
// The wire shape of one type position
// ---------------------------------------------------------------------------

/// What a value of one typl type position becomes in a proto3 message.
#[derive(Debug, Clone)]
enum Wire {
    /// A varint or a fixed-width value.
    Scalar(Scalar),
    /// A length-delimited UTF-8 string; `Some` when a named scalar wraps it.
    Text(Option<NamedScalar>, Option<v1::Scalar>),
    /// Length-delimited bytes; `Some` when a named scalar wraps it.
    Bytes(Option<NamedScalar>, Option<v1::Scalar>),
    /// A length-delimited nested message: a struct, a union or an induced
    /// tuple. `owner` names the generated Rust item whose functions encode
    /// it (the dotted reference for one of another package), `proto` the
    /// `.proto` message, which differs from the Rust name only for a tuple.
    Message { owner: String, proto: String },
    /// A `repeated` field: packed when the element is a scalar, one record
    /// per element otherwise.
    Array {
        element: Box<Wire>,
        min: u64,
        max: u64,
    },
    /// A `map<K, V>` field: one entry message per pair.
    Map {
        key: Box<Wire>,
        value: Box<Wire>,
        min: u64,
        max: u64,
    },
}

impl Wire {
    /// The wire type the field's tag carries. A packed array, a record
    /// array and a map entry are all length-delimited; an unpacked element
    /// of a scalar array carries the element's own wire type, which the
    /// reader accepts beside the packed form.
    fn wire_type(&self) -> pb::WireType {
        match self {
            Wire::Scalar(scalar) => scalar.scalar.wire_type(),
            Wire::Text(..)
            | Wire::Bytes(..)
            | Wire::Message { .. }
            | Wire::Array { .. }
            | Wire::Map { .. } => pb::WireType::Len,
        }
    }

    /// Whether the proto3 default of the field's type is a legal value of
    /// it, decided at generation time: an absent non-optional field reads as
    /// that default when it is, and is refused when it is not. A nested
    /// message decides for itself, field by field, when its empty message is
    /// verified; a collection's default is the empty one, legal when the
    /// count lower bound is 0.
    fn default_is_legal(&self) -> bool {
        match self {
            Wire::Scalar(scalar) => scalar.zero_legal,
            Wire::Text(_, constraint) | Wire::Bytes(_, constraint) => {
                constraint
                    .as_ref()
                    .and_then(|sc| sc.constraint.as_ref())
                    .and_then(|constraint| constraint.len_min)
                    .unwrap_or(0)
                    == 0
            }
            Wire::Message { .. } => true,
            Wire::Array { min, .. } | Wire::Map { min, .. } => *min == 0,
        }
    }

    /// Whether the field is `repeated` in the schema: an array or a map.
    fn repeated(&self) -> bool {
        matches!(self, Wire::Array { .. } | Wire::Map { .. })
    }

    /// The scalar's spelling or the message's name, for [`MessageLayout`]:
    /// the `.proto` names an enum by its name (the bare name when local,
    /// `pkg.Name` when foreign), inlines an enum set as its scalar, names a
    /// repeated field by its element, and spells a map as `map<K, V>`.
    fn layout_name(&self) -> String {
        match self {
            Wire::Scalar(Scalar {
                repr: Repr::Enum { name, .. },
                ..
            }) => name.clone(),
            Wire::Scalar(scalar) => scalar.scalar.as_str().to_string(),
            Wire::Text(..) => pb::Scalar::String.as_str().to_string(),
            Wire::Bytes(..) => pb::Scalar::Bytes.as_str().to_string(),
            Wire::Message { proto, .. } => proto.clone(),
            Wire::Array { element, .. } => element.layout_name(),
            Wire::Map { key, value, .. } => {
                format!("map<{}, {}>", key.layout_name(), value.layout_name())
            }
        }
    }
}

/// How the domain value and the wire scalar convert into each other.
#[derive(Debug, Clone)]
enum Repr {
    /// The `boolean` primitive, or an inline `boolean` scalar.
    Bool,
    /// An `i64` in the language layer (typl Appendix D).
    Int,
    /// An `f64` in the language layer.
    Float,
    /// A named scalar over one of the three above.
    Named(NamedScalar),
    /// A generated `#[repr(i64)]` enum, written as a proto3 enum: an `int32`
    /// varint. `first` is the variant a discharged read decodes to; `zero`
    /// is the variant whose value is 0, which an absent field reads as, when
    /// the enum declares one.
    Enum {
        name: String,
        first: String,
        zero: Option<String>,
    },
    /// A generated enum set, written as the scalar of its width.
    EnumSet { name: String },
}

#[derive(Debug, Clone)]
struct Scalar {
    /// The proto3 scalar the value is written as. An enum is `Int64`: its
    /// `int32` is written sign-extended to 64 bits, which is the same
    /// varint, and read back through `narrow_i32`.
    scalar: pb::Scalar,
    repr: Repr,
    /// The inline constraint `verify` checks, for an anonymous scalar.
    constraint: Option<v1::Scalar>,
    /// Whether 0, the proto3 default, is a legal value of the type
    /// (`crate::zero_is_legal`; an enum declares a zero member; an enum set
    /// always is).
    zero_legal: bool,
}

impl Scalar {
    /// The language-layer value (`i64`, `f64` or `bool`) of the domain value
    /// `expr`.
    fn language(&self, expr: TokenStream) -> TokenStream {
        match &self.repr {
            Repr::Bool | Repr::Int | Repr::Float => expr,
            Repr::Named(_) => quote! { #expr.get() },
            Repr::Enum { .. } | Repr::EnumSet { .. } => {
                quote! { ::core::primitive::i64::from(#expr) }
            }
        }
    }

    /// The value the writer puts after the tag, from a language-layer value
    /// `__s`: the `u64` of a varint, or the bits of a fixed-width value. Zero
    /// here is the proto3 default, for every scalar, so the caller tests the
    /// result against 0 to decide whether the field is written; a float is
    /// therefore compared by its bits, and `-0.0` is written.
    fn raw(&self, language: TokenStream) -> TokenStream {
        match self.scalar {
            pb::Scalar::Bool => quote! { ::core::primitive::u64::from(#language) },
            pb::Scalar::Uint32 | pb::Scalar::Uint64 | pb::Scalar::Int64 => {
                quote! { #language as ::core::primitive::u64 }
            }
            pb::Scalar::Sint32 => quote! {
                ::core::primitive::u64::from(
                    ::ridl_rt::proto3::zigzag32(#language as ::core::primitive::i32),
                )
            },
            pb::Scalar::Sint64 => quote! { ::ridl_rt::proto3::zigzag64(#language) },
            pb::Scalar::Float => quote! { (#language as ::core::primitive::f32).to_bits() },
            pb::Scalar::Double => quote! { #language.to_bits() },
            // A string or bytes backing never reaches a `Scalar`.
            pb::Scalar::String | pb::Scalar::Bytes => quote! { 0u64 },
        }
    }

    /// The bytes the value `__r` ([`Scalar::raw`]) takes after the tag.
    fn value_len(&self) -> TokenStream {
        match self.scalar.wire_type() {
            pb::WireType::Varint => quote! { ::ridl_rt::proto3::varint_len(__r) },
            pb::WireType::I32 => quote! { 4usize },
            pb::WireType::I64 => quote! { 8usize },
            pb::WireType::Len => quote! { 0usize },
        }
    }

    /// The width of a fixed-width value, or `None` for a varint.
    fn fixed_width(&self) -> Option<usize> {
        match self.scalar.wire_type() {
            pb::WireType::I32 => Some(4),
            pb::WireType::I64 => Some(8),
            pb::WireType::Varint | pb::WireType::Len => None,
        }
    }

    /// The statements that read one fixed-width element `__chunk` of a
    /// packed run into `__raw`: four bytes for an `I32`, eight for an `I64`.
    fn fixed_read(&self) -> TokenStream {
        match self.scalar.wire_type() {
            pb::WireType::I32 => quote! {
                let mut __bytes = [0u8; 4];
                __bytes.copy_from_slice(__chunk);
                let __raw = ::core::primitive::u32::from_le_bytes(__bytes);
            },
            _ => quote! {
                let mut __bytes = [0u8; 8];
                __bytes.copy_from_slice(__chunk);
                let __raw = ::core::primitive::u64::from_le_bytes(__bytes);
            },
        }
    }

    /// The writer call for the value `__r`.
    fn write(&self) -> TokenStream {
        match self.scalar.wire_type() {
            pb::WireType::Varint => quote! { writer.varint(__r); },
            pb::WireType::I32 => quote! { writer.fixed32(__r); },
            pb::WireType::I64 => quote! { writer.fixed64(__r); },
            pb::WireType::Len => quote! {},
        }
    }

    /// The `Value` variant the field's wire type is read as.
    fn value_variant(&self) -> Ident {
        wire_variant(self.scalar.wire_type())
    }

    /// The language-layer value read from the raw wire value `__raw`, and
    /// whether that read can fail with `Malformed::Varint`.
    fn widen(&self) -> (TokenStream, bool) {
        match self.scalar {
            pb::Scalar::Bool => (quote! { __raw != 0u64 }, false),
            pb::Scalar::Uint32 => (
                quote! {
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_u32(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    )
                },
                true,
            ),
            // A wire value above `i64::MAX` wraps to a negative `i64`, the
            // inverse of the encoder's `i64 as u64` cast. A `uint64` integer
            // field has a declared range inside `i64` with a non-negative
            // minimum (the width is derived from the range), so the range
            // check that follows refuses the wrapped value as
            // `Contract(Range)`. A `uint64` enum-set field has no range: its
            // width is u64 because bit 63 is a declared member, the wrapped
            // value is the set's own `i64` representation, and the
            // `TryFrom<i64>` check that follows accepts it exactly when
            // every set bit is a member.
            pb::Scalar::Uint64 => (quote! { __raw as ::core::primitive::i64 }, false),
            pb::Scalar::Sint32 => (
                quote! {
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::unzigzag32_checked(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    )
                },
                true,
            ),
            pb::Scalar::Sint64 => (quote! { ::ridl_rt::proto3::unzigzag64(__raw) }, false),
            pb::Scalar::Int64 => (
                quote! {
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_i32(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    )
                },
                true,
            ),
            pb::Scalar::Float => (
                quote! { ::core::primitive::f64::from(::core::primitive::f32::from_bits(__raw)) },
                false,
            ),
            pb::Scalar::Double => (quote! { ::core::primitive::f64::from_bits(__raw) }, false),
            pb::Scalar::String | pb::Scalar::Bytes => (quote! { 0i64 }, false),
        }
    }

    /// The language-layer value `decode` reads from `__raw`, which cannot
    /// fail: a read `verify` would have refused is discharged with 0.
    fn widen_total(&self) -> TokenStream {
        match self.scalar {
            pb::Scalar::Bool => quote! { __raw != 0u64 },
            pb::Scalar::Uint32 => quote! {
                ::core::primitive::i64::from(::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32))
            },
            pb::Scalar::Uint64 => quote! { __raw as ::core::primitive::i64 },
            pb::Scalar::Sint32 => quote! {
                ::core::primitive::i64::from(
                    ::ridl_rt::proto3::unzigzag32_checked(__raw).unwrap_or(0i32),
                )
            },
            pb::Scalar::Sint64 => quote! { ::ridl_rt::proto3::unzigzag64(__raw) },
            pb::Scalar::Int64 => quote! {
                ::core::primitive::i64::from(::ridl_rt::proto3::narrow_i32(__raw).unwrap_or(0i32))
            },
            pb::Scalar::Float => {
                quote! { ::core::primitive::f64::from(::core::primitive::f32::from_bits(__raw)) }
            }
            pb::Scalar::Double => quote! { ::core::primitive::f64::from_bits(__raw) },
            pb::Scalar::String | pb::Scalar::Bytes => quote! { 0i64 },
        }
    }

    /// The language-layer zero: what an absent value or a discharged read
    /// is, before the domain constructor.
    fn language_zero(&self) -> TokenStream {
        match self.scalar {
            pb::Scalar::Bool => quote! { false },
            pb::Scalar::Float | pb::Scalar::Double => quote! { 0.0f64 },
            _ => quote! { 0i64 },
        }
    }

    /// The domain value built from a language-layer value `language`, for
    /// `decode`.
    fn domain(&self, language: TokenStream) -> TokenStream {
        match &self.repr {
            Repr::Bool | Repr::Int | Repr::Float => language,
            Repr::Named(named) => {
                let ty = type_path(&named.name);
                let ctor = format_ident!("{}", named.ctor);
                quote! { #ty::#ctor(#language) }
            }
            Repr::Enum { name, first, .. } => {
                let ty = type_path(name);
                let variant = ident(first);
                quote! {
                    <#ty as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(#language)
                        .unwrap_or(#ty::#variant)
                }
            }
            Repr::EnumSet { name } => {
                let ty = type_path(name);
                quote! {
                    <#ty as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(#language)
                        .unwrap_or(#ty(0i64))
                }
            }
        }
    }

    /// The domain value an absent field reads as: the proto3 default, 0, as
    /// its own type spells it. `verify` has refused an absent field whose
    /// type excludes 0, so for an enum this is its zero member; the first
    /// variant stands in only where `verify` makes the read unreachable.
    fn decode_default(&self) -> TokenStream {
        let zero = self.language_zero();
        match &self.repr {
            Repr::Bool | Repr::Int | Repr::Float => zero,
            Repr::Named(named) => {
                let ty = type_path(&named.name);
                let ctor = format_ident!("{}", named.ctor);
                quote! { #ty::#ctor(#zero) }
            }
            Repr::Enum { name, first, zero } => {
                let ty = type_path(name);
                let variant = ident(zero.as_ref().unwrap_or(first));
                quote! { #ty::#variant }
            }
            Repr::EnumSet { name } => {
                let ty = type_path(name);
                quote! { #ty(0i64) }
            }
        }
    }

    /// The constraint check `verify` runs over the language-layer value
    /// `__v`, or `None` when the type checks nothing.
    fn check(&self, owner: &str) -> Option<TokenStream> {
        match &self.repr {
            Repr::Enum { name, .. } | Repr::EnumSet { name } => {
                let ty = type_path(name);
                Some(quote! {
                    <#ty as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(__v)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                })
            }
            Repr::Named(named) => named_scalar_check(named, quote! { &__v }),
            Repr::Bool | Repr::Int | Repr::Float => self.constraint.as_ref().map(|sc| {
                scalar_constraint_check(sc, owner, quote! { __v }, self.scalar == pb::Scalar::Float)
            }),
        }
    }

    /// The statements that check one raw wire value `__raw`: the widening,
    /// which may refuse an over-wide varint, then the constraint. Empty when
    /// neither can refuse.
    fn raw_checks(&self, owner: &str) -> TokenStream {
        let (widened, fallible) = self.widen();
        match (self.check(owner), fallible) {
            (Some(check), _) => quote! {
                let __v = #widened;
                #check
            },
            (None, true) => quote! { let _ = #widened; },
            (None, false) => quote! {},
        }
    }
}

// ---------------------------------------------------------------------------
// A message and its fields
// ---------------------------------------------------------------------------

/// How one field's value is reached from the value being encoded.
#[derive(Debug, Clone)]
enum Access {
    /// A named field of a struct or an induced tuple struct.
    Field(String),
    /// The whole value: a box root's `value`.
    Whole,
    /// A position of a map entry's pair, `__e.0` or `__e.1`.
    Position(usize),
}

/// Where a value is written from, which decides whether it is tested
/// against its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    /// A field of a struct, a tuple or a box: an implicit-presence field is
    /// omitted at its default when that default is legal, a nested message
    /// is always written.
    Field,
    /// A map entry's key or value: omitted at its default, and a message
    /// omitted when its own encoding is empty, which is what every proto3
    /// writer does inside an entry.
    Entry,
    /// An element of a record array, or a union's arm: always written.
    Element,
}

#[derive(Debug, Clone)]
struct Field {
    /// The field number: the ordinal, the position from 1 in a tuple, 1 for
    /// a box's `value`.
    number: u32,
    /// The Rust name, used for the decoded field.
    name: String,
    wire: Wire,
    optional: bool,
    access: Access,
}

impl Field {
    /// Whether the length and encode passes emit a statement for this
    /// field: every field but a fixed-length array of zero elements, which
    /// has nothing to count and nothing to write (`array_statement`).
    fn has_statement(&self) -> bool {
        !matches!(self.wire, Wire::Array { min, max, .. } if min == max && max == 0)
    }

    fn operand(&self, base: &TokenStream) -> Operand {
        match &self.access {
            Access::Field(name) => {
                let field = ident(name);
                Operand::owned(quote! { #base.#field })
            }
            Access::Whole => Operand::borrowed(base.clone()),
            Access::Position(position) => {
                let index = Literal::usize_unsuffixed(*position);
                Operand::owned(quote! { #base.#index })
            }
        }
    }

    fn label(&self) -> Label {
        if self.wire.repeated() {
            Label::Repeated
        } else if self.optional {
            Label::Optional
        } else {
            Label::Implicit
        }
    }

    /// A field for one position of a value reached through a collection or
    /// a union: the same number, the element's or the arm's wire shape, no
    /// `?`, reached through the binding the caller names.
    fn element(number: u32, wire: Wire) -> Field {
        Field {
            number,
            name: String::new(),
            wire,
            optional: false,
            access: Access::Whole,
        }
    }
}

/// What `decode` builds: a struct literal over the fields, a box's one
/// value, or a union's one arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Build {
    Struct,
    Box,
    Union,
}

/// What a message was classified from.
#[derive(Debug, Clone)]
enum Source {
    /// The declaration at this index of the model.
    Declaration(u32),
    /// An induced tuple of the model, with its bound when an interaction
    /// names it as a payload of its own and that payload's proto3 state is
    /// bounded; `None` for a tuple encoded only inside its owner.
    Tuple { root: Option<u32> },
}

#[derive(Debug, Clone)]
struct Message {
    /// The generated Rust item the functions are named after: the declared
    /// name, or the tuple's Rust name.
    owner: String,
    /// The `.proto` message name: the declared name, `<Name>Box`, or the
    /// tuple's wire name.
    proto: String,
    source: Source,
    visibility: i32,
    /// The fields, in field-number order; a union's arms.
    fields: Vec<Field>,
    build: Build,
}

impl Message {
    fn layout(&self) -> MessageLayout {
        MessageLayout {
            name: self.proto.clone(),
            fields: self
                .fields
                .iter()
                .map(|field| (field.number, field.wire.layout_name(), field.label()))
                .collect(),
        }
    }
}

/// The four function names of one owner, spelled from the declared name
/// through [`crate::ident`] rather than through `snake_case`, for the reason
/// the FlatBuffers codec gives: `snake_case` is not injective over the names
/// TYPL-009 accepts.
fn len_ident(owner: &str) -> Ident {
    format_ident!("__ridl_pb_len_{}", ident(owner))
}

fn encode_ident(owner: &str) -> Ident {
    format_ident!("__ridl_pb_encode_{}", ident(owner))
}

fn verify_ident(owner: &str) -> Ident {
    format_ident!("__ridl_pb_verify_{}", ident(owner))
}

fn decode_ident(owner: &str) -> Ident {
    format_ident!("__ridl_pb_decode_{}", ident(owner))
}

/// The path to the item `id(name)` of `owner` at a reference site: the bare
/// item for a local owner, and for one of another package of the build the
/// item behind [`crate::package_prefix`].
fn owner_path(owner: &str, id: impl Fn(&str) -> Ident) -> TokenStream {
    match owner.rsplit_once('.') {
        Some((package, name)) => {
            let id = id(name);
            let prefix = crate::package_prefix(package, &id);
            quote! { #prefix #id }
        }
        None => {
            let id = id(owner);
            quote! { #id }
        }
    }
}

/// One parameter of a message function, underscored when the message has no
/// field and the body therefore reads nothing; the generated crate is built
/// under `-D warnings`.
fn param_binding(name: &str, used: bool) -> Ident {
    if used {
        format_ident!("{}", name)
    } else {
        format_ident!("_{}", name)
    }
}

/// The bytes the tag of field `number` takes: the varint of the number
/// shifted past the three wire-type bits. The same arithmetic as
/// `ridl_rt::proto3::tag_len`, which this crate does not link; the emitted
/// code reads `varint_len` from the runtime for every length it computes at
/// run time, and this one is a generation-time literal. A test ties the two
/// at every width boundary (`the_generation_time_tag_length_is_the_runtime_one`).
pub(crate) fn tag_len(number: u32) -> usize {
    let tag = u64::from(number) << 3;
    let bits = 64 - (tag | 1).leading_zeros() as usize;
    bits.div_ceil(7)
}

/// The variant name a projection wire type has in `ridl_rt::proto3`, the
/// same in `WireType` and in `Value`.
fn wire_variant(wire: pb::WireType) -> Ident {
    format_ident!(
        "{}",
        match wire {
            pb::WireType::Varint => "Varint",
            pb::WireType::I64 => "I64",
            pb::WireType::Len => "Len",
            pb::WireType::I32 => "I32",
        }
    )
}

/// The `WireType` variant of `ridl_rt::proto3` for a projection wire type.
fn wire_type_tokens(wire: pb::WireType) -> TokenStream {
    let variant = wire_variant(wire);
    quote! { ::ridl_rt::proto3::WireType::#variant }
}

/// The `verify` statement for a fault of the message's structure.
fn structure_error(variant: &str) -> TokenStream {
    let variant = format_ident!("{}", variant);
    quote! {
        return ::core::result::Result::Err(
            ::ridl_rt::payload::VerifyError::Structure(
                ::ridl_rt::payload::Malformed::#variant,
            ),
        );
    }
}

/// The `verify` statement for a map entry past the map's declared count,
/// the same violation `counter_check` raises after the walk.
fn length_error(owner: &str) -> TokenStream {
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some(#owner),
                rule: ::ridl_rt::payload::Rule::Length,
            },
        ));
    }
}

/// The `verify` statement for a map key that an earlier entry already
/// holds, the typl uniqueness rule the FlatBuffers codec refuses by the same
/// violation.
fn unique_error(owner: &str) -> TokenStream {
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some(#owner),
                rule: ::ridl_rt::payload::Rule::Unique,
            },
        ));
    }
}

/// The `encode` statement for the same fault, before the map is written.
fn encode_unique_error(owner: &str) -> TokenStream {
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::EncodeError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some(#owner),
                rule: ::ridl_rt::payload::Rule::Unique,
            },
        ));
    }
}

/// The counter `verify` keeps for the repeated field at `index`.
fn count_ident(index: usize) -> Ident {
    format_ident!("__c{}", index)
}

/// The statement inside a walk over a message's fields that runs the body of
/// the known field `__field.number` names, and skips an unknown field, which
/// the reader has already moved past. Two or more known fields are a
/// `match`; one is an `if`, and none reads nothing, so the emitted code draws
/// neither `clippy::single_match` nor `clippy::match_single_binding`.
///
/// The `if` holds the `if let` of the field's body. On edition 2024 clippy
/// reads the two as `clippy::collapsible_if`, whose fix is a let chain, which
/// edition 2021 does not have; every generated function that walks a message
/// allows the lint.
fn number_dispatch(arms: &[(Literal, TokenStream)]) -> TokenStream {
    match arms {
        // Defensive: every caller has at least one known field.
        [] => quote! { let _ = __field; },
        [(number, body)] => quote! {
            if __field.number == #number {
                #body
            }
        },
        _ => {
            let arms = arms
                .iter()
                .map(|(number, body)| quote! { #number => { #body } });
            quote! {
                match __field.number {
                    #(#arms)*
                    _ => {}
                }
            }
        }
    }
}

/// The check `verify` runs on the count `counter` of a bounded repeated
/// field once the walk is over: the rule and the owner of the shared
/// `count_check_of`, with a two-sided bound written as a range `contains`,
/// because `counter` is a local the walk counts and a pair of comparisons on
/// it draws `clippy::manual_range_contains`.
fn counter_check(owner: &str, min: u64, max: u64, counter: &Ident) -> TokenStream {
    if min == 0 || min == max || max == u64::MAX {
        return count_check_of(owner, min, max, quote! { #counter });
    }
    let low = Literal::usize_suffixed(min as usize);
    let high = Literal::usize_suffixed(max as usize);
    quote! {
        if !(#low..=#high).contains(&#counter) {
            return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(
                ::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some(#owner),
                    rule: ::ridl_rt::payload::Rule::Length,
                },
            ));
        }
    }
}

/// The two fields of a map entry message: the key at 1 and the value at 2,
/// reached as the two positions of the domain pair.
fn entry_fields(key: &Wire, value: &Wire) -> [Field; 2] {
    [
        Field {
            number: 1,
            name: "key".to_string(),
            wire: key.clone(),
            optional: false,
            access: Access::Position(0),
        },
        Field {
            number: 2,
            name: "value".to_string(),
            wire: value.clone(),
            optional: false,
            access: Access::Position(1),
        },
    ]
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

impl<'a> Codec<'a> {
    /// The codec over `ctx`, or an error for a model lowered with no scope
    /// package: a type reference is resolved against that name, so without
    /// it no reference of this package could be told from a foreign one.
    fn new(ctx: &'a Ctx<'a>) -> Result<Codec<'a>, GenerateError> {
        let scope = ctx
            .model
            .scope
            .as_ref()
            .map(|scope| scope.package.as_str())
            .filter(|package| !package.is_empty())
            .ok_or_else(|| GenerateError {
                message: "the lowered model names no scope package, so no type reference can \
                          be resolved for the proto3 codec"
                    .to_string(),
            })?;
        Ok(Codec { ctx, scope })
    }

    /// Resolves a type reference against this package first and then the
    /// others of the build, returning the declaration and the owner string a
    /// reference site writes: the bare declared name for a type of this
    /// package, the dotted reference for one of another package.
    fn resolve(&self, reference: &v1::TypeRef) -> Option<(&'a v1::Declaration, String)> {
        if reference.package.is_empty() {
            return None;
        }
        if reference.package == self.scope {
            let decl = self.ctx.declaration(reference.index)?;
            return Some((decl, declared(decl.name.as_ref()).to_string()));
        }
        let foreign = self.ctx.model.foreign.get(reference.index as usize)?;
        Some((foreign.declaration.as_ref()?, reference.reference.clone()))
    }

    /// The messages this codec emits: every declaration that projects a
    /// type, in declaration order, then every induced tuple those
    /// declarations and the interaction roots reach, in discovery order.
    fn messages(&self) -> Result<Vec<Message>, GenerateError> {
        let mut messages: Vec<Message> = Vec::new();
        for (index, decl) in self.ctx.model.declarations.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| GenerateError {
                message: "the lowered model carries more declarations than a u32 indexes"
                    .to_string(),
            })?;
            if let Some(message) = self.message_of(decl, index)? {
                messages.push(message);
            }
        }
        let roots = self.interaction_roots();
        for index in self.reachable_tuples(&roots) {
            let induced = self.ctx.tuple(index).ok_or_else(|| GenerateError {
                message: "the lowered model names a tuple it does not carry".to_string(),
            })?;
            messages.push(self.tuple_message(induced, roots.get(&index).copied())?);
        }
        Ok(messages)
    }

    /// The induced tuples the interactions of this package send on their
    /// own — a call's argument message, a query's tuple reply — each with
    /// its bound, the proto3 state the model carries for that interaction's
    /// payload, when that state is bounded.
    ///
    /// A root whose state is not bounded is left out, as the FlatBuffers
    /// codec leaves out a tuple root with no finite bound: it gets no
    /// `Payload<Proto3>`, and the descriptor emitter skips its interface
    /// with the refusal that names the cause (`descriptors::induced_name`),
    /// so the package's other interfaces and its types still generate.
    /// Every interface is walked, a service's inline shape included, as the
    /// FlatBuffers projection roots every induced tuple that crosses the
    /// wire whatever its interface's identity; no name is needed, because
    /// nothing here refuses.
    fn interaction_roots(&self) -> HashMap<u32, u32> {
        tuple_roots(self.ctx, |sizes| sizes.proto3.as_ref())
    }

    /// The induced tuples reachable from the declarations and from the
    /// bounded interaction roots, in discovery order: a root tuple itself,
    /// and every tuple a struct field, a tuple field, an array element or a
    /// map value holds. Only a reached tuple gets functions, so no dead code
    /// reaches a consumer's build; a tuple root with no bound is reached
    /// only when a declaration holds it.
    fn reachable_tuples(&self, roots: &HashMap<u32, u32>) -> Vec<u32> {
        let mut found: Vec<u32> = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut queue: Vec<&'a v1::Type> = Vec::new();

        for decl in &self.ctx.model.declarations {
            if let Some(v1::declaration::Kind::Struct(def)) = decl.kind.as_ref() {
                for slot in &def.slots {
                    if let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_ref()
                        && let Some(ty) = field.r#type.as_ref()
                    {
                        queue.push(ty);
                    }
                }
            }
        }
        let mut root_indices: Vec<u32> = roots.keys().copied().collect();
        root_indices.sort_unstable();
        for index in root_indices {
            let Some(tuple) = self.ctx.tuple(index) else {
                continue;
            };
            if !self.ctx.tuple_name(index).is_empty() && seen.insert(index) {
                found.push(index);
                for field in &tuple.fields {
                    if let Some(ty) = field.r#type.as_ref() {
                        queue.push(ty);
                    }
                }
            }
        }

        let mut at = 0;
        while at < queue.len() {
            let ty = queue[at];
            at += 1;
            match ty.kind.as_ref() {
                Some(v1::r#type::Kind::Tuple(reference)) => {
                    let Some(tuple) = self.ctx.tuple(reference.index) else {
                        continue;
                    };
                    if !self.ctx.tuple_name(reference.index).is_empty()
                        && seen.insert(reference.index)
                    {
                        found.push(reference.index);
                        for field in &tuple.fields {
                            if let Some(inner) = field.r#type.as_ref() {
                                queue.push(inner);
                            }
                        }
                    }
                }
                Some(v1::r#type::Kind::Array(array)) => {
                    if let Some(element) = array.element.as_deref() {
                        queue.push(element);
                    }
                }
                Some(v1::r#type::Kind::Map(map)) => {
                    if let Some(key) = map.key.as_deref() {
                        queue.push(key);
                    }
                    if let Some(value) = map.value.as_deref() {
                        queue.push(value);
                    }
                }
                _ => {}
            }
        }
        found
    }

    fn items(
        &self,
    ) -> Result<(Vec<TokenStream>, Vec<crate::transcode::PayloadOwner>), GenerateError> {
        let mut items: Vec<TokenStream> = Vec::new();
        let mut owners: Vec<crate::transcode::PayloadOwner> = Vec::new();
        for message in self.messages()? {
            // `None`: a tuple encoded only inside its owner, which crosses
            // the wire on its own in no encoding and gets no implementation.
            let bound: Option<u32> = match &message.source {
                Source::Declaration(index) => {
                    let decl = self.ctx.declaration(*index).ok_or_else(|| GenerateError {
                        message: "the proto3 codec names a declaration the lowered model does \
                                  not carry"
                            .to_string(),
                    })?;
                    Some(self.bound(decl)?)
                }
                Source::Tuple { root } => *root,
            };
            let owner = message.owner.as_str();
            let ty = ident(owner);
            let mut functions = vec![
                self.len_fn(&message, &ty)?,
                self.encode_fn(&message, &ty)?,
                self.verify_fn(&message)?,
                self.decode_fn(&message, &ty)?,
            ];
            if let Some(bound) = bound {
                functions.push(self.payload_impl(owner, &ty, bound));
                owners.push(crate::transcode::PayloadOwner {
                    name: owner.to_string(),
                    visibility: message.visibility,
                });
            }
            items.extend(
                functions
                    .into_iter()
                    .map(|item| allow_dead_code_if_internal(message.visibility, item)),
            );
        }
        Ok((items, owners))
    }

    /// The proto3 bound of the declaration's root message, or the refusal
    /// for a declaration whose state is not bounded.
    fn bound(&self, decl: &v1::Declaration) -> Result<u32, GenerateError> {
        let pkg = self.ctx.package_name();
        let name = declared(decl.name.as_ref());
        bound_state(decl.proto3.as_ref()).map_err(|cause| GenerateError {
            message: format!("`{pkg}.{name}` has no proto3 codec: {cause}"),
        })
    }

    // -----------------------------------------------------------------
    // Classification
    // -----------------------------------------------------------------

    /// The message of one declaration: a struct over its fields, a union
    /// over its arms, a named scalar, an enum or an enum set over its box.
    /// `None` for a constant, which projects no type.
    fn message_of(
        &self,
        decl: &v1::Declaration,
        index: u32,
    ) -> Result<Option<Message>, GenerateError> {
        let owner = declared(decl.name.as_ref());
        match decl.kind.as_ref() {
            Some(v1::declaration::Kind::Struct(def)) => {
                let mut fields: Vec<Field> = Vec::new();
                for slot in &def.slots {
                    let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_ref() else {
                        // A reserved tombstone holds its number and emits no
                        // field (typl §7.4).
                        continue;
                    };
                    let name = declared(field.name.as_ref());
                    let Some(ty) = field.r#type.as_ref() else {
                        return Err(GenerateError {
                            message: format!("`{owner}.{name}` carries no type"),
                        });
                    };
                    refuse_optional_repeated(owner, name, false, ty)?;
                    fields.push(Field {
                        number: field_number(owner, name, slot.ordinal)?,
                        name: snake_of(field.name.as_ref()).to_string(),
                        wire: self.wire(ty)?,
                        optional: ty.optional,
                        access: Access::Field(snake_of(field.name.as_ref()).to_string()),
                    });
                }
                // Field-number order, whatever the declaration order.
                fields.sort_by_key(|field| field.number);
                Ok(Some(Message {
                    owner: owner.to_string(),
                    proto: owner.to_string(),
                    source: Source::Declaration(index),
                    visibility: decl.visibility,
                    fields,
                    build: Build::Struct,
                }))
            }
            Some(v1::declaration::Kind::Union(def)) => {
                if def.arms.is_empty() {
                    return Err(GenerateError {
                        message: format!("`{owner}` declares no arm, so it encodes no value"),
                    });
                }
                let mut fields: Vec<Field> = Vec::new();
                for arm in &def.arms {
                    let name = declared(arm.name.as_ref());
                    let reference = arm.r#type.as_ref().ok_or_else(|| GenerateError {
                        message: format!("`{owner}.{name}` names no type"),
                    })?;
                    let Some((decl, arm_owner)) = self.resolve(reference) else {
                        let text = &reference.reference;
                        return Err(GenerateError {
                            message: format!(
                                "`{text}` resolves in no package of this build, so no proto3 \
                                 codec can be emitted for it"
                            ),
                        });
                    };
                    fields.push(Field {
                        number: field_number(owner, name, arm.ordinal)?,
                        name: crate::camel_of(arm.name.as_ref()).to_string(),
                        wire: self.declaration_wire(decl, arm_owner, &reference.reference)?,
                        optional: false,
                        access: Access::Whole,
                    });
                }
                fields.sort_by_key(|field| field.number);
                Ok(Some(Message {
                    owner: owner.to_string(),
                    proto: owner.to_string(),
                    source: Source::Declaration(index),
                    visibility: decl.visibility,
                    fields,
                    build: Build::Union,
                }))
            }
            Some(
                v1::declaration::Kind::Scalar(_)
                | v1::declaration::Kind::Enum(_)
                | v1::declaration::Kind::EnumSet(_),
            ) => {
                let wire = self.declaration_wire(decl, owner.to_string(), owner)?;
                Ok(Some(Message {
                    owner: owner.to_string(),
                    proto: format!("{owner}Box"),
                    source: Source::Declaration(index),
                    visibility: decl.visibility,
                    fields: vec![Field {
                        number: 1,
                        name: "value".to_string(),
                        wire,
                        optional: false,
                        access: Access::Whole,
                    }],
                    build: Build::Box,
                }))
            }
            // A constant projects no type.
            _ => Ok(None),
        }
    }

    /// The message of one induced tuple: positional fields numbered from 1,
    /// as `emit_induced_tuple` of the proto backend numbers them, named in
    /// Rust by the tuple field's own name. A tuple the lowering left without
    /// a Rust name is an error: nothing could name its functions.
    fn tuple_message(
        &self,
        induced: &v1::InducedTuple,
        root: Option<u32>,
    ) -> Result<Message, GenerateError> {
        let owner = induced
            .name
            .as_ref()
            .map(|name| name.rust.as_str())
            .filter(|rust| !rust.is_empty())
            .ok_or_else(|| GenerateError {
                message: format!(
                    "an induced tuple of `{}` carries no Rust name, so the proto3 codec cannot \
                     name its functions",
                    self.ctx.package_name()
                ),
            })?;
        let proto = induced
            .name
            .as_ref()
            .map(|name| name.wire.as_str())
            .filter(|wire| !wire.is_empty())
            .unwrap_or(owner);
        let mut fields: Vec<Field> = Vec::new();
        for (index, field) in induced.fields.iter().enumerate() {
            let name = declared(field.name.as_ref());
            let Some(ty) = field.r#type.as_ref() else {
                return Err(GenerateError {
                    message: format!("`{owner}.{name}` carries no type"),
                });
            };
            let number = u32::try_from(index + 1).map_err(|_| GenerateError {
                message: format!(
                    "`{owner}` has more tuple fields than a proto3 field number holds"
                ),
            })?;
            refuse_optional_repeated(owner, name, true, ty)?;
            fields.push(Field {
                number: field_number(owner, name, number)?,
                name: snake_of(field.name.as_ref()).to_string(),
                wire: self.wire(ty)?,
                optional: ty.optional,
                access: Access::Field(snake_of(field.name.as_ref()).to_string()),
            });
        }
        Ok(Message {
            owner: owner.to_string(),
            proto: proto.to_string(),
            source: Source::Tuple { root },
            visibility: induced.visibility,
            fields,
            build: Build::Struct,
        })
    }

    /// The wire shape of one type position.
    fn wire(&self, ty: &v1::Type) -> Result<Wire, GenerateError> {
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Primitive(primitive)) => {
                match v1::PrimitiveType::try_from(*primitive).ok() {
                    Some(v1::PrimitiveType::Boolean) => Ok(Wire::Scalar(Scalar {
                        scalar: pb::Scalar::Bool,
                        repr: Repr::Bool,
                        constraint: None,
                        zero_legal: true,
                    })),
                    Some(v1::PrimitiveType::Integer) => Ok(Wire::Scalar(Scalar {
                        scalar: pb::primitive(*primitive),
                        repr: Repr::Int,
                        constraint: None,
                        zero_legal: true,
                    })),
                    Some(v1::PrimitiveType::Float) => Ok(Wire::Scalar(Scalar {
                        scalar: pb::primitive(*primitive),
                        repr: Repr::Float,
                        constraint: None,
                        zero_legal: true,
                    })),
                    // A bare `string` or `bytes` carries no length bound, so
                    // it has no finite bound (typl §15.3 keeps it out of a
                    // field position anyway).
                    _ => Err(GenerateError {
                        message: "a proto3 codec cannot carry an unbounded primitive".to_string(),
                    }),
                }
            }
            Some(v1::r#type::Kind::Inline(sc)) => self.scalar_wire(sc, None),
            Some(v1::r#type::Kind::Named(reference)) => {
                let Some((decl, owner)) = self.resolve(reference) else {
                    let text = &reference.reference;
                    return Err(GenerateError {
                        message: format!(
                            "`{text}` resolves in no package of this build, so no proto3 codec \
                             can be emitted for it"
                        ),
                    });
                };
                self.declaration_wire(decl, owner, &reference.reference)
            }
            // A tuple generates a named struct, encoded as a nested message
            // (typl §11). The lowering named it twice: for Rust and for the
            // wire.
            Some(v1::r#type::Kind::Tuple(reference)) => {
                let induced = self
                    .ctx
                    .tuple(reference.index)
                    .ok_or_else(|| GenerateError {
                        message: "the lowered model names a tuple it does not carry".to_string(),
                    })?;
                let owner = self.ctx.tuple_name(reference.index);
                let proto = induced
                    .name
                    .as_ref()
                    .map(|name| name.wire.as_str())
                    .filter(|wire| !wire.is_empty())
                    .unwrap_or(owner);
                Ok(Wire::Message {
                    owner: owner.to_string(),
                    proto: proto.to_string(),
                })
            }
            Some(v1::r#type::Kind::Array(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an array carries no element type".to_string(),
                })?;
                refuse_optional(element, "an array element")?;
                let element = self.wire(element)?;
                if element.repeated() {
                    return Err(GenerateError {
                        message: "an array of arrays or of maps is one proto3 does not admit, \
                                  as the proto backend refuses it"
                            .to_string(),
                    });
                }
                Ok(Wire::Array {
                    element: Box::new(element),
                    min: array.min,
                    max: array.max,
                })
            }
            Some(v1::r#type::Kind::Map(map)) => {
                let key = map.key.as_deref().ok_or_else(|| GenerateError {
                    message: "a map carries no key type".to_string(),
                })?;
                let value = map.value.as_deref().ok_or_else(|| GenerateError {
                    message: "a map carries no value type".to_string(),
                })?;
                // Defence in depth: `ridl check` already refuses an optional
                // map key (TYPL-209), so no checked source reaches this.
                refuse_optional(key, "a map key")?;
                refuse_optional(value, "a map value")?;
                let key = self.wire(key)?;
                let admitted = match &key {
                    Wire::Scalar(scalar) => scalar.scalar.admitted_as_map_key(),
                    Wire::Text(..) => true,
                    _ => false,
                };
                if !admitted {
                    return Err(GenerateError {
                        message: "a map key must be an integral or string type, as the proto \
                                  backend requires"
                            .to_string(),
                    });
                }
                let value = self.wire(value)?;
                if value.repeated() {
                    return Err(GenerateError {
                        message: "a map whose value is an array or a map is one proto3 does \
                                  not admit, as the proto backend refuses it"
                            .to_string(),
                    });
                }
                Ok(Wire::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                    min: map.min,
                    max: map.max,
                })
            }
            _ => Err(GenerateError {
                message: "a proto3 codec cannot carry this type position".to_string(),
            }),
        }
    }

    /// The wire shape of one resolved reference, or of the declaration being
    /// boxed: the declaration decides.
    fn declaration_wire(
        &self,
        decl: &v1::Declaration,
        owner: String,
        reference: &str,
    ) -> Result<Wire, GenerateError> {
        let name = declared(decl.name.as_ref());
        match decl.kind.as_ref() {
            Some(v1::declaration::Kind::Scalar(sc)) => self.scalar_wire(
                sc,
                Some(NamedScalar {
                    name: owner,
                    ctor: if sc.vacuous { "new" } else { "new_unchecked" },
                }),
            ),
            Some(v1::declaration::Kind::Enum(def)) => {
                let Some(first) = def.values.first() else {
                    return Err(GenerateError {
                        message: format!(
                            "`{name}` declares no value, so no proto3 codec can decode one"
                        ),
                    });
                };
                // proto3 enum and reserved values are int32; the proto
                // backend refuses a value outside it, so the codec cannot
                // carry one either.
                let outside = def
                    .values
                    .iter()
                    .map(|value| (declared(value.name.as_ref()), value.value))
                    .chain(def.retired.iter().filter_map(|retired| {
                        retired
                            .value
                            .map(|value| (declared(retired.name.as_ref()), value))
                    }))
                    .find(|(_, value)| i32::try_from(*value).is_err());
                if let Some((member, value)) = outside {
                    let full = if owner.contains('.') {
                        owner.clone()
                    } else {
                        format!("{}.{owner}", self.ctx.package_name())
                    };
                    return Err(GenerateError {
                        message: format!(
                            "`{full}` has no proto3 codec: its value `{member}` is {value}, \
                             outside int32, which the proto backend refuses"
                        ),
                    });
                }
                let zero = def
                    .zero_member
                    .and_then(|index| def.values.get(index as usize))
                    .map(|zero| pascal_of(zero.name.as_ref()));
                Ok(Wire::Scalar(Scalar {
                    scalar: pb::Scalar::Int64,
                    zero_legal: zero.is_some(),
                    repr: Repr::Enum {
                        name: owner,
                        first: pascal_of(first.name.as_ref()),
                        zero,
                    },
                    constraint: None,
                }))
            }
            Some(v1::declaration::Kind::EnumSet(def)) => {
                let scalar = v1::IntWidth::try_from(def.width)
                    .ok()
                    .and_then(pb::int_width_scalar)
                    .ok_or_else(|| GenerateError {
                        message: format!("`{name}` carries no integer width"),
                    })?;
                Ok(Wire::Scalar(Scalar {
                    scalar,
                    repr: Repr::EnumSet { name: owner },
                    constraint: None,
                    zero_legal: true,
                }))
            }
            Some(v1::declaration::Kind::Struct(_) | v1::declaration::Kind::Union(_)) => {
                Ok(Wire::Message {
                    proto: owner.clone(),
                    owner,
                })
            }
            _ => Err(GenerateError {
                message: format!("`{reference}` names a declaration a proto3 codec cannot carry"),
            }),
        }
    }

    fn scalar_wire(
        &self,
        sc: &v1::Scalar,
        named: Option<NamedScalar>,
    ) -> Result<Wire, GenerateError> {
        let backing = class_backing(sc.class);
        let repr = |named: Option<NamedScalar>| match named {
            Some(named) => Repr::Named(named),
            None => match backing {
                ScalarBacking::Boolean => Repr::Bool,
                ScalarBacking::Integer => Repr::Int,
                _ => Repr::Float,
            },
        };
        match &sc.width {
            Some(v1::scalar::Width::IntWidth(width)) => {
                let scalar = v1::IntWidth::try_from(*width)
                    .ok()
                    .and_then(pb::int_width_scalar)
                    .ok_or_else(|| GenerateError {
                        message: "a scalar carries no integer width".to_string(),
                    })?;
                Ok(Wire::Scalar(Scalar {
                    scalar,
                    repr: repr(named),
                    constraint: Some(sc.clone()),
                    zero_legal: zero_is_legal(sc.constraint.as_ref()),
                }))
            }
            Some(v1::scalar::Width::FloatWidth(width)) => {
                let scalar = v1::FloatWidth::try_from(*width)
                    .ok()
                    .and_then(pb::float_width_scalar)
                    .ok_or_else(|| GenerateError {
                        message: "a scalar carries no float width".to_string(),
                    })?;
                Ok(Wire::Scalar(Scalar {
                    scalar,
                    repr: repr(named),
                    constraint: Some(sc.clone()),
                    zero_legal: zero_is_legal(sc.constraint.as_ref()),
                }))
            }
            None => match backing {
                ScalarBacking::Boolean => Ok(Wire::Scalar(Scalar {
                    scalar: pb::Scalar::Bool,
                    repr: repr(named),
                    constraint: Some(sc.clone()),
                    zero_legal: zero_is_legal(sc.constraint.as_ref()),
                })),
                ScalarBacking::String => Ok(Wire::Text(named, Some(sc.clone()))),
                ScalarBacking::Bytes => Ok(Wire::Bytes(named, Some(sc.clone()))),
                // The compiler derives a width for every numeric scalar, so
                // this is IR handed in directly; the `.proto` emitter's
                // `string` fallback is not followed.
                ScalarBacking::Float | ScalarBacking::Integer => Err(GenerateError {
                    message: "a numeric scalar carries no width".to_string(),
                }),
            },
        }
    }

    // -----------------------------------------------------------------
    // len and encode
    // -----------------------------------------------------------------

    fn len_fn(&self, message: &Message, ty: &Ident) -> Result<TokenStream, GenerateError> {
        let name = len_ident(&message.owner);
        let used = message.fields.iter().any(Field::has_statement);
        let value = param_binding("value", used);
        let body = if message.build == Build::Union {
            let arms = self.union_arms(message, ty, Pass::Len)?;
            quote! {
                let mut __n = 0usize;
                match value {
                    #(#arms)*
                }
                __n
            }
        } else if used {
            let mut counts: Vec<TokenStream> = Vec::new();
            for field in &message.fields {
                counts.push(self.field_statement(&message.owner, field, Pass::Len)?);
            }
            quote! {
                let mut __n = 0usize;
                #(#counts)*
                __n
            }
        } else {
            quote! { 0usize }
        };
        // The exact encoded length of the value. An internal: no doc.
        Ok(quote! {
            #[allow(deprecated, non_snake_case)]
            pub(crate) fn #name(#value: &#ty) -> ::core::primitive::usize {
                #body
            }
        })
    }

    fn encode_fn(&self, message: &Message, ty: &Ident) -> Result<TokenStream, GenerateError> {
        let name = encode_ident(&message.owner);
        let used = message.fields.iter().any(Field::has_statement);
        let value = param_binding("value", used);
        let writer = param_binding("writer", used);
        let body = if message.build == Build::Union {
            let arms = self.union_arms(message, ty, Pass::Encode)?;
            quote! {
                match value {
                    #(#arms)*
                }
            }
        } else {
            let mut writes: Vec<TokenStream> = Vec::new();
            for field in &message.fields {
                writes.push(self.field_statement(&message.owner, field, Pass::Encode)?);
            }
            quote! { #(#writes)* }
        };
        // Writes the value's fields in number order. The caller has sized the
        // value with the length function and checked the capacity, so no
        // write fails for lack of room; what can fail is an anonymous length
        // or a collection count outside its bound, or a map key held twice,
        // which is `EncodeError::Contract`. An internal: no doc.
        Ok(quote! {
            #[allow(deprecated, non_snake_case)]
            pub(crate) fn #name(
                #value: &#ty,
                #writer: &mut ::ridl_rt::proto3::Writer<'_>,
            ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
                #body
                ::core::result::Result::Ok(())
            }
        })
    }

    /// One `match` arm per union arm, over `value: &Ty`: the arm's record is
    /// always written, even at the arm's default.
    fn union_arms(
        &self,
        message: &Message,
        ty: &Ident,
        pass: Pass,
    ) -> Result<Vec<TokenStream>, GenerateError> {
        let mut arms: Vec<TokenStream> = Vec::new();
        for field in &message.fields {
            let variant = ident(&field.name);
            let statement = self.field_value(
                &message.owner,
                field,
                &Operand::borrowed(quote! { __a }),
                pass,
                Site::Element,
            )?;
            arms.push(quote! { #ty::#variant(__a) => { #statement } });
        }
        Ok(arms)
    }

    /// The statement that counts (`Pass::Len`) or writes (`Pass::Encode`)
    /// one field of `value`.
    fn field_statement(
        &self,
        owner: &str,
        field: &Field,
        pass: Pass,
    ) -> Result<TokenStream, GenerateError> {
        let operand = field.operand(&quote! { value });
        if field.optional {
            // Written when present, whatever its value.
            let place = &operand.place;
            let inner = self.field_value(
                owner,
                field,
                &Operand::borrowed(quote! { __v }),
                pass,
                Site::Element,
            )?;
            return Ok(quote! {
                if let ::core::option::Option::Some(__v) = &#place {
                    #inner
                }
            });
        }
        self.field_value(owner, field, &operand, pass, Site::Field)
    }

    /// The statements that count or write one present value at `site`.
    fn field_value(
        &self,
        owner: &str,
        field: &Field,
        operand: &Operand,
        pass: Pass,
        site: Site,
    ) -> Result<TokenStream, GenerateError> {
        let tag_len = Literal::usize_suffixed(tag_len(field.number));
        let number = Literal::u32_suffixed(field.number);
        let wire_type = wire_type_tokens(field.wire.wire_type());
        let place = &operand.place;
        let reference = &operand.reference;
        let receiver = operand.receiver();
        // At a struct field, an implicit-presence value is tested against
        // its default only when that default is a legal value, since
        // otherwise it never holds it; inside a map entry it is always
        // tested, as every proto3 writer does; an element or an arm is
        // written whatever its value.
        let test = match site {
            Site::Field => field.wire.default_is_legal(),
            Site::Entry => true,
            Site::Element => false,
        };
        Ok(match &field.wire {
            Wire::Scalar(scalar) => {
                let raw = scalar.raw(scalar.language(quote! { __s }));
                let value_len = scalar.value_len();
                let action = match pass {
                    Pass::Len => quote! { __n = __n.saturating_add(#tag_len + #value_len); },
                    Pass::Encode => {
                        let write = scalar.write();
                        quote! {
                            writer.tag(#number, #wire_type);
                            #write
                        }
                    }
                };
                let action = if test {
                    quote! { if __r != 0 { #action } }
                } else {
                    action
                };
                // The value is bound before it is converted, so that a place
                // like `*__v` never has a method call attached to it.
                quote! {
                    {
                        let __s = #place;
                        let __r = #raw;
                        #action
                    }
                }
            }
            Wire::Text(named, constraint) | Wire::Bytes(named, constraint) => {
                let text = matches!(field.wire, Wire::Text(..));
                let value = match (named, text) {
                    (Some(_), _) => quote! { #receiver.get() },
                    (None, true) => quote! { #receiver.as_str() },
                    (None, false) => quote! { #receiver.as_slice() },
                };
                let bytes = if text {
                    quote! { __s.as_bytes() }
                } else {
                    quote! { __s }
                };
                let action = match pass {
                    Pass::Len => quote! {
                        __n = __n
                            .saturating_add(#tag_len)
                            .saturating_add(::ridl_rt::proto3::varint_len(
                                __s.len() as ::core::primitive::u64,
                            ))
                            .saturating_add(__s.len());
                    },
                    Pass::Encode => {
                        let check =
                            inline_length_check(owner, named.is_none(), constraint.as_ref(), text);
                        quote! {
                            #check
                            writer.tag(#number, #wire_type);
                            writer.varint(__s.len() as ::core::primitive::u64);
                            writer.bytes(#bytes);
                        }
                    }
                };
                let action = if test {
                    quote! { if !__s.is_empty() { #action } }
                } else {
                    action
                };
                quote! {
                    {
                        let __s = #value;
                        #action
                    }
                }
            }
            Wire::Message { owner: name, .. } => {
                let len = owner_path(name, len_ident);
                let action = match pass {
                    Pass::Len => quote! {
                        __n = __n
                            .saturating_add(#tag_len)
                            .saturating_add(::ridl_rt::proto3::varint_len(
                                __l as ::core::primitive::u64,
                            ))
                            .saturating_add(__l);
                    },
                    Pass::Encode => {
                        let encode = owner_path(name, encode_ident);
                        quote! {
                            writer.tag(#number, #wire_type);
                            writer.varint(__l as ::core::primitive::u64);
                            #encode(#reference, writer)?;
                        }
                    }
                };
                // A nested message at a field is always written, as a record
                // that may be empty; inside a map entry it is omitted when
                // empty.
                let action = if site == Site::Entry {
                    quote! { if __l != 0usize { #action } }
                } else {
                    action
                };
                quote! {
                    {
                        let __l = #len(#reference);
                        #action
                    }
                }
            }
            Wire::Array { element, min, max } => {
                self.array_statement(owner, field, element, *min, *max, reference, pass)?
            }
            Wire::Map {
                key,
                value,
                min,
                max,
            } => self.map_statement(owner, field, key, value, *min, *max, reference, pass)?,
        })
    }

    /// The statement that counts or writes one array, bound to `__c`: a
    /// scalar element makes one packed record, any other element one record
    /// per element, and an empty array is not written. A fixed-length array
    /// has nothing to count and no emptiness to test, because its Rust type
    /// is `[T; N]`.
    #[allow(clippy::too_many_arguments)]
    fn array_statement(
        &self,
        owner: &str,
        field: &Field,
        element: &Wire,
        min: u64,
        max: u64,
        reference: &TokenStream,
        pass: Pass,
    ) -> Result<TokenStream, GenerateError> {
        let fixed = min == max;
        if fixed && max == 0 {
            return Ok(quote! {});
        }
        let tag_len = Literal::usize_suffixed(tag_len(field.number));
        let number = Literal::u32_suffixed(field.number);
        let count = match pass {
            Pass::Encode if !fixed => encode_count_check(owner, min, max, &quote! { __c }),
            _ => quote! {},
        };
        let body = match element {
            Wire::Scalar(scalar) => {
                let raw = scalar.raw(scalar.language(quote! { __s }));
                let write = scalar.write();
                let run_len = match scalar.fixed_width() {
                    Some(width) => {
                        let width = Literal::usize_suffixed(width);
                        quote! { let __l = __c.len().saturating_mul(#width); }
                    }
                    None => quote! {
                        let mut __l = 0usize;
                        for __e in __c.iter() {
                            let __s = *__e;
                            let __r = #raw;
                            __l = __l.saturating_add(::ridl_rt::proto3::varint_len(__r));
                        }
                    },
                };
                let action = match pass {
                    Pass::Len => quote! {
                        __n = __n
                            .saturating_add(#tag_len)
                            .saturating_add(::ridl_rt::proto3::varint_len(
                                __l as ::core::primitive::u64,
                            ))
                            .saturating_add(__l);
                    },
                    Pass::Encode => quote! {
                        writer.tag(#number, ::ridl_rt::proto3::WireType::Len);
                        writer.varint(__l as ::core::primitive::u64);
                        for __e in __c.iter() {
                            let __s = *__e;
                            let __r = #raw;
                            #write
                        }
                    },
                };
                let packed = quote! {
                    #run_len
                    #action
                };
                if fixed {
                    packed
                } else {
                    quote! { if !__c.is_empty() { #packed } }
                }
            }
            other => {
                let element = Field::element(field.number, other.clone());
                let statement = self.field_value(
                    owner,
                    &element,
                    &Operand::borrowed(quote! { __e }),
                    pass,
                    Site::Element,
                )?;
                quote! {
                    for __e in __c.iter() {
                        #statement
                    }
                }
            }
        };
        Ok(quote! {
            {
                let __c = #reference;
                #count
                #body
            }
        })
    }

    /// The statement that counts or writes one map, bound to `__c`: one
    /// entry message per pair, in the order of the `Vec`, with the key at 1
    /// and the value at 2 under the entry rule. `encode` checks the count
    /// and the uniqueness of the keys first, as the FlatBuffers encoder
    /// does.
    #[allow(clippy::too_many_arguments)]
    fn map_statement(
        &self,
        owner: &str,
        field: &Field,
        key: &Wire,
        value: &Wire,
        min: u64,
        max: u64,
        reference: &TokenStream,
        pass: Pass,
    ) -> Result<TokenStream, GenerateError> {
        let tag_len = Literal::usize_suffixed(tag_len(field.number));
        let number = Literal::u32_suffixed(field.number);
        let [key_field, value_field] = entry_fields(key, value);
        let base = quote! { __e };
        let key_len = self.field_value(
            owner,
            &key_field,
            &key_field.operand(&base),
            Pass::Len,
            Site::Entry,
        )?;
        let value_len = self.field_value(
            owner,
            &value_field,
            &value_field.operand(&base),
            Pass::Len,
            Site::Entry,
        )?;
        let entry_len = quote! {
            let __l = {
                let mut __n = 0usize;
                #key_len
                #value_len
                __n
            };
        };
        let (checks, action) = match pass {
            Pass::Len => (
                quote! {},
                quote! {
                    __n = __n
                        .saturating_add(#tag_len)
                        .saturating_add(::ridl_rt::proto3::varint_len(
                            __l as ::core::primitive::u64,
                        ))
                        .saturating_add(__l);
                },
            ),
            Pass::Encode => {
                let count = encode_count_check(owner, min, max, &quote! { __c });
                let unique = encode_unique_error(owner);
                let key_encode = self.field_value(
                    owner,
                    &key_field,
                    &key_field.operand(&base),
                    Pass::Encode,
                    Site::Entry,
                )?;
                let value_encode = self.field_value(
                    owner,
                    &value_field,
                    &value_field.operand(&base),
                    Pass::Encode,
                    Site::Entry,
                )?;
                (
                    quote! {
                        #count
                        for (__i, __e) in __c.iter().enumerate() {
                            for __p in __c[..__i].iter() {
                                if __p.0 == __e.0 {
                                    #unique
                                }
                            }
                        }
                    },
                    quote! {
                        writer.tag(#number, ::ridl_rt::proto3::WireType::Len);
                        writer.varint(__l as ::core::primitive::u64);
                        #key_encode
                        #value_encode
                    },
                )
            }
        };
        Ok(quote! {
            {
                let __c = #reference;
                #checks
                for __e in __c.iter() {
                    #entry_len
                    #action
                }
            }
        })
    }

    // -----------------------------------------------------------------
    // verify
    // -----------------------------------------------------------------

    fn verify_fn(&self, message: &Message) -> Result<TokenStream, GenerateError> {
        let name = verify_ident(&message.owner);
        let walk = match message.build {
            Build::Union => self.union_verify_walk(message)?,
            Build::Struct | Build::Box => {
                self.message_walk(&message.owner, &message.fields, &quote! { buf })?
            }
        };
        // Checks the proto3 message in `buf` against the declaration's
        // shape: the structure, each known field's wire type, a singular
        // field written once, a union holding one arm, enum discriminants,
        // named and anonymous scalar ranges, steps and lengths, collection
        // counts, map key uniqueness, and the absence rule. Pattern checks
        // require `validate-pattern`. An internal: no doc.
        Ok(quote! {
            #[allow(deprecated, non_snake_case, clippy::collapsible_if)]
            pub(crate) fn #name(
                buf: &[::core::primitive::u8],
            ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                #walk
                ::core::result::Result::Ok(())
            }
        })
    }

    /// The walk of one message's bytes `buf` over `fields`: each known field
    /// is checked by number, a singular one refused when written twice, a
    /// repeated one counted, an unknown one skipped; then the absence rule
    /// and the count bounds. The same walk serves a struct, a tuple, a box
    /// and a map entry, so it binds `__seen`, `__reader` and `__field`,
    /// which an entry walk shadows inside its own block.
    fn message_walk(
        &self,
        owner: &str,
        fields: &[Field],
        buf: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        if fields.is_empty() {
            // No field to match: the walk checks the message's structure and
            // nothing else.
            return Ok(quote! {
                let mut __reader = ::ridl_rt::proto3::Reader::new(#buf);
                while let ::core::option::Option::Some(__field) = __reader.next() {
                    __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                }
            });
        }
        let count = Literal::usize_suffixed(fields.len());
        let mut counters: Vec<TokenStream> = Vec::new();
        let mut arms: Vec<(Literal, TokenStream)> = Vec::new();
        let mut after: Vec<TokenStream> = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            let number = Literal::u32_suffixed(field.number);
            let seen = Literal::usize_suffixed(index);
            let counter = count_ident(index);
            if field.wire.repeated() {
                counters.push(quote! { let mut #counter = 0usize; });
            }
            let body = self.verify_arm(owner, field, index, buf)?;
            arms.push((number, body));
            let when_absent = self.verify_absent(owner, field);
            if !when_absent.is_empty() {
                after.push(quote! {
                    if !__seen[#seen] {
                        #when_absent
                    }
                });
            }
            match &field.wire {
                Wire::Array { min, max, .. } | Wire::Map { min, max, .. } if !field.optional => {
                    after.push(counter_check(owner, *min, *max, &counter));
                }
                _ => {}
            }
        }
        let dispatch = number_dispatch(&arms);
        Ok(quote! {
            let mut __seen = [false; #count];
            #(#counters)*
            let mut __reader = ::ridl_rt::proto3::Reader::new(#buf);
            while let ::core::option::Option::Some(__field) = __reader.next() {
                let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                #dispatch
            }
            #(#after)*
        })
    }

    /// The body of the `match` arm for one known field at `index` of the
    /// walk over `buf`.
    fn verify_arm(
        &self,
        owner: &str,
        field: &Field,
        index: usize,
        buf: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        let seen = Literal::usize_suffixed(index);
        let counter = count_ident(index);
        let wrong_type = structure_error("WireType");
        let duplicate = structure_error("Duplicate");
        Ok(match &field.wire {
            Wire::Array { element, .. } => {
                let run = match element.as_ref() {
                    Wire::Scalar(scalar) => self.packed_run_verify(owner, scalar, &counter),
                    other => {
                        let (pattern, checks) = self.present_checks(owner, other)?;
                        quote! {
                            #pattern => {
                                #checks
                                #counter += 1usize;
                            }
                        }
                    }
                };
                quote! {
                    __seen[#seen] = true;
                    match __field.value {
                        #run
                        _ => { #wrong_type }
                    }
                }
            }
            Wire::Map {
                key, value, max, ..
            } => {
                let entry =
                    self.map_entry_verify(owner, field.number, key, value, *max, &counter, buf)?;
                quote! {
                    __seen[#seen] = true;
                    match __field.value {
                        ::ridl_rt::proto3::Value::Len(__b) => { #entry }
                        _ => { #wrong_type }
                    }
                }
            }
            _ => {
                let (pattern, checks) = self.present_checks(owner, &field.wire)?;
                quote! {
                    if __seen[#seen] {
                        #duplicate
                    }
                    __seen[#seen] = true;
                    match __field.value {
                        #pattern => { #checks }
                        _ => { #wrong_type }
                    }
                }
            }
        })
    }

    /// The two `match` arms over `__field.value` that verify one record of
    /// a repeated scalar field, counting its elements in `counter`: a packed
    /// run, read element by element, and one unpacked element.
    fn packed_run_verify(&self, owner: &str, scalar: &Scalar, counter: &Ident) -> TokenStream {
        let checks = scalar.raw_checks(owner);
        let variant = scalar.value_variant();
        let single = if checks.is_empty() {
            quote! {
                ::ridl_rt::proto3::Value::#variant(_) => { #counter += 1usize; }
            }
        } else {
            quote! {
                ::ridl_rt::proto3::Value::#variant(__raw) => {
                    #checks
                    #counter += 1usize;
                }
            }
        };
        let packed = match scalar.fixed_width() {
            Some(width) => {
                let width = Literal::usize_suffixed(width);
                let out_of_bounds = structure_error("OutOfBounds");
                let read = scalar.fixed_read();
                let elements = if checks.is_empty() {
                    quote! { #counter += __run.len() / #width; }
                } else {
                    quote! {
                        for __chunk in __run.chunks(#width) {
                            #read
                            #checks
                            #counter += 1usize;
                        }
                    }
                };
                // A run whose length is not a whole number of elements is
                // refused before any element is read. `chunks` and its last
                // chunk rather than `chunks_exact` and its remainder, and no
                // `%`: clippy proposes `as_chunks` and `is_multiple_of` for
                // those, which Rust before 1.88 does not have (the form was
                // chosen when the minimum was 1.83).
                quote! {
                    if __run
                        .chunks(#width)
                        .next_back()
                        .is_some_and(|__last| __last.len() != #width)
                    {
                        #out_of_bounds
                    }
                    #elements
                }
            }
            None => {
                let read = if checks.is_empty() {
                    quote! {
                        ::ridl_rt::proto3::read_varint(__run, &mut __at)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    }
                } else {
                    quote! {
                        let __raw = ::ridl_rt::proto3::read_varint(__run, &mut __at)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #checks
                    }
                };
                quote! {
                    let mut __at = 0usize;
                    while __at < __run.len() {
                        #read
                        #counter += 1usize;
                    }
                }
            }
        };
        quote! {
            ::ridl_rt::proto3::Value::Len(__run) => { #packed }
            #single
        }
    }

    /// The statements that verify one map entry `__b` of the field
    /// `number`, counted in `counter`: the entry's own walk, the count
    /// against the declared maximum `max`, then its key against the keys of
    /// the entries before it in the message `buf`.
    ///
    /// The count check comes before the uniqueness walk, so an entry past
    /// the maximum is refused for its count before any earlier entry is
    /// re-read: the uniqueness walk compares at most `max` earlier entries
    /// per entry. Each re-read starts at the message's first byte and
    /// passes over every field before those entries, unknown fields
    /// included, so the walk costs `max` times the fields before the
    /// entries, per entry, and is quadratic in the declared maximum.
    #[allow(clippy::too_many_arguments)]
    fn map_entry_verify(
        &self,
        owner: &str,
        number: u32,
        key: &Wire,
        value: &Wire,
        max: u64,
        counter: &Ident,
        buf: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        let number = Literal::u32_suffixed(number);
        let entry_walk = self.message_walk(owner, &entry_fields(key, value), &quote! { __b })?;
        let over_count = match usize::try_from(max) {
            Ok(max) => {
                let max = Literal::usize_suffixed(max);
                let length = length_error(owner);
                quote! {
                    if #counter >= #max {
                        #length
                    }
                }
            }
            Err(_) => quote! {},
        };
        let key_of_this = self.key_of(key, &quote! { __b });
        let key_of_previous = self.key_of(key, &quote! { __pb });
        let unique = unique_error(owner);
        Ok(quote! {
            { #entry_walk }
            #over_count
            // The entries before this one have passed; their keys are read
            // again rather than kept, so no index is allocated from an
            // untrusted count.
            let __k = #key_of_this;
            let mut __left = #counter;
            let mut __prev = ::ridl_rt::proto3::Reader::new(#buf);
            while __left > 0usize {
                match __prev.next() {
                    ::core::option::Option::Some(::core::result::Result::Ok(__p)) => {
                        if __p.number == #number {
                            __left -= 1usize;
                            if let ::ridl_rt::proto3::Value::Len(__pb) = __p.value {
                                let __pk = #key_of_previous;
                                if __pk == __k {
                                    #unique
                                }
                            }
                        }
                    }
                    _ => break,
                }
            }
            #counter += 1usize;
        })
    }

    /// The expression reading a map entry's key from the entry bytes
    /// `bytes`, as a value two entries can be compared by: the language
    /// value of a scalar key, the bytes of a string key, the key's default
    /// when the entry omits it.
    fn key_of(&self, key: &Wire, bytes: &TokenStream) -> TokenStream {
        let (init, pattern, value) = match key {
            Wire::Scalar(scalar) => {
                let variant = scalar.value_variant();
                (
                    scalar.language_zero(),
                    quote! { ::ridl_rt::proto3::Value::#variant(__raw) },
                    scalar.widen_total(),
                )
            }
            _ => (
                quote! { &[] as &[::core::primitive::u8] },
                quote! { ::ridl_rt::proto3::Value::Len(__kb) },
                quote! { __kb },
            ),
        };
        quote! {
            {
                let mut __k = #init;
                let mut __reader = ::ridl_rt::proto3::Reader::new(#bytes);
                while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) =
                    __reader.next()
                {
                    if __field.number == 1u32 {
                        if let #pattern = __field.value {
                            __k = #value;
                        }
                    }
                }
                __k
            }
        }
    }

    /// The walk of a union's bytes: exactly one arm, written once.
    fn union_verify_walk(&self, message: &Message) -> Result<TokenStream, GenerateError> {
        let mut arms: Vec<(Literal, TokenStream)> = Vec::new();
        let duplicate = structure_error("Duplicate");
        let wrong_type = structure_error("WireType");
        for field in &message.fields {
            let number = Literal::u32_suffixed(field.number);
            let (pattern, checks) = self.present_checks(&message.owner, &field.wire)?;
            arms.push((
                number,
                quote! {
                        if __arm {
                            #duplicate
                        }
                        __arm = true;
                        match __field.value {
                            #pattern => { #checks }
                            _ => { #wrong_type }
                        }
                },
            ));
        }
        let missing = missing_required();
        let dispatch = number_dispatch(&arms);
        Ok(quote! {
            let mut __arm = false;
            let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
            while let ::core::option::Option::Some(__field) = __reader.next() {
                let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                #dispatch
            }
            // A union holds exactly one arm: no arm is no value.
            if !__arm {
                #missing
            }
        })
    }

    /// The pattern over `__field.value` that one present singular value
    /// takes — the wire type its type takes — and the statements that check
    /// it.
    fn present_checks(
        &self,
        owner: &str,
        wire: &Wire,
    ) -> Result<(TokenStream, TokenStream), GenerateError> {
        Ok(match wire {
            Wire::Scalar(scalar) => {
                let variant = scalar.value_variant();
                let body = scalar.raw_checks(owner);
                if body.is_empty() {
                    (quote! { ::ridl_rt::proto3::Value::#variant(_) }, body)
                } else {
                    (quote! { ::ridl_rt::proto3::Value::#variant(__raw) }, body)
                }
            }
            Wire::Text(named, constraint) => {
                let check = named
                    .as_ref()
                    .and_then(|named| named_scalar_check(named, quote! { __s }))
                    .or_else(|| {
                        constraint
                            .as_ref()
                            .filter(|_| named.is_none())
                            .map(|sc| scalar_constraint_check(sc, owner, quote! { __s }, false))
                    });
                // The UTF-8 check is emitted once, whether or not a
                // constraint check follows it. The checker gives every
                // string a length bound (a bare `string` field is refused,
                // TYPL-208, and a bare `type X : string` takes the default
                // bound), so `check` is `None` only for IR built by hand;
                // the binding's leading underscore keeps that case free of
                // an unused-variable lint.
                let body = quote! {
                    let __s = ::ridl_rt::proto3::utf8(__b)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    #check
                };
                (quote! { ::ridl_rt::proto3::Value::Len(__b) }, body)
            }
            Wire::Bytes(named, constraint) => {
                let check = named
                    .as_ref()
                    .and_then(|named| named_scalar_check(named, quote! { __b }))
                    .or_else(|| {
                        constraint
                            .as_ref()
                            .filter(|_| named.is_none())
                            .map(|sc| scalar_constraint_check(sc, owner, quote! { __b }, false))
                    });
                match check {
                    Some(check) => (quote! { ::ridl_rt::proto3::Value::Len(__b) }, check),
                    None => (quote! { ::ridl_rt::proto3::Value::Len(_) }, quote! {}),
                }
            }
            Wire::Message { owner: name, .. } => {
                let verify = owner_path(name, verify_ident);
                (
                    quote! { ::ridl_rt::proto3::Value::Len(__b) },
                    quote! { #verify(__b)?; },
                )
            }
            Wire::Array { .. } | Wire::Map { .. } => {
                return Err(GenerateError {
                    message: "a collection is not a singular value".to_string(),
                });
            }
        })
    }

    /// What `verify` does with an absent non-optional field: nothing when
    /// the proto3 default is a legal value of its type, `MissingRequired`
    /// when it is not, and for a nested message the verification of the
    /// empty message, which decides by the same rule over its own fields —
    /// and refuses a union, whose empty message holds no arm. A string whose
    /// empty value is legal by length but which declares a pattern has that
    /// pattern checked on the empty string, so an absent field reads exactly
    /// as a present empty one would. An absent collection is empty, legal
    /// when its count lower bound is 0.
    fn verify_absent(&self, owner: &str, field: &Field) -> TokenStream {
        if field.optional {
            return quote! {};
        }
        match &field.wire {
            Wire::Message { owner: name, .. } => {
                let verify = owner_path(name, verify_ident);
                quote! { #verify(&[])?; }
            }
            Wire::Text(named, constraint) if field.wire.default_is_legal() => {
                let has_pattern = constraint
                    .as_ref()
                    .and_then(|sc| sc.constraint.as_ref())
                    .is_some_and(|c| c.pattern.is_some() || c.pattern_const.is_some());
                if !has_pattern {
                    return quote! {};
                }
                named
                    .as_ref()
                    .and_then(|named| named_scalar_check(named, quote! { "" }))
                    .or_else(|| {
                        constraint
                            .as_ref()
                            .filter(|_| named.is_none())
                            .map(|sc| scalar_constraint_check(sc, owner, quote! { "" }, false))
                    })
                    .unwrap_or_default()
            }
            wire if wire.default_is_legal() => quote! {},
            _ => missing_required(),
        }
    }

    // -----------------------------------------------------------------
    // decode
    // -----------------------------------------------------------------

    fn decode_fn(&self, message: &Message, ty: &Ident) -> Result<TokenStream, GenerateError> {
        let name = decode_ident(&message.owner);
        let used = !message.fields.is_empty();
        let buf = param_binding("buf", used);
        if message.build == Build::Union {
            return Ok(self.union_decode_fn(message, ty, &name));
        }
        let mut inits: Vec<TokenStream> = Vec::new();
        let mut arms: Vec<(Literal, TokenStream)> = Vec::new();
        let mut builds: Vec<TokenStream> = Vec::new();
        for (index, field) in message.fields.iter().enumerate() {
            let binding = format_ident!("__f{}", index);
            let number = Literal::u32_suffixed(field.number);
            let default = if field.optional {
                quote! { ::core::option::Option::None }
            } else {
                self.decode_default(&field.wire)
            };
            inits.push(quote! { let mut #binding = #default; });
            let body = self.decode_arm(field, &binding)?;
            arms.push((number, body));
            builds.push(match &field.wire {
                // A fixed-length array is decoded into a `Vec` and moved into
                // its `[T; N]`; `verify` has checked the count, so the
                // fallback element is never reached.
                Wire::Array { element, min, max } if min == max && !field.optional => {
                    let fill = self.decode_default(element);
                    quote! {
                        {
                            let mut __it = #binding.into_iter();
                            ::core::array::from_fn(|_| __it.next().unwrap_or_else(|| #fill))
                        }
                    }
                }
                _ => quote! { #binding },
            });
        }
        let walk = if used {
            let dispatch = number_dispatch(&arms);
            quote! {
                let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
                while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) =
                    __reader.next()
                {
                    #dispatch
                }
            }
        } else {
            quote! {}
        };
        let build = match message.build {
            Build::Struct => {
                let names = message.fields.iter().map(|field| ident(&field.name));
                quote! { #ty { #(#names: #builds),* } }
            }
            Build::Box | Build::Union => {
                let build = &builds[0];
                quote! { #build }
            }
        };
        // Builds the value from the proto3 message in `buf`, which `verify`
        // accepted. An internal: no doc. It cannot fail: an absent field
        // reads as its default, a read that could fail is discharged with
        // the neutral value of its own type, and `verify` is what makes
        // those branches unreachable. A named scalar is built with its
        // unchecked constructor over a value `verify` has already checked.
        Ok(quote! {
            #[allow(deprecated, non_snake_case, clippy::collapsible_if)]
            pub(crate) fn #name(#buf: &[::core::primitive::u8]) -> #ty {
                #(#inits)*
                #walk
                #build
            }
        })
    }

    /// The decode function of a union: the first arm at its default until
    /// the bytes name an arm, which `verify` has required.
    fn union_decode_fn(&self, message: &Message, ty: &Ident, name: &Ident) -> TokenStream {
        let first = &message.fields[0];
        let first_variant = ident(&first.name);
        let first_default = self.decode_default(&first.wire);
        let mut arms: Vec<(Literal, TokenStream)> = Vec::new();
        for field in &message.fields {
            let number = Literal::u32_suffixed(field.number);
            let variant = ident(&field.name);
            let (pattern, value) = self.decode_present(&field.wire);
            arms.push((
                number,
                quote! {
                    if let #pattern = __field.value {
                        __out = #ty::#variant(#value);
                    }
                },
            ));
        }
        let dispatch = number_dispatch(&arms);
        quote! {
            #[allow(deprecated, non_snake_case, clippy::collapsible_if)]
            pub(crate) fn #name(buf: &[::core::primitive::u8]) -> #ty {
                let mut __out = #ty::#first_variant(#first_default);
                let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
                while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) =
                    __reader.next()
                {
                    #dispatch
                }
                __out
            }
        }
    }

    /// The body of the `match` arm that reads one known field into
    /// `binding`.
    fn decode_arm(&self, field: &Field, binding: &Ident) -> Result<TokenStream, GenerateError> {
        Ok(match &field.wire {
            Wire::Array { element, .. } => match element.as_ref() {
                Wire::Scalar(scalar) => {
                    let variant = scalar.value_variant();
                    let domain = scalar.domain(scalar.widen_total());
                    let packed = match scalar.fixed_width() {
                        Some(width) => {
                            let width = Literal::usize_suffixed(width);
                            let read = scalar.fixed_read();
                            // `verify` has refused a run that is not a whole
                            // number of elements, so every chunk is whole.
                            quote! {
                                for __chunk in __run.chunks(#width) {
                                    #read
                                    #binding.push(#domain);
                                }
                            }
                        }
                        None => quote! {
                            let mut __at = 0usize;
                            while __at < __run.len() {
                                match ::ridl_rt::proto3::read_varint(__run, &mut __at) {
                                    ::core::result::Result::Ok(__raw) => {
                                        #binding.push(#domain);
                                    }
                                    ::core::result::Result::Err(_) => break,
                                }
                            }
                        },
                    };
                    quote! {
                        match __field.value {
                            ::ridl_rt::proto3::Value::Len(__run) => { #packed }
                            ::ridl_rt::proto3::Value::#variant(__raw) => {
                                #binding.push(#domain);
                            }
                            _ => {}
                        }
                    }
                }
                other => {
                    let (pattern, value) = self.decode_present(other);
                    quote! {
                        if let #pattern = __field.value {
                            #binding.push(#value);
                        }
                    }
                }
            },
            Wire::Map { key, value, .. } => {
                let key_default = self.decode_default(key);
                let value_default = self.decode_default(value);
                let (key_pattern, key_value) = self.decode_present(key);
                let (value_pattern, value_value) = self.decode_present(value);
                quote! {
                    if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                        let mut __k = #key_default;
                        let mut __v = #value_default;
                        let mut __reader = ::ridl_rt::proto3::Reader::new(__b);
                        while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) =
                            __reader.next()
                        {
                            match __field.number {
                                1u32 => {
                                    if let #key_pattern = __field.value {
                                        __k = #key_value;
                                    }
                                }
                                2u32 => {
                                    if let #value_pattern = __field.value {
                                        __v = #value_value;
                                    }
                                }
                                _ => {}
                            }
                        }
                        #binding.push((__k, __v));
                    }
                }
            }
            _ => {
                let (pattern, value) = self.decode_present(&field.wire);
                let value = if field.optional {
                    quote! { ::core::option::Option::Some(#value) }
                } else {
                    value
                };
                quote! {
                    if let #pattern = __field.value {
                        #binding = #value;
                    }
                }
            }
        })
    }

    /// The domain value an absent non-optional value of `wire` decodes as.
    fn decode_default(&self, wire: &Wire) -> TokenStream {
        match wire {
            Wire::Scalar(scalar) => scalar.decode_default(),
            Wire::Text(named, _) => named_value(named, quote! { ::std::string::String::new() }),
            Wire::Bytes(named, _) => named_value(named, quote! { ::std::vec::Vec::new() }),
            Wire::Message { owner, .. } => {
                let decode = owner_path(owner, decode_ident);
                quote! { #decode(&[]) }
            }
            Wire::Array { .. } | Wire::Map { .. } => quote! { ::std::vec::Vec::new() },
        }
    }

    /// The pattern over `__field.value` and the domain value read from it,
    /// for one singular value of `wire`. A collection is read element by
    /// element by [`Codec::decode_arm`] and never reaches this.
    fn decode_present(&self, wire: &Wire) -> (TokenStream, TokenStream) {
        match wire {
            Wire::Scalar(scalar) => {
                let variant = scalar.value_variant();
                (
                    quote! { ::ridl_rt::proto3::Value::#variant(__raw) },
                    scalar.domain(scalar.widen_total()),
                )
            }
            Wire::Text(named, _) => (
                quote! { ::ridl_rt::proto3::Value::Len(__b) },
                named_value(
                    named,
                    quote! {
                        ::std::string::String::from(
                            ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                        )
                    },
                ),
            ),
            Wire::Bytes(named, _) => (
                quote! { ::ridl_rt::proto3::Value::Len(__b) },
                named_value(named, quote! { __b.to_vec() }),
            ),
            Wire::Message { owner, .. } => {
                let decode = owner_path(owner, decode_ident);
                (
                    quote! { ::ridl_rt::proto3::Value::Len(__b) },
                    quote! { #decode(__b) },
                )
            }
            Wire::Array { .. } | Wire::Map { .. } => (
                quote! { ::ridl_rt::proto3::Value::Len(_) },
                quote! { ::std::vec::Vec::new() },
            ),
        }
    }

    // -----------------------------------------------------------------
    // The Payload implementation
    // -----------------------------------------------------------------

    fn payload_impl(&self, owner: &str, ty: &Ident, bound: u32) -> TokenStream {
        let len = len_ident(owner);
        let encode = encode_ident(owner);
        let verify = verify_ident(owner);
        let decode = decode_ident(owner);
        let max = Literal::usize_suffixed(bound as usize);
        let max_doc =
            format!(" The largest proto3 message any legal `{owner}` encodes to: {bound} bytes.");
        quote! {
            #[allow(deprecated)]
            impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for #ty {
                // The bound is the proto3 size state the lowered model
                // carries for the declaration's root message, or for the
                // interaction whose payload an induced tuple is, which the
                // catalog descriptor and the interaction sizes read too, so
                // one number serves every reader. A literal, so the doc
                // states the bound and not how it is computed.
                #[doc = #max_doc]
                const MAX_SIZE: ::core::primitive::usize = #max;

                type Buffer = [::core::primitive::u8; #max];

                const BUFFER: Self::Buffer = [0u8; #max];

                type View<'a> = ::ridl_rt::proto3::Verified<'a>;

                fn encode<'o>(
                    &self,
                    out: &'o mut [::core::primitive::u8],
                ) -> ::core::result::Result<
                    ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
                    ::ridl_rt::payload::EncodeError,
                > {
                    // Sized first, so that a buffer too small is reported
                    // with the exact length needed before anything is
                    // written.
                    let __needed = #len(self);
                    ::ridl_rt::proto3::check_capacity(__needed, out)?;
                    let mut __writer = ::ridl_rt::proto3::Writer::new(out);
                    #encode(self, &mut __writer)?;
                    let bytes = __writer.finish()?;
                    ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
                        bytes,
                        view: ::ridl_rt::proto3::Verified::new(bytes),
                    })
                }

                fn verify(
                    buf: &[::core::primitive::u8],
                ) -> ::core::result::Result<
                    ::ridl_rt::proto3::Verified<'_>,
                    ::ridl_rt::payload::VerifyError,
                > {
                    #verify(buf)?;
                    ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
                }

                #[allow(clippy::collapsible_if)]
                fn decode(
                    r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
                ) -> Self {
                    #decode(::ridl_rt::payload::View::bytes(r.view()))
                }
            }
        }
    }
}

/// The induced tuples the interactions of a package send on their own, each
/// with the bound `select` reads from the interaction's payload sizes, when
/// that state is bounded. The proto3 and podbuf codecs share the walk and
/// differ in the encoding's state.
pub(crate) fn tuple_roots(
    ctx: &Ctx,
    select: fn(&v1::PayloadSizes) -> Option<&v1::SizeState>,
) -> HashMap<u32, u32> {
    let mut roots: HashMap<u32, u32> = HashMap::new();
    for interface in &ctx.model.interfaces {
        for (_, interaction) in interactions(interface) {
            let payloads: Vec<(Option<&v1::TupleRef>, Option<&v1::PayloadSizes>)> =
                match interaction.shape.as_ref() {
                    Some(v1::interaction::Shape::Command(command)) => {
                        vec![(
                            command.request_tuple.as_ref(),
                            command.request_sizes.as_ref(),
                        )]
                    }
                    Some(v1::interaction::Shape::Query(query)) => vec![
                        (query.request_tuple.as_ref(), query.request_sizes.as_ref()),
                        (query.reply_tuple.as_ref(), query.reply_sizes.as_ref()),
                    ],
                    _ => Vec::new(),
                };
            for (reference, sizes) in payloads {
                let Some(reference) = reference else {
                    continue;
                };
                let state = sizes.and_then(select);
                if let Ok(bound) = bound_state(state) {
                    roots.insert(reference.index, bound);
                }
            }
        }
    }
    roots
}

/// The bound a proto3 size state carries, or the cause of its absence as
/// the refusal states it. Only a `Bounded` state gives a codec: every
/// absent cause — `NoBound` (a member with no finite bound), `Overflow` (a
/// bound past `u32::MAX`, a sum or product that does not fit, a nesting past
/// the sizer's depth limit), `RefusedMember` (a member proto3 cannot
/// represent), `Unresolved`, `NoMessage`, `EncodingUndefined` and an
/// unspecified cause — the unbounded state, and a model that carries no
/// state are refusals, each naming its cause, so that every emitted
/// `MAX_SIZE` is finite and the size limit stays in the type.
pub(crate) fn bound_state(state: Option<&v1::SizeState>) -> Result<u32, &'static str> {
    match state.and_then(|state| state.state.as_ref()) {
        Some(v1::size_state::State::Bounded(bound)) => Ok(*bound),
        Some(v1::size_state::State::Absent(absent)) => Err(
            match v1::AbsentCause::try_from(absent.cause).unwrap_or(v1::AbsentCause::Unspecified) {
                v1::AbsentCause::NoBound => {
                    "a member of it has no finite bound (NoBound): a string or bytes with no \
                     length bound; declare the bound in the type"
                }
                v1::AbsentCause::Overflow => {
                    "its bound does not fit a u32 (Overflow): a bound past u32::MAX, a sum or \
                     product of bounds that does not fit, or a nesting past the sizer's depth \
                     limit; lower the bounds in the type"
                }
                v1::AbsentCause::RefusedMember => {
                    "a member of it is one proto3 cannot represent (RefusedMember): an optional \
                     array or map, an array of arrays or of maps, a map whose value is a \
                     collection, a map key outside the integral and string scalars, a field \
                     number proto3 reserves or exceeds, a name that does not resolve, or an \
                     enum value outside int32"
                }
                v1::AbsentCause::Unresolved => {
                    "it names a declaration the sizer did not resolve (Unresolved)"
                }
                v1::AbsentCause::NoMessage => "no proto3 message defines it (NoMessage)",
                v1::AbsentCause::EncodingUndefined => {
                    "no proto3 message defines it (EncodingUndefined)"
                }
                v1::AbsentCause::Unspecified => "no proto3 message defines it (Unspecified)",
            },
        ),
        Some(v1::size_state::State::Unbounded(_)) => Err("no finite bound exists for it"),
        None => Err("the lowered model carries no proto3 size state for it"),
    }
}

/// The field number `number` of `owner.name`, or the refusal for one proto3
/// reserves (19000 to 19999) or does not admit (0, or above 2^29 - 1), as
/// the proto backend refuses it.
fn field_number(owner: &str, name: &str, number: u32) -> Result<u32, GenerateError> {
    if number == 0 || pb::PROTO_RESERVED.contains(&number) || number > pb::PROTO_MAX_FIELD_NUMBER {
        return Err(GenerateError {
            message: format!(
                "`{owner}.{name}` has field number {number}, which proto3 reserves or does not \
                 admit, as the proto backend refuses it"
            ),
        });
    }
    Ok(number)
}

/// Refuses an optional array or map field of a struct or a tuple, as the
/// proto backend's `refuse_optional_repeated` does: proto3 cannot mark a
/// `repeated` or `map` field absent, and no value is spare to carry the fact
/// in-band, so ADR-0013 decision 7's last clause applies. Any other type
/// passes, optional or not. The sizer gives such a payload `RefusedMember`;
/// this check runs first, so the refusal names the field, for a struct and
/// for a tuple a declaration holds. A tuple only an interaction reaches is
/// not walked when its root is not bounded, so the field is never reached
/// here and the interface's note carries the sizer's cause alone.
fn refuse_optional_repeated(
    owner: &str,
    name: &str,
    tuple: bool,
    ty: &v1::Type,
) -> Result<(), GenerateError> {
    let (kind, proto_field) = match ty.kind.as_ref() {
        Some(v1::r#type::Kind::Array(_)) if ty.optional => ("array", "repeated"),
        Some(v1::r#type::Kind::Map(_)) if ty.optional => ("map", "map"),
        _ => return Ok(()),
    };
    let advice = if tuple {
        format!(
            "Remove the `?` from the tuple field, or declare a struct that holds the {kind} \
             and give the tuple field that struct's type, whose absence proto3 can track."
        )
    } else {
        format!("Remove the `?`, or wrap the {kind} in a struct, whose absence proto3 can track.")
    };
    Err(GenerateError {
        message: format!(
            "`{owner}.{name}` is an optional {kind}, and proto3 cannot mark a {proto_field} \
             field absent: an absent {kind} is indistinguishable from an empty one, and no \
             value is spare to carry the fact in-band, so this backend refuses rather than \
             choosing silently (ADR-0013 decision 7). {advice}"
        ),
    })
}

/// An `optional` marker is a field's property. A proto3 `repeated` field
/// has no absent element and a map entry no absent value, so a `?` in one
/// of those positions is refused rather than silently written as present,
/// as the FlatBuffers codec refuses it.
fn refuse_optional(ty: &v1::Type, what: &str) -> Result<(), GenerateError> {
    if ty.optional {
        return Err(GenerateError {
            message: format!(
                "{what} is optional, which proto3 cannot represent — only a field may be absent"
            ),
        });
    }
    Ok(())
}

/// Which of the two passes over a message's fields a statement serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Len,
    Encode,
}

/// `value` wrapped in the named scalar's constructor, when the position is a
/// named scalar.
fn named_value(named: &Option<NamedScalar>, value: TokenStream) -> TokenStream {
    match named {
        Some(named) => {
            let ty = type_path(&named.name);
            let ctor = format_ident!("{}", named.ctor);
            quote! { #ty::#ctor(#value) }
        }
        None => value,
    }
}
