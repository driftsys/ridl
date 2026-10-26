//! The podbuf codec: the `#[repr(C, align(8))]` head struct of each struct,
//! union and named tuple, with its pad fields and the `const` assertions that
//! pin the layout law's numbers, and the encoder that writes the preamble,
//! the head and the tail (`docs/wip/2026-10-24-podbuf-design.md`, §5, §6, §7
//! "Encoder duty", §8.1 and §14 "Tail order").
//!
//! The layout is not computed here. It is read from the model's podbuf
//! projection ([`v1::PodbufProjection`]), which `ridl_ir::projection::podbuf`
//! computes once for every emitter (ADR-0020 decisions 8 and 9). This module
//! pairs each placement of a head with the Rust value that fills it — the
//! domain struct's field, the tuple struct's field, the union's arm, the map
//! entry's pair — and writes that value through `ridl_rt::podbuf::Writer`:
//! every store is a bounds-checked little-endian store, and no emitted line
//! holds `unsafe`.
//!
//! Per type `T`, the codec emits `TPodbufHead`, the head struct, and
//! `impl T { __ridl_podbuf_encode, __ridl_podbuf_encode_at }`. The first
//! reserves the head and the second writes a value at a given head offset,
//! which is how a nested struct, an array element and a union arm are
//! written in place. The `Payload<Podbuf>` implementation, `verify`, the
//! decoder and the `TPodbufView` type are not emitted yet (driftsys/ridl#317);
//! the view's name is already claimed (`crate::claims`).
//!
//! The encoder's duties (§7): every pad byte, the preamble's `reserved` word
//! and every byte of an absent optional are zero, which `Writer` guarantees
//! by zero-filling what it reserves; the tail holds only the data present, in
//! field order depth first (§14); a map's entries are sorted by key (§6); a
//! string holding U+0000, a duplicate map key and a NaN map key are refused;
//! a deprecated field is written as its init value (§8.1).

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::codegen::v1;
use ridl_ir::codegen::v1::podbuf_descriptor::Kind as Descriptor;
use ridl_ir::codegen::v1::podbuf_form::Kind as Form;
use ridl_ir::codegen::v1::podbuf_head::Source;
use ridl_ir::codegen::v1::podbuf_source::Kind as Placed;

use crate::codec::{Operand, encode_count_check, inline_length_check};
use crate::transcode::PayloadOwner;
use crate::{
    Ctx, GenerateError, ScalarBacking, allow_dead_code_if_internal, camel_of, class_backing,
    declared, defaults, ident, model_type_tokens, snake_of, tuple_name, type_path, vis_tokens,
};

/// The layout one head struct was emitted from, as the drift test reads it:
/// the numbers the emitted `const` assertions carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadLayout {
    /// The head struct's name, `<T>PodbufHead`.
    pub name: String,
    /// The head size in bytes, a multiple of 8.
    pub size: u32,
    /// Every member of the struct by offset, the pads included: the Rust
    /// field name, its offset and its size.
    pub fields: Vec<(String, u32, u32)>,
}

/// A type the codec emits a head struct and an encoder for, by index into
/// the model's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadOwner {
    /// Index into `Model.declarations`: a struct or a union.
    Declaration(u32),
    /// Index into `Model.tuples`: a named induced tuple.
    Tuple(u32),
}

/// The owners of the heads [`package_items`] emits, in head order, which the
/// claim table (`crate::claims`) claims the head and view names of before
/// the codec runs. A projection the model does not carry, and a refused
/// head, claim nothing: the codec refuses the package afterwards.
pub(crate) fn head_owners(ctx: &Ctx) -> Vec<HeadOwner> {
    let Some(projection) = ctx.model.podbuf.as_ref() else {
        return Vec::new();
    };
    projection
        .heads
        .iter()
        .filter_map(|head| match head.source.as_ref()? {
            Source::Declaration(index) => Some(HeadOwner::Declaration(*index)),
            Source::Tuple(index) => {
                (!ctx.tuple_name(*index).is_empty()).then_some(HeadOwner::Tuple(*index))
            }
            _ => None,
        })
        .collect()
}

/// `<owner>PodbufHead`.
pub(crate) fn head_ident(owner: &str) -> Ident {
    ident(&format!("{owner}PodbufHead"))
}

/// `<owner>PodbufView`.
pub(crate) fn view_ident(owner: &str) -> Ident {
    ident(&format!("{owner}PodbufView"))
}

/// Emits the podbuf items of the package `ctx` was built over, and the
/// types they belong to, so the transcode functions follow the codec's own
/// rule.
pub(crate) fn package_items(
    ctx: &Ctx,
) -> Result<(Vec<TokenStream>, Vec<PayloadOwner>), GenerateError> {
    Codec::new(ctx)?.items()
}

/// The layouts [`package_items`] emits the head structs from, one per head
/// struct, in output order.
pub(crate) fn layout(ctx: &Ctx) -> Result<Vec<HeadLayout>, GenerateError> {
    Codec::new(ctx)?.layouts()
}

fn fail<T>(message: String) -> Result<T, GenerateError> {
    Err(GenerateError { message })
}

/// A fixed-width primitive of the wire, with the `Writer` method that stores
/// it and the Rust type the language-layer value is cast to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prim(&'static str);

impl Prim {
    fn of_int(width: i32) -> Result<Prim, GenerateError> {
        Ok(Prim(match v1::IntWidth::try_from(width) {
            Ok(v1::IntWidth::U8) => "u8",
            Ok(v1::IntWidth::I8) => "i8",
            Ok(v1::IntWidth::U16) => "u16",
            Ok(v1::IntWidth::I16) => "i16",
            Ok(v1::IntWidth::U32) => "u32",
            Ok(v1::IntWidth::I32) => "i32",
            Ok(v1::IntWidth::U64) => "u64",
            Ok(v1::IntWidth::I64) => "i64",
            _ => return fail("a podbuf integer carries no width".to_string()),
        }))
    }

    fn of_float(width: i32) -> Result<Prim, GenerateError> {
        Ok(Prim(match v1::FloatWidth::try_from(width) {
            Ok(v1::FloatWidth::F32) => "f32",
            Ok(v1::FloatWidth::F64) => "f64",
            _ => return fail("a podbuf float carries no width".to_string()),
        }))
    }

    fn put(self) -> Ident {
        format_ident!("put_{}", self.0)
    }

