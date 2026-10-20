//! The proto3 payload codec, emitted beside the FlatBuffers codec
//! (`crate::codec`) when a package's `encodings` names `proto3`.
//!
//! For every declaration that projects a type — a struct over its own
//! message, and a named scalar, an enum and an enum set over the box message
//! `<Name>Box { <type> value = 1; }` the proto backend emits for them
//! (ADR-0017 decision 6) — this module emits, at the generated package's
//! module scope and `pub(crate)` for the reasons the FlatBuffers codec gives
//! for its own functions:
//!
//! - `__ridl_pb_len_<Name>(&T) -> usize`, the exact encoded length;
//! - `__ridl_pb_encode_<Name>(&T, &mut Writer)`, which writes forwards;
//! - `__ridl_pb_verify_<Name>(&[u8])`, which walks the message's fields and
//!   checks every typl constraint the FlatBuffers `verify` checks, through
//!   the same generated `check` functions and the same inline checks;
//! - `__ridl_pb_decode_<Name>(&[u8]) -> T`, which is total;
//! - the `Payload<::ridl_rt::encoding::Proto3>` implementation, whose view
//!   is `::ridl_rt::proto3::Verified`, the verified bytes with no accessor.
//!
//! **A declaration whose shape reaches an array, a map, a tuple or a union
//! gets nothing yet**: collections and unions are not emitted yet, and such
//! a declaration is skipped with no error, which also skips every struct
//! that reaches it through a field. Nothing else is skipped: a declaration
//! this module does emit and whose proto3 size state is not bounded is
//! refused with a [`GenerateError`] naming it and the cause.
//!
//! # The wire mapping
//!
//! A field's number is its ordinal, and the scalar a width projects to is
//! `ridl_ir::projection::proto3`'s table, the one the proto backend and the
//! proto3 sizer read, so the codec cannot disagree with the emitted `.proto`
//! on either. The writer writes the canonical form: fields in number order,
//! an implicit-presence field omitted at its proto3 default — 0, `false`,
//! the empty string or bytes, enum value 0, the empty set — a `?` field
//! written whenever present, a non-optional nested struct always written, as
//! a record that may be empty, and a float compared by its bits, so `-0.0`
//! is written. The reader accepts fields in any order, skips an unknown
//! field number, and refuses a singular field written twice
//! (`Malformed::Duplicate`), a known field under a wire type its type does
//! not take (`Malformed::WireType`), and a varint wider than its field's
//! scalar (`Malformed::Varint`).
//!
//! An absent implicit-presence field reads as its default when that default
//! is a legal value of its type, decided at generation time by the predicate
//! the FlatBuffers codec reads (`crate::zero_is_legal`, a string's or bytes'
//! length lower bound, an enum's zero member), and is
//! `Malformed::MissingRequired` otherwise. An absent non-optional nested
//! struct is the message decoded from zero bytes, so `verify` verifies the
//! empty message in its place, which refuses it exactly when one of its own
//! fields is refused when absent. A field whose default is not legal is never
//! at its default, so the writer writes it without testing it.
//!
//! `encode` sizes the value first, with the length function, and returns
//! `EncodeError::Capacity` carrying that exact `needed` before it writes a
//! byte; what it then writes is a prefix of the output buffer. `verify`
//! applies no size ceiling: a buffer from a newer writer may carry fields
//! this version skips.

use std::collections::HashSet;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::codegen::v1;
use ridl_ir::projection::proto3 as pb;

use crate::codec::{
    NamedScalar, Operand, inline_length_check, missing_required, named_scalar_check,
    scalar_constraint_check,
};
use crate::{
    Ctx, GenerateError, ScalarBacking, allow_dead_code_if_internal, class_backing, declared, ident,
    pascal_of, snake_of, type_path, zero_is_legal,
};

/// Emits the proto3 codec for the package `ctx` was built over.
pub(crate) fn package_items(ctx: &Ctx) -> Result<Vec<TokenStream>, GenerateError> {
    Codec { ctx }.items()
}

/// The layout the codec emits from, per emitted message, for the test that
/// compares it with the emitted `.proto` schema field by field.
pub(crate) fn layout(ctx: &Ctx) -> Vec<MessageLayout> {
    Codec { ctx }
        .messages()
        .unwrap_or_default()
        .into_iter()
        .map(|message| message.layout())
        .collect()
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
/// the scalar's spelling or the referenced message's name, and the label.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageLayout {
    pub name: String,
    pub fields: Vec<(u32, String, Label)>,
}

