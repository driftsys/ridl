//! IR v2 package to Rust source plus an extern-C header (ADR-0004 section 7,
//! ADR-0007 decision 13).
//!
//! The full E1.12 backend over the typl surface. Each declaration in a
//! [`v2::Package`] is realized twice: once as idiomatic Rust (the language
//! layer of typl reference Appendix D — every integer is `i64`, every float is
//! `f64`) and once, where the C ABI admits it, as an entry in a companion C
//! header. Named scalar types become `#[repr(transparent)]` newtypes so unit
//! safety survives into generated code (typl reference §5.7); composites map to
//! structs, enums, and Rust `enum` unions.
//!
//! Rust source is built as a [`proc_macro2::TokenStream`] with `quote` and
//! formatted with `prettyplease`, never by shelling out to `rustfmt`.
//!
//! Default derivation follows the leaf-recursion rule: an `impl Default` is
//! emitted for a type only when every field it transitively contains is
//! derivable. The IR `InitValue.derivable` flag on a composite-typed field is a
//! one-level flag, so same-package composite references are re-checked by
//! recursion rather than trusted (see the `defaults` module).

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use ridl_ir::v2;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

mod defaults;

/// The generated artifact for one package: Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub rust_source: String,
}

/// A failure to generate code from a package.
///
/// Carried as a value so codegen stays total: no stage in the pipeline panics
/// (ADR-0004 section 5). The `compile` driver folds `message` into its
/// diagnostic list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates Rust source and the extern-C header for `package`.
///
/// The call is total: it returns [`GenerateError`] rather than panicking. Every
/// emitted identifier is produced through `ident`, which escapes Rust
/// keywords as raw identifiers, so a typl name that happens to be a Rust
/// keyword (for example a field named `override`) is emitted as `r#override`
/// rather than panicking `format_ident!`. As a final guard the assembled token
/// stream is parsed with `syn::parse2`; a parse failure (a codegen bug) surfaces
/// as a `GenerateError` instead of unformatted output.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    let ctx = Ctx::new(package);

    let mut items: Vec<TokenStream> = Vec::new();
    let mut tuples: Vec<InducedTuple> = Vec::new();

    for decl in &package.decls {
        items.push(emit_decl(&ctx, decl, &mut tuples));
    }

    // Tuple types generate a named nested struct each (typl §11). Process the
    // worklist: emitting a tuple struct's fields can discover further nested
    // tuples, which are appended and drained here.
    //
    // A name is emitted once. The same tuple genuinely arrives twice — the
    // interaction module pre-discovers nested tuples to learn their names, and
    // emitting the outer tuple's fields finds them again — so a repeat of an
    // *identical* discovery is expected and skipped. A repeat under one name
    // with a different shape is a collision, and it is refused rather than
    // deduplicated; see [`tuple_collision`].
    let mut seen: HashMap<String, InducedTuple> = HashMap::new();
    let mut index = 0;
    while index < tuples.len() {
        let induced = tuples[index].clone();
        index += 1;
        if let Some(previous) = seen.get(&induced.name) {
            if previous.tuple != induced.tuple || previous.visibility != induced.visibility {
                return Err(tuple_collision(previous, &induced));
            }
            continue;
        }
        seen.insert(induced.name.clone(), induced.clone());
        items.push(emit_tuple_struct(&ctx, &induced, &mut tuples));
    }

    let tokens = quote! { #(#items)* };
    let file: syn::File = syn::parse2(tokens).map_err(|err| GenerateError {
        message: format!("generated Rust does not parse: {err}"),
    })?;

    Ok(Generated {
        rust_source: prettyplease::unparse(&file),
    })
}