    fn ty(self) -> TokenStream {
        let name = format_ident!("{}", self.0);
        quote! { ::core::primitive::#name }
    }

    /// `lang`, an `i64` or `f64` of the language layer, as this primitive. An
    /// `i64` and an `f64` need no cast, and a cast to the same type draws
    /// `clippy::unnecessary_cast` in a consumer's build.
    /// The literal `value` with this primitive's suffix, so that no cast is
    /// written on a literal (`clippy::unnecessary_cast`).
    fn literal(self, value: u32) -> TokenStream {
        format!("{value}{}", self.0)
            .parse()
            .unwrap_or_else(|_| quote! { #value })
    }

    fn cast(self, lang: TokenStream) -> TokenStream {
        if self.0 == "i64" || self.0 == "f64" {
            return lang;
        }
        let ty = self.ty();
        quote! { #lang as #ty }
    }
}

/// The Rust shape a head is written from.
#[derive(Clone, Copy)]
enum Shape<'a> {
    Struct(&'a v1::Declaration, &'a v1::Struct),
    Union(&'a v1::Declaration, &'a v1::Union),
    Tuple(&'a v1::InducedTuple),
}

impl<'a> Shape<'a> {
    /// The Rust type's name: the declared name, or the tuple struct's name.
    fn name(&self) -> &'a str {
        match self {
            Shape::Struct(decl, _) | Shape::Union(decl, _) => declared(decl.name.as_ref()),
            Shape::Tuple(tuple) => tuple_name(tuple),
        }
    }

    fn visibility(&self) -> i32 {
        match self {
            Shape::Struct(decl, _) | Shape::Union(decl, _) => decl.visibility,
            Shape::Tuple(tuple) => tuple.visibility,
        }
    }

    /// The field a placement reads: a struct field by its typl name, a tuple
    /// field by its position.
    fn field_of(&self, placed: &Placed) -> Option<&'a v1::Field> {
        match (*self, placed) {
            (Shape::Struct(_, sd), Placed::Field(named)) => {
                sd.slots
                    .iter()
                    .find_map(|slot| match slot.occupant.as_ref() {
                        Some(v1::slot::Occupant::Field(field))
                            if declared(field.name.as_ref()) == named.name =>
                        {
                            Some(&**field)
                        }
                        _ => None,
                    })
            }
            (Shape::Tuple(tuple), Placed::Position(position)) => {
                tuple.fields.get((*position as usize).checked_sub(1)?)
            }
            _ => None,
        }
    }
}

/// One member of a head struct: a field or a pad, by offset.
struct Member {
    name: Ident,
    offset: u32,
    size: u32,
    /// The Rust type, or `None` for a field that takes no byte, which the
    /// struct leaves out.
    ty: Option<TokenStream>,
}

/// What one placement of a head is written from, once the shape has been
/// read: the operand and the type position of the value, or `None` for a
/// placement that writes nothing (a flags byte, which is computed from the
/// optional fields, or a deprecated field whose init value is not
/// derivable, whose bytes stay zero).
struct Slot<'a> {
    field: &'a v1::PodbufField,
    value: Option<(Operand, v1::Type)>,
}

struct Codec<'a> {
    ctx: &'a Ctx<'a>,
    heads: &'a [v1::PodbufHead],
}

