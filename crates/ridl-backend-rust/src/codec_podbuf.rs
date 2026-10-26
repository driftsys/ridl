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
//! Per type `T`, the codec emits `TPodbufHead`, the head struct;
//! `TPodbufView<'a>`, the read-in-place view with one accessor per field
//! (`TPodbufArm<'a>` beside it for a union, one variant per arm); and
//! `impl T { __ridl_podbuf_encode, __ridl_podbuf_encode_at,
//! __ridl_podbuf_verify, __ridl_podbuf_verify_at, __ridl_podbuf_decode }`.
//! The `_at` forms work at a given head offset, which is how a nested struct,
//! an array element and a union arm are written and checked in place. The
//! `Payload<Podbuf>` implementation over them is not emitted yet
//! (driftsys/ridl#317).
//!
//! `verify` checks the rules of §7 in one pass over the data present: the
//! preamble, each boolean, enum, enum set and flags byte, each typl
//! constraint, each descriptor's bounds, stride, terminator and key order,
//! and each element, entry and arm with the same rules. It is not recursive:
//! a nested head is checked by that type's own `__ridl_podbuf_verify_at`,
//! and typl has no recursive shape, so the call depth is bounded by the type
//! (§14 "Verify depth"). Pad bytes, uncovered tail bytes and overlapping
//! descriptors are not checked, by design. The view's accessors and `verify`
//! read through `ridl_rt::podbuf::Region`, which reads 0 past the size the
//! producer wrote (§8.1), and allocate nothing; `decode` allocates for a
//! `String`, a `Vec` and a map as the other codecs do.
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