struct Codec<'a> {
    ctx: &'a Ctx<'a>,
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
    /// A length-delimited nested message of the named generated struct.
    Message(String),
}

impl Wire {
    /// The wire type the field's tag carries.
    fn wire_type(&self) -> pb::WireType {
        match self {
            Wire::Scalar(scalar) => scalar.scalar.wire_type(),
            Wire::Text(..) | Wire::Bytes(..) | Wire::Message(_) => pb::WireType::Len,
        }
    }

    /// Whether the proto3 default of the field's type is a legal value of
    /// it, decided at generation time: an absent non-optional field reads as
    /// that default when it is, and is refused when it is not. A nested
    /// struct decides for itself, field by field, when its empty message is
    /// verified.
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
            Wire::Message(_) => true,
        }
    }

    /// The scalar's spelling or the message's name, for [`MessageLayout`].
    fn layout_name(&self) -> String {
        match self {
            Wire::Scalar(scalar) => scalar.scalar.as_str().to_string(),
            Wire::Text(..) => pb::Scalar::String.as_str().to_string(),
            Wire::Bytes(..) => pb::Scalar::Bytes.as_str().to_string(),
            Wire::Message(owner) => owner.clone(),
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

    /// The writer call for the value `__r`.
    fn write(&self) -> TokenStream {
        match self.scalar.wire_type() {
            pb::WireType::Varint => quote! { writer.varint(__r); },
            pb::WireType::I32 => quote! { writer.fixed32(__r); },
            pb::WireType::I64 => quote! { writer.fixed64(__r); },
            pb::WireType::Len => quote! {},
        }
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
        let zero = match self.scalar {
            pb::Scalar::Bool => quote! { false },
            pb::Scalar::Float | pb::Scalar::Double => quote! { 0.0f64 },
            _ => quote! { 0i64 },
        };
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
}

// ---------------------------------------------------------------------------
// A message and its fields
// ---------------------------------------------------------------------------

/// How one field's value is reached from the value being encoded.
#[derive(Debug, Clone)]
enum Access {
    /// A named field of a struct.
    Field(String),
    /// The whole value: a box root's `value`.
    Whole,
}

#[derive(Debug, Clone)]
struct Field {
    /// The field number: the ordinal, or 1 for a box's `value`.
    number: u32,
    /// The typl name, used for the decoded field.
    name: String,
    wire: Wire,
    optional: bool,
    access: Access,
}

impl Field {
    fn operand(&self, base: &TokenStream) -> Operand {
        match &self.access {
            Access::Field(name) => {
                let field = ident(name);
                Operand::owned(quote! { #base.#field })
            }
            Access::Whole => Operand::borrowed(base.clone()),
        }
    }

    fn label(&self) -> Label {
        if self.optional {
            Label::Optional
        } else {
            Label::Implicit
        }
    }
}

/// What `decode` builds: a struct literal over the fields, or a box's one
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Build {
    Struct,
    Box,
}

#[derive(Debug, Clone)]
struct Message {
    /// The declared name of the owning declaration.
    owner: String,
    /// The index of the declaration in the model.
    declaration: u32,
    /// The fields, in field-number order.
    fields: Vec<Field>,
    build: Build,
}

impl Message {
    /// The `.proto` message name: the declared name, or `<Name>Box`.
    fn proto_name(&self) -> String {
        match self.build {
            Build::Struct => self.owner.clone(),
            Build::Box => format!("{}Box", self.owner),
        }
    }

    fn layout(&self) -> MessageLayout {
        MessageLayout {
            name: self.proto_name(),
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
/// run time, and this one is a generation-time literal.
fn tag_len(number: u32) -> usize {
    let tag = u64::from(number) << 3;
    let bits = 64 - (tag | 1).leading_zeros() as usize;
    bits.div_ceil(7)
}

/// The `WireType` variant of `ridl_rt::proto3` for a projection wire type.
fn wire_type_tokens(wire: pb::WireType) -> TokenStream {
    let variant = format_ident!(
        "{}",
        match wire {
            pb::WireType::Varint => "Varint",
            pb::WireType::I64 => "I64",
            pb::WireType::Len => "Len",
            pb::WireType::I32 => "I32",
        }
    );
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

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

impl<'a> Codec<'a> {
    /// The name of the package this model was lowered for.
    fn scope_package(&self) -> &str {
        self.ctx
            .model
            .scope
            .as_ref()
            .map(|scope| scope.package.as_str())
            .unwrap_or_default()
    }

    /// Resolves a type reference against this package first and then the
    /// others of the build, returning the declaration and the owner string a
    /// reference site writes: the bare declared name for a type of this
    /// package, the dotted reference for one of another package.
    fn resolve(&self, reference: &v1::TypeRef) -> Option<(&'a v1::Declaration, String)> {
        if reference.package.is_empty() {
            return None;
        }
        if reference.package == self.scope_package() {
            let decl = self.ctx.declaration(reference.index)?;
            return Some((decl, declared(decl.name.as_ref()).to_string()));
        }
        let foreign = self.ctx.model.foreign.get(reference.index as usize)?;
        Some((foreign.declaration.as_ref()?, reference.reference.clone()))
    }

    /// The messages this codec emits, in declaration order.
    ///
    /// A declaration whose shape reaches an array, a map, a tuple or a union
    /// is left out: collections and unions are not emitted yet. A struct
    /// that reaches such a declaration through a field is left out with it,
    /// because the functions it would call do not exist, which is settled by
    /// a fixpoint over the local references.
    fn messages(&self) -> Result<Vec<Message>, GenerateError> {
        let mut messages: Vec<Message> = Vec::new();
        let mut later: HashSet<String> = HashSet::new();
        for (index, decl) in self.ctx.model.declarations.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| GenerateError {
                message: "the lowered model carries more declarations than a u32 indexes"
                    .to_string(),
            })?;
            match self.message_of(decl, index)? {
                Some(message) => messages.push(message),
                None => {
                    later.insert(declared(decl.name.as_ref()).to_string());
                }
            }
        }
        loop {
            let before = messages.len();
            messages.retain(|message| {
                let reaches_later = message.fields.iter().any(|field| match &field.wire {
                    Wire::Message(owner) => later.contains(owner),
                    _ => false,
                });
                if reaches_later {
                    later.insert(message.owner.clone());
                }
                !reaches_later
            });
            if messages.len() == before {
                break;
            }
        }
        Ok(messages)
    }

    fn items(&self) -> Result<Vec<TokenStream>, GenerateError> {
        let mut items: Vec<TokenStream> = Vec::new();
        for message in self.messages()? {
            let decl = self
                .ctx
                .declaration(message.declaration)
                .ok_or_else(|| GenerateError {
                    message: "the proto3 codec names a declaration the lowered model does not \
                              carry"
                        .to_string(),
                })?;
            let bound = self.bound(decl)?;
            let visibility = decl.visibility;
            let owner = message.owner.as_str();
            let ty = ident(owner);
            let functions = [
                self.len_fn(&message, &ty)?,
                self.encode_fn(&message, &ty)?,
                self.verify_fn(&message)?,
                self.decode_fn(&message, &ty)?,
                self.payload_impl(owner, &ty, bound),
            ];
            items.extend(
                functions
                    .into_iter()
                    .map(|item| allow_dead_code_if_internal(visibility, item)),
            );
        }
        Ok(items)
    }

    /// The proto3 bound of the declaration's root message, or the refusal
    /// for a declaration whose state is not bounded.
    fn bound(&self, decl: &v1::Declaration) -> Result<u32, GenerateError> {
        let pkg = self.ctx.package_name();
        let name = declared(decl.name.as_ref());
        let cause = match decl.proto3.as_ref().and_then(|state| state.state.as_ref()) {
            Some(v1::size_state::State::Bounded(bound)) => return Ok(*bound),
            Some(v1::size_state::State::Absent(absent)) => {
                match v1::AbsentCause::try_from(absent.cause)
                    .unwrap_or(v1::AbsentCause::Unspecified)
                {
                    v1::AbsentCause::RefusedMember => {
                        "a member of it is one the proto backend refuses: an enum value outside \
                         int32, a field number proto3 reserves or exceeds, an optional array or \
                         map, an array of arrays or of maps, a map whose value is a collection, \
                         or a map key outside the integral and string scalars"
                    }
                    v1::AbsentCause::NoBound => {
                        "a member of it is a string or bytes with no length bound, or a scalar \
                         with no width"
                    }
                    v1::AbsentCause::Overflow => {
                        "its bound exceeds u32::MAX, or its nesting passes the sizer's depth limit"
                    }
                    v1::AbsentCause::Unresolved => {
                        "it names a declaration the sizer did not resolve"
                    }
                    v1::AbsentCause::NoMessage
                    | v1::AbsentCause::EncodingUndefined
                    | v1::AbsentCause::Unspecified => "no proto3 message defines it",
                }
            }
            Some(v1::size_state::State::Unbounded(_)) => "no finite bound exists for it",
            None => "the lowered model carries no proto3 size state for it",
        };
        Err(GenerateError {
            message: format!("`{pkg}.{name}` has no proto3 bound: {cause}"),
        })
    }

    // -----------------------------------------------------------------
    // Classification
    // -----------------------------------------------------------------

    /// The message of one declaration: a struct over its fields, a named
    /// scalar, an enum or an enum set over its box. `None` for a declaration
    /// this codec does not emit yet, and for one that projects no type.
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
                    let Some(wire) = self.wire(ty)? else {
                        return Ok(None);
                    };
                    fields.push(Field {
                        number: slot.ordinal,
                        name: snake_of(field.name.as_ref()).to_string(),
                        wire,
                        optional: ty.optional,
                        access: Access::Field(snake_of(field.name.as_ref()).to_string()),
                    });
                }
                // Field-number order, whatever the declaration order.
                fields.sort_by_key(|field| field.number);
                Ok(Some(Message {
                    owner: owner.to_string(),
                    declaration: index,
                    fields,
                    build: Build::Struct,
                }))
            }
            Some(
                v1::declaration::Kind::Scalar(_)
                | v1::declaration::Kind::Enum(_)
                | v1::declaration::Kind::EnumSet(_),
            ) => {
                let Some(wire) = self.declaration_wire(decl, owner.to_string(), owner)? else {
                    return Ok(None);
                };
                Ok(Some(Message {
                    owner: owner.to_string(),
                    declaration: index,
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
            // A union is not emitted yet; a constant projects no type.
            _ => Ok(None),
        }
    }

    /// The wire shape of one type position, or `None` for a position this
    /// codec does not emit yet: an array, a map, a tuple, or a reference to a
    /// union.
    fn wire(&self, ty: &v1::Type) -> Result<Option<Wire>, GenerateError> {
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Primitive(primitive)) => {
                match v1::PrimitiveType::try_from(*primitive).ok() {
                    Some(v1::PrimitiveType::Boolean) => Ok(Some(Wire::Scalar(Scalar {
                        scalar: pb::Scalar::Bool,
                        repr: Repr::Bool,
                        constraint: None,
                        zero_legal: true,
                    }))),
                    Some(v1::PrimitiveType::Integer) => Ok(Some(Wire::Scalar(Scalar {
                        scalar: pb::primitive(*primitive),
                        repr: Repr::Int,
                        constraint: None,
                        zero_legal: true,
                    }))),
                    Some(v1::PrimitiveType::Float) => Ok(Some(Wire::Scalar(Scalar {
                        scalar: pb::primitive(*primitive),
                        repr: Repr::Float,
                        constraint: None,
                        zero_legal: true,
                    }))),
                    // A bare `string` or `bytes` carries no length bound, so
                    // it has no finite bound (typl §15.3 keeps it out of a
                    // field position anyway).
                    _ => Err(GenerateError {
                        message: "a proto3 codec cannot carry an unbounded primitive".to_string(),
                    }),
                }
            }
            Some(v1::r#type::Kind::Inline(sc)) => self.scalar_wire(sc, None).map(Some),
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
            // Collections and tuples are not emitted yet.
            Some(
                v1::r#type::Kind::Array(_) | v1::r#type::Kind::Map(_) | v1::r#type::Kind::Tuple(_),
            ) => Ok(None),
            _ => Err(GenerateError {
                message: "a proto3 codec cannot carry this type position".to_string(),
            }),
        }
    }

    /// The wire shape of one resolved reference, or of the declaration being
    /// boxed: the declaration decides. `None` for a union, which is not
    /// emitted yet.
    fn declaration_wire(
        &self,
        decl: &v1::Declaration,
        owner: String,
        reference: &str,
    ) -> Result<Option<Wire>, GenerateError> {
        let name = declared(decl.name.as_ref());
        match decl.kind.as_ref() {
            Some(v1::declaration::Kind::Scalar(sc)) => self
                .scalar_wire(
                    sc,
                    Some(NamedScalar {
                        name: owner,
                        ctor: if sc.vacuous { "new" } else { "new_unchecked" },
                    }),
                )
                .map(Some),
            Some(v1::declaration::Kind::Enum(def)) => {
                let Some(first) = def.values.first() else {
                    return Err(GenerateError {
                        message: format!(
                            "`{name}` declares no value, so no proto3 codec can decode one"
                        ),
                    });
                };
                let zero = def
                    .zero_member
                    .and_then(|index| def.values.get(index as usize))
                    .map(|zero| pascal_of(zero.name.as_ref()));
                Ok(Some(Wire::Scalar(Scalar {
                    scalar: pb::Scalar::Int64,
                    zero_legal: zero.is_some(),
                    repr: Repr::Enum {
                        name: owner,
                        first: pascal_of(first.name.as_ref()),
                        zero,
                    },
                    constraint: None,
                })))
            }
            Some(v1::declaration::Kind::EnumSet(def)) => {
                let scalar = v1::IntWidth::try_from(def.width)
                    .ok()
                    .and_then(pb::int_width_scalar)
                    .ok_or_else(|| GenerateError {
                        message: format!("`{name}` carries no integer width"),
                    })?;
                Ok(Some(Wire::Scalar(Scalar {
                    scalar,
                    repr: Repr::EnumSet { name: owner },
                    constraint: None,
                    zero_legal: true,
                })))
            }
            Some(v1::declaration::Kind::Struct(_)) => Ok(Some(Wire::Message(owner))),
            // A union is not emitted yet.
            Some(v1::declaration::Kind::Union(_)) => Ok(None),
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
        let used = !message.fields.is_empty();
        let value = param_binding("value", used);
        let body = if used {
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
        let used = !message.fields.is_empty();
        let value = param_binding("value", used);
        let writer = param_binding("writer", used);
        let mut writes: Vec<TokenStream> = Vec::new();
        for field in &message.fields {
            writes.push(self.field_statement(&message.owner, field, Pass::Encode)?);
        }
        // Writes the value's fields in number order. The caller has sized the
        // value with the length function and checked the capacity, so no
        // write fails for lack of room; what can fail is an anonymous length
        // outside its bound, which is `EncodeError::Contract`. An internal:
        // no doc.
        Ok(quote! {
            #[allow(deprecated, non_snake_case)]
            pub(crate) fn #name(
                #value: &#ty,
                #writer: &mut ::ridl_rt::proto3::Writer<'_>,
            ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
                #(#writes)*
                ::core::result::Result::Ok(())
            }
        })
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
            let inner = self.field_value(owner, field, &Operand::borrowed(quote! { __v }), pass)?;
            return Ok(quote! {
                if let ::core::option::Option::Some(__v) = &#place {
                    #inner
                }
            });
        }
        self.field_value(owner, field, &operand, pass)
    }

    /// The statements that count or write one present value: a field whose
    /// default is legal is omitted at it, a field whose default is not legal
    /// is never at it and is written without a test, and a nested struct is
    /// always written.
    fn field_value(
        &self,
        owner: &str,
        field: &Field,
        operand: &Operand,
        pass: Pass,
    ) -> Result<TokenStream, GenerateError> {
        let tag_len = Literal::usize_suffixed(tag_len(field.number));
        let number = Literal::u32_suffixed(field.number);
        let wire_type = wire_type_tokens(field.wire.wire_type());
        let place = &operand.place;
        let reference = &operand.reference;
        let receiver = operand.receiver();
        // A `?` field is written whatever its value; an implicit-presence
        // field is tested against its default only when that default is a
        // legal value, since otherwise it never holds it.
        let test = !field.optional && field.wire.default_is_legal();
        Ok(match &field.wire {
            Wire::Scalar(scalar) => {
                let raw = scalar.raw(scalar.language(quote! { __s }));
                let value_len = scalar.value_len();
                let action = match pass {
                    Pass::Len => quote! { __n += #tag_len + #value_len; },
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
                        __n += #tag_len
                            + ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64)
                            + __s.len();
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
            Wire::Message(name) => {
                let len = owner_path(name, len_ident);
                match pass {
                    Pass::Len => quote! {
                        {
                            let __l = #len(#reference);
                            __n += #tag_len
                                + ::ridl_rt::proto3::varint_len(__l as ::core::primitive::u64)
                                + __l;
                        }
                    },
                    Pass::Encode => {
                        let encode = owner_path(name, encode_ident);
                        quote! {
                            {
                                let __l = #len(#reference);
                                writer.tag(#number, #wire_type);
                                writer.varint(__l as ::core::primitive::u64);
                                #encode(#reference, writer)?;
                            }
                        }
                    }
                }
            }
        })
    }

    // -----------------------------------------------------------------
    // verify
    // -----------------------------------------------------------------

    fn verify_fn(&self, message: &Message) -> Result<TokenStream, GenerateError> {
        let name = verify_ident(&message.owner);
        let count = message.fields.len();
        let mut arms: Vec<TokenStream> = Vec::new();
        let mut absent: Vec<TokenStream> = Vec::new();
        for (index, field) in message.fields.iter().enumerate() {
            let number = Literal::u32_suffixed(field.number);
            let seen = Literal::usize_suffixed(index);
            let present = self.verify_present(&message.owner, field)?;
            let duplicate = structure_error("Duplicate");
            arms.push(quote! {
                #number => {
                    if __seen[#seen] {
                        #duplicate
                    }
                    __seen[#seen] = true;
                    #present
                }
            });
            let when_absent = self.verify_absent(field);
            if !when_absent.is_empty() {
                absent.push(quote! {
                    if !__seen[#seen] {
                        #when_absent
                    }
                });
            }
        }
        let walk = if count == 0 {
            // No field to match: the walk checks the message's structure and
            // nothing else.
            quote! {
                let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
                while let ::core::option::Option::Some(__field) = __reader.next() {
                    __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                }
            }
        } else {
            let count = Literal::usize_suffixed(count);
            quote! {
                let mut __seen = [false; #count];
                let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
                while let ::core::option::Option::Some(__field) = __reader.next() {
                    let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    match __field.number {
                        #(#arms)*
                        // An unknown field: the reader has moved past it.
                        _ => {}
                    }
                }
                #(#absent)*
            }
        };
        // Checks the proto3 message in `buf` against the declaration's
        // shape: the structure, each known field's wire type, a singular
        // field written once, enum discriminants, named and anonymous scalar
        // ranges, steps and lengths, and the absence rule. Pattern checks
        // require `validate-pattern`. An internal: no doc.
        Ok(quote! {
            #[allow(deprecated, non_snake_case)]
            pub(crate) fn #name(
                buf: &[::core::primitive::u8],
            ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                #walk
                ::core::result::Result::Ok(())
            }
        })
    }

    /// The statements that check one present field's value, over
    /// `__field.value`: the wire type its type takes, then the constraints.
    fn verify_present(&self, owner: &str, field: &Field) -> Result<TokenStream, GenerateError> {
        let wrong_type = structure_error("WireType");
        let (pattern, checks) = match &field.wire {
            Wire::Scalar(scalar) => {
                let variant = match scalar.scalar.wire_type() {
                    pb::WireType::Varint => "Varint",
                    pb::WireType::I32 => "I32",
                    pb::WireType::I64 => "I64",
                    pb::WireType::Len => "Len",
                };
                let variant = format_ident!("{}", variant);
                let (widened, fallible) = scalar.widen();
                let check = scalar.check(owner);
                let body = match (check, fallible) {
                    (Some(check), _) => quote! {
                        let __v = #widened;
                        #check
                    },
                    (None, true) => quote! { let _ = #widened; },
                    (None, false) => quote! {},
                };
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
                let body = match check {
                    Some(check) => quote! {
                        let __s = ::ridl_rt::proto3::utf8(__b)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #check
                    },
                    None => quote! {
                        ::ridl_rt::proto3::utf8(__b)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    },
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
            Wire::Message(name) => {
                let verify = owner_path(name, verify_ident);
                (
                    quote! { ::ridl_rt::proto3::Value::Len(__b) },
                    quote! { #verify(__b)?; },
                )
            }
        };
        Ok(quote! {
            match __field.value {
                #pattern => { #checks }
                _ => { #wrong_type }
            }
        })
    }

    /// What `verify` does with an absent non-optional field: nothing when
    /// the proto3 default is a legal value of its type, `MissingRequired`
    /// when it is not, and for a nested struct the verification of the empty
    /// message, which decides by the same rule over its own fields.
    fn verify_absent(&self, field: &Field) -> TokenStream {
        if field.optional {
            return quote! {};
        }
        match &field.wire {
            Wire::Message(name) => {
                let verify = owner_path(name, verify_ident);
                quote! { #verify(&[])?; }
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
        let mut inits: Vec<TokenStream> = Vec::new();
        let mut arms: Vec<TokenStream> = Vec::new();
        let mut bindings: Vec<Ident> = Vec::new();
        for (index, field) in message.fields.iter().enumerate() {
            let binding = format_ident!("__f{}", index);
            let number = Literal::u32_suffixed(field.number);
            let default = if field.optional {
                quote! { ::core::option::Option::None }
            } else {
                self.decode_default(field)
            };
            inits.push(quote! { let mut #binding = #default; });
            let (pattern, value) = self.decode_present(field)?;
            let value = if field.optional {
                quote! { ::core::option::Option::Some(#value) }
            } else {
                value
            };
            arms.push(quote! {
                #number => {
                    if let #pattern = __field.value {
                        #binding = #value;
                    }
                }
            });
            bindings.push(binding);
        }
        let walk = if used {
            quote! {
                let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
                while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) =
                    __reader.next()
                {
                    match __field.number {
                        #(#arms)*
                        _ => {}
                    }
                }
            }
        } else {
            quote! {}
        };
        let build = match message.build {
            Build::Struct => {
                let names = message.fields.iter().map(|field| ident(&field.name));
                quote! { #ty { #(#names: #bindings),* } }
            }
            Build::Box => {
                let binding = &bindings[0];
                quote! { #binding }
            }
        };
        // Builds the value from the proto3 message in `buf`, which `verify`
        // accepted. An internal: no doc. It cannot fail: an absent field
        // reads as its default, a read that could fail is discharged with
        // the neutral value of its own type, and `verify` is what makes
        // those branches unreachable. A named scalar is built with its
        // unchecked constructor over a value `verify` has already checked.
        Ok(quote! {
            #[allow(deprecated, non_snake_case)]
            pub(crate) fn #name(#buf: &[::core::primitive::u8]) -> #ty {
                #(#inits)*
                #walk
                #build
            }
        })
    }

    /// The domain value an absent non-optional field decodes as.
    fn decode_default(&self, field: &Field) -> TokenStream {
        match &field.wire {
            Wire::Scalar(scalar) => scalar.decode_default(),
            Wire::Text(named, _) => named_value(named, quote! { ::std::string::String::new() }),
            Wire::Bytes(named, _) => named_value(named, quote! { ::std::vec::Vec::new() }),
            Wire::Message(name) => {
                let decode = owner_path(name, decode_ident);
                quote! { #decode(&[]) }
            }
        }
    }

    /// The pattern over `__field.value` and the domain value read from it,
    /// for `decode`.
    fn decode_present(&self, field: &Field) -> Result<(TokenStream, TokenStream), GenerateError> {
        Ok(match &field.wire {
            Wire::Scalar(scalar) => {
                let variant = match scalar.scalar.wire_type() {
                    pb::WireType::Varint => "Varint",
                    pb::WireType::I32 => "I32",
                    pb::WireType::I64 => "I64",
                    pb::WireType::Len => "Len",
                };
                let variant = format_ident!("{}", variant);
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
            Wire::Message(name) => {
                let decode = owner_path(name, decode_ident);
                (
                    quote! { ::ridl_rt::proto3::Value::Len(__b) },
                    quote! { #decode(__b) },
                )
            }
        })
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
                // carries for the declaration's root message, which the
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

                fn decode(
                    r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
                ) -> Self {
                    #decode(::ridl_rt::payload::View::bytes(r.view()))
                }
            }
        }
    }
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