/// One tuple type reached from a declaration, with the visibility that
/// declaration was declared at (typl §11).
///
/// A tuple has no name in source; the struct it generates is named after the
/// path that reached it and is emitted at module scope beside the declaration
/// that induced it. The visibility travels with the discovery for the same
/// reason [`v2::InterfaceShape::visibility`] carries a service's: the value is
/// authoritative at the point of discovery and nowhere else. By the time
/// [`emit_tuple_struct`] runs, the tuple is one entry in a flat worklist and
/// the declaration it came from is out of reach — which is exactly how the
/// struct came to be emitted `pub` over an `internal` declaration's payload
/// (issue #167).
///
/// One name is one struct. The same discovery repeated — the interaction
/// module pre-discovers a nested tuple's name and the drain finds it again —
/// is skipped; a repeat under one name with a different shape or visibility is
/// a collision and is refused ([`tuple_collision`]), because carrying a
/// visibility onto a name two declarations share has no sound answer. See
/// [`generate`].
#[derive(Debug, Clone)]
pub(crate) struct InducedTuple {
    /// The generated struct name — the CamelCase of the path that reached the
    /// tuple.
    pub(crate) name: String,
    pub(crate) tuple: v2::TupleType,
    /// The visibility of the declaration this tuple was reached from: an
    /// `internal struct`'s field, or an `internal interface`'s query return.
    pub(crate) visibility: i32,
}

/// Refuses a package in which two different tuples generate one struct name.
///
/// The name is the CamelCase of the path that reaches the tuple, and nothing
/// upstream keeps two paths from mangling to one string: `struct AB { c : … }`
/// and `struct A { bC : … }` both reach `ABC`, and neither draws a ridl
/// diagnostic. There is no sound way to pick between them, which is why this is
/// a refusal rather than a rule:
///
/// - **Keeping the first** — what the worklist did before — gives the second
///   declaration the *first one's shape*. `ridlc check` exits 0, the module
///   compiles, and the contract is silently wrong. It is also how carrying an
///   inducing declaration's visibility (issue #167) could narrow a struct a
///   public declaration uses, turning a silent wrong shape into a
///   `private_interfaces` build failure.
/// - **Keeping the widest visibility** would publish a package-private type's
///   shape to escape that build failure, which is the defect #167 fixed.
///
/// So neither dedup rule is sound and only rejection is. This is the same
/// answer `interact::check_name_collisions` gives every other generated-name
/// clash: codegen names the failure itself rather than handing rustc a module
/// whose meaning it cannot state.
///
/// The two shapes are named because the mangled name cannot distinguish them —
/// that is the whole defect — and the field lists are what a reader greps for.
fn tuple_collision(previous: &InducedTuple, current: &InducedTuple) -> GenerateError {
    fn shape(induced: &InducedTuple) -> String {
        let fields: Vec<String> = induced
            .tuple
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        format!("({})", fields.join(", "))
    }
    GenerateError {
        message: format!(
            "the generated name {name} is claimed by two different tuple types, {a} and {b}; \
             a tuple generates a struct named for the path that reaches it, and these two paths \
             spell one name — rename a field or a declaration so they differ",
            name = current.name,
            a = shape(previous),
            b = shape(current),
        ),
    }
}

// ---------------------------------------------------------------------------
// Package context — same-package name lookups for the leaf-recursion rules.
// ---------------------------------------------------------------------------

/// Read-only view of a package indexed by declaration name, so the emitter and
/// the default-derivation pass can resolve a same-package reference to its
/// declaration (cross-package references stay unresolved by design — this
/// backend generates one package at a time).
pub(crate) struct Ctx<'a> {
    decls: HashMap<&'a str, &'a v2::Decl>,
    /// The set of declaration names currently being expanded by the
    /// Default-derivation recursion. It guards against a cyclic IR: a
    /// same-package composite that reaches itself would otherwise recurse
    /// forever (C1b). The recursion inserts a name on entry and removes it on
    /// exit, so between top-level declarations the set is empty.
    visiting: RefCell<HashSet<String>>,
}

impl<'a> Ctx<'a> {
    fn new(package: &'a v2::Package) -> Self {
        let decls = package
            .decls
            .iter()
            .map(|decl| (decl.name.as_str(), decl))
            .collect();
        Ctx {
            decls,
            visiting: RefCell::new(HashSet::new()),
        }
    }