impl<'a> Codec<'a> {
    fn new(ctx: &'a Ctx<'a>) -> Result<Codec<'a>, GenerateError> {
        let Some(projection) = ctx.model.podbuf.as_ref() else {
            return fail("the model carries no podbuf projection".to_string());
        };
        // A head the projection refused has no layout to write from, and so
        // has every head that needs it. The checker reports the cause in
        // source (RIDL-153); the refusal here names it for a model handed in
        // directly, as the C header backend does.
        if let Some(refused) = projection.refused.first() {
            let what = match refused.source {
                Some(v1::podbuf_refused::Source::Declaration(index)) => {
                    ctx.declaration(index).map(|decl| {
                        format!("`{}.{}`", ctx.package_name(), declared(decl.name.as_ref()))
                    })
                }
                Some(v1::podbuf_refused::Source::Tuple(index)) => ctx
                    .tuple(index)
                    .map(|tuple| format!("the tuple `{}`", tuple_name(tuple))),
                None => None,
            }
            .unwrap_or_else(|| "a type".to_string());
            return fail(format!(
                "{what} cannot be laid out as a podbuf head, so the package gets no podbuf \
                 codec: {}",
                refused.cause
            ));
        }
        Ok(Codec {
            ctx,
            heads: &projection.heads,
        })
    }

    /// The heads this package emits a struct and an encoder for, in head
    /// order: each local struct and union, and each named tuple.
    fn shapes(&self) -> Vec<(usize, Shape<'a>)> {
        self.heads
            .iter()
            .enumerate()
            .filter_map(|(at, head)| {
                let shape = match head.source.as_ref()? {
                    Source::Declaration(index) => {
                        let decl = self.ctx.declaration(*index)?;
                        match decl.kind.as_ref()? {
                            v1::declaration::Kind::Struct(sd) => Shape::Struct(decl, sd),
                            v1::declaration::Kind::Union(ud) => Shape::Union(decl, ud),
                            _ => return None,
                        }
                    }
                    Source::Tuple(index) => {
                        let tuple = self.ctx.tuple(*index)?;
                        if tuple_name(tuple).is_empty() {
                            return None;
                        }
                        Shape::Tuple(tuple)
                    }
                    _ => return None,
                };
                Some((at, shape))
            })
            .collect()
    }

    fn items(&self) -> Result<(Vec<TokenStream>, Vec<PayloadOwner>), GenerateError> {
        let mut items = Vec::new();
        let mut owners = Vec::new();
        for (at, shape) in self.shapes() {
            let head_struct = self.head_struct(at, &shape)?;
            let encoder = self.encoder(at, &shape)?;
            items.push(allow_dead_code_if_internal(
                shape.visibility(),
                quote! { #head_struct #encoder },
            ));
            owners.push(PayloadOwner {
                name: shape.name().to_string(),
                visibility: shape.visibility(),
            });
        }
        Ok((items, owners))
    }

    fn layouts(&self) -> Result<Vec<HeadLayout>, GenerateError> {
        let mut layouts = Vec::new();
        for (at, shape) in self.shapes() {
            let members = self.members(at, &shape)?;
            layouts.push(HeadLayout {
                name: head_ident(shape.name()).to_string(),
                size: self.heads[at].size,
                fields: members
                    .iter()
                    .filter(|member| member.ty.is_some())
                    .map(|member| (member.name.to_string(), member.offset, member.size))
                    .collect(),
            });
        }
        Ok(layouts)
    }

    // -----------------------------------------------------------------
    // Reading the model
    // -----------------------------------------------------------------

    /// The declaration a resolved reference names, in this package or in
    /// another of the build.
    fn resolve(&self, reference: &v1::TypeRef) -> Result<&'a v1::Declaration, GenerateError> {
        if !reference.resolved {
            return fail(format!(
                "`{}` resolves in no package of this build, so no podbuf codec can be emitted \
                 for it",
                reference.reference
            ));
        }
        let decl = if reference.foreign {
            self.ctx
                .model
                .foreign
                .get(reference.index as usize)
                .and_then(|foreign| foreign.declaration.as_ref())
        } else {
            self.ctx.declaration(reference.index)
        };
        decl.ok_or_else(|| GenerateError {
            message: format!(
                "the lowered model names `{}` at an index it does not carry",
                reference.reference
            ),
        })
    }

    /// The size in bytes of a value of `form`.
    fn form_size(&self, form: &v1::PodbufForm) -> u32 {
        match form.kind.as_ref() {
            Some(Form::Boolean(_)) => 1,
            Some(Form::Int(int)) => int_bytes(int.width),
            Some(Form::Float(width)) => float_bytes(*width),
            Some(Form::Enum(value)) => int_bytes(value.width),
            Some(Form::EnumSet(value)) => int_bytes(value.width),
            Some(Form::InlineHead(index)) => {
                self.heads.get(*index as usize).map_or(0, |head| head.size)
            }
            Some(Form::InlineArray(array)) => array
                .element
                .as_deref()
                .map_or(0, |element| self.form_size(element))
                .saturating_mul(array.count),
            Some(Form::Descriptor(_)) => 8,
            None => 0,
        }
    }

    /// The alignment in bytes of a value of `form`, which is where its data
    /// starts in the tail.
    fn form_align(&self, form: &v1::PodbufForm) -> u32 {
        match form.kind.as_ref() {
            Some(Form::InlineHead(_)) => 8,
            Some(Form::InlineArray(array)) => array
                .element
                .as_deref()
                .map_or(1, |element| self.form_align(element)),
            Some(Form::Descriptor(_)) => 4,
            _ => self.form_size(form).max(1),
        }
    }

    /// `ty` with its `optional` flag cleared: the type of a present value.
    fn present(ty: &v1::Type) -> v1::Type {
        let mut inner = ty.clone();
        inner.optional = false;
        inner
    }

    /// The members of head `at`: its fields, named as the Rust shape names
    /// them, and its pads, sorted by offset.
    fn members(&self, at: usize, shape: &Shape<'a>) -> Result<Vec<Member>, GenerateError> {
        let head = &self.heads[at];
        let mut members = Vec::new();
        for field in &head.fields {
            let placed = field
                .source
                .as_ref()
                .and_then(|source| source.kind.as_ref())
                .ok_or_else(|| GenerateError {
                    message: format!("a podbuf field of `{}` has no source", shape.name()),
                })?;
            let (name, ty) = match placed {
                Placed::Field(_) | Placed::Position(_) => {
                    let member = shape.field_of(placed).ok_or_else(|| GenerateError {
                        message: format!(
                            "the podbuf layout of `{}` names a field the type does not have",
                            shape.name()
                        ),
                    })?;
                    let ty = member.r#type.as_ref().ok_or_else(|| GenerateError {
                        message: format!("a field of `{}` carries no type", shape.name()),
                    })?;
                    (
                        ident(snake_of(member.name.as_ref())),
                        self.member_type(field, Some(ty))?,
                    )
                }
                Placed::Flags(index) => (
                    format_ident!("_flags{index}"),
                    Some(quote! { ::core::primitive::u8 }),
                ),
                Placed::Tag(_) => (format_ident!("tag"), self.member_type(field, None)?),
                Placed::Arm(_) => (format_ident!("arm"), self.member_type(field, None)?),
                Placed::Key(_) | Placed::Value(_) => {
                    return fail(format!(
                        "the podbuf head of `{}` holds a map entry's field",
                        shape.name()
                    ));
                }
            };
            members.push(Member {
                name,
                offset: field.offset,
                size: field.size,
                ty,
            });
        }
        for pad in &head.pads {
            let size = Literal::usize_unsuffixed(pad.size as usize);
            members.push(Member {
                name: format_ident!("_pad{}", pad.offset),
                offset: pad.offset,
                size: pad.size,
                ty: Some(quote! { [::core::primitive::u8; #size] }),
            });
        }
        members.sort_by_key(|member| member.offset);
        Ok(members)
    }

    /// The Rust type of one head field, or `None` when the field takes no
    /// byte (an inline head with no field). `ty` is the type position the
    /// field is written from, which names the head struct of an inline struct
    /// or tuple; a union's tag and arm descriptor have none.
    fn member_type(
        &self,
        field: &v1::PodbufField,
        ty: Option<&v1::Type>,
    ) -> Result<Option<TokenStream>, GenerateError> {
        let Some(form) = field.form.as_ref() else {
            return fail("a podbuf field carries no form".to_string());
        };
        self.form_type(form, ty)
    }

    fn form_type(
        &self,
        form: &v1::PodbufForm,
        ty: Option<&v1::Type>,
    ) -> Result<Option<TokenStream>, GenerateError> {
        Ok(Some(match form.kind.as_ref() {
            Some(Form::Boolean(_)) => quote! { ::core::primitive::u8 },
            Some(Form::Int(int)) => Prim::of_int(int.width)?.ty(),
            Some(Form::Float(width)) => Prim::of_float(*width)?.ty(),
            Some(Form::Enum(value)) => Prim::of_int(value.width)?.ty(),
            Some(Form::EnumSet(value)) => Prim::of_int(value.width)?.ty(),
            Some(Form::InlineHead(index)) => {
                if self
                    .heads
                    .get(*index as usize)
                    .is_none_or(|head| head.size == 0)
                {
                    return Ok(None);
                }
                self.inline_head_type(ty)?
            }
            Some(Form::InlineArray(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an inline array carries no element form".to_string(),
                })?;
                let element_ty = match ty.and_then(|ty| ty.kind.as_ref()) {
                    Some(v1::r#type::Kind::Array(inner)) => inner.element.as_deref(),
                    _ => None,
                };
                let Some(element) = self.form_type(element, element_ty)? else {
                    return Ok(None);
                };
                let count = Literal::usize_unsuffixed(array.count as usize);
                quote! { [#element; #count] }
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Array(_)) | Some(Descriptor::Map(_)) => {
                    quote! { ::ridl_rt::podbuf::ArrayDesc }
                }
                Some(Descriptor::Text(_))
                | Some(Descriptor::Bytes(_))
                | Some(Descriptor::Arms(_)) => {
                    quote! { ::ridl_rt::podbuf::Desc }
                }
                None => return fail("a podbuf descriptor carries no kind".to_string()),
            },
            None => return fail("a podbuf field carries no form".to_string()),
        }))
    }

    /// The head struct of the struct, union or tuple a type position names:
    /// `<T>PodbufHead` for one of this package, `crate::<pkg>::<T>PodbufHead`
    /// for one of another package.
    fn inline_head_type(&self, ty: Option<&v1::Type>) -> Result<TokenStream, GenerateError> {
        match ty.and_then(|ty| ty.kind.as_ref()) {
            Some(v1::r#type::Kind::Named(reference)) => {
                let decl = self.resolve(reference)?;
                let name = format!("{}PodbufHead", declared(decl.name.as_ref()));
                Ok(if reference.foreign {
                    type_path(&format!("{}.{name}", reference.package))
                } else {
                    let id = ident(&name);
                    quote! { #id }
                })
            }
            Some(v1::r#type::Kind::Tuple(reference)) => {
                let name = self.ctx.tuple_name(reference.index);
                if name.is_empty() {
                    return fail("an inline tuple has no Rust struct".to_string());
                }
                let id = head_ident(name);
                Ok(quote! { #id })
            }
            _ => {
                fail("an inline podbuf head is written from no struct, union or tuple".to_string())
            }
        }
    }

    /// The path of the type whose `__ridl_podbuf_encode_at` writes a value
    /// of `ty` in place.
    fn encoder_path(&self, ty: &v1::Type) -> Result<TokenStream, GenerateError> {
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Named(reference)) => {
                self.resolve(reference)?;
                Ok(type_path(&reference.reference))
            }
            Some(v1::r#type::Kind::Tuple(reference)) => {
                let name = self.ctx.tuple_name(reference.index);
                if name.is_empty() {
                    return fail("an inline tuple has no Rust struct".to_string());
                }
                let id = ident(name);
                Ok(quote! { #id })
            }
            _ => {
                fail("an inline podbuf head is written from no struct, union or tuple".to_string())
            }
        }
    }

    /// What a scalar type position is, read from the declaration it names.
    /// The result borrows the inline scalar from `ty`, which may be a copy
    /// the caller made of a type position, so its lifetime is `ty`'s.
    fn scalar_of<'t>(&self, ty: &'t v1::Type) -> Result<ScalarShape<'t>, GenerateError>
    where
        'a: 't,
    {
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Named(reference)) => {
                let decl = self.resolve(reference)?;
                match decl.kind.as_ref() {
                    Some(v1::declaration::Kind::Scalar(sc)) => Ok(ScalarShape::Named(
                        declared(decl.name.as_ref()),
                        class_backing(sc.class),
                    )),
                    Some(v1::declaration::Kind::Enum(_) | v1::declaration::Kind::EnumSet(_)) => {
                        Ok(ScalarShape::Discriminant)
                    }
                    _ => fail(format!(
                        "`{}` is not a scalar, an enum or an enum set",
                        reference.reference
                    )),
                }
            }
            Some(v1::r#type::Kind::Inline(sc)) => {
                Ok(ScalarShape::Inline(class_backing(sc.class), Some(sc)))
            }
            Some(v1::r#type::Kind::Primitive(primitive)) => {
                let backing = match v1::PrimitiveType::try_from(*primitive) {
                    Ok(v1::PrimitiveType::Boolean) => ScalarBacking::Boolean,
                    Ok(v1::PrimitiveType::Integer) => ScalarBacking::Integer,
                    Ok(v1::PrimitiveType::Float) => ScalarBacking::Float,
                    _ => return fail("a bare string or bytes has no podbuf form".to_string()),
                };
                Ok(ScalarShape::Inline(backing, None))
            }
            _ => fail("a podbuf scalar is written from no scalar type".to_string()),
        }
    }

    /// The language-layer value (`i64`, `f64` or `bool`) of a scalar, an enum
    /// or an enum set at `operand`.
    fn lang(&self, ty: &v1::Type, operand: &Operand) -> Result<TokenStream, GenerateError> {
        let place = &operand.place;
        let receiver = operand.receiver();
        Ok(match self.scalar_of(ty)? {
            ScalarShape::Named(_, ScalarBacking::String | ScalarBacking::Bytes) => {
                return fail("a string or bytes has no language-layer number".to_string());
            }
            ScalarShape::Named(..) => quote! { #receiver.get() },
            ScalarShape::Discriminant => quote! { ::core::primitive::i64::from(#place) },
            ScalarShape::Inline(..) => quote! { #place },
        })
    }

    /// The `&str` of a string at `operand`, and the named scalar's name and
    /// whether the position is anonymous, for the length check.
    fn text<'t>(
        &self,
        ty: &'t v1::Type,
        operand: &Operand,
    ) -> Result<(TokenStream, TextCheck<'t>), GenerateError>
    where
        'a: 't,
    {
        let receiver = operand.receiver();
        match self.scalar_of(ty)? {
            ScalarShape::Named(name, ScalarBacking::String) => {
                Ok((quote! { #receiver.get() }, TextCheck::Named(name)))
            }
            ScalarShape::Inline(ScalarBacking::String, sc) => {
                Ok((quote! { #receiver.as_str() }, TextCheck::Anonymous(sc)))
            }
            _ => fail("a podbuf string is written from no string type".to_string()),
        }
    }

    /// The `&[u8]` of a bytes value at `operand`, as [`Codec::text`].
    fn bytes<'t>(
        &self,
        ty: &'t v1::Type,
        operand: &Operand,
    ) -> Result<(TokenStream, TextCheck<'t>), GenerateError>
    where
        'a: 't,
    {
        let receiver = operand.receiver();
        match self.scalar_of(ty)? {
            ScalarShape::Named(name, ScalarBacking::Bytes) => {
                Ok((quote! { #receiver.get() }, TextCheck::Named(name)))
            }
            ScalarShape::Inline(ScalarBacking::Bytes, sc) => {
                Ok((quote! { #receiver.as_slice() }, TextCheck::Anonymous(sc)))
            }
            _ => fail("a podbuf bytes field is written from no bytes type".to_string()),
        }
    }

    // -----------------------------------------------------------------
    // The head struct
    // -----------------------------------------------------------------

    /// The `#[repr(C, align(8))]` head struct of head `at`, its members by
    /// offset with the pads among them, and the `const` assertions on its
    /// size, its alignment and every member's offset and size.
    fn head_struct(&self, at: usize, shape: &Shape<'a>) -> Result<TokenStream, GenerateError> {
        let head = &self.heads[at];
        let name = head_ident(shape.name());
        let vis = vis_tokens(shape.visibility());
        let members = self.members(at, shape)?;
        let fields = members.iter().filter_map(|member| {
            let ty = member.ty.as_ref()?;
            let field = &member.name;
            Some(quote! { pub #field: #ty })
        });
        let size = Literal::usize_suffixed(head.size as usize);
        let asserts = members.iter().filter_map(|member| {
            let ty = member.ty.as_ref()?;
            let field = &member.name;
            let offset = Literal::usize_suffixed(member.offset as usize);
            let bytes = Literal::usize_suffixed(member.size as usize);
            Some(quote! {
                assert!(::core::mem::offset_of!(#name, #field) == #offset);
                assert!(::core::mem::size_of::<#ty>() == #bytes);
            })
        });
        let doc = format!(
            " The podbuf head of `{}`: every field at the offset the first-fit law gives \
             it, with each unused byte run written out as a pad. A buffer holds it after \
             the 8-byte preamble; its size is {} bytes.",
            shape.name(),
            head.size
        );
        Ok(quote! {
            #[doc = #doc]
            #[allow(deprecated)]
            #[derive(Clone, Copy, Debug)]
            #[repr(C, align(8))]
            #vis struct #name {
                #(#fields,)*
            }
            const _: () = {
                assert!(::core::mem::size_of::<#name>() == #size);
                assert!(::core::mem::align_of::<#name>() == 8usize);
                #(#asserts)*
            };
        })
    }

    // -----------------------------------------------------------------
    // The encoder
    // -----------------------------------------------------------------

    /// `impl T { __ridl_podbuf_encode, __ridl_podbuf_encode_at }` for head
    /// `at`.
    fn encoder(&self, at: usize, shape: &Shape<'a>) -> Result<TokenStream, GenerateError> {
        let ty = ident(shape.name());
        let size = Literal::u32_suffixed(self.heads[at].size);
        let base = format_ident!("__at");
        let body = match shape {
            Shape::Union(decl, ud) => self.union_body(at, decl, ud, &base)?,
            Shape::Struct(..) | Shape::Tuple(_) => self.head_body(at, shape, &base)?,
        };
        let used = !body.is_empty();
        let writer = if used {
            format_ident!("w")
        } else {
            format_ident!("_w")
        };
        let at_param = if used { base } else { format_ident!("_at") };
        Ok(quote! {
            #[allow(deprecated)]
            impl #ty {
                // Writes the preamble, this value's head and its tail into
                // `w`. An internal of the podbuf codec: no doc.
                pub(crate) fn __ridl_podbuf_encode(
                    &self,
                    w: &mut ::ridl_rt::podbuf::Writer<'_>,
                ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
                    let __at = w.reserve_head(#size)?;
                    self.__ridl_podbuf_encode_at(w, __at)
                }

                // Writes this value's head at offset `__at` of the buffer,
                // which `w` has reserved and zeroed, and appends its tail.
                pub(crate) fn __ridl_podbuf_encode_at(
                    &self,
                    #writer: &mut ::ridl_rt::podbuf::Writer<'_>,
                    #at_param: ::core::primitive::usize,
                ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
                    #body
                    ::core::result::Result::Ok(())
                }
            }
        })
    }

    /// The statements that write the fields of head `at` from `self`, in
    /// offset order, each at `base + offset`.
    fn head_body(
        &self,
        at: usize,
        shape: &Shape<'a>,
        base: &Ident,
    ) -> Result<TokenStream, GenerateError> {
        let head = &self.heads[at];
        let owner = shape.name();
        let mut preludes: Vec<TokenStream> = Vec::new();
        let mut slots: Vec<Slot<'_>> = Vec::new();
        for (index, field) in head.fields.iter().enumerate() {
            let placed = field
                .source
                .as_ref()
                .and_then(|source| source.kind.as_ref())
                .ok_or_else(|| GenerateError {
                    message: format!("a podbuf field of `{owner}` has no source"),
                })?;
            let value = match placed {
                Placed::Field(_) | Placed::Position(_) => {
                    let member = shape.field_of(placed).ok_or_else(|| GenerateError {
                        message: format!(
                            "the podbuf layout of `{owner}` names a field the type does not have"
                        ),
                    })?;
                    let ty = member.r#type.clone().ok_or_else(|| GenerateError {
                        message: format!("a field of `{owner}` carries no type"),
                    })?;
                    if member.deprecated.is_some() {
                        // §8.1: a deprecated field is written as its init
                        // value, so a reader that still reads it sees a
                        // valid value. With no derivable init the bytes stay
                        // zero; the checker refuses that case when the type
                        // excludes 0 (RIDL-155).
                        match defaults::field_default_expr(self.ctx, member) {
                            Some(expr) => {
                                let binding = format_ident!("__dep{index}");
                                let rust_ty = model_type_tokens(self.ctx, &ty);
                                preludes.push(quote! { let #binding: #rust_ty = #expr; });
                                Some((Operand::owned(quote! { #binding }), ty))
                            }
                            None => None,
                        }
                    } else {
                        let name = ident(snake_of(member.name.as_ref()));
                        Some((Operand::owned(quote! { self.#name }), ty))
                    }
                }
                Placed::Flags(_) => None,
                Placed::Tag(_) | Placed::Arm(_) => {
                    return fail(format!(
                        "the podbuf head of `{owner}` holds a union's field"
                    ));
                }
                Placed::Key(_) | Placed::Value(_) => {
                    return fail(format!(
                        "the podbuf head of `{owner}` holds a map entry's field"
                    ));
                }
            };
            slots.push(Slot { field, value });
        }
        let mut writes: Vec<TokenStream> = Vec::new();
        for slot in &slots {
            let placed = slot.field.source.as_ref().and_then(|s| s.kind.as_ref());
            if let Some(Placed::Flags(flags)) = placed {
                writes.push(self.flags_write(&slots, *flags, offset_at(base, slot.field.offset)));
                continue;
            }
            let Some((operand, ty)) = slot.value.as_ref() else {
                continue;
            };
            writes.push(self.field_write(
                slot.field,
                ty,
                operand,
                offset_at(base, slot.field.offset),
                owner,
            )?);
        }
        Ok(quote! { #(#preludes)* #(#writes)* })
    }

    /// The statement that writes flags byte `flags`: bit `i` set when the
    /// optional field it records is present. A deprecated optional field with
    /// no derivable init is absent.
    fn flags_write(&self, slots: &[Slot<'_>], flags: u32, at: TokenStream) -> TokenStream {
        let bits: Vec<TokenStream> = slots
            .iter()
            .filter_map(|slot| {
                let bit = slot
                    .field
                    .optional
                    .as_ref()
                    .filter(|bit| bit.flags == flags)?;
                let (operand, _) = slot.value.as_ref()?;
                let receiver = operand.receiver();
                // Bit 0 is `1u8` itself: a shift by 0 is `clippy::identity_op`
                // in a consumer's build.
                let mask = if bit.bit == 0 {
                    quote! { 1u8 }
                } else {
                    let shift = Literal::u32_unsuffixed(bit.bit);
                    quote! { 1u8 << #shift }
                };
                Some(quote! { if #receiver.is_some() { __f |= #mask; } })
            })
            .collect();
        if bits.is_empty() {
            return TokenStream::new();
        }
        quote! {
            {
                let mut __f = 0u8;
                #(#bits)*
                w.put_u8(#at, __f);
            }
        }
    }

    /// The statements that write one field at `at`: a present optional is
    /// written through its binding, an absent one leaves its bytes zero.
    fn field_write(
        &self,
        field: &v1::PodbufField,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let form = field.form.as_ref().ok_or_else(|| GenerateError {
            message: format!("a podbuf field of `{owner}` carries no form"),
        })?;
        if field.optional.is_none() {
            return self.write_form(form, ty, operand, at, owner);
        }
        let inner = Self::present(ty);
        let write = self.write_form(form, &inner, &Operand::borrowed(quote! { __o }), at, owner)?;
        if write.is_empty() {
            return Ok(write);
        }
        let reference = &operand.reference;
        Ok(quote! {
            if let ::core::option::Option::Some(__o) = #reference {
                #write
            }
        })
    }

    /// The statements that write a present value of `form` at `at`, from
    /// the type position `ty` at `operand`, and append its tail data.
    fn write_form(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let reference = &operand.reference;
        Ok(match form.kind.as_ref() {
            Some(Form::Boolean(_)) => {
                let lang = self.lang(ty, operand)?;
                quote! { w.put_u8(#at, ::core::primitive::u8::from(#lang)); }
            }
            Some(Form::Int(int)) => {
                let prim = Prim::of_int(int.width)?;
                let put = prim.put();
                let raw = prim.cast(self.lang(ty, operand)?);
                quote! { w.#put(#at, #raw); }
            }
            Some(Form::Float(width)) => {
                let prim = Prim::of_float(*width)?;
                let put = prim.put();
                let raw = prim.cast(self.lang(ty, operand)?);
                quote! { w.#put(#at, #raw); }
            }
            Some(Form::Enum(value)) => {
                let prim = Prim::of_int(value.width)?;
                let put = prim.put();
                let raw = prim.cast(self.lang(ty, operand)?);
                quote! { w.#put(#at, #raw); }
            }
            Some(Form::EnumSet(value)) => {
                let prim = Prim::of_int(value.width)?;
                let put = prim.put();
                let raw = prim.cast(self.lang(ty, operand)?);
                quote! { w.#put(#at, #raw); }
            }
            Some(Form::InlineHead(index)) => {
                if self
                    .heads
                    .get(*index as usize)
                    .is_none_or(|head| head.size == 0)
                {
                    // A head with no field writes nothing.
                    return Ok(TokenStream::new());
                }
                let path = self.encoder_path(ty)?;
                quote! { #path::__ridl_podbuf_encode_at(#reference, w, #at)?; }
            }
            Some(Form::InlineArray(array)) => {
                self.inline_array_write(array, ty, operand, at, owner)?
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Text(_)) => self.text_write(ty, operand, at, owner)?,
                Some(Descriptor::Bytes(_)) => self.bytes_write(ty, operand, at, owner)?,
                Some(Descriptor::Array(array)) => {
                    self.array_write(array, ty, operand, at, owner)?
                }
                Some(Descriptor::Map(map)) => self.map_write(map, ty, operand, at, owner)?,
                Some(Descriptor::Arms(_)) => {
                    return fail(format!(
                        "the podbuf layout of `{owner}` holds an arm descriptor outside a union"
                    ));
                }
                None => return fail("a podbuf descriptor carries no kind".to_string()),
            },
            None => return fail("a podbuf field carries no form".to_string()),
        })
    }

    /// An exact-length array `[T; N]` inline: each element at its stride. An
    /// exact-length `bytes` is a `Vec<u8>` in Rust, so its length is checked
    /// before its bytes are copied in.
    fn inline_array_write(
        &self,
        array: &v1::PodbufInlineArray,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let element = array.element.as_deref().ok_or_else(|| GenerateError {
            message: "an inline array carries no element form".to_string(),
        })?;
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Array(inner)) => {
                let element_ty = inner.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an array carries no element type".to_string(),
                })?;
                let stride = self.form_size(element);
                let write = self.write_form(
                    element,
                    element_ty,
                    &Operand::borrowed(quote! { __e }),
                    quote! { __ea },
                    owner,
                )?;
                if write.is_empty() {
                    return Ok(write);
                }
                let receiver = operand.receiver();
                let step = stride_at(&at, stride);
                Ok(quote! {
                    for (__i, __e) in #receiver.iter().enumerate() {
                        let __ea = #step;
                        #write
                    }
                })
            }
            _ => {
                let (bytes, check) = self.bytes(ty, operand)?;
                let count = Literal::usize_suffixed(array.count as usize);
                let refused = contract_error(&check.owner(owner), "Length");
                Ok(quote! {
                    {
                        let __b: &[::core::primitive::u8] = #bytes;
                        if __b.len() != #count {
                            #refused
                        }
                        w.put_bytes(#at, __b);
                    }
                })
            }
        }
    }

    /// A bounded string: the descriptor at `at`, the UTF-8 bytes and one NUL
    /// in the tail. An empty string is a zero descriptor with no tail. A
    /// string holding U+0000 is refused (§6): its NUL terminator could not
    /// be told from its data.
    fn text_write(
        &self,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let (text, check) = self.text(ty, operand)?;
        let length = check.length_check(owner, true);
        let nul = contract_error(&check.owner(owner), "Pattern");
        Ok(quote! {
            {
                let __s: &::core::primitive::str = #text;
                #length
                if __s.as_bytes().contains(&0u8) {
                    #nul
                }
                if !__s.is_empty() {
                    let __n = __s.len();
                    let __off = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                    w.put_bytes(__off, __s.as_bytes());
                    w.put_desc(#at, ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    });
                }
            }
        })
    }

    /// Bounded bytes: the descriptor at `at` and the bytes in the tail; empty
    /// bytes are a zero descriptor with no tail.
    fn bytes_write(
        &self,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let (bytes, check) = self.bytes(ty, operand)?;
        let length = check.length_check(owner, false);
        Ok(quote! {
            {
                let __s: &[::core::primitive::u8] = #bytes;
                #length
                if !__s.is_empty() {
                    let __n = __s.len();
                    let __off = w.tail(__n as ::core::primitive::u64, 1usize)?;
                    w.put_bytes(__off, __s);
                    w.put_desc(#at, ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    });
                }
            }
        })
    }

    /// A bounded array: the count is checked against its bound, the block
    /// of elements is reserved at the element's alignment, the descriptor is
    /// written, and each element is written at its stride, its own tail
    /// after the block (§14). An empty array is a zero descriptor.
    fn array_write(
        &self,
        array: &v1::PodbufArray,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let element = array.element.as_deref().ok_or_else(|| GenerateError {
            message: "an array carries no element form".to_string(),
        })?;
        let Some(v1::r#type::Kind::Array(inner)) = ty.kind.as_ref() else {
            return fail(format!(
                "a podbuf array of `{owner}` is written from no array type"
            ));
        };
        let element_ty = inner.element.as_deref().ok_or_else(|| GenerateError {
            message: "an array carries no element type".to_string(),
        })?;
        let count = encode_count_check(owner, array.min, array.max, &quote! { __c });
        let write = self.write_form(
            element,
            element_ty,
            &Operand::borrowed(quote! { __e }),
            quote! { __ea },
            owner,
        )?;
        let reference = &operand.reference;
        let block = self.block(
            array.elem_size,
            self.form_align(element),
            quote! { __c },
            &at,
            owner,
        )?;
        let step = stride_at(&quote! { __off }, array.elem_size);
        Ok(quote! {
            {
                let __c = #reference;
                #count
                if !__c.is_empty() {
                    #block
                    for (__i, __e) in __c.iter().enumerate() {
                        let __ea = #step;
                        #write
                    }
                }
            }
        })
    }

    /// A map: the count is checked, a NaN key is refused, the entries are
    /// sorted by key in the order of §6 (bytewise for a string or bytes,
    /// numeric otherwise) and a duplicate key is refused, then the block of
    /// entry heads is written at alignment 8, each entry `{ key, value }` at
    /// its stride with its own tail after the block.
    fn map_write(
        &self,
        map: &v1::PodbufMap,
        ty: &v1::Type,
        operand: &Operand,
        at: TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let Some(v1::r#type::Kind::Map(inner)) = ty.kind.as_ref() else {
            return fail(format!(
                "a podbuf map of `{owner}` is written from no map type"
            ));
        };
        let key_ty = inner.key.as_deref().ok_or_else(|| GenerateError {
            message: "a map carries no key type".to_string(),
        })?;
        let value_ty = inner.value.as_deref().ok_or_else(|| GenerateError {
            message: "a map carries no value type".to_string(),
        })?;
        let entry = self
            .heads
            .get(map.entry_head as usize)
            .ok_or_else(|| GenerateError {
                message: "the podbuf layout names an entry head it does not carry".to_string(),
            })?;
        let count = encode_count_check(owner, map.min, map.max, &quote! { __c });
        let order = self.key_order(key_ty, owner)?;
        let nan = order.nan;
        let compare = order.compare;
        let equal = order.equal;
        let unique = contract_error(owner, "Unique");
        let mut writes: Vec<TokenStream> = Vec::new();
        for field in &entry.fields {
            let form = field.form.as_ref().ok_or_else(|| GenerateError {
                message: format!("a podbuf field of `{owner}` carries no form"),
            })?;
            let (position, position_ty) = match field
                .source
                .as_ref()
                .and_then(|source| source.kind.as_ref())
            {
                Some(Placed::Key(_)) => (quote! { __e.0 }, key_ty),
                Some(Placed::Value(_)) => (quote! { __e.1 }, value_ty),
                _ => {
                    return fail(format!(
                        "the podbuf entry head of `{owner}` holds a field that is neither \
                             the key nor the value"
                    ));
                }
            };
            writes.push(self.write_form(
                form,
                position_ty,
                &Operand::owned(position),
                offset_at(&format_ident!("__ea"), field.offset),
                owner,
            )?);
        }
        let reference = &operand.reference;
        let block = self.block(map.elem_size, 8, quote! { __s }, &at, owner)?;
        let step = stride_at(&quote! { __off }, map.elem_size);
        Ok(quote! {
            {
                let __c = #reference;
                #count
                #nan
                let mut __s: ::std::vec::Vec<_> = __c.iter().collect();
                __s.sort_by(|__a, __b| #compare);
                for __p in __s.windows(2) {
                    if #equal {
                        #unique
                    }
                }
                if !__s.is_empty() {
                    #block
                    for (__i, __e) in __s.iter().enumerate() {
                        let __ea = #step;
                        #(#writes)*
                    }
                }
            }
        })
    }

    /// Reserves the block of `collection.len()` elements at `stride` bytes
    /// each, at `align`, binding its offset to `__off`, and writes the array
    /// descriptor at `#at`. The caller has checked the count against its
    /// bound, and the checker has bounded the count and the stride by the
    /// descriptor's `u16` fields (RIDL-154), so the narrowing casts hold.
    fn block(
        &self,
        stride: u32,
        align: u32,
        collection: TokenStream,
        at: &TokenStream,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let Ok(stride_u16) = u16::try_from(stride) else {
            return fail(format!(
                "`{owner}` holds an array or map whose element is {stride} bytes, past the \
                 descriptor's limit of {} bytes",
                u16::MAX
            ));
        };
        let stride_lit = Literal::usize_suffixed(stride as usize);
        let stride_u16 = Literal::u16_suffixed(stride_u16);
        let align = Literal::usize_suffixed(align as usize);
        Ok(quote! {
            let __off = w.tail(
                (#collection.len() * #stride_lit) as ::core::primitive::u64,
                #align,
            )?;
            w.put_array_desc(#at, ::ridl_rt::podbuf::ArrayDesc {
                off: __off as ::core::primitive::u32,
                len: #collection.len() as ::core::primitive::u16,
                elem_size: #stride_u16,
            });
        })
    }

    /// How two keys of a map compare and whether one is NaN, over the
    /// bindings `__a`, `__b` (two entries, in `sort_by`), `__p` (a window of
    /// two sorted entries) and `__e` (one entry). The comparison is written
    /// as a trait call, because the method form inside `sort_by` is
    /// `clippy::unnecessary_sort_by` in a consumer's build, and a key that
    /// borrows from its entry cannot move to `sort_by_key`.
    fn key_order(&self, key_ty: &v1::Type, owner: &str) -> Result<KeyOrder, GenerateError> {
        let a = Operand::owned(quote! { __a.0 });
        let b = Operand::owned(quote! { __b.0 });
        let p0 = Operand::owned(quote! { __p[0].0 });
        let p1 = Operand::owned(quote! { __p[1].0 });
        let shape = self.scalar_of(key_ty).map_err(|_| GenerateError {
            message: format!(
                "a podbuf map key of `{owner}` must be a string, bytes, a boolean, an integer, \
                 a float, an enum or an enum set"
            ),
        })?;
        let backing = match shape {
            ScalarShape::Named(_, backing) | ScalarShape::Inline(backing, _) => Some(backing),
            ScalarShape::Discriminant => None,
        };
        Ok(match backing {
            Some(ScalarBacking::String) => {
                let (ta, _) = self.text(key_ty, &a)?;
                let (tb, _) = self.text(key_ty, &b)?;
                let (t0, _) = self.text(key_ty, &p0)?;
                let (t1, _) = self.text(key_ty, &p1)?;
                KeyOrder {
                    compare: quote! { ::core::cmp::Ord::cmp(#ta.as_bytes(), #tb.as_bytes()) },
                    equal: quote! { #t0 == #t1 },
                    nan: TokenStream::new(),
                }
            }
            Some(ScalarBacking::Bytes) => {
                let (ba, _) = self.bytes(key_ty, &a)?;
                let (bb, _) = self.bytes(key_ty, &b)?;
                let (b0, _) = self.bytes(key_ty, &p0)?;
                let (b1, _) = self.bytes(key_ty, &p1)?;
                KeyOrder {
                    compare: quote! { ::core::cmp::Ord::cmp(#ba, #bb) },
                    equal: quote! { #b0 == #b1 },
                    nan: TokenStream::new(),
                }
            }
            Some(ScalarBacking::Float) => {
                let la = self.lang(key_ty, &a)?;
                let lb = self.lang(key_ty, &b)?;
                let l0 = self.lang(key_ty, &p0)?;
                let l1 = self.lang(key_ty, &p1)?;
                let le = self.lang(key_ty, &Operand::owned(quote! { __e.0 }))?;
                let refused = contract_error(owner, "Range");
                KeyOrder {
                    // A NaN key is refused first, so the partial order is
                    // total here; `+0.0` and `-0.0` compare equal and are
                    // one key (§6).
                    compare: quote! {
                        ::core::cmp::PartialOrd::partial_cmp(&#la, &#lb)
                            .unwrap_or(::core::cmp::Ordering::Equal)
                    },
                    equal: quote! { #l0 == #l1 },
                    nan: quote! {
                        for __e in __c.iter() {
                            if #le.is_nan() {
                                #refused
                            }
                        }
                    },
                }
            }
            Some(ScalarBacking::Integer) | Some(ScalarBacking::Boolean) | None => {
                let la = self.lang(key_ty, &a)?;
                let lb = self.lang(key_ty, &b)?;
                let l0 = self.lang(key_ty, &p0)?;
                let l1 = self.lang(key_ty, &p1)?;
                KeyOrder {
                    compare: quote! { ::core::cmp::Ord::cmp(&#la, &#lb) },
                    equal: quote! { #l0 == #l1 },
                    nan: TokenStream::new(),
                }
            }
        })
    }

    /// The `match` over a union's arms: the tag, then the arm's head in the
    /// tail behind the arm descriptor. An arm whose head has no field is a
    /// zero descriptor (§14 "Empty union arm").
    fn union_body(
        &self,
        at: usize,
        decl: &v1::Declaration,
        ud: &v1::Union,
        base: &Ident,
    ) -> Result<TokenStream, GenerateError> {
        let head = &self.heads[at];
        let owner = declared(decl.name.as_ref());
        let ty = ident(owner);
        let placed = |field: &'a v1::PodbufField| {
            field
                .source
                .as_ref()
                .and_then(|source| source.kind.as_ref())
        };
        let tag = head
            .fields
            .iter()
            .find(|field| matches!(placed(field), Some(Placed::Tag(_))))
            .ok_or_else(|| GenerateError {
                message: format!("the podbuf head of `{owner}` has no tag"),
            })?;
        let tag_prim = match tag.form.as_ref().and_then(|form| form.kind.as_ref()) {
            Some(Form::Int(int)) => Prim::of_int(int.width)?,
            _ => return fail(format!("the podbuf tag of `{owner}` is not an integer")),
        };
        let arm_field = head
            .fields
            .iter()
            .find(|field| matches!(placed(field), Some(Placed::Arm(_))))
            .ok_or_else(|| GenerateError {
                message: format!("the podbuf head of `{owner}` has no arm descriptor"),
            })?;
        let arms = match arm_field.form.as_ref().and_then(|form| form.kind.as_ref()) {
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Arms(arms)) => &arms.arms,
                _ => {
                    return fail(format!(
                        "the podbuf arm field of `{owner}` is no arm descriptor"
                    ));
                }
            },
            _ => {
                return fail(format!(
                    "the podbuf arm field of `{owner}` is no descriptor"
                ));
            }
        };
        let tag_at = offset_at(base, tag.offset);
        let desc_at = offset_at(base, arm_field.offset);
        let put_tag = tag_prim.put();
        let mut matches: Vec<TokenStream> = Vec::new();
        for arm in &ud.arms {
            let variant = ident(camel_of(arm.name.as_ref()));
            let ordinal = tag_prim.literal(arm.ordinal);
            let arm_head = arms
                .iter()
                .find(|candidate| candidate.ordinal == arm.ordinal)
                .and_then(|candidate| self.heads.get(candidate.head as usize))
                .ok_or_else(|| GenerateError {
                    message: format!(
                        "the podbuf layout of `{owner}` has no head for arm `{}`",
                        declared(arm.name.as_ref())
                    ),
                })?;
            let write_tag = quote! { w.#put_tag(#tag_at, #ordinal); };
            if arm_head.size == 0 {
                matches.push(quote! { #ty::#variant(_) => { #write_tag } });
                continue;
            }
            let reference = arm.r#type.as_ref().ok_or_else(|| GenerateError {
                message: format!(
                    "arm `{}` of `{owner}` names no type",
                    declared(arm.name.as_ref())
                ),
            })?;
            let arm_ty = v1::Type {
                optional: false,
                kind: Some(v1::r#type::Kind::Named(reference.clone())),
            };
            let payload = match self.resolve(reference)?.kind.as_ref() {
                Some(v1::declaration::Kind::Struct(_) | v1::declaration::Kind::Union(_)) => {
                    let path = type_path(&reference.reference);
                    quote! { #path::__ridl_podbuf_encode_at(__a, w, __off)?; }
                }
                _ => {
                    // A scalar, enum or enum set arm: the one value field of
                    // the arm's head.
                    let value = arm_head
                        .fields
                        .iter()
                        .find(|field| matches!(placed(field), Some(Placed::Arm(_))))
                        .ok_or_else(|| GenerateError {
                            message: format!("the podbuf arm head of `{owner}` has no value field"),
                        })?;
                    let form = value.form.as_ref().ok_or_else(|| GenerateError {
                        message: format!("a podbuf field of `{owner}` carries no form"),
                    })?;
                    self.write_form(
                        form,
                        &arm_ty,
                        &Operand::borrowed(quote! { __a }),
                        offset_at(&format_ident!("__off"), value.offset),
                        owner,
                    )?
                }
            };
            let size = Literal::u32_suffixed(arm_head.size);
            let size_bytes = Literal::u64_suffixed(u64::from(arm_head.size));
            matches.push(quote! {
                #ty::#variant(__a) => {
                    #write_tag
                    let __off = w.tail(#size_bytes, 8usize)?;
                    w.put_desc(#desc_at, ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: #size,
                    });
                    #payload
                }
            });
        }
        Ok(quote! {
            match self {
                #(#matches)*
            }
        })
    }
}

/// What a scalar type position is: a named scalar (its name, backing and
/// shape), an enum or enum set, or an inline or primitive scalar.
enum ScalarShape<'a> {
    Named(&'a str, ScalarBacking),
    Discriminant,
    Inline(ScalarBacking, Option<&'a v1::Scalar>),
}

/// Which length check a string or bytes value gets before it is written: a
/// named scalar keeps its constructor's check and its `new_unchecked`
/// opt-out, as the other codecs leave it; an anonymous one is checked here
/// against its declared bound (`crate::codec::inline_length_check`).
enum TextCheck<'a> {
    Named(&'a str),
    Anonymous(Option<&'a v1::Scalar>),
}

impl TextCheck<'_> {
    /// The type a violation names: the named scalar, or the enclosing type.
    fn owner(&self, enclosing: &str) -> String {
        match self {
            TextCheck::Named(name) => (*name).to_string(),
            TextCheck::Anonymous(_) => enclosing.to_string(),
        }
    }

    fn length_check(&self, enclosing: &str, text: bool) -> TokenStream {
        match self {
            TextCheck::Named(_) => TokenStream::new(),
            TextCheck::Anonymous(sc) => inline_length_check(enclosing, true, *sc, text),
        }
    }
}

/// The comparisons of two map keys ([`Codec::key_order`]).
struct KeyOrder {
    compare: TokenStream,
    equal: TokenStream,
    nan: TokenStream,
}

/// `base + offset`, or `base` alone at offset 0, which `clippy::identity_op`
/// would otherwise flag in a consumer's build.
fn offset_at(base: &Ident, offset: u32) -> TokenStream {
    if offset == 0 {
        return quote! { #base };
    }
    let offset = Literal::usize_suffixed(offset as usize);
    quote! { #base + #offset }
}

/// `start + __i * stride`, for the element at index `__i` of a block.
fn stride_at(start: &TokenStream, stride: u32) -> TokenStream {
    if stride == 1 {
        return quote! { #start + __i };
    }
    let stride = Literal::usize_suffixed(stride as usize);
    quote! { #start + __i * #stride }
}

/// The `return Err(EncodeError::Contract(..))` statement for `rule` on
/// `owner`.
fn contract_error(owner: &str, rule: &str) -> TokenStream {
    let rule = format_ident!("{rule}");
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::EncodeError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some(#owner),
                rule: ::ridl_rt::payload::Rule::#rule,
            },
        ));
    }
}

fn int_bytes(width: i32) -> u32 {
    match v1::IntWidth::try_from(width) {
        Ok(v1::IntWidth::U8 | v1::IntWidth::I8) => 1,
        Ok(v1::IntWidth::U16 | v1::IntWidth::I16) => 2,
        Ok(v1::IntWidth::U32 | v1::IntWidth::I32) => 4,
        _ => 8,
    }
}

fn float_bytes(width: i32) -> u32 {
    if width == v1::FloatWidth::F32 as i32 {
        4
    } else {
        8
    }
}