use crate::codec::{
    NamedScalar, Operand, count_check_of, encode_count_check, inline_length_check,
    named_scalar_check, scalar_constraint_check,
};
use crate::transcode::PayloadOwner;
use crate::{
    Ctx, GenerateError, ScalarBacking, allow_dead_code_if_internal, camel_of, class_backing,
    declared, defaults, ident, model_type_tokens, pascal_of, snake_of, tuple_name, type_path,
    vis_tokens,
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

/// `<owner>PodbufArm`, the enum of arm views a union's view hands back.
pub(crate) fn arm_ident(owner: &str) -> Ident {
    ident(&format!("{owner}PodbufArm"))
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

    /// The `Region` load of this primitive.
    fn at(self) -> Ident {
        format_ident!("{}_at", self.0)
    }

    /// `raw`, a load of this primitive, as the language-layer `i64` or
    /// `f64`. An `i64` and an `f64` need no conversion.
    fn widen(self, raw: TokenStream) -> TokenStream {
        match self.0 {
            "i64" | "f64" => raw,
            "u64" => quote! { #raw as ::core::primitive::i64 },
            "f32" => quote! { ::core::primitive::f64::from(#raw) },
            _ => quote! { ::core::primitive::i64::from(#raw) },
        }
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
            let view = self.view(at, &shape)?;
            let verifier = self.verifier(at, &shape)?;
            let decoder = self.decoder(at, &shape)?;
            items.push(allow_dead_code_if_internal(
                shape.visibility(),
                quote! { #head_struct #encoder #view #verifier #decoder },
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
                let path = type_path(&reference.reference);
                match decl.kind.as_ref() {
                    Some(v1::declaration::Kind::Scalar(sc)) => Ok(ScalarShape::Named {
                        name: declared(decl.name.as_ref()),
                        backing: class_backing(sc.class),
                        scalar: sc,
                        path,
                        reference: &reference.reference,
                    }),
                    Some(v1::declaration::Kind::Enum(ed)) => Ok(ScalarShape::Enum(ed, path)),
                    Some(v1::declaration::Kind::EnumSet(_)) => Ok(ScalarShape::EnumSet(path)),
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
            ScalarShape::Named {
                backing: ScalarBacking::String | ScalarBacking::Bytes,
                ..
            } => {
                return fail("a string or bytes has no language-layer number".to_string());
            }
            ScalarShape::Named { .. } => quote! { #receiver.get() },
            ScalarShape::Enum(..) | ScalarShape::EnumSet(_) => {
                quote! { ::core::primitive::i64::from(#place) }
            }
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
            ScalarShape::Named {
                name,
                backing: ScalarBacking::String,
                ..
            } => Ok((quote! { #receiver.get() }, TextCheck::Named(name))),
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
            ScalarShape::Named {
                name,
                backing: ScalarBacking::Bytes,
                ..
            } => Ok((quote! { #receiver.get() }, TextCheck::Named(name))),
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
        // A unit stride is not written: `len() * 1` is `clippy::identity_op`
        // in a consumer's build.
        let bytes = if stride == 1 {
            quote! { #collection.len() }
        } else {
            let stride_lit = Literal::usize_suffixed(stride as usize);
            quote! { (#collection.len() * #stride_lit) }
        };
        let stride_u16 = Literal::u16_suffixed(stride_u16);
        let align = Literal::usize_suffixed(align as usize);
        Ok(quote! {
            let __off = w.tail(#bytes as ::core::primitive::u64, #align)?;
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
            ScalarShape::Named { backing, .. } | ScalarShape::Inline(backing, _) => Some(backing),
            ScalarShape::Enum(..) | ScalarShape::EnumSet(_) => None,
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

// ---------------------------------------------------------------------------
// The view, the verifier and the decoder
// ---------------------------------------------------------------------------

/// Where a head's bytes are read from: the buffer, the offset of the head in
/// it, and the size the producer wrote for it. Every load goes through a
/// `Region` over the three, which reads 0 past `size` (§8.1).
#[derive(Clone)]
struct Place {
    buf: TokenStream,
    at: TokenStream,
    size: TokenStream,
}

impl Place {
    fn new(buf: TokenStream, at: TokenStream, size: TokenStream) -> Place {
        Place { buf, at, size }
    }

    /// The `Region` over this place.
    fn region(&self) -> TokenStream {
        let (buf, at, size) = (&self.buf, &self.at, &self.size);
        quote! { ::ridl_rt::podbuf::Region::new(#buf, #at, #size) }
    }

    /// The place of a head inline at `off` of this one: its bytes past the
    /// producer's size are the parent's.
    fn inline(&self, off: &Off) -> Place {
        let at = off.from(&self.at);
        let size = &self.size;
        let size = match off {
            Off::Lit(0) => size.clone(),
            _ => {
                let off = off.tokens();
                quote! { #size.saturating_sub(#off) }
            }
        };
        Place::new(self.buf.clone(), at, size)
    }

    /// The place of a tail block the descriptor `__d` names: `off` bytes
    /// into the buffer, `len` bytes long.
    fn of_desc(buf: &TokenStream) -> Place {
        Place::new(
            buf.clone(),
            quote! { __d.off as ::core::primitive::usize },
            quote! { __d.len as ::core::primitive::usize },
        )
    }

    /// The place of element `__i` of the block the array descriptor `__d`
    /// names, bound to `__es` and `__ez`.
    fn element_bindings() -> TokenStream {
        quote! {
            let __ez = ::core::primitive::usize::from(__d.elem_size);
            let __es = (__d.off as ::core::primitive::usize).saturating_add(__i.saturating_mul(__ez));
        }
    }

    fn element(buf: &TokenStream) -> Place {
        Place::new(buf.clone(), quote! { __es }, quote! { __ez })
    }
}

/// An offset into a place: a literal of the layout, or an expression a loop
/// computes. Kept apart so that a zero literal and a unit stride are not
/// written, which `clippy::identity_op` refuses in a consumer's build.
#[derive(Clone)]
enum Off {
    Lit(u32),
    Expr(TokenStream),
}

impl Off {
    fn tokens(&self) -> TokenStream {
        match self {
            Off::Lit(n) => {
                let n = Literal::usize_suffixed(*n as usize);
                quote! { #n }
            }
            Off::Expr(expr) => expr.clone(),
        }
    }

    /// `base + self`.
    fn from(&self, base: &TokenStream) -> TokenStream {
        match self {
            Off::Lit(0) => base.clone(),
            _ => {
                let off = self.tokens();
                quote! { #base + #off }
            }
        }
    }

    /// `self + __i * stride`, for element `__i` of an inline array.
    fn index(&self, stride: u32) -> Off {
        let step = if stride == 1 {
            quote! { __i }
        } else {
            let stride = Literal::usize_suffixed(stride as usize);
            quote! { __i * #stride }
        };
        match self {
            Off::Lit(0) => Off::Expr(step),
            _ => {
                let base = self.tokens();
                Off::Expr(quote! { #base + #step })
            }
        }
    }
}

/// The accessor of one field of a view: its name, its return type and the
/// expression that reads it.
struct Accessor {
    name: Ident,
    ty: TokenStream,
    read: TokenStream,
}

/// The `return Err(VerifyError::Structure(Malformed::<variant>))` statement.
fn malformed(variant: &str) -> TokenStream {
    let variant = format_ident!("{variant}");
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Structure(
            ::ridl_rt::payload::Malformed::#variant,
        ));
    }
}

/// The `return Err(VerifyError::Contract(..))` statement for `rule` on
/// `owner`.
fn violation(owner: &str, rule: &str) -> TokenStream {
    let rule = format_ident!("{rule}");
    quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some(#owner),
                rule: ::ridl_rt::payload::Rule::#rule,
            },
        ));
    }
}

impl<'a> Codec<'a> {
    /// The fields of head `at` a view has an accessor for, with the type
    /// position each is read from. A flags byte has none.
    fn view_fields(
        &self,
        at: usize,
        shape: &Shape<'a>,
    ) -> Result<Vec<(&'a v1::PodbufField, Ident, v1::Type)>, GenerateError> {
        let head = &self.heads[at];
        let owner = shape.name();
        let mut fields = Vec::new();
        for field in &head.fields {
            let placed = field
                .source
                .as_ref()
                .and_then(|source| source.kind.as_ref())
                .ok_or_else(|| GenerateError {
                    message: format!("a podbuf field of `{owner}` has no source"),
                })?;
            match placed {
                Placed::Field(_) | Placed::Position(_) => {
                    let member = shape.field_of(placed).ok_or_else(|| GenerateError {
                        message: format!(
                            "the podbuf layout of `{owner}` names a field the type does not have"
                        ),
                    })?;
                    let ty = member.r#type.clone().ok_or_else(|| GenerateError {
                        message: format!("a field of `{owner}` carries no type"),
                    })?;
                    fields.push((field, ident(snake_of(member.name.as_ref())), ty));
                }
                Placed::Flags(_) => {}
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
            }
        }
        Ok(fields)
    }

    /// The offset of flags byte `index` in head `at`.
    fn flags_offset(&self, at: usize, index: u32, owner: &str) -> Result<u32, GenerateError> {
        self.heads[at]
            .fields
            .iter()
            .find(|field| {
                matches!(
                    field.source.as_ref().and_then(|source| source.kind.as_ref()),
                    Some(Placed::Flags(flags)) if *flags == index
                )
            })
            .map(|field| field.offset)
            .ok_or_else(|| GenerateError {
                message: format!("the podbuf head of `{owner}` has no flags byte {index}"),
            })
    }

    /// `(flags byte offset, mask)` of an optional field, or `None` for a
    /// required one.
    fn presence(
        &self,
        at: usize,
        field: &v1::PodbufField,
        owner: &str,
    ) -> Result<Option<(u32, TokenStream)>, GenerateError> {
        let Some(bit) = field.optional.as_ref() else {
            return Ok(None);
        };
        let flags = self.flags_offset(at, bit.flags, owner)?;
        let mask = if bit.bit == 0 {
            quote! { 1u8 }
        } else {
            let shift = Literal::u32_unsuffixed(bit.bit);
            quote! { 1u8 << #shift }
        };
        Ok(Some((flags, mask)))
    }

    /// `<region>.u8_at(flags) & mask != 0`.
    fn present_bit(place: &Place, flags: u32, mask: &TokenStream) -> TokenStream {
        let region = place.region();
        let flags = Off::Lit(flags).tokens();
        quote! { #region.u8_at(#flags) & #mask != 0 }
    }

    /// Whether a value of `form` is handed back as a borrow of the buffer
    /// (a view, a slice or an iterator), which gives the arm enum of a union
    /// its lifetime.
    fn borrows(&self, form: &v1::PodbufForm) -> bool {
        matches!(
            form.kind.as_ref(),
            Some(Form::InlineHead(_) | Form::InlineArray(_) | Form::Descriptor(_))
        )
    }

    /// The path of the view of the struct, union or tuple `ty` names.
    fn view_path(&self, ty: &v1::Type) -> Result<TokenStream, GenerateError> {
        match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Named(reference)) => {
                let decl = self.resolve(reference)?;
                let name = format!("{}PodbufView", declared(decl.name.as_ref()));
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
                let id = view_ident(name);
                Ok(quote! { #id })
            }
            _ => fail("an inline podbuf head is read into no struct, union or tuple".to_string()),
        }
    }

    /// The view of the head `ty` names at `place`: a struct literal over the
    /// place's three fields.
    fn nested_view(&self, ty: &v1::Type, place: &Place) -> Result<TokenStream, GenerateError> {
        let view = self.view_path(ty)?;
        let (buf, at, size) = (&place.buf, &place.at, &place.size);
        Ok(quote! { #view { buf: #buf, at: #at, size: #size } })
    }

    /// The element type and the key and value types of an array or map
    /// type position.
    fn array_element(ty: &v1::Type) -> Option<&v1::Type> {
        match ty.kind.as_ref()? {
            v1::r#type::Kind::Array(inner) => inner.element.as_deref(),
            _ => None,
        }
    }

    fn map_types(ty: &v1::Type) -> Option<(&v1::Type, &v1::Type)> {
        match ty.kind.as_ref()? {
            v1::r#type::Kind::Map(inner) => Some((inner.key.as_deref()?, inner.value.as_deref()?)),
            _ => None,
        }
    }

    /// The key and value fields of the entry head of `map`, with their
    /// forms.
    fn entry_fields(
        &self,
        map: &v1::PodbufMap,
        owner: &str,
    ) -> Result<(&'a v1::PodbufField, &'a v1::PodbufField), GenerateError> {
        let entry = self
            .heads
            .get(map.entry_head as usize)
            .ok_or_else(|| GenerateError {
                message: "the podbuf layout names an entry head it does not carry".to_string(),
            })?;
        let find = |want: fn(&Placed) -> bool| {
            entry.fields.iter().find(|field| {
                field
                    .source
                    .as_ref()
                    .and_then(|source| source.kind.as_ref())
                    .is_some_and(want)
            })
        };
        let key = find(|placed| matches!(placed, Placed::Key(_)));
        let value = find(|placed| matches!(placed, Placed::Value(_)));
        match (key, value) {
            (Some(key), Some(value)) => Ok((key, value)),
            _ => fail(format!(
                "the podbuf entry head of `{owner}` lacks its key or its value"
            )),
        }
    }

    fn form_of(
        field: &'a v1::PodbufField,
        owner: &str,
    ) -> Result<&'a v1::PodbufForm, GenerateError> {
        field.form.as_ref().ok_or_else(|| GenerateError {
            message: format!("a podbuf field of `{owner}` carries no form"),
        })
    }

    // -----------------------------------------------------------------
    // The view
    // -----------------------------------------------------------------

    /// `<T>PodbufView<'a>`, its `View` implementation and its accessors,
    /// and for a union the `<T>PodbufArm` enum its `arm` accessor returns.
    fn view(&self, at: usize, shape: &Shape<'a>) -> Result<TokenStream, GenerateError> {
        let owner = shape.name();
        let view = view_ident(owner);
        let vis = vis_tokens(shape.visibility());
        let (accessors, arm_enum) = match shape {
            Shape::Union(decl, ud) => self.union_view(at, decl, ud)?,
            Shape::Struct(..) | Shape::Tuple(_) => (self.accessors(at, shape)?, TokenStream::new()),
        };
        let methods = accessors.iter().map(|accessor| {
            let (name, ty, read) = (&accessor.name, &accessor.ty, &accessor.read);
            let doc = format!(" Reads `{owner}`'s `{name}` field in place.");
            quote! {
                #[doc = #doc]
                #vis fn #name(&self) -> #ty {
                    let __b = self.buf;
                    let __a = self.at;
                    let __z = self.size;
                    #read
                }
            }
        });
        let doc = format!(
            " A `{owner}` value in podbuf bytes, read in place without copying it. Each \
             accessor is a bounds-checked load at the field's offset; a field past the \
             size the producer wrote reads as 0."
        );
        Ok(quote! {
            #[doc = #doc]
            #[derive(Debug, Clone, Copy)]
            #[allow(deprecated)]
            #vis struct #view<'a> {
                // `pub(crate)`, not private: the codec of another package of
                // this build names this view by a path and builds it with a
                // struct literal.
                pub(crate) buf: &'a [::core::primitive::u8],
                pub(crate) at: ::core::primitive::usize,
                pub(crate) size: ::core::primitive::usize,
            }

            #[allow(deprecated)]
            impl<'a> ::ridl_rt::payload::View<'a> for #view<'a> {
                fn bytes(&self) -> &'a [::core::primitive::u8] {
                    self.buf
                }
            }

            #[allow(deprecated)]
            impl<'a> #view<'a> {
                #(#methods)*
            }

            #arm_enum
        })
    }

    /// One accessor per field of a struct or tuple head, over the place
    /// `(__b, __a, __z)`.
    fn accessors(&self, at: usize, shape: &Shape<'a>) -> Result<Vec<Accessor>, GenerateError> {
        let owner = shape.name();
        let place = Place::new(quote! { __b }, quote! { __a }, quote! { __z });
        let mut accessors = Vec::new();
        for (field, name, ty) in self.view_fields(at, shape)? {
            let form = Self::form_of(field, owner)?;
            let off = Off::Lit(field.offset);
            let (ty, read) = match self.presence(at, field, owner)? {
                None => (
                    self.view_type(form, &ty)?,
                    self.view_expr(form, &ty, &place, &off)?,
                ),
                Some((flags, mask)) => {
                    let inner_ty = Self::present(&ty);
                    let inner = self.view_expr(form, &inner_ty, &place, &off)?;
                    let present = Self::present_bit(&place, flags, &mask);
                    let ty = self.view_type(form, &inner_ty)?;
                    (
                        quote! { ::core::option::Option<#ty> },
                        quote! {
                            if #present {
                                ::core::option::Option::Some(#inner)
                            } else {
                                ::core::option::Option::None
                            }
                        },
                    )
                }
            };
            accessors.push(Accessor { name, ty, read });
        }
        Ok(accessors)
    }

    /// What an accessor hands back for a value of `form` at type position
    /// `ty`: the domain value for a boolean, a number, an enum and an enum
    /// set; a borrow for a string and for bytes; a view for a nested head;
    /// an iterator for an array and for a map.
    fn view_type(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
    ) -> Result<TokenStream, GenerateError> {
        Ok(match form.kind.as_ref() {
            Some(
                Form::Boolean(_) | Form::Int(_) | Form::Float(_) | Form::Enum(_) | Form::EnumSet(_),
            ) => {
                let bare = v1::Type {
                    optional: false,
                    kind: ty.kind.clone(),
                };
                model_type_tokens(self.ctx, &bare)
            }
            Some(Form::InlineHead(_)) => {
                let view = self.view_path(ty)?;
                quote! { #view<'a> }
            }
            Some(Form::InlineArray(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an inline array carries no element form".to_string(),
                })?;
                let item = match Self::array_element(ty) {
                    Some(element_ty) => self.view_type(element, element_ty)?,
                    None => quote! { ::core::primitive::u8 },
                };
                quote! { impl ::core::iter::Iterator<Item = #item> + 'a }
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Text(_)) => quote! { &'a ::core::primitive::str },
                Some(Descriptor::Bytes(_)) => quote! { &'a [::core::primitive::u8] },
                Some(Descriptor::Array(array)) => {
                    let element = array.element.as_deref().ok_or_else(|| GenerateError {
                        message: "an array carries no element form".to_string(),
                    })?;
                    let element_ty = Self::array_element(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf array is read into no array type".to_string(),
                    })?;
                    let item = self.view_type(element, element_ty)?;
                    quote! { impl ::core::iter::Iterator<Item = #item> + 'a }
                }
                Some(Descriptor::Map(map)) => {
                    let (key_ty, value_ty) = Self::map_types(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf map is read into no map type".to_string(),
                    })?;
                    let (key, value) = self.entry_fields(map, "a map")?;
                    let key = self.view_type(Self::form_of(key, "a map")?, key_ty)?;
                    let value = self.view_type(Self::form_of(value, "a map")?, value_ty)?;
                    quote! { impl ::core::iter::Iterator<Item = (#key, #value)> + 'a }
                }
                Some(Descriptor::Arms(_)) => {
                    return fail("an arm descriptor is read through the union's view".to_string());
                }
                None => return fail("a podbuf descriptor carries no kind".to_string()),
            },
            None => return fail("a podbuf field carries no form".to_string()),
        })
    }

    /// The domain value of the scalar, enum or enum set `form` at `off` of
    /// `place`.
    fn scalar_read(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        place: &Place,
        off: &Off,
    ) -> Result<TokenStream, GenerateError> {
        let region = place.region();
        let off = off.tokens();
        let shape = self.scalar_of(ty)?;
        let prim = match form.kind.as_ref() {
            Some(Form::Boolean(_)) => {
                let raw = quote! { #region.u8_at(#off) != 0u8 };
                return Ok(match shape.ctor() {
                    Some((path, ctor)) => quote! { #path::#ctor(#raw) },
                    None => raw,
                });
            }
            Some(Form::Int(int)) => Prim::of_int(int.width)?,
            Some(Form::Float(width)) => Prim::of_float(*width)?,
            Some(Form::Enum(value)) => Prim::of_int(value.width)?,
            Some(Form::EnumSet(value)) => Prim::of_int(value.width)?,
            _ => return fail("a podbuf scalar is read from no scalar form".to_string()),
        };
        let load = prim.at();
        let widened = prim.widen(quote! { #region.#load(#off) });
        Ok(match shape {
            ScalarShape::Named { .. } => {
                let (path, ctor) = shape.ctor().unwrap_or_else(|| unreachable!());
                quote! { #path::#ctor(#widened) }
            }
            ScalarShape::Enum(ed, path) => {
                let first = ed.values.first().ok_or_else(|| GenerateError {
                    message: "an enum with no value cannot be read".to_string(),
                })?;
                let first = ident(&pascal_of(first.name.as_ref()));
                quote! {
                    <#path as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(#widened)
                        .unwrap_or(#path::#first)
                }
            }
            ScalarShape::EnumSet(path) => quote! {
                <#path as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(#widened)
                    .unwrap_or(#path(0i64))
            },
            ScalarShape::Inline(..) => widened,
        })
    }

    /// The byte range the descriptor `__d` names, with no bounds assumption:
    /// a range `get` does not find is empty.
    fn desc_slice(buf: &TokenStream) -> TokenStream {
        quote! {
            #buf.get(
                (__d.off as ::core::primitive::usize)
                    ..(__d.off as ::core::primitive::usize)
                        .saturating_add(__d.len as ::core::primitive::usize),
            )
            .unwrap_or(&[])
        }
    }

    /// The iterator over the elements of the block the array descriptor at
    /// `off` names, each read by `element` over the place `(__b, __es, __ez)`.
    /// The closure captures `__b`, the buffer of every view and decode place,
    /// by copy, so the iterator lives as long as the buffer.
    fn block_iter(place: &Place, off: &Off, element: TokenStream) -> TokenStream {
        let region = place.region();
        let off = off.tokens();
        let bindings = Place::element_bindings();
        quote! {
            {
                let __d = ::ridl_rt::podbuf::ArrayDesc::read(&#region, #off);
                (0..::core::primitive::usize::from(__d.len)).map(move |__i| {
                    #bindings
                    #element
                })
            }
        }
    }

    /// The value a view hands back for `form` at `off` of `place`.
    fn view_expr(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        place: &Place,
        off: &Off,
    ) -> Result<TokenStream, GenerateError> {
        let buf = &place.buf;
        Ok(match form.kind.as_ref() {
            Some(
                Form::Boolean(_) | Form::Int(_) | Form::Float(_) | Form::Enum(_) | Form::EnumSet(_),
            ) => self.scalar_read(form, ty, place, off)?,
            Some(Form::InlineHead(_)) => self.nested_view(ty, &place.inline(off))?,
            Some(Form::InlineArray(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an inline array carries no element form".to_string(),
                })?;
                let count = Literal::usize_suffixed(array.count as usize);
                match Self::array_element(ty) {
                    Some(element_ty) => {
                        let stride = self.form_size(element);
                        let item =
                            self.view_expr(element, element_ty, place, &off.index(stride))?;
                        let (buf, at, size) = (&place.buf, &place.at, &place.size);
                        quote! {
                            {
                                let (__b, __a, __z) = (#buf, #at, #size);
                                (0..#count).map(move |__i| #item)
                            }
                        }
                    }
                    None => {
                        let region = place.region();
                        let off = off.tokens();
                        quote! { #region.bytes_at(#off, #count) }
                    }
                }
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Text(_)) => {
                    let region = place.region();
                    let off = off.tokens();
                    let slice = Self::desc_slice(buf);
                    quote! {
                        {
                            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #off);
                            ::core::str::from_utf8(#slice).unwrap_or("")
                        }
                    }
                }
                Some(Descriptor::Bytes(_)) => {
                    let region = place.region();
                    let off = off.tokens();
                    let slice = Self::desc_slice(buf);
                    quote! {
                        {
                            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #off);
                            #slice
                        }
                    }
                }
                Some(Descriptor::Array(array)) => {
                    let element = array.element.as_deref().ok_or_else(|| GenerateError {
                        message: "an array carries no element form".to_string(),
                    })?;
                    let element_ty = Self::array_element(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf array is read into no array type".to_string(),
                    })?;
                    let item = self.view_expr(
                        element,
                        element_ty,
                        &Place::element(&quote! { __b }),
                        &Off::Lit(0),
                    )?;
                    Self::block_iter(place, off, item)
                }
                Some(Descriptor::Map(map)) => {
                    let (key_ty, value_ty) = Self::map_types(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf map is read into no map type".to_string(),
                    })?;
                    let (key, value) = self.entry_fields(map, "a map")?;
                    let entry = Place::element(&quote! { __b });
                    let key_read = self.view_expr(
                        Self::form_of(key, "a map")?,
                        key_ty,
                        &entry,
                        &Off::Lit(key.offset),
                    )?;
                    let value_read = self.view_expr(
                        Self::form_of(value, "a map")?,
                        value_ty,
                        &entry,
                        &Off::Lit(value.offset),
                    )?;
                    Self::block_iter(place, off, quote! { (#key_read, #value_read) })
                }
                Some(Descriptor::Arms(_)) => {
                    return fail("an arm descriptor is read through the union's view".to_string());
                }
                None => return fail("a podbuf descriptor carries no kind".to_string()),
            },
            None => return fail("a podbuf field carries no form".to_string()),
        })
    }

    /// The tag field and the arm descriptor field of union head `at`, with
    /// the tag's primitive and the arm list.
    fn union_fields(
        &self,
        at: usize,
        owner: &str,
    ) -> Result<
        (
            &'a v1::PodbufField,
            Prim,
            &'a v1::PodbufField,
            &'a [v1::PodbufArm],
        ),
        GenerateError,
    > {
        let head = &self.heads[at];
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
                Some(Descriptor::Arms(arms)) => &arms.arms[..],
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
        Ok((tag, tag_prim, arm_field, arms))
    }

    /// One arm of a union as the codec reads it: the variant, the tag
    /// literal, the arm head, the arm's type position and, for a scalar,
    /// enum or enum set arm, the one value field of the arm head.
    fn union_arm(
        &self,
        arm: &'a v1::Arm,
        arms: &[v1::PodbufArm],
        owner: &str,
    ) -> Result<UnionArm<'a>, GenerateError> {
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
        let reference = arm.r#type.as_ref().ok_or_else(|| GenerateError {
            message: format!(
                "arm `{}` of `{owner}` names no type",
                declared(arm.name.as_ref())
            ),
        })?;
        let ty = v1::Type {
            optional: false,
            kind: Some(v1::r#type::Kind::Named(reference.clone())),
        };
        let aggregate = matches!(
            self.resolve(reference)?.kind.as_ref(),
            Some(v1::declaration::Kind::Struct(_) | v1::declaration::Kind::Union(_))
        );
        let value = if aggregate {
            None
        } else {
            Some(
                arm_head
                    .fields
                    .iter()
                    .find(|field| {
                        matches!(
                            field
                                .source
                                .as_ref()
                                .and_then(|source| source.kind.as_ref()),
                            Some(Placed::Arm(_))
                        )
                    })
                    .ok_or_else(|| GenerateError {
                        message: format!("the podbuf arm head of `{owner}` has no value field"),
                    })?,
            )
        };
        Ok(UnionArm {
            name: declared(arm.name.as_ref()),
            variant: ident(camel_of(arm.name.as_ref())),
            ordinal: Literal::i64_suffixed(i64::from(arm.ordinal)),
            head: arm_head,
            ty,
            value,
        })
    }

    /// The `arm` accessor of a union's view and the `<T>PodbufArm` enum it
    /// returns, one variant per arm holding that arm's view.
    fn union_view(
        &self,
        at: usize,
        decl: &v1::Declaration,
        ud: &'a v1::Union,
    ) -> Result<(Vec<Accessor>, TokenStream), GenerateError> {
        let owner = declared(decl.name.as_ref());
        let vis = vis_tokens(decl.visibility);
        let arm_enum = arm_ident(owner);
        let (tag, tag_prim, arm_field, arms) = self.union_fields(at, owner)?;
        let place = Place::new(quote! { __b }, quote! { __a }, quote! { __z });
        let arm_place = Place::of_desc(&quote! { __b });
        let mut variants: Vec<TokenStream> = Vec::new();
        let mut matches: Vec<TokenStream> = Vec::new();
        let mut builds: Vec<TokenStream> = Vec::new();
        let mut borrows = false;
        for arm in &ud.arms {
            let arm = self.union_arm(arm, arms, owner)?;
            let (variant, ordinal) = (&arm.variant, &arm.ordinal);
            let (ty, read) = match arm.value {
                None => {
                    let view = self.view_path(&arm.ty)?;
                    (quote! { #view<'a> }, self.nested_view(&arm.ty, &arm_place)?)
                }
                Some(value) => {
                    let form = Self::form_of(value, owner)?;
                    (
                        self.view_type(form, &arm.ty)?,
                        self.view_expr(form, &arm.ty, &arm_place, &Off::Lit(value.offset))?,
                    )
                }
            };
            borrows |= arm
                .value
                .is_none_or(|value| value.form.as_ref().is_some_and(|form| self.borrows(form)));
            let doc = format!(" The `{}` arm.", arm.name);
            variants.push(quote! { #[doc = #doc] #variant(#ty) });
            let build = quote! { #arm_enum::#variant(#read) };
            matches.push(quote! { #ordinal => #build });
            builds.push(build);
        }
        let first = builds.first().cloned().ok_or_else(|| GenerateError {
            message: format!("`{owner}` declares no arm, so it encodes no value"),
        })?;
        // The tag was checked by `verify`; the fallback arm is the first,
        // which no verified buffer reaches.
        let fallback = quote! { _ => #first };
        let lifetime = if borrows {
            quote! { <'a> }
        } else {
            TokenStream::new()
        };
        let region = place.region();
        let tag_at = Off::Lit(tag.offset).tokens();
        let desc_at = Off::Lit(arm_field.offset).tokens();
        let load = tag_prim.at();
        let widened = tag_prim.widen(quote! { #region.#load(#tag_at) });
        let read = quote! {
            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #desc_at);
            match #widened {
                #(#matches,)*
                #fallback
            }
        };
        let doc = format!(
            " The arms of `{owner}` as a view reads them: the active arm's view, or its \
             value for a scalar, enum or enum set arm."
        );
        let enum_item = quote! {
            #[doc = #doc]
            #[derive(Debug, Clone, Copy)]
            #[allow(deprecated)]
            #vis enum #arm_enum #lifetime {
                #(#variants,)*
            }
        };
        let accessor = Accessor {
            name: format_ident!("arm"),
            ty: quote! { #arm_enum #lifetime },
            read,
        };
        Ok((vec![accessor], enum_item))
    }

    // -----------------------------------------------------------------
    // The verifier
    // -----------------------------------------------------------------

    /// `impl T { __ridl_podbuf_verify, __ridl_podbuf_verify_at }` for head
    /// `at`: the first reads the preamble and checks the root head, the
    /// second checks a head at a given offset with the size its producer
    /// wrote, which is how a nested head, an element and an arm are checked.
    fn verifier(&self, at: usize, shape: &Shape<'a>) -> Result<TokenStream, GenerateError> {
        let owner = shape.name();
        let ty = ident(owner);
        let view = view_ident(owner);
        let place = Place::new(quote! { buf }, quote! { at }, quote! { size });
        let body = match shape {
            Shape::Union(decl, ud) => self.union_verify(at, decl, ud, &place)?,
            Shape::Struct(..) | Shape::Tuple(_) => self.head_verify(at, shape, &place)?,
        };
        let used = !body.is_empty();
        let (buf, at_param, size) = if used {
            (quote! { buf }, quote! { at }, quote! { size })
        } else {
            (quote! { _buf }, quote! { _at }, quote! { _size })
        };
        let tail = if body.to_string().contains("__tail") {
            quote! { __tail }
        } else {
            quote! { _tail }
        };
        Ok(quote! {
            #[allow(deprecated)]
            impl #ty {
                // Checks the preamble and the root head of `buf` (the rules
                // of the podbuf design, §7) and returns its view. An
                // internal of the podbuf codec: no doc.
                pub(crate) fn __ridl_podbuf_verify(
                    buf: &[::core::primitive::u8],
                ) -> ::core::result::Result<#view<'_>, ::ridl_rt::payload::VerifyError> {
                    let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
                    let __size = __p.head_size as ::core::primitive::usize;
                    let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
                    Self::__ridl_podbuf_verify_at(
                        buf,
                        ::ridl_rt::podbuf::PREAMBLE_SIZE,
                        __size,
                        __tail,
                    )?;
                    ::core::result::Result::Ok(#view {
                        buf,
                        at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                        size: __size,
                    })
                }

                // Checks the head at `at` of `buf`, `size` bytes as its
                // producer wrote it; `tail` is the first byte of the tail,
                // below which no descriptor may point.
                pub(crate) fn __ridl_podbuf_verify_at(
                    #buf: &[::core::primitive::u8],
                    #at_param: ::core::primitive::usize,
                    #size: ::core::primitive::usize,
                    #tail: ::core::primitive::usize,
                ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                    #body
                    ::core::result::Result::Ok(())
                }
            }
        })
    }

    /// The checks of every field of a struct or tuple head, in offset order.
    fn head_verify(
        &self,
        at: usize,
        shape: &Shape<'a>,
        place: &Place,
    ) -> Result<TokenStream, GenerateError> {
        let owner = shape.name();
        let mut checks: Vec<TokenStream> = Vec::new();
        for (field, _, ty) in self.view_fields(at, shape)? {
            let form = Self::form_of(field, owner)?;
            let off = Off::Lit(field.offset);
            match self.presence(at, field, owner)? {
                None => checks.push(self.verify_form(form, &ty, place, &off, owner)?),
                Some((flags, mask)) => {
                    // §7 rule 2: an absent optional's bytes are zero; a
                    // present one is checked as any value.
                    let present = Self::present_bit(place, flags, &mask);
                    let inner = self.verify_form(form, &Self::present(&ty), place, &off, owner)?;
                    let region = place.region();
                    let off = off.tokens();
                    let size = Literal::usize_suffixed(field.size as usize);
                    let refused = violation(owner, "Variant");
                    checks.push(quote! {
                        if #present {
                            #inner
                        } else if #region.bytes_at(#off, #size).any(|__byte| __byte != 0u8) {
                            #refused
                        }
                    });
                }
            }
        }
        Ok(quote! { #(#checks)* })
    }

    /// The constraint check of the scalar at `off`, or nothing when the type
    /// has none.
    fn scalar_verify(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        place: &Place,
        off: &Off,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let region = place.region();
        let off = off.tokens();
        let shape = self.scalar_of(ty)?;
        let (prim, wire_f32) = match form.kind.as_ref() {
            Some(Form::Boolean(_)) => {
                let refused = violation(owner, "Range");
                return Ok(quote! { if #region.u8_at(#off) > 1u8 { #refused } });
            }
            Some(Form::Int(int)) => (Prim::of_int(int.width)?, false),
            Some(Form::Float(width)) => (
                Prim::of_float(*width)?,
                *width == v1::FloatWidth::F32 as i32,
            ),
            Some(Form::Enum(value)) => (Prim::of_int(value.width)?, false),
            Some(Form::EnumSet(value)) => (Prim::of_int(value.width)?, false),
            _ => return fail("a podbuf scalar is checked from no scalar form".to_string()),
        };
        let load = prim.at();
        let widened = prim.widen(quote! { #region.#load(#off) });
        Ok(match &shape {
            ScalarShape::Named { .. } => {
                let named = shape.named_scalar().unwrap_or_else(|| unreachable!());
                named_scalar_check(&named, quote! { &(#widened) }).unwrap_or_default()
            }
            ScalarShape::Enum(_, path) | ScalarShape::EnumSet(path) => quote! {
                <#path as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(#widened)
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            },
            ScalarShape::Inline(_, Some(sc)) => {
                scalar_constraint_check(sc, owner, widened, wire_f32)
            }
            ScalarShape::Inline(_, None) => TokenStream::new(),
        })
    }

    /// The statements that check a value of `form` at `off` of `place`: the
    /// rules of §7 for that shape, then its elements, entries or arm with
    /// the same rules. Over `__tail`, the first byte of the tail.
    fn verify_form(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        place: &Place,
        off: &Off,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let buf = &place.buf;
        let out_of_bounds = malformed("OutOfBounds");
        Ok(match form.kind.as_ref() {
            Some(
                Form::Boolean(_) | Form::Int(_) | Form::Float(_) | Form::Enum(_) | Form::EnumSet(_),
            ) => self.scalar_verify(form, ty, place, off, owner)?,
            Some(Form::InlineHead(index)) => {
                if self
                    .heads
                    .get(*index as usize)
                    .is_none_or(|head| head.size == 0)
                {
                    return Ok(TokenStream::new());
                }
                let path = self.encoder_path(ty)?;
                let inner = place.inline(off);
                let (buf, at, size) = (&inner.buf, &inner.at, &inner.size);
                quote! { #path::__ridl_podbuf_verify_at(#buf, #at, #size, __tail)?; }
            }
            Some(Form::InlineArray(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an inline array carries no element form".to_string(),
                })?;
                // Exact-length bytes have nothing to check: their length is
                // the type's.
                let Some(element_ty) = Self::array_element(ty) else {
                    return Ok(TokenStream::new());
                };
                let stride = self.form_size(element);
                let inner = self.verify_form(
                    element,
                    element_ty,
                    place,
                    &Off::Expr(quote! { __o }),
                    owner,
                )?;
                if inner.is_empty() {
                    return Ok(TokenStream::new());
                }
                let count = Literal::usize_suffixed(array.count as usize);
                let step = off.index(stride).tokens();
                quote! {
                    for __i in 0..#count {
                        let __o = #step;
                        #inner
                    }
                }
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Text(bound)) => {
                    let region = place.region();
                    let off = off.tokens();
                    let max4 = Literal::u64_suffixed(bound.max.saturating_mul(4));
                    let shape = self.scalar_of(ty)?;
                    let check = match &shape {
                        ScalarShape::Named { .. } => {
                            let named = shape.named_scalar().unwrap_or_else(|| unreachable!());
                            named_scalar_check(&named, quote! { __s })
                        }
                        ScalarShape::Inline(_, Some(sc)) => {
                            Some(scalar_constraint_check(sc, owner, quote! { __s }, false))
                        }
                        _ => None,
                    };
                    // §7 rule 4: a `len` of 0 is a zero descriptor with no
                    // data; otherwise `len` is at most four bytes per
                    // scalar value of the bound, and the data and its NUL
                    // are in the tail. Rule 6 checks the bytes, then rule 3
                    // the scalar-value count.
                    let bytes = quote! {
                        if ::core::primitive::u64::from(__d.len) > #max4 { #out_of_bounds }
                        let __g = ::ridl_rt::podbuf::tail_range(
                            __d.off,
                            ::core::primitive::u64::from(__d.len) + 1u64,
                            #buf.len(),
                            __tail,
                            1usize,
                        )?;
                        ::ridl_rt::podbuf::text(#buf.get(__g).unwrap_or(&[]))?
                    };
                    match check {
                        Some(check) => quote! {
                            {
                                let __d = ::ridl_rt::podbuf::Desc::read(&#region, #off);
                                let __s: &::core::primitive::str = if __d.len == 0 {
                                    if __d.off != 0 { #out_of_bounds }
                                    ""
                                } else {
                                    #bytes
                                };
                                #check
                            }
                        },
                        None => quote! {
                            {
                                let __d = ::ridl_rt::podbuf::Desc::read(&#region, #off);
                                if __d.len == 0 {
                                    if __d.off != 0 { #out_of_bounds }
                                } else {
                                    #bytes;
                                }
                            }
                        },
                    }
                }
                Some(Descriptor::Bytes(bound)) => {
                    let region = place.region();
                    let off = off.tokens();
                    let shape = self.scalar_of(ty)?;
                    let check = match &shape {
                        ScalarShape::Named { .. } => {
                            let named = shape.named_scalar().unwrap_or_else(|| unreachable!());
                            let slice = Self::desc_slice(buf);
                            named_scalar_check(&named, slice).unwrap_or_default()
                        }
                        _ => count_check_of(
                            owner,
                            bound.min,
                            bound.max,
                            quote! { (__d.len as ::core::primitive::usize) },
                        ),
                    };
                    quote! {
                        {
                            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #off);
                            if __d.len == 0 {
                                if __d.off != 0 { #out_of_bounds }
                            } else {
                                ::ridl_rt::podbuf::tail_range(
                                    __d.off,
                                    ::core::primitive::u64::from(__d.len),
                                    #buf.len(),
                                    __tail,
                                    1usize,
                                )?;
                            }
                            #check
                        }
                    }
                }
                Some(Descriptor::Array(array)) => {
                    let element = array.element.as_deref().ok_or_else(|| GenerateError {
                        message: "an array carries no element form".to_string(),
                    })?;
                    let element_ty = Self::array_element(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf array is checked from no array type".to_string(),
                    })?;
                    let inner = self.verify_form(
                        element,
                        element_ty,
                        &Place::element(buf),
                        &Off::Lit(0),
                        owner,
                    )?;
                    self.block_verify(
                        place,
                        off,
                        self.stride_check(element),
                        self.form_align(element),
                        array.min,
                        array.max,
                        inner,
                        owner,
                    )
                }
                Some(Descriptor::Map(map)) => {
                    let (key_ty, value_ty) = Self::map_types(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf map is checked from no map type".to_string(),
                    })?;
                    let (key, value) = self.entry_fields(map, owner)?;
                    let key_form = Self::form_of(key, owner)?;
                    let entry = Place::element(buf);
                    let key_check =
                        self.verify_form(key_form, key_ty, &entry, &Off::Lit(key.offset), owner)?;
                    let value_check = self.verify_form(
                        Self::form_of(value, owner)?,
                        value_ty,
                        &entry,
                        &Off::Lit(value.offset),
                        owner,
                    )?;
                    let order = self.key_order_check(key_form, buf, key.offset, owner)?;
                    let inner = quote! { #key_check #value_check #order };
                    self.block_verify(
                        place,
                        off,
                        quote! { !__d.elem_size.is_multiple_of(8u16) },
                        8,
                        map.min,
                        map.max,
                        inner,
                        owner,
                    )
                }
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

    /// §7 rule 5: the condition under which `__d.elem_size` is wrong for
    /// `element`: not a multiple of 8 for a head, not the exact size for
    /// anything else.
    fn stride_check(&self, element: &v1::PodbufForm) -> TokenStream {
        match element.kind.as_ref() {
            Some(Form::InlineHead(_)) => quote! { !__d.elem_size.is_multiple_of(8u16) },
            _ => {
                let size = Literal::u32_suffixed(self.form_size(element));
                quote! { ::core::primitive::u32::from(__d.elem_size) != #size }
            }
        }
    }

    /// The checks of the block an array or map descriptor at `off` names
    /// (§7 rules 4, 5 and 9): a `len` of 0 is a zero descriptor; otherwise
    /// the stride is checked, the block is in the tail at `align`, the count
    /// is within `min..=max`, and `inner` runs over each element at
    /// `(__b, __es, __ez)`.
    #[allow(clippy::too_many_arguments)]
    fn block_verify(
        &self,
        place: &Place,
        off: &Off,
        bad_stride: TokenStream,
        align: u32,
        min: u64,
        max: u64,
        inner: TokenStream,
        owner: &str,
    ) -> TokenStream {
        let region = place.region();
        let off = off.tokens();
        let buf = &place.buf;
        let out_of_bounds = malformed("OutOfBounds");
        let stride = malformed("Stride");
        let align = Literal::usize_suffixed(align as usize);
        let count = count_check_of(
            owner,
            min,
            max,
            quote! { ::core::primitive::usize::from(__d.len) },
        );
        let elements = if inner.is_empty() {
            TokenStream::new()
        } else {
            let bindings = Place::element_bindings();
            quote! {
                for __i in 0..::core::primitive::usize::from(__d.len) {
                    #bindings
                    #inner
                }
            }
        };
        quote! {
            {
                let __d = ::ridl_rt::podbuf::ArrayDesc::read(&#region, #off);
                if __d.len == 0 {
                    if __d.off != 0 || __d.elem_size != 0 { #out_of_bounds }
                } else {
                    if #bad_stride { #stride }
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len) * ::core::primitive::u64::from(__d.elem_size),
                        #buf.len(),
                        __tail,
                        #align,
                    )?;
                }
                #count
                #elements
            }
        }
    }

    /// §7 rule 8 and §6: over entry `__i` at `(__es, __ez)`, a NaN float
    /// key is refused, and the key is strictly above the previous entry's
    /// key: bytewise for a string or bytes, numeric at its width otherwise.
    fn key_order_check(
        &self,
        form: &v1::PodbufForm,
        buf: &TokenStream,
        offset: u32,
        owner: &str,
    ) -> Result<TokenStream, GenerateError> {
        let off = Off::Lit(offset).tokens();
        let key_order = malformed("KeyOrder");
        let (nan, refuse) = match form.kind.as_ref() {
            Some(Form::Boolean(_)) => (
                TokenStream::new(),
                quote! { if __q.u8_at(#off) >= __r.u8_at(#off) { #key_order } },
            ),
            Some(Form::Int(int)) => {
                let load = Prim::of_int(int.width)?.at();
                (
                    TokenStream::new(),
                    quote! { if __q.#load(#off) >= __r.#load(#off) { #key_order } },
                )
            }
            Some(Form::Enum(value)) => {
                let load = Prim::of_int(value.width)?.at();
                (
                    TokenStream::new(),
                    quote! { if __q.#load(#off) >= __r.#load(#off) { #key_order } },
                )
            }
            Some(Form::EnumSet(value)) => {
                let load = Prim::of_int(value.width)?.at();
                (
                    TokenStream::new(),
                    quote! { if __q.#load(#off) >= __r.#load(#off) { #key_order } },
                )
            }
            Some(Form::Float(width)) => {
                let load = Prim::of_float(*width)?.at();
                let refused = violation(owner, "Range");
                (
                    quote! { if __r.#load(#off).is_nan() { #refused } },
                    quote! {
                        if ::core::cmp::PartialOrd::partial_cmp(&__q.#load(#off), &__r.#load(#off))
                            != ::core::option::Option::Some(::core::cmp::Ordering::Less)
                        {
                            #key_order
                        }
                    },
                )
            }
            Some(Form::Descriptor(descriptor))
                if matches!(
                    descriptor.kind.as_ref(),
                    Some(Descriptor::Text(_) | Descriptor::Bytes(_))
                ) =>
            {
                // Both keys were checked, so their bytes are in the buffer;
                // they are bound before the `if`, which
                // `clippy::blocks_in_conditions` asks of a consumer's build.
                let slice = Self::desc_slice(buf);
                (
                    TokenStream::new(),
                    quote! {
                        {
                            let __ka: &[::core::primitive::u8] = {
                                let __d = ::ridl_rt::podbuf::Desc::read(&__q, #off);
                                #slice
                            };
                            let __kb: &[::core::primitive::u8] = {
                                let __d = ::ridl_rt::podbuf::Desc::read(&__r, #off);
                                #slice
                            };
                            if __ka >= __kb { #key_order }
                        }
                    },
                )
            }
            _ => {
                return fail(format!(
                    "a podbuf map key of `{owner}` must be a string, bytes, a boolean, an integer, \
                     a float, an enum or an enum set"
                ));
            }
        };
        Ok(quote! {
            let __r = ::ridl_rt::podbuf::Region::new(#buf, __es, __ez);
            #nan
            if __i > 0 {
                let __q = ::ridl_rt::podbuf::Region::new(#buf, __es.saturating_sub(__ez), __ez);
                #refuse
            }
        })
    }

    /// §7 rule 7 over a union head: the tag names a declared arm and the arm
    /// descriptor's `len` is a multiple of 8; an arm with no field is a zero
    /// descriptor (§14 "Empty union arm"); otherwise the arm is in the tail
    /// and checked with the same rules.
    fn union_verify(
        &self,
        at: usize,
        decl: &v1::Declaration,
        ud: &'a v1::Union,
        place: &Place,
    ) -> Result<TokenStream, GenerateError> {
        let owner = declared(decl.name.as_ref());
        let (tag, tag_prim, arm_field, arms) = self.union_fields(at, owner)?;
        let buf = &place.buf;
        let region = place.region();
        let union = malformed("Union");
        let out_of_bounds = malformed("OutOfBounds");
        let arm_place = Place::of_desc(buf);
        let mut matches: Vec<TokenStream> = Vec::new();
        for arm in &ud.arms {
            let arm = self.union_arm(arm, arms, owner)?;
            let ordinal = &arm.ordinal;
            if arm.head.size == 0 {
                matches.push(quote! { #ordinal => { if !__d.is_zero() { #out_of_bounds } } });
                continue;
            }
            let bounds = quote! {
                if __d.len == 0 {
                    if __d.off != 0 { #out_of_bounds }
                } else {
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len),
                        #buf.len(),
                        __tail,
                        8usize,
                    )?;
                }
            };
            let check = match arm.value {
                None => {
                    let path = self.encoder_path(&arm.ty)?;
                    let (at, size) = (&arm_place.at, &arm_place.size);
                    quote! { #path::__ridl_podbuf_verify_at(#buf, #at, #size, __tail)?; }
                }
                Some(value) => self.verify_form(
                    Self::form_of(value, owner)?,
                    &arm.ty,
                    &arm_place,
                    &Off::Lit(value.offset),
                    owner,
                )?,
            };
            matches.push(quote! { #ordinal => { #bounds #check } });
        }
        let tag_at = Off::Lit(tag.offset).tokens();
        let desc_at = Off::Lit(arm_field.offset).tokens();
        let load = tag_prim.at();
        let widened = tag_prim.widen(quote! { #region.#load(#tag_at) });
        Ok(quote! {
            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #desc_at);
            if !__d.len.is_multiple_of(8u32) { #union }
            match #widened {
                #(#matches)*
                _ => { #union }
            }
        })
    }

    // -----------------------------------------------------------------
    // The decoder
    // -----------------------------------------------------------------

    /// `impl T { __ridl_podbuf_decode }` for head `at`: the owned domain
    /// value from a view `verify` returned.
    fn decoder(&self, at: usize, shape: &Shape<'a>) -> Result<TokenStream, GenerateError> {
        let owner = shape.name();
        let ty = ident(owner);
        let view = view_ident(owner);
        let place = Place::new(quote! { __b }, quote! { __a }, quote! { __z });
        let body = match shape {
            Shape::Union(decl, ud) => self.union_decode(at, decl, ud, &place)?,
            Shape::Struct(..) | Shape::Tuple(_) => {
                let mut fields: Vec<TokenStream> = Vec::new();
                for (field, name, field_ty) in self.view_fields(at, shape)? {
                    let form = Self::form_of(field, owner)?;
                    let off = Off::Lit(field.offset);
                    let value = match self.presence(at, field, owner)? {
                        None => self.decode_expr(form, &field_ty, &place, &off)?,
                        Some((flags, mask)) => {
                            let present = Self::present_bit(&place, flags, &mask);
                            let inner =
                                self.decode_expr(form, &Self::present(&field_ty), &place, &off)?;
                            quote! {
                                if #present {
                                    ::core::option::Option::Some(#inner)
                                } else {
                                    ::core::option::Option::None
                                }
                            }
                        }
                    };
                    fields.push(quote! { #name: #value });
                }
                quote! { #ty { #(#fields),* } }
            }
        };
        Ok(quote! {
            #[allow(deprecated)]
            impl #ty {
                // Builds the value from a view `verify` returned. It cannot
                // fail: every read is of bytes that were checked. An
                // internal of the podbuf codec: no doc.
                pub(crate) fn __ridl_podbuf_decode(view: #view<'_>) -> Self {
                    // A head with no field reads none of the three.
                    #[allow(unused_variables)]
                    let (__b, __a, __z) = (view.buf, view.at, view.size);
                    #body
                }
            }
        })
    }

    /// The owned value of `form` at `off` of `place`.
    fn decode_expr(
        &self,
        form: &v1::PodbufForm,
        ty: &v1::Type,
        place: &Place,
        off: &Off,
    ) -> Result<TokenStream, GenerateError> {
        Ok(match form.kind.as_ref() {
            Some(
                Form::Boolean(_) | Form::Int(_) | Form::Float(_) | Form::Enum(_) | Form::EnumSet(_),
            ) => self.scalar_read(form, ty, place, off)?,
            Some(Form::InlineHead(_)) => {
                let path = self.encoder_path(ty)?;
                let view = self.nested_view(ty, &place.inline(off))?;
                quote! { #path::__ridl_podbuf_decode(#view) }
            }
            Some(Form::InlineArray(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an inline array carries no element form".to_string(),
                })?;
                let count = Literal::usize_suffixed(array.count as usize);
                match Self::array_element(ty) {
                    Some(element_ty) => {
                        let stride = self.form_size(element);
                        let item =
                            self.decode_expr(element, element_ty, place, &off.index(stride))?;
                        quote! { ::core::array::from_fn(|__i| #item) }
                    }
                    None => {
                        let region = place.region();
                        let off = off.tokens();
                        let bytes = quote! {
                            #region.bytes_at(#off, #count).collect::<::std::vec::Vec<::core::primitive::u8>>()
                        };
                        match self.scalar_of(ty)?.ctor() {
                            Some((path, ctor)) => quote! { #path::#ctor(#bytes) },
                            None => bytes,
                        }
                    }
                }
            }
            Some(Form::Descriptor(descriptor)) => match descriptor.kind.as_ref() {
                Some(Descriptor::Text(_)) => {
                    let text = self.view_expr(form, ty, place, off)?;
                    let owned = quote! { ::std::string::String::from(#text) };
                    match self.scalar_of(ty)?.ctor() {
                        Some((path, ctor)) => quote! { #path::#ctor(#owned) },
                        None => owned,
                    }
                }
                Some(Descriptor::Bytes(_)) => {
                    let bytes = self.view_expr(form, ty, place, off)?;
                    let owned = quote! { #bytes.to_vec() };
                    match self.scalar_of(ty)?.ctor() {
                        Some((path, ctor)) => quote! { #path::#ctor(#owned) },
                        None => owned,
                    }
                }
                Some(Descriptor::Array(array)) => {
                    let element = array.element.as_deref().ok_or_else(|| GenerateError {
                        message: "an array carries no element form".to_string(),
                    })?;
                    let element_ty = Self::array_element(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf array is decoded into no array type".to_string(),
                    })?;
                    let item = self.decode_expr(
                        element,
                        element_ty,
                        &Place::element(&quote! { __b }),
                        &Off::Lit(0),
                    )?;
                    let item_ty = model_type_tokens(self.ctx, element_ty);
                    let iter = Self::block_iter(place, off, item);
                    quote! { #iter.collect::<::std::vec::Vec<#item_ty>>() }
                }
                Some(Descriptor::Map(map)) => {
                    let (key_ty, value_ty) = Self::map_types(ty).ok_or_else(|| GenerateError {
                        message: "a podbuf map is decoded into no map type".to_string(),
                    })?;
                    let (key, value) = self.entry_fields(map, "a map")?;
                    let entry = Place::element(&quote! { __b });
                    let key_read = self.decode_expr(
                        Self::form_of(key, "a map")?,
                        key_ty,
                        &entry,
                        &Off::Lit(key.offset),
                    )?;
                    let value_read = self.decode_expr(
                        Self::form_of(value, "a map")?,
                        value_ty,
                        &entry,
                        &Off::Lit(value.offset),
                    )?;
                    let key_ty = model_type_tokens(self.ctx, key_ty);
                    let value_ty = model_type_tokens(self.ctx, value_ty);
                    let iter = Self::block_iter(place, off, quote! { (#key_read, #value_read) });
                    quote! { #iter.collect::<::std::vec::Vec<(#key_ty, #value_ty)>>() }
                }
                Some(Descriptor::Arms(_)) => {
                    return fail("an arm descriptor is decoded through the union".to_string());
                }
                None => return fail("a podbuf descriptor carries no kind".to_string()),
            },
            None => return fail("a podbuf field carries no form".to_string()),
        })
    }

    /// The `match` over a union's tag that builds the arm's value.
    fn union_decode(
        &self,
        at: usize,
        decl: &v1::Declaration,
        ud: &'a v1::Union,
        place: &Place,
    ) -> Result<TokenStream, GenerateError> {
        let owner = declared(decl.name.as_ref());
        let ty = ident(owner);
        let (tag, tag_prim, arm_field, arms) = self.union_fields(at, owner)?;
        let region = place.region();
        let arm_place = Place::of_desc(&place.buf);
        let mut matches: Vec<TokenStream> = Vec::new();
        let mut builds: Vec<TokenStream> = Vec::new();
        for arm in &ud.arms {
            let arm = self.union_arm(arm, arms, owner)?;
            let (variant, ordinal) = (&arm.variant, &arm.ordinal);
            let build = match arm.value {
                None => {
                    let path = self.encoder_path(&arm.ty)?;
                    let view = self.nested_view(&arm.ty, &arm_place)?;
                    quote! { #path::__ridl_podbuf_decode(#view) }
                }
                Some(value) => self.decode_expr(
                    Self::form_of(value, owner)?,
                    &arm.ty,
                    &arm_place,
                    &Off::Lit(value.offset),
                )?,
            };
            let build = quote! { #ty::#variant(#build) };
            matches.push(quote! { #ordinal => #build });
            builds.push(build);
        }
        let first = builds.first().cloned().ok_or_else(|| GenerateError {
            message: format!("`{owner}` declares no arm, so it decodes no value"),
        })?;
        let tag_at = Off::Lit(tag.offset).tokens();
        let desc_at = Off::Lit(arm_field.offset).tokens();
        let load = tag_prim.at();
        let widened = tag_prim.widen(quote! { #region.#load(#tag_at) });
        Ok(quote! {
            let __d = ::ridl_rt::podbuf::Desc::read(&#region, #desc_at);
            match #widened {
                #(#matches,)*
                _ => #first,
            }
        })
    }
}

/// One arm of a union as the view, the verifier and the decoder read it.
struct UnionArm<'a> {
    name: &'a str,
    variant: Ident,
    ordinal: Literal,
    head: &'a v1::PodbufHead,
    ty: v1::Type,
    /// The one value field of the arm head, for a scalar, enum or enum set
    /// arm; `None` for a struct or union arm, whose own codec reads it.
    value: Option<&'a v1::PodbufField>,
}

/// What a scalar type position is: a named scalar (its name, backing,
/// declaration and path), an enum, an enum set, or an inline or primitive
/// scalar.
enum ScalarShape<'a> {
    Named {
        name: &'a str,
        backing: ScalarBacking,
        scalar: &'a v1::Scalar,
        /// The declaration's path, `Name` or `crate::<pkg>::Name`.
        path: TokenStream,
        /// The reference as the model spells it, which `NamedScalar` reads.
        reference: &'a str,
    },
    Enum(&'a v1::Enum, TokenStream),
    EnumSet(TokenStream),
    Inline(ScalarBacking, Option<&'a v1::Scalar>),
}

impl ScalarShape<'_> {
    /// The named scalar of this shape, for the shared `check` call.
    fn named_scalar(&self) -> Option<NamedScalar> {
        match self {
            ScalarShape::Named {
                reference, scalar, ..
            } => Some(NamedScalar {
                name: (*reference).to_string(),
                ctor: if scalar.vacuous {
                    "new"
                } else {
                    "new_unchecked"
                },
            }),
            _ => None,
        }
    }

    /// The constructor of the named scalar: `new` for a vacuous type, whose
    /// `new` is infallible, `new_unchecked` otherwise.
    fn ctor(&self) -> Option<(TokenStream, Ident)> {
        match self {
            ScalarShape::Named { path, scalar, .. } => Some((
                path.clone(),
                format_ident!(
                    "{}",
                    if scalar.vacuous {
                        "new"
                    } else {
                        "new_unchecked"
                    }
                ),
            )),
            _ => None,
        }
    }
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