    /// The declaration named `name` in this package, or `None` for a
    /// cross-package (dotted) or unknown reference.
    pub(crate) fn lookup(&self, name: &str) -> Option<&'a v2::Decl> {
        self.decls.get(name).copied()
    }

    /// Marks `name` as being expanded by the Default recursion. Returns `true`
    /// when it was newly inserted, `false` when it is already on the expansion
    /// stack — a reference cycle that the caller must not recurse into (C1b).
    pub(crate) fn enter_default(&self, name: &str) -> bool {
        self.visiting.borrow_mut().insert(name.to_string())
    }

    /// Removes `name` from the Default-recursion expansion stack, balancing a
    /// prior [`enter_default`](Ctx::enter_default) that returned `true`.
    pub(crate) fn leave_default(&self, name: &str) {
        self.visiting.borrow_mut().remove(name);
    }
}

// ---------------------------------------------------------------------------
// Declaration emission.
// ---------------------------------------------------------------------------

fn emit_decl(ctx: &Ctx, decl: &v2::Decl, tuples: &mut Vec<InducedTuple>) -> TokenStream {
    let item = match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => emit_type_def(decl, td),
        Some(v2::decl::Kind::ConstDef(cd)) => return emit_const(ctx, decl, cd),
        Some(v2::decl::Kind::StructDef(sd)) => emit_struct(decl, sd, tuples),
        Some(v2::decl::Kind::EnumDef(ed)) => emit_enum(decl, ed),
        Some(v2::decl::Kind::EnumSetDef(esd)) => emit_enum_set(decl, esd),
        Some(v2::decl::Kind::UnionDef(ud)) => emit_union(decl, ud),
        // Interaction kinds ride `Interface.interactions`, never a package
        // decl, so none of them reaches this match; interfaces and services are
        // emitted by the `interact` module.
        Some(_) | None => return quote! {},
    };

    let default_impl = defaults::decl_default_expr(ctx, decl)
        .map(|expr| {
            let name = ident(&decl.name);
            quote! { impl Default for #name { fn default() -> Self { #expr } } }
        })
        .unwrap_or_default();

    quote! { #item #default_impl }
}

/// A named scalar type becomes a `#[repr(transparent)]` newtype (typl §5.7).
fn emit_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream {
    let name = ident(&decl.name);
    let inner = newtype_inner(td);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);
    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(#vis #inner);
    }
}

/// A constant becomes a `pub const`. A constant of a `String`-backed named type
/// (or of the `string` primitive, or a regex constant) is realized as a
/// `&'static str` rather than a value of the newtype: `String` cannot be
/// constructed in a `const` context. This asymmetry is documented in the C
/// header and here.
fn emit_const(ctx: &Ctx, decl: &v2::Decl, cd: &v2::ConstDef) -> TokenStream {
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);
    let name = ident(&decl.name);

    // A regex constant declares no type; it holds the pattern source text. The
    // IR stores that text with its typl `/…/` delimiters, which are syntax, not
    // pattern content, so they are stripped before the `&str` value is emitted:
    // the const holds the pattern a consumer can feed to a regex engine (M1).
    if let Some(regex) = &cd.regex {
        let pattern = strip_regex_delimiters(regex);
        return quote! { #attrs #vis const #name: &str = #pattern; };
    }

    let Some(type_ref) = cd.type_ref.as_deref() else {
        return quote! {};
    };

    // A named-type constant resolves through the type's backing; a
    // primitive-keyword constant reads the keyword directly.
    if let Some(backing) = same_package_scalar_backing(ctx, type_ref) {
        match backing {
            ScalarBacking::Float => {
                let value = numeric_tokens(&cd.value, true);
                let type_name = type_path(type_ref);
                quote! { #attrs #vis const #name: #type_name = #type_name(#value); }
            }
            ScalarBacking::Integer => {
                let value = numeric_tokens(&cd.value, false);
                let type_name = type_path(type_ref);
                quote! { #attrs #vis const #name: #type_name = #type_name(#value); }
            }
            ScalarBacking::Boolean => {
                let value = bool_tokens(&cd.value);
                let type_name = type_path(type_ref);
                quote! { #attrs #vis const #name: #type_name = #type_name(#value); }
            }
            ScalarBacking::String => {
                let value = cd.value.as_str();
                quote! { #attrs #vis const #name: &str = #value; }
            }
            ScalarBacking::Bytes => quote! {},
        }
    } else if let Some(prim) = primitive_keyword(type_ref) {
        match prim {
            v2::PrimitiveType::Integer => {
                let value = numeric_tokens(&cd.value, false);
                quote! { #attrs #vis const #name: i64 = #value; }
            }
            v2::PrimitiveType::Float => {
                let value = numeric_tokens(&cd.value, true);
                quote! { #attrs #vis const #name: f64 = #value; }
            }
            v2::PrimitiveType::Boolean => {
                let value = bool_tokens(&cd.value);
                quote! { #attrs #vis const #name: bool = #value; }
            }
            v2::PrimitiveType::String => {
                let value = cd.value.as_str();
                quote! { #attrs #vis const #name: &str = #value; }
            }
            v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => quote! {},
        }
    } else {
        // A cross-package or unresolved constant type: the backing is unknown
        // here, so the constant is skipped rather than mis-typed.
        quote! {}
    }
}

fn emit_struct(decl: &v2::Decl, sd: &v2::StructDef, tuples: &mut Vec<InducedTuple>) -> TokenStream {
    let name = ident(&decl.name);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);
    let repr = if sd.fixed_layout {
        quote! { #[repr(C)] }
    } else {
        quote! {}
    };

    let fields = sd.members.iter().filter_map(|member| match &member.member {
        Some(v2::struct_member::Member::Field(field)) => {
            Some(emit_field(&decl.name, decl.visibility, field, tuples))
        }
        // A reserved tombstone occupies an ordinal but emits no field
        // (typl §7.4).
        Some(v2::struct_member::Member::Reserved(_)) | None => None,
    });

    quote! {
        #attrs
        #repr
        #vis struct #name {
            #(#fields),*
        }
    }
}

fn emit_field(
    parent: &str,
    visibility: i32,
    field: &v2::Field,
    tuples: &mut Vec<InducedTuple>,
) -> TokenStream {
    let field_name = ident(&field.name);
    let attrs = field_attrs(field);
    let hint = format!("{}{}", camel_case(parent), camel_case(&field.name));
    let ty = field
        .r#type
        .as_ref()
        .map(|ft| field_type_tokens(ft, &hint, visibility, tuples))
        .unwrap_or_else(|| quote! { () });
    quote! { #attrs pub #field_name: #ty }
}

/// An enum becomes `#[repr(i64)]` with the declared discriminants (typl §8).
/// Variant names keep their typl `SCREAMING_SNAKE` spelling.
fn emit_enum(decl: &v2::Decl, ed: &v2::EnumDef) -> TokenStream {
    let name = ident(&decl.name);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);

    let variants = ed.values.iter().map(|value| {
        let vname = ident(&value.name);
        let disc = int_tokens(value.value);
        let vdoc = doc_attrs(&value.doc);
        quote! { #vdoc #vname = #disc }
    });

    quote! {
        #attrs
        #[repr(i64)]
        #vis enum #name {
            #(#variants),*
        }
    }
}

/// An enum set becomes a `#[repr(transparent)]` newtype over `i64` (the
/// language layer width, Appendix D) with one associated bit constant per bit
/// position (typl §9).
fn emit_enum_set(decl: &v2::Decl, esd: &v2::EnumSetDef) -> TokenStream {
    let name = ident(&decl.name);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);

    let bits = esd.bits.iter().map(|bit| {
        let bname = ident(&bit.name);
        let shift = int_tokens(bit.value);
        quote! { #vis const #bname: #name = #name(1 << #shift); }
    });

    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(#vis i64);
        impl #name {
            #(#bits)*
        }
    }
}

/// A union becomes a `pub enum` with one variant per arm; arm names are
/// CamelCased (typl §10). Reserved arms are skipped.
fn emit_union(decl: &v2::Decl, ud: &v2::UnionDef) -> TokenStream {
    let name = ident(&decl.name);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);

    let variants = ud.arms.iter().map(|arm| {
        let vname = ident(&camel_case(&arm.name));
        let ty = type_path(&arm.type_ref);
        let vdoc = doc_attrs(&arm.doc);
        quote! { #vdoc #vname(#ty) }
    });

    quote! {
        #attrs
        #vis enum #name {
            #(#variants),*
        }
    }
}

/// Emits the generated struct for one tuple type (typl §11), plus its `Default`
/// impl when every tuple field is derivable.
///
/// The struct carries the visibility of the declaration that induced it, and a
/// tuple nested inside it inherits the same one — a tuple has no visibility of
/// its own to declare, so the only visibility it can have is the one it was
/// reached at (see [`InducedTuple`] and [`vis_tokens`]). The fields stay `pub`,
/// as they are on a declared `struct`: a field's effective visibility is capped
/// by the item's, so `pub(crate) struct T { pub f: Private }` exposes nothing.
fn emit_tuple_struct(
    ctx: &Ctx,
    induced: &InducedTuple,
    tuples: &mut Vec<InducedTuple>,
) -> TokenStream {
    let InducedTuple {
        name,
        tuple,
        visibility,
    } = induced;
    let name_id = ident(name);
    let vis = vis_tokens(*visibility);
    let fields = tuple.fields.iter().map(|field| {
        let fname = ident(&field.name);
        let hint = format!("{}{}", name, camel_case(&field.name));
        let ty = field
            .r#type
            .as_ref()
            .map(|ft| field_type_tokens(ft, &hint, *visibility, tuples))
            .unwrap_or_else(|| quote! { () });
        quote! { pub #fname: #ty }
    });

    let struct_item = quote! {
        #vis struct #name_id {
            #(#fields),*
        }
    };

    let default_impl = defaults::tuple_default_expr(ctx, name, tuple)
        .map(|expr| quote! { impl Default for #name_id { fn default() -> Self { #expr } } })
        .unwrap_or_default();

    quote! { #struct_item #default_impl }
}

// ---------------------------------------------------------------------------
// Type mapping.
// ---------------------------------------------------------------------------

/// The Rust type of a field. Tuple field types generate a named nested struct
/// (recorded in `tuples`); the struct name is `hint` (CamelCase of the path).
///
/// `visibility` is the visibility of the declaration this position belongs to.
/// It is carried rather than derived because a tuple is anonymous in source and
/// declares none of its own, and because it reaches [`emit_tuple_struct`]
/// through a flat worklist that has forgotten where it came from
/// ([`InducedTuple`]).
pub(crate) fn field_type_tokens(
    ft: &v2::FieldType,
    hint: &str,
    visibility: i32,
    tuples: &mut Vec<InducedTuple>,
) -> TokenStream {
    let inner = match &ft.kind {
        Some(v2::field_type::Kind::Named(name)) => type_path(name),
        Some(v2::field_type::Kind::Primitive(prim)) => primitive_tokens(*prim),
        Some(v2::field_type::Kind::InlineScalar(td)) => inline_scalar_tokens(td),
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            let tuple_name = hint.to_string();
            tuples.push(InducedTuple {
                name: tuple_name.clone(),
                tuple: tuple.clone(),
                visibility,
            });
            let id = ident(&tuple_name);
            quote! { #id }
        }
        Some(v2::field_type::Kind::Array(array)) => {
            let element = array
                .element
                .as_ref()
                .map(|el| field_type_tokens(el, &format!("{hint}Element"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            if array.min == array.max {
                let len = usize_tokens(array.min);
                quote! { [#element; #len] }
            } else {
                quote! { Vec<#element> }
            }
        }
        Some(v2::field_type::Kind::Map(map)) => {
            let key = map
                .key
                .as_ref()
                .map(|k| field_type_tokens(k, &format!("{hint}Key"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            let value = map
                .value
                .as_ref()
                .map(|v| field_type_tokens(v, &format!("{hint}Value"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            quote! { Vec<(#key, #value)> }
        }
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct or tuple field in checked IR. Kept total.
        Some(v2::field_type::Kind::Stream(_)) | None => quote! { () },
    };

    if ft.optional {
        quote! { Option<#inner> }
    } else {
        inner
    }
}

/// The Rust newtype inner type for a named scalar backing (Appendix D language
/// layer): unit and float back to `f64`, integer to `i64`.
fn newtype_inner(td: &v2::TypeDef) -> TokenStream {
    match backing_scalar(td) {
        ScalarBacking::Float => quote! { f64 },
        ScalarBacking::Integer => quote! { i64 },
        ScalarBacking::Boolean => quote! { bool },
        ScalarBacking::String => quote! { String },
        ScalarBacking::Bytes => quote! { Vec<u8> },
    }
}

fn inline_scalar_tokens(td: &v2::TypeDef) -> TokenStream {
    match backing_scalar(td) {
        ScalarBacking::Float => quote! { f64 },
        ScalarBacking::Integer => quote! { i64 },
        ScalarBacking::Boolean => quote! { bool },
        ScalarBacking::String => quote! { String },
        ScalarBacking::Bytes => quote! { Vec<u8> },
    }
}

pub(crate) fn primitive_tokens(prim: i32) -> TokenStream {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Boolean => quote! { bool },
        v2::PrimitiveType::Integer => quote! { i64 },
        v2::PrimitiveType::Float => quote! { f64 },
        v2::PrimitiveType::String => quote! { String },
        v2::PrimitiveType::Bytes => quote! { Vec<u8> },
        v2::PrimitiveType::Unspecified => quote! { () },
    }
}

/// A resolved type reference: a bare `Ident` for a same-package name, a
/// `crate::`-anchored path for a cross-package `pkg.Name` reference (typl §3.2).
/// The dotted package path maps directly to Rust module path segments. The
/// `crate::` anchor lets a consumer compose several generated packages as
/// sibling modules rooted at the crate — `crate::veh::common::Speed` resolves
/// from any module, whereas a bare `veh::common::Speed` only resolves from the
/// crate root (I4).
pub(crate) fn type_path(reference: &str) -> TokenStream {
    if reference.contains('.') {
        let segments = reference.split('.').map(ident);
        quote! { crate #(:: #segments)* }
    } else {
        let id = ident(reference);
        quote! { #id }
    }
}

/// Strips a regex literal's surrounding `/…/` delimiters, leaving the pattern
/// body. A value without both delimiters is returned unchanged.
fn strip_regex_delimiters(regex: &str) -> &str {
    regex
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
        .unwrap_or(regex)
}

// ---------------------------------------------------------------------------
// Scalar backing classification (shared by emission and default derivation).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarBacking {
    Float,
    Integer,
    Boolean,
    String,
    Bytes,
}

/// The Rust-layer scalar class of a type definition's backing. A unit backing
/// implies float (typl §5.1).
pub(crate) fn backing_scalar(td: &v2::TypeDef) -> ScalarBacking {
    match td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(_)) => ScalarBacking::Float,
        Some(v2::backing::Kind::Primitive(prim)) => {
            match v2::PrimitiveType::try_from(*prim).unwrap_or(v2::PrimitiveType::Unspecified) {
                v2::PrimitiveType::Boolean => ScalarBacking::Boolean,
                v2::PrimitiveType::Integer => ScalarBacking::Integer,
                v2::PrimitiveType::Float => ScalarBacking::Float,
                v2::PrimitiveType::String => ScalarBacking::String,
                v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => ScalarBacking::Bytes,
            }
        }
        None => ScalarBacking::Float,
    }
}

/// The backing class of a same-package named scalar type, or `None` when the
/// reference does not name a scalar `TypeDef` in this package.
fn same_package_scalar_backing(ctx: &Ctx, reference: &str) -> Option<ScalarBacking> {
    match &ctx.lookup(reference)?.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Some(backing_scalar(td)),
        _ => None,
    }
}

/// Maps a typl primitive keyword written as a type reference to its primitive.
fn primitive_keyword(reference: &str) -> Option<v2::PrimitiveType> {
    match reference {
        "boolean" => Some(v2::PrimitiveType::Boolean),
        "integer" => Some(v2::PrimitiveType::Integer),
        "float" => Some(v2::PrimitiveType::Float),
        "string" => Some(v2::PrimitiveType::String),
        "bytes" => Some(v2::PrimitiveType::Bytes),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Attributes: docs, deprecation, visibility.
// ---------------------------------------------------------------------------

fn decl_attrs(decl: &v2::Decl) -> TokenStream {
    let doc = doc_attrs(&decl.doc);
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    quote! { #doc #deprecated }
}

fn field_attrs(field: &v2::Field) -> TokenStream {
    let doc = doc_attrs(&field.doc);
    let deprecated = deprecated_attr(field.deprecated.as_deref());
    quote! { #doc #deprecated }
}

/// One `#[doc]` attribute per line; prettyplease renders these as `///`
/// comments. A leading space makes the rendered comment read `/// text`.
pub(crate) fn doc_attrs(doc: &str) -> TokenStream {
    if doc.is_empty() {
        return quote! {};
    }
    let lines = doc.split('\n').map(|line| {
        let text = format!(" {line}");
        quote! { #[doc = #text] }
    });
    quote! { #(#lines)* }
}

/// `@deprecated` maps to `#[deprecated]`; a present-but-empty reason (the IR's
/// `Some("")`) still emits the bare attribute (typl §14.2).
pub(crate) fn deprecated_attr(reason: Option<&str>) -> TokenStream {
    match reason {
        Some("") => quote! { #[deprecated] },
        Some(reason) => quote! { #[deprecated(note = #reason)] },
        None => quote! {},
    }
}

/// `internal` maps to `pub(crate)` — Rust's package-private mechanism
/// (ADR-0002 §8, ADR-0008 decision 7, typl §3.3). The rule is per declaration,
/// not per module: a package holding one `internal` and one public declaration
/// generates one `pub(crate)` item and one `pub` item.
///
/// It governs the item a declaration is realized as **and the auxiliary types
/// that item's shape induces**. A tuple in a field or an interaction position
/// generates a named struct of its own ([`emit_tuple_struct`]), and that struct
/// carries the visibility of the declaration that induced it, the way #160
/// derived one visibility per interface and applied it to all four of that
/// interface's names.
///
/// Until issue #167 the induced struct was fixed at `pub`, which is wrong in
/// both directions. It publishes the shape of a declaration the keyword hides —
/// the argument #160 made for the interface's own four names applies unchanged
/// to a fifth name the same declaration generates. And it does not compile: an
/// `internal` declaration may name `internal` declarations freely (typl §3.3),
/// so `internal struct Holder { t : (a : Hidden) }` puts a `pub(crate)` type in
/// a `pub` struct's field and rustc reports `private_interfaces`. The corpus
/// denies that lint by name, and `ridlc check` accepts the source, so the two
/// halves disagreed until the visibility was carried.
///
/// The reverse direction is closed by TYPL-005 on the **source** route: a
/// public declaration naming an `internal` one is rejected, so a `pub` induced
/// struct never holds a `pub(crate)` type. That is not the same as an invariant,
/// and the difference is load-bearing. Two declarations whose paths mangle to
/// one struct name reach the same state by a route TYPL-005 cannot see — one
/// declaration `internal`, the other public, no `internal` payload type
/// anywhere — and carrying a visibility onto a name two declarations share
/// would make a program that compiled today fail `private_interfaces`. That is
/// why [`tuple_collision`] refuses the collision instead: the invariant holds
/// because the state that breaks it is not generated, not because it cannot be
/// described.
pub(crate) fn vis_tokens(visibility: i32) -> TokenStream {
    match v2::Visibility::try_from(visibility).unwrap_or(v2::Visibility::Unspecified) {
        v2::Visibility::Internal => quote! { pub(crate) },
        _ => quote! { pub },
    }
}

// ---------------------------------------------------------------------------
// Literals and identifiers.
// ---------------------------------------------------------------------------

/// A Rust identifier for a typl name. typl names are always character-valid
/// identifiers (typl §2.3); the only conflict is a name that is a Rust keyword,
/// escaped here as a raw identifier (`r#override`). The four keywords that
/// cannot be raw identifiers (`crate`, `self`, `Self`, `super`) and the bare
/// underscore are mangled with a trailing underscore.
///
/// The call is total, per the codegen contract (ADR-0004 §5, and the
/// never-panics guarantee `ridlc::compile` documents). A valid typl name is
/// never empty, so an empty `name` only arrives from malformed IR — but the
/// backend is also reachable from the language server over half-written
/// source, so it must not panic. An empty name lowers to Rust's wildcard `_`.
/// It cannot collide with a real name: a typl name of `_` is mangled to `__`
/// on the branch above.
///
/// How far `_` is caught depends on the position, and the split is not
/// uniform:
///
/// - **Declaration-name positions** — a struct, enum, trait or type-alias
///   name, an enum variant, a `fn`, `static` or `mod` name, a trait or impl
///   method. `_` is rejected by the `syn::parse2` gate in [`generate`], which
///   returns a [`GenerateError`], so the malformed name is reported rather
///   than emitted.
/// - **Field and binding positions** — a struct field, a tuple-struct field, a
///   `fn` parameter, a `const` name. syn *accepts* `_` here
///   (`syn::Field::parse_named` calls `Ident::parse_any` once it peeks
///   `Token![_]`), so [`emit_field`] would emit `pub _: T` and the gate would
///   not catch it. `rustc` still rejects the field, so the output is never
///   silently valid, but no [`GenerateError`] is raised. A derived `Default`
///   usually catches it anyway, because the struct *expression* it builds has
///   no valid `Member` — but `defaults::struct_default` returns `None` for a
///   non-constructible field (a cross-package reference carrying a declared
///   init, for one), and then nothing is left to catch it.
///
/// A field-position empty name is unreachable today, and for a structural
/// reason rather than a lucky one: `ridl_syntax`'s `Parser::block_body` and
/// `Parser::param_list` announce a member only on `SyntaxKind::Ident`, so
/// `field_def`, `param`, `enum_value` and `union_arm` are never entered
/// without a name. `Parser::interface_body` is the exception — it announces
/// members by the *interaction keyword*, so the name can be missing — which is
/// precisely and only why interactions were the vulnerable site.
/// `generate_emits_an_empty_field_name_without_a_derivable_default` pins that
/// gap, so a regression that makes a nameless field reachable is visible
/// rather than silent.
pub(crate) fn ident(name: &str) -> Ident {
    if let Ok(parsed) = syn::parse_str::<Ident>(name) {
        return parsed;
    }
    if matches!(name, "crate" | "self" | "Self" | "super" | "_") {
        return Ident::new(&format!("{name}_"), Span::call_site());
    }
    if name.is_empty() {
        // `Ident::new_raw("")` and `Ident::new("")` both panic; `Ident::new`
        // accepts `_` (`Ident::new_raw` does not — `r#_` is not a raw
        // identifier).
        return Ident::new("_", Span::call_site());
    }
    Ident::new_raw(name, Span::call_site())
}

/// Numeric literal tokens from a canonical decimal string. The int/float kind
/// comes from the caller (derived from the backing width), never from the
/// string form: the IR drops the float form, so a float value can read `"0"`.
/// A float literal is given a decimal point so it stays a float in Rust.
pub(crate) fn numeric_tokens(value: &str, is_float: bool) -> TokenStream {
    let text = if is_float && !value.contains('.') && !value.contains('e') && !value.contains('E') {
        format!("{value}.0")
    } else {
        value.to_string()
    };
    text.parse().unwrap_or_else(|_| quote! { 0 })
}

fn int_tokens(value: i64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}

fn usize_tokens(value: u64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}

pub(crate) fn bool_tokens(value: &str) -> TokenStream {
    if value == "true" {
        quote! { true }
    } else {
        quote! { false }
    }
}

/// CamelCase of a snake, screaming-snake, or camel name. Used for union variant
/// names and generated tuple struct names.
pub(crate) fn camel_case(name: &str) -> String {
    name.split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
