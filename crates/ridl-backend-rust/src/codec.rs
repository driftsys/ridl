//! The FlatBuffers payload codec, emitted into [`crate::generate`]'s output
//! (design note D-1 as amended, plan Task 4, stage K5).
//!
//! The codec as built, including what it does not promise and what is not
//! built, is `docs/design/flatbuffers-codec.md`. The design note this
//! module's comments cite by decision number is archived at
//! `docs/archive/2026-09-20-flatbuffers-codec-design.md`; read it for the
//! reasoning behind a decision, not for what the code does.
//!
//! For every declaration the projection names a root table for
//! ([`fb_projection::root_table`]) — which, since ADR-0019 decision 8, is
//! every declaration that projects a type at all: a struct over its own
//! table, a union over its wrapper, and a named scalar, an enum and an enum
//! set over a box table — this module emits, beside the domain type:
//!
//! - a view struct `<T>FbView<'a>`, the accessor D-3 asks for: it holds the
//!   verified bytes and a table offset and reads a field in place;
//! - the `Payload<FlatBuffers>` implementation, carrying `MAX_SIZE` as a
//!   literal (D-6), `encode` (D-2), `verify` (D-5) and `decode`;
//! - the free functions `__ridl_fb_encode_*`, `__ridl_fb_verify_*` and
//!   `__ridl_fb_decode_*`, one set per table the shape reaches, which is what
//!   lets a nested struct and an induced tuple struct be written from more
//!   than one place without repeating their bodies.
//!
//! The functions sit at the generated package's own module scope rather than
//! in a submodule, so a same-package reference is spelled exactly as
//! [`crate::type_path`] spells it everywhere else, and so they can read a
//! generated type's private inner value the way any other item of that module
//! can. Their names begin with `__ridl_fb_`, which no typl name collides with
//! (typl §15.1 gives a declaration a CamelCase name and a constant a
//! SCREAMING_SNAKE one).
//!
//! # What is withheld, and what is refused
//!
//! D-7's refusal is per-type, and this is where it is finally called. For a
//! root-table declaration whose [`fb_projection::max_size`] answers `None`,
//! the cause is attributed by [`crate::unbounded_member`]. A cause this
//! backend can name — an unbounded member, a member with no type, a layout
//! error, or an aggregate overflow — is a [`GenerateError`]. A cause it
//! cannot judge — a cross-package reference it does not resolve, or a
//! same-package cycle — withholds that one type's codec and nothing else,
//! which is the exemption stage K4 built and §4a of the design note records.
//!
//! # The inline layout
//!
//! `ridl_ir::projection::flatbuffers` owns the slot ids, the union
//! discriminant and the size bound, because two emitters must agree on them.
//! The *inline* layout — which byte of a table a field starts at, and how
//! large the table is — is computed here, because only this emitter can
//! observe it: a `.fbs` schema states no offsets. The bound stays sound
//! whatever order is chosen, because it charges
//! [`fb_projection::ALIGN_SLACK`] once per vtable slot and once more for the
//! table's own `soffset`, which is the worst case any order can reach. The
//! order taken is declaration order, each field aligned to its own width,
//! which is what makes the encoding deterministic (D-8).

use std::collections::{HashMap, HashSet};

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::name::{camel_case, snake_case};
use ridl_ir::projection::flatbuffers as fb_projection;
use ridl_ir::v2;

use crate::{
    Ctx, GenerateError, InducedTuple, ScalarBacking, backing_scalar, check_flatbuffers_bound,
    field_type_tokens, ident, type_path, unjudgeable_members, vis_tokens,
};

/// The alignment every buffer this codec writes is finished at: eight, the
/// widest scalar the projection emits. The bound charges
/// [`fb_projection::ALIGN_SLACK`] for the root, so aligning every buffer to
/// eight is inside it whether or not a particular value needs it, and a
/// buffer whose length does not depend on which fields happened to be present
/// is one less thing for a reader in another language to get wrong.
const BUFFER_ALIGN: usize = 8;

/// Emits the codec for `package`.
///
/// `tuples` is the induced tuple set [`crate::generate`] discovered, in
/// discovery order; a tuple's generated struct is a table like any other and
/// needs its own three functions.
pub(crate) fn package_items(
    ctx: &Ctx,
    package: &v2::Package,
    tuples: &[InducedTuple],
) -> Result<Vec<TokenStream>, GenerateError> {
    Codec {
        ctx,
        package,
        tuples: tuples
            .iter()
            .map(|induced| (induced.name.clone(), induced))
            .collect(),
    }
    .items()
}

struct Codec<'a> {
    ctx: &'a Ctx<'a>,
    package: &'a v2::Package,
    tuples: HashMap<String, &'a InducedTuple>,
}

// ---------------------------------------------------------------------------
// The wire shape of one type position
// ---------------------------------------------------------------------------

/// What a value of one typl type position becomes in a FlatBuffers buffer.
#[derive(Debug, Clone)]
enum Wire {
    /// An inline scalar.
    Scalar(Scalar),
    /// An out-of-line UTF-8 string; `Some` when a named scalar wraps it.
    Text(Option<NamedScalar>),
    /// An out-of-line `[ubyte]`; `Some` when a named scalar wraps it.
    Bytes(Option<NamedScalar>),
    /// An offset to the table of the named generated type.
    Table(String),
    /// An offset to the wrapper table of the named generated union.
    Union(String),
    /// A vector. `min == max` is a fixed array, which decodes to `[T; N]`.
    Vector {
        element: Box<Wire>,
        min: u64,
        max: u64,
    },
    /// A vector of entry tables (typl §12.2).
    Map {
        entry: Box<Table>,
        min: u64,
        max: u64,
    },
}

impl Wire {
    /// The bytes this value occupies inside its table: its own width for a
    /// scalar, a `uoffset_t` for everything else.
    fn inline_width(&self) -> usize {
        match self {
            Wire::Scalar(scalar) => scalar.prim.width(),
            _ => 4,
        }
    }
}

/// A named scalar the codec constructs on the way back.
#[derive(Debug, Clone)]
struct NamedScalar {
    name: String,
    /// `new_unchecked` for a constrained type, `new` for a vacuous one whose
    /// `new` is infallible and `const` (Epic 10 Task 4, `crate::scalar_ctor`).
    ctor: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prim {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Prim {
    fn width(self) -> usize {
        match self {
            Prim::Bool | Prim::I8 | Prim::U8 => 1,
            Prim::I16 | Prim::U16 => 2,
            Prim::I32 | Prim::U32 | Prim::F32 => 4,
            Prim::I64 | Prim::U64 | Prim::F64 => 8,
        }
    }

    /// The `ridl_rt::flatbuffers::Field` variant that carries it.
    fn field_variant(self) -> &'static str {
        match self {
            Prim::Bool => "Bool",
            Prim::I8 => "I8",
            Prim::U8 => "U8",
            Prim::I16 => "I16",
            Prim::U16 => "U16",
            Prim::I32 => "I32",
            Prim::U32 => "U32",
            Prim::I64 => "I64",
            Prim::U64 => "U64",
            Prim::F32 => "F32",
            Prim::F64 => "F64",
        }
    }

    fn read_fn(self) -> &'static str {
        match self {
            Prim::Bool => "read_bool",
            Prim::I8 => "read_i8",
            Prim::U8 => "read_u8",
            Prim::I16 => "read_i16",
            Prim::U16 => "read_u16",
            Prim::I32 => "read_i32",
            Prim::U32 => "read_u32",
            Prim::I64 => "read_i64",
            Prim::U64 => "read_u64",
            Prim::F32 => "read_f32",
            Prim::F64 => "read_f64",
        }
    }

    fn rust_name(self) -> &'static str {
        match self {
            Prim::Bool => "bool",
            Prim::I8 => "i8",
            Prim::U8 => "u8",
            Prim::I16 => "i16",
            Prim::U16 => "u16",
            Prim::I32 => "i32",
            Prim::U32 => "u32",
            Prim::I64 => "i64",
            Prim::U64 => "u64",
            Prim::F32 => "f32",
            Prim::F64 => "f64",
        }
    }

    /// The value a failed read is discharged with. `verify` makes it
    /// unreachable; it exists so that `decode` cannot fail.
    fn neutral(self) -> TokenStream {
        match self {
            Prim::Bool => quote! { false },
            Prim::F32 => quote! { 0.0f32 },
            Prim::F64 => quote! { 0.0f64 },
            other => {
                let literal: TokenStream = format!("0{}", other.rust_name())
                    .parse()
                    .unwrap_or_else(|_| quote! { 0 });
                literal
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
    /// A generated `#[repr(i64)]` enum. `first` is the variant a discharged
    /// read decodes to.
    Enum { name: String, first: String },
    /// A generated enum set: a `#[repr(transparent)]` newtype over `i64`.
    EnumSet { name: String },
}

#[derive(Debug, Clone)]
struct Scalar {
    prim: Prim,
    repr: Repr,
}

impl Scalar {
    /// The primitive-typed value to write, from a domain value expression.
    fn raw(&self, expr: TokenStream) -> TokenStream {
        let base = match &self.repr {
            Repr::Bool | Repr::Int | Repr::Float => quote! { #expr },
            Repr::Named(_) => quote! { #expr.get() },
            Repr::Enum { .. } | Repr::EnumSet { .. } => quote! { i64::from(#expr) },
        };
        match self.prim {
            Prim::Bool | Prim::I64 | Prim::F64 => base,
            other => {
                let ty = format_ident!("{}", other.rust_name());
                quote! { #base as #ty }
            }
        }
    }

    /// The `Result<prim, Malformed>` read of this scalar.
    fn read(&self, buf: &TokenStream, at: &TokenStream) -> TokenStream {
        let call = format_ident!("{}", self.prim.read_fn());
        quote! { ::ridl_rt::flatbuffers::#call(#buf, #at) }
    }

    /// The language-layer value (`i64`, `f64` or `bool`) of a primitive read.
    ///
    /// The `u64` cast carries no parentheses of its own: every caller either
    /// uses the result as a whole expression or passes it as a sole call
    /// argument, where `as` binds tightly enough, and a redundant pair draws
    /// `unused_parens` in a consumer's build. The one caller that needs
    /// grouping — the borrow `verify` hands `check` — writes its own.
    fn widen(&self, raw: TokenStream) -> TokenStream {
        match self.prim {
            Prim::Bool | Prim::I64 | Prim::F64 => raw,
            Prim::U64 => quote! { #raw as i64 },
            Prim::F32 => quote! { f64::from(#raw) },
            _ => quote! { i64::from(#raw) },
        }
    }

    /// The domain value, from bytes `verify` has already accepted.
    fn decode(&self, buf: &TokenStream, at: &TokenStream) -> TokenStream {
        let read = self.read(buf, at);
        let neutral = self.prim.neutral();
        let widened = self.widen(quote! { #read.unwrap_or(#neutral) });
        match &self.repr {
            Repr::Bool | Repr::Int | Repr::Float => widened,
            Repr::Named(named) => {
                let ty = type_path(&named.name);
                let ctor = format_ident!("{}", named.ctor);
                quote! { #ty::#ctor(#widened) }
            }
            Repr::Enum { name, first } => {
                let ty = type_path(name);
                let variant = ident(first);
                quote! {
                    <#ty as ::core::convert::TryFrom<i64>>::try_from(#widened)
                        .unwrap_or(#ty::#variant)
                }
            }
            Repr::EnumSet { name } => {
                let ty = type_path(name);
                quote! {
                    <#ty as ::core::convert::TryFrom<i64>>::try_from(#widened)
                        .unwrap_or(#ty(0i64))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A table and its slots
// ---------------------------------------------------------------------------

/// The two forms one value is reached by while encoding.
///
/// A scalar is read out by value and every other shape by reference, and
/// which of the two a given position hands over depends on where it sits: a
/// struct field is a place, a vector element is already a reference. Carrying
/// both keeps every generated expression free of a `&*` or a `*&` that rustc
/// warns about in a consumer's build.
#[derive(Clone)]
struct Operand {
    /// A place expression of the value's own type.
    place: TokenStream,
    /// An expression of type `&T`.
    reference: TokenStream,
}

impl Operand {
    /// A place and the reference taken from it: a struct field, or a
    /// position of a map entry's pair.
    fn owned(place: TokenStream) -> Self {
        Operand {
            reference: quote! { &#place },
            place,
        }
    }

    /// A binding that is already a reference: a vector element, or the inside
    /// of an `Option`.
    fn borrowed(reference: TokenStream) -> Self {
        Operand {
            place: quote! { *#reference },
            reference,
        }
    }
}

/// How one slot's value is reached from the value being encoded.
#[derive(Debug, Clone)]
enum Access {
    /// A named field of a struct or an induced tuple struct.
    Field(String),
    /// A position of the `(K, V)` pair a map entry is encoded from.
    Position(usize),
}

#[derive(Debug, Clone)]
struct Slot {
    /// The FlatBuffers id, which is the vtable slot.
    id: u16,
    /// The field's first byte, measured from the start of the table.
    offset: u16,
    wire: Wire,
    optional: bool,
    /// The typl name, used for the accessor and for the decoded field.
    name: String,
    /// The IR type, which is what the accessor's return type is read from.
    field_type: v2::FieldType,
    /// The name an induced tuple struct at this position carries, which the
    /// accessor's return type needs whenever the tuple sits inside a
    /// collection.
    hint: String,
    access: Access,
}

impl Slot {
    fn operand(&self, base: &TokenStream) -> Operand {
        match &self.access {
            Access::Field(name) => {
                let field = ident(&snake_case(name));
                Operand::owned(quote! { #base.#field })
            }
            Access::Position(index) => {
                let position = Literal::usize_unsuffixed(*index);
                Operand::owned(quote! { #base.#position })
            }
        }
    }
}

/// One named field on its way to becoming a [`Slot`], before the inline
/// layout has placed it.
struct Entry {
    id: u16,
    name: String,
    field_type: v2::FieldType,
    hint: String,
    wire: Wire,
}

#[derive(Debug, Clone)]
struct Table {
    slots: Vec<Slot>,
    /// The table's inline size, which its vtable states.
    size: usize,
    /// The alignment its first byte needs.
    align: usize,
    /// The number of vtable entries: one per id up to the highest one used,
    /// which is [`fb_projection::TableLayout::vtable_slots`].
    vtable_slots: u16,
}

/// Places `widths` in declaration order, each aligned to its own width, after
/// the four bytes a table begins with — the signed offset back to its vtable.
///
/// Returns the offsets, the table's inline size and its alignment.
fn place(widths: &[usize]) -> (Vec<u16>, usize, usize) {
    let mut cursor = 4usize;
    let mut align = 4usize;
    let mut offsets = Vec::with_capacity(widths.len());
    for width in widths {
        let width = *width;
        cursor = cursor.div_ceil(width) * width;
        offsets.push(cursor as u16);
        cursor += width;
        align = align.max(width);
    }
    (offsets, cursor.max(4), align)
}

/// The name of the struct a tuple in `parent.field` induces — the same string
/// [`crate::emit_field`] builds for it.
fn field_hint(parent: &str, field: &str) -> String {
    format!("{}{}", camel_case(parent), camel_case(field))
}

fn view_ident(owner: &str) -> Ident {
    format_ident!("{}FbView", ident(owner))
}

fn encode_ident(owner: &str) -> Ident {
    format_ident!("__ridl_fb_encode_{}", snake_case(owner))
}

fn verify_ident(owner: &str) -> Ident {
    format_ident!("__ridl_fb_verify_{}", snake_case(owner))
}

fn decode_ident(owner: &str) -> Ident {
    format_ident!("__ridl_fb_decode_{}", snake_case(owner))
}

/// An owner as written at a *reference* site: the declared name for a type of
/// this package, and the dotted reference for one of another package of the
/// same build (driftsys/ridl#467).
///
/// The four `*_ident` functions above name an item where it is **defined**,
/// which is always this package — every root the codec emits comes from
/// `self.package.decls`, so a definition never carries a module prefix. The
/// four `*_path` functions below name one where it is **used**, which may be
/// another package's module in the tree `ridlc` writes.
///
/// This is why every `__ridl_fb_*` function is emitted `pub(crate)` rather
/// than as the bare `fn` it was before #467: a caller may now sit in another
/// module of the emitted crate. The emitted crate is one crate per build, so
/// `pub(crate)` reaches every generated caller and adds nothing to the
/// crate's public surface. The generated `check` of a named scalar carries
/// the same visibility, for the same reason, and says so where it is
/// emitted.
fn split_owner(owner: &str) -> (Option<&str>, &str) {
    match owner.rsplit_once('.') {
        Some((package, name)) => (Some(package), name),
        None => (None, owner),
    }
}

/// `crate::<segments>::` for a foreign owner, and nothing for a local one.
///
/// The segments are spelled through [`crate::ident`], which is the same
/// spelling [`crate::module_segment`] gives the module tree `ridlc` writes, so
/// a path emitted here and the module it names cannot drift apart.
fn owner_prefix(package: Option<&str>) -> TokenStream {
    match package {
        Some(package) => {
            let segments = package.split('.').map(ident);
            quote! { crate #(:: #segments)* :: }
        }
        None => quote! {},
    }
}

fn view_path(owner: &str) -> TokenStream {
    let (package, name) = split_owner(owner);
    let prefix = owner_prefix(package);
    let id = view_ident(name);
    quote! { #prefix #id }
}

fn encode_path(owner: &str) -> TokenStream {
    let (package, name) = split_owner(owner);
    let prefix = owner_prefix(package);
    let id = encode_ident(name);
    quote! { #prefix #id }
}

fn verify_path(owner: &str) -> TokenStream {
    let (package, name) = split_owner(owner);
    let prefix = owner_prefix(package);
    let id = verify_ident(name);
    quote! { #prefix #id }
}

fn decode_path(owner: &str) -> TokenStream {
    let (package, name) = split_owner(owner);
    let prefix = owner_prefix(package);
    let id = decode_ident(name);
    quote! { #prefix #id }
}

/// The typl constraint check for a named scalar's value, over a borrow
/// (design note D-4, plan Task 5, stage K6). `value` is an expression
/// already of `check`'s own parameter type — `&f64`/`&i64`/`&bool` for a
/// numeric or boolean backing, `&str` for a string backing, `&[u8]` for a
/// bytes backing (`crate::check_param_type`).
///
/// `None` when the type is vacuous (`crate::emit_vacuous_type_def`): such a
/// type emits no `check`, because it has no constraint to check, so this is
/// exactly [`NamedScalar::ctor`]'s two cases — `"new_unchecked"` (a
/// constrained type, which does emit `check`) and `"new"` (a vacuous one,
/// which does not).
fn named_scalar_check(named: &NamedScalar, value: TokenStream) -> Option<TokenStream> {
    if named.ctor != "new_unchecked" {
        return None;
    }
    // The owner may name another package of the build (driftsys/ridl#467), so
    // this is a path rather than an identifier.
    let ty = type_path(&named.name);
    Some(quote! {
        #ty::check(#value)
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    })
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

impl<'a> Codec<'a> {
    /// Resolves a type reference against this package first and then the
    /// others of the build, returning the declaration and the owner string a
    /// reference site writes (driftsys/ridl#467).
    ///
    /// The owner is the bare declared name for a type of this package, and the
    /// dotted reference for one of another package — which is what
    /// [`split_owner`] reads back to decide whether a path carries a module
    /// prefix. A same-package name shadows a foreign one, which is the
    /// resolution order the rest of the backend already uses.
    fn resolve(&self, reference: &str) -> Option<(&'a v2::Decl, String)> {
        if let Some(decl) = self.ctx.lookup(reference) {
            return Some((decl, decl.name.clone()));
        }
        let (package_name, name) = reference.rsplit_once('.')?;
        let package = self
            .ctx
            .others
            .iter()
            .find(|other| other.name == package_name)?;
        let decl = package.decls.iter().find(|decl| decl.name == name)?;
        Some((decl, reference.to_string()))
    }

    /// The projection's view of this build: this package, and the others the
    /// caller handed in.
    ///
    /// `others` was `&[]` until E11.14 (driftsys/ridl#467): the codec read one
    /// package and withheld every type that reached another, because a foreign
    /// named scalar's FlatBuffers width is a fact of the package that declares
    /// it. The pipeline now hands every package of the build, so a
    /// cross-package reference is sized like a local one.
    fn packages(&self) -> fb_projection::Packages<'_> {
        fb_projection::Packages {
            package: self.package,
            others: self.ctx.others,
        }
    }

    fn items(&self) -> Result<Vec<TokenStream>, GenerateError> {
        let mut roots: Vec<&v2::Decl> = Vec::new();
        let mut withheld: Vec<&v2::Decl> = Vec::new();
        for decl in &self.package.decls {
            if fb_projection::root_table(decl).is_none() {
                continue;
            }
            match fb_projection::max_size(self.packages(), decl) {
                Some(_) => roots.push(decl),
                // D-7's refusal, wired in per type. An `Ok` here means the
                // cause is one this backend cannot judge, so the type
                // carries no codec and says so.
                None => {
                    check_flatbuffers_bound(self.ctx, self.package, decl)?;
                    withheld.push(decl);
                }
            }
        }

        let mut items: Vec<TokenStream> = Vec::new();
        for decl in &withheld {
            items.push(self.withheld_note(decl));
        }
        for decl in &roots {
            items.extend(self.decl_items(decl)?);
        }
        for name in self.reachable_tuples(&roots) {
            let induced = self.tuples[&name];
            let table = self.tuple_table(induced)?;
            items.extend(self.table_items(&induced.name, induced.visibility, &table)?);
        }
        Ok(items)
    }

    /// The note left in the generated source where a codec is withheld.
    ///
    /// A type this backend cannot judge gets no `Payload<FlatBuffers>`
    /// implementation, which a consumer otherwise meets as an unsatisfied
    /// trait bound in their own crate, far from the cause. The note names the
    /// type, the member that could not be judged, and the issue tracking it
    /// (driftsys/ridl#467), so what the consumer meets is a reason.
    ///
    /// It is a `const` rather than a bare comment because `quote!` emits
    /// tokens, and a doc attribute is the only comment that survives into
    /// `prettyplease`'s output. The name cannot collide with a typl constant:
    /// typl §15.1 gives one a SCREAMING_SNAKE name, and no typl name begins
    /// with an underscore.
    fn withheld_note(&self, decl: &v2::Decl) -> TokenStream {
        let name = format_ident!(
            "__RIDL_FB_NO_CODEC_{}",
            snake_case(&decl.name).to_uppercase()
        );
        let members = unjudgeable_members(self.ctx, decl);
        let cause = if members.is_empty() {
            " No member of it could be judged.".to_string()
        } else {
            format!(
                " The member{} {} reach{} a reference this backend does not resolve — a \
                 cross-package reference, a same-package cycle, or a stream.",
                if members.len() == 1 { "" } else { "s" },
                members
                    .iter()
                    .map(|member| format!("`{member}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                if members.len() == 1 { "es" } else { "" },
            )
        };
        let headline = format!(
            " `{}` carries no `Payload<FlatBuffers>` implementation.",
            decl.name
        );
        quote! {
            #[doc = #headline]
            ///
            #[doc = #cause]
            /// `ridl-backend-rust` generates one package at a time and reads
            /// no other, so it can neither size nor encode such a type: a
            /// foreign named scalar's FlatBuffers width is a fact of the
            /// package that declares it.
            ///
            /// This is a silent omission in the sense ADR-0016 decision 6 and
            /// ADR-0017 decision 4 rule out, and it is deliberate for now.
            /// driftsys/ridl#467 tracks it and states the fix: `generate`
            /// handed the other packages.
            #[allow(dead_code)]
            const #name: () = ();
        }
    }

    /// The induced tuples reachable from `roots`, in discovery order.
    ///
    /// Only a reached tuple gets functions. A tuple induced by a declaration
    /// whose codec was withheld has no caller, and emitting its functions
    /// would put dead code in a consumer's build.
    fn reachable_tuples(&self, roots: &[&v2::Decl]) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: Vec<(String, v2::FieldType)> = Vec::new();

        for decl in roots {
            if let Some(v2::decl::Kind::StructDef(def)) = &decl.kind {
                for member in &def.members {
                    if let Some(v2::struct_member::Member::Field(field)) = &member.member
                        && let Some(ty) = field.r#type.clone()
                    {
                        queue.push((field_hint(&decl.name, &field.name), ty));
                    }
                }
            }
        }

        let mut index = 0;
        while index < queue.len() {
            let (hint, ty) = queue[index].clone();
            index += 1;
            match ty.kind {
                Some(v2::field_type::Kind::Tuple(tuple)) => {
                    if self.tuples.contains_key(&hint) && seen.insert(hint.clone()) {
                        found.push(hint.clone());
                        for field in &tuple.fields {
                            if let Some(inner) = field.r#type.clone() {
                                queue.push((format!("{hint}{}", camel_case(&field.name)), inner));
                            }
                        }
                    }
                }
                Some(v2::field_type::Kind::Array(array)) => {
                    if let Some(element) = array.element {
                        queue.push((format!("{hint}Element"), *element));
                    }
                }
                Some(v2::field_type::Kind::Map(map)) => {
                    if let Some(key) = map.key {
                        queue.push((format!("{hint}Key"), *key));
                    }
                    if let Some(value) = map.value {
                        queue.push((format!("{hint}Value"), *value));
                    }
                }
                _ => {}
            }
        }
        found
    }

    fn decl_items(&self, decl: &v2::Decl) -> Result<Vec<TokenStream>, GenerateError> {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => {
                let table = self.struct_table(&decl.name, def)?;
                let mut items = self.table_items(&decl.name, decl.visibility, &table)?;
                items.push(self.payload_impl(decl)?);
                Ok(items)
            }
            Some(v2::decl::Kind::UnionDef(def)) => {
                let mut items = self.union_items(decl, def)?;
                items.push(self.payload_impl(decl)?);
                Ok(items)
            }
            // A named scalar, an enum and an enum set are rooted in a box
            // table (ADR-0019 decision 8).
            Some(
                v2::decl::Kind::TypeDef(_)
                | v2::decl::Kind::EnumDef(_)
                | v2::decl::Kind::EnumSetDef(_),
            ) => {
                let mut items = self.root_box_items(decl)?;
                items.push(self.payload_impl(decl)?);
                Ok(items)
            }
            // `root_table` admits nothing else.
            _ => Ok(Vec::new()),
        }
    }

    // -----------------------------------------------------------------
    // Classification
    // -----------------------------------------------------------------

    /// The wire shape of one type position. `hint` is the name an induced
    /// tuple struct at this position carries.
    fn wire(&self, ty: &v2::FieldType, hint: &str) -> Result<Wire, GenerateError> {
        match ty.kind.as_ref() {
            Some(v2::field_type::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive).ok() {
                    Some(v2::PrimitiveType::Boolean) => Ok(Wire::Scalar(Scalar {
                        prim: Prim::Bool,
                        repr: Repr::Bool,
                    })),
                    Some(v2::PrimitiveType::Integer) => Ok(Wire::Scalar(Scalar {
                        prim: Prim::I64,
                        repr: Repr::Int,
                    })),
                    Some(v2::PrimitiveType::Float) => Ok(Wire::Scalar(Scalar {
                        prim: Prim::F64,
                        repr: Repr::Float,
                    })),
                    // A bare `string` or `bytes` carries no length bound, so
                    // it has no finite bound and the type was refused before
                    // this point (typl §15.3 keeps it out of a field position
                    // anyway).
                    _ => Err(GenerateError {
                        message: "a FlatBuffers codec cannot carry an unbounded primitive"
                            .to_string(),
                    }),
                }
            }
            Some(v2::field_type::Kind::InlineScalar(td)) => self.scalar_wire(td, None),
            Some(v2::field_type::Kind::Named(reference)) => {
                let Some((decl, owner)) = self.resolve(reference) else {
                    return Err(GenerateError {
                        message: format!(
                            "`{reference}` resolves in no package of this build, so no \
                             FlatBuffers codec can be emitted for it"
                        ),
                    });
                };
                match &decl.kind {
                    Some(v2::decl::Kind::TypeDef(td)) => self.scalar_wire(
                        td,
                        Some(NamedScalar {
                            name: owner.clone(),
                            ctor: if v2::constraint_is_vacuous(td.constraint.as_ref()) {
                                "new"
                            } else {
                                "new_unchecked"
                            },
                        }),
                    ),
                    Some(v2::decl::Kind::EnumDef(def)) => {
                        let Some(first) = def.values.first() else {
                            return Err(GenerateError {
                                message: format!(
                                    "`{}` declares no value, so no FlatBuffers codec can decode \
                                     one",
                                    decl.name
                                ),
                            });
                        };
                        Ok(Wire::Scalar(Scalar {
                            // Every typl enum is emitted at one underlying
                            // width, `long`, which is what the projection
                            // charges it.
                            prim: Prim::I64,
                            repr: Repr::Enum {
                                name: owner.clone(),
                                first: first.name.clone(),
                            },
                        }))
                    }
                    Some(v2::decl::Kind::EnumSetDef(def)) => Ok(Wire::Scalar(Scalar {
                        prim: int_prim(def.width).ok_or_else(|| GenerateError {
                            message: format!("`{}` carries no integer width", decl.name),
                        })?,
                        repr: Repr::EnumSet {
                            name: owner.clone(),
                        },
                    })),
                    Some(v2::decl::Kind::StructDef(_)) => Ok(Wire::Table(owner.clone())),
                    Some(v2::decl::Kind::UnionDef(_)) => Ok(Wire::Union(owner.clone())),
                    _ => Err(GenerateError {
                        message: format!(
                            "`{reference}` names a declaration a FlatBuffers codec cannot carry"
                        ),
                    }),
                }
            }
            Some(v2::field_type::Kind::Array(array)) => {
                let element = array.element.as_deref().ok_or_else(|| GenerateError {
                    message: "an array carries no element type".to_string(),
                })?;
                self.refuse_optional(element, "an array element")?;
                Ok(Wire::Vector {
                    element: Box::new(self.wire(element, &format!("{hint}Element"))?),
                    min: array.min,
                    max: array.max,
                })
            }
            Some(v2::field_type::Kind::Map(map)) => {
                let key = map.key.as_deref().ok_or_else(|| GenerateError {
                    message: "a map carries no key type".to_string(),
                })?;
                let value = map.value.as_deref().ok_or_else(|| GenerateError {
                    message: "a map carries no value type".to_string(),
                })?;
                self.refuse_optional(key, "a map key")?;
                self.refuse_optional(value, "a map value")?;
                let key_hint = format!("{hint}Key");
                let value_hint = format!("{hint}Value");
                let entry = self.map_entry_table(
                    self.wire(key, &key_hint)?,
                    key.clone(),
                    key_hint,
                    self.wire(value, &value_hint)?,
                    value.clone(),
                    value_hint,
                );
                Ok(Wire::Map {
                    entry: Box::new(entry),
                    min: map.min,
                    max: map.max,
                })
            }
            // A tuple generates a named struct, which is a table like any
            // other (typl §11).
            Some(v2::field_type::Kind::Tuple(_)) => Ok(Wire::Table(hint.to_string())),
            _ => Err(GenerateError {
                message: "a FlatBuffers codec cannot carry this type position".to_string(),
            }),
        }
    }

    /// An `optional` marker is a table field's property. A FlatBuffers vector
    /// has no absent element and a map entry no absent half, so a `?` in one
    /// of those positions is refused rather than silently written as present.
    fn refuse_optional(&self, ty: &v2::FieldType, what: &str) -> Result<(), GenerateError> {
        if ty.optional {
            return Err(GenerateError {
                message: format!(
                    "{what} is optional, which FlatBuffers cannot represent — only a table field \
                     may be absent"
                ),
            });
        }
        Ok(())
    }

    fn scalar_wire(
        &self,
        td: &v2::TypeDef,
        named: Option<NamedScalar>,
    ) -> Result<Wire, GenerateError> {
        let backing = backing_scalar(td);
        match &td.width {
            Some(v2::type_def::Width::IntWidth(width)) => {
                let prim = int_prim(*width).ok_or_else(|| GenerateError {
                    message: "a scalar carries no integer width".to_string(),
                })?;
                Ok(Wire::Scalar(Scalar {
                    prim,
                    repr: scalar_repr(named, backing),
                }))
            }
            Some(v2::type_def::Width::FloatWidth(width)) => {
                let prim = match v2::FloatWidth::try_from(*width).ok() {
                    Some(v2::FloatWidth::F32) => Prim::F32,
                    Some(v2::FloatWidth::F64) => Prim::F64,
                    _ => {
                        return Err(GenerateError {
                            message: "a scalar carries no float width".to_string(),
                        });
                    }
                };
                Ok(Wire::Scalar(Scalar {
                    prim,
                    repr: scalar_repr(named, backing),
                }))
            }
            None => match backing {
                ScalarBacking::Boolean => Ok(Wire::Scalar(Scalar {
                    prim: Prim::Bool,
                    repr: scalar_repr(named, backing),
                })),
                ScalarBacking::String => Ok(Wire::Text(named)),
                ScalarBacking::Bytes => Ok(Wire::Bytes(named)),
                // A unit backing implies float and always carries a derived
                // width, so it never reaches here (typl §5.1).
                ScalarBacking::Float | ScalarBacking::Integer => Err(GenerateError {
                    message: "a numeric scalar carries no width".to_string(),
                }),
            },
        }
    }

    // -----------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------

    fn struct_table(&self, owner: &str, def: &v2::StructDef) -> Result<Table, GenerateError> {
        let layout = fb_projection::struct_table(owner, def).map_err(|err| GenerateError {
            message: err.message,
        })?;
        let mut entries: Vec<Entry> = Vec::new();
        for member in &def.members {
            let Some(v2::struct_member::Member::Field(field)) = &member.member else {
                // A reserved tombstone holds its id and emits no field
                // (typl §7.4), so it takes no inline bytes here.
                continue;
            };
            let Some(ty) = field.r#type.as_ref() else {
                return Err(GenerateError {
                    message: format!("`{owner}.{}` carries no type", field.name),
                });
            };
            let id = u16::try_from(field.ordinal.saturating_sub(1)).map_err(|_| GenerateError {
                message: format!(
                    "`{owner}.{}` has ordinal {}, which a FlatBuffers vtable cannot carry",
                    field.name, field.ordinal
                ),
            })?;
            let hint = field_hint(owner, &field.name);
            let wire = self.wire(ty, &hint)?;
            entries.push(Entry {
                id,
                name: field.name.clone(),
                field_type: ty.clone(),
                hint,
                wire,
            });
        }
        self.assemble(owner, layout.vtable_slots(), entries)
    }

    fn tuple_table(&self, induced: &InducedTuple) -> Result<Table, GenerateError> {
        let layout = fb_projection::tuple_table(&induced.name, &induced.tuple).map_err(|err| {
            GenerateError {
                message: err.message,
            }
        })?;
        let mut entries: Vec<Entry> = Vec::new();
        for (index, field) in induced.tuple.fields.iter().enumerate() {
            let Some(ty) = field.r#type.as_ref() else {
                return Err(GenerateError {
                    message: format!("`{}.{}` carries no type", induced.name, field.name),
                });
            };
            let id = u16::try_from(index).map_err(|_| GenerateError {
                message: format!(
                    "`{}` has more tuple fields than a FlatBuffers vtable can carry",
                    induced.name
                ),
            })?;
            let hint = format!("{}{}", induced.name, camel_case(&field.name));
            let wire = self.wire(ty, &hint)?;
            entries.push(Entry {
                id,
                name: field.name.clone(),
                field_type: ty.clone(),
                hint,
                wire,
            });
        }
        self.assemble(&induced.name, layout.vtable_slots(), entries)
    }

    #[allow(clippy::too_many_arguments)]
    fn map_entry_table(
        &self,
        key: Wire,
        key_type: v2::FieldType,
        key_hint: String,
        value: Wire,
        value_type: v2::FieldType,
        value_hint: String,
    ) -> Table {
        let widths = vec![key.inline_width(), value.inline_width()];
        let (offsets, size, align) = place(&widths);
        Table {
            slots: vec![
                Slot {
                    id: fb_projection::MAP_ENTRY_KEY_ID as u16,
                    offset: offsets[0],
                    wire: key,
                    optional: false,
                    name: "key".to_string(),
                    field_type: key_type,
                    hint: key_hint,
                    access: Access::Position(0),
                },
                Slot {
                    id: fb_projection::MAP_ENTRY_VALUE_ID as u16,
                    offset: offsets[1],
                    wire: value,
                    optional: false,
                    name: "value".to_string(),
                    field_type: value_type,
                    hint: value_hint,
                    access: Access::Position(1),
                },
            ],
            size,
            align,
            vtable_slots: 2,
        }
    }

    fn assemble(
        &self,
        owner: &str,
        vtable_slots: u64,
        entries: Vec<Entry>,
    ) -> Result<Table, GenerateError> {
        let widths: Vec<usize> = entries
            .iter()
            .map(|entry| entry.wire.inline_width())
            .collect();
        let (offsets, size, align) = place(&widths);
        if size > usize::from(u16::MAX) {
            return Err(GenerateError {
                message: format!(
                    "`{owner}` needs a {size}-byte FlatBuffers table, and a vtable states a \
                     table's size as a u16"
                ),
            });
        }
        let vtable_slots = u16::try_from(vtable_slots).map_err(|_| GenerateError {
            message: format!("`{owner}` needs more vtable slots than a FlatBuffers table carries"),
        })?;
        let slots = entries
            .into_iter()
            .zip(offsets)
            .map(|(entry, offset)| Slot {
                id: entry.id,
                offset,
                optional: entry.field_type.optional,
                access: Access::Field(entry.name.clone()),
                name: entry.name,
                field_type: entry.field_type,
                hint: entry.hint,
                wire: entry.wire,
            })
            .collect();
        Ok(Table {
            slots,
            size,
            align,
            vtable_slots,
        })
    }

    // -----------------------------------------------------------------
    // A table's view and its three functions
    // -----------------------------------------------------------------

    fn table_items(
        &self,
        owner: &str,
        visibility: i32,
        table: &Table,
    ) -> Result<Vec<TokenStream>, GenerateError> {
        let vis = vis_tokens(visibility);
        let ty = ident(owner);
        let view = view_ident(owner);
        let doc =
            format!(" A zero-copy accessor over FlatBuffers bytes `{owner}`'s `verify` accepted.");

        let mut accessors: Vec<TokenStream> = Vec::new();
        for slot in &table.slots {
            accessors.push(self.accessor(owner, visibility, slot)?);
        }

        let view_item = quote! {
            #[doc = #doc]
            ///
            /// A scalar, a string, a byte sequence and a nested table are
            /// read in place and allocate nothing. A union and a collection
            /// are decoded on access instead: a union's arms and a
            /// collection's elements have no one view type to hand back.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #[allow(deprecated)]
            #vis struct #view<'a> {
                buf: &'a [u8],
                table: usize,
            }

            #[allow(deprecated)]
            impl<'a> #view<'a> {
                /// The verified bytes this view reads.
                #vis fn bytes(&self) -> &'a [u8] {
                    self.buf
                }

                #(#accessors)*
            }
        };

        Ok(vec![
            view_item,
            self.table_encode_fn(owner, &ty, table)?,
            self.table_verify_fn(owner, table)?,
            self.table_decode_fn(owner, &ty, table)?,
        ])
    }

    fn accessor(
        &self,
        owner: &str,
        visibility: i32,
        slot: &Slot,
    ) -> Result<TokenStream, GenerateError> {
        let vis = vis_tokens(visibility);
        let name = ident(&snake_case(&slot.name));
        let id = Literal::u16_suffixed(slot.id);
        let width = Literal::usize_suffixed(slot.wire.inline_width());
        let inner = self.view_expr(&slot.wire, &quote! { __p })?;
        let inner_ty = self.view_type(&slot.wire, &slot.field_type, &slot.hint, visibility);
        // The doc names the projected field, not the typl spelling. The
        // written name reaches no generated site, which is what ADR-0016's
        // pinned transform means and what
        // `a_struct_field_name_is_projected_to_snake_case` pins.
        let doc = format!(" Reads `{owner}`'s `{}` field in place.", name);
        if slot.optional {
            Ok(quote! {
                #[doc = #doc]
                #vis fn #name(&self) -> Option<#inner_ty> {
                    match ::ridl_rt::flatbuffers::field(self.buf, self.table, #id, #width) {
                        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                            ::core::option::Option::Some(#inner)
                        }
                        _ => ::core::option::Option::None,
                    }
                }
            })
        } else {
            Ok(quote! {
                #[doc = #doc]
                #vis fn #name(&self) -> #inner_ty {
                    let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, #id, #width)
                        .unwrap_or(::core::option::Option::None)
                        .unwrap_or(0usize);
                    #inner
                }
            })
        }
    }

    /// What an accessor hands back: the domain value for a scalar and for a
    /// collection, a borrow for a string and for bytes, a nested view for a
    /// table and for a union.
    fn view_type(
        &self,
        wire: &Wire,
        ty: &v2::FieldType,
        hint: &str,
        visibility: i32,
    ) -> TokenStream {
        match wire {
            Wire::Text(_) => quote! { &'a str },
            Wire::Bytes(_) => quote! { &'a [u8] },
            Wire::Table(name) | Wire::Union(name) => {
                let view = view_path(name);
                quote! { #view<'a> }
            }
            _ => {
                let mut discard = Vec::new();
                let bare = v2::FieldType {
                    optional: false,
                    kind: ty.kind.clone(),
                };
                field_type_tokens(&bare, hint, visibility, &mut discard)
            }
        }
    }

    fn view_expr(&self, wire: &Wire, at: &TokenStream) -> Result<TokenStream, GenerateError> {
        Ok(match wire {
            Wire::Text(_) => quote! {
                ::ridl_rt::flatbuffers::string(self.buf, #at).unwrap_or("")
            },
            Wire::Bytes(_) => quote! {
                {
                    let __v = ::ridl_rt::flatbuffers::vector(self.buf, #at, 1usize)
                        .unwrap_or(::ridl_rt::flatbuffers::Vector { len: 0, first: 0 });
                    self.buf.get(__v.first..__v.first + __v.len).unwrap_or(&[])
                }
            },
            Wire::Table(name) | Wire::Union(name) => {
                let view = view_path(name);
                quote! {
                    #view {
                        buf: self.buf,
                        table: ::ridl_rt::flatbuffers::follow(self.buf, #at).unwrap_or(0usize),
                    }
                }
            }
            other => self.decode_expr(other, &quote! { self.buf }, at)?,
        })
    }

    // -----------------------------------------------------------------
    // encode
    // -----------------------------------------------------------------

    fn table_encode_fn(
        &self,
        owner: &str,
        ty: &Ident,
        table: &Table,
    ) -> Result<TokenStream, GenerateError> {
        let name = encode_ident(owner);
        let body = self.table_encode_body(table, &quote! { value })?;
        let doc = format!(" Writes `{owner}` as a FlatBuffers table and returns its position.");
        Ok(quote! {
            #[doc = #doc]
            #[allow(deprecated)]
            pub(crate) fn #name(
                value: &#ty,
                builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
            ) -> ::core::result::Result<
                ::ridl_rt::flatbuffers::Pos,
                ::ridl_rt::payload::EncodeError,
            > {
                #body
            }
        })
    }

    /// The statements that write one table. Each field's value expression
    /// writes whatever that field places out of line as it is evaluated, so
    /// every child reaches the buffer before the table that names it, which
    /// is what a back-to-front builder needs.
    fn table_encode_body(
        &self,
        table: &Table,
        value: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        let size = Literal::usize_suffixed(table.size);
        let align = Literal::usize_suffixed(table.align);
        let slots = Literal::u16_suffixed(table.vtable_slots);

        if table.slots.is_empty() {
            return Ok(quote! { builder.push_table(#size, #align, #slots, &[]) });
        }

        let count = Literal::usize_suffixed(table.slots.len());
        let mut writes: Vec<TokenStream> = Vec::new();
        for slot in &table.slots {
            let id = Literal::u16_suffixed(slot.id);
            let offset = Literal::u16_suffixed(slot.offset);
            if slot.optional {
                let field = self.encode_field(&slot.wire, &Operand::borrowed(quote! { __v }))?;
                let access = slot.operand(value).place;
                writes.push(quote! {
                    if let ::core::option::Option::Some(__v) = &#access {
                        __fields[__n] = ::ridl_rt::flatbuffers::TableField {
                            slot: #id,
                            offset: #offset,
                            value: #field,
                        };
                        __n += 1;
                    }
                });
            } else {
                let field = self.encode_field(&slot.wire, &slot.operand(value))?;
                writes.push(quote! {
                    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
                        slot: #id,
                        offset: #offset,
                        value: #field,
                    };
                    __n += 1;
                });
            }
        }

        Ok(quote! {
            // A placeholder the writes below overwrite. Only `&__fields[..__n]`
            // is read, so a slot never filled is never seen.
            let mut __fields = [::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 4u16,
                value: ::ridl_rt::flatbuffers::Field::Bool(false),
            }; #count];
            let mut __n = 0usize;
            #(#writes)*
            builder.push_table(#size, #align, #slots, &__fields[..__n])
        })
    }

    fn encode_field(&self, wire: &Wire, expr: &Operand) -> Result<TokenStream, GenerateError> {
        Ok(match wire {
            Wire::Scalar(scalar) => {
                let variant = format_ident!("{}", scalar.prim.field_variant());
                let place = &expr.place;
                let raw = scalar.raw(quote! { __s });
                // The value is bound before it is converted, so that a place
                // like `*__v` never has a method call or a cast attached
                // straight to it.
                quote! {
                    {
                        let __s = #place;
                        ::ridl_rt::flatbuffers::Field::#variant(#raw)
                    }
                }
            }
            other => {
                let pos = self.encode_pos(other, expr)?;
                quote! { ::ridl_rt::flatbuffers::Field::Offset(#pos) }
            }
        })
    }

    /// An out-of-line object: the expression writes it and evaluates to its
    /// position.
    fn encode_pos(&self, wire: &Wire, expr: &Operand) -> Result<TokenStream, GenerateError> {
        let reference = &expr.reference;
        Ok(match wire {
            Wire::Scalar(_) => {
                return Err(GenerateError {
                    message: "a FlatBuffers scalar is written inline, not out of line".to_string(),
                });
            }
            Wire::Text(named) => {
                let text = match named {
                    Some(_) => quote! { #reference.get() },
                    None => quote! { #reference.as_str() },
                };
                quote! { builder.push_string(#text)? }
            }
            Wire::Bytes(named) => {
                let bytes = match named {
                    Some(_) => quote! { #reference.get() },
                    None => quote! { #reference.as_slice() },
                };
                quote! { builder.push_vector(#bytes, 1usize)? }
            }
            Wire::Table(name) | Wire::Union(name) => {
                let call = encode_path(name);
                quote! { #call(#reference, builder)? }
            }
            Wire::Vector { element, .. } => {
                let stride = Literal::usize_suffixed(element.inline_width());
                match element.as_ref() {
                    Wire::Scalar(scalar) => {
                        let raw = scalar.raw(quote! { __s });
                        quote! {
                            {
                                let __c = #reference;
                                let mut __bytes: Vec<u8> = Vec::with_capacity(__c.len() * #stride);
                                for __e in __c.iter() {
                                    let __s = *__e;
                                    let __r = #raw;
                                    __bytes.extend_from_slice(&__r.to_le_bytes());
                                }
                                builder.push_vector(&__bytes, #stride)?
                            }
                        }
                    }
                    other => {
                        let pos = self.encode_pos(other, &Operand::borrowed(quote! { __e }))?;
                        quote! {
                            {
                                let __c = #reference;
                                let mut __offsets: Vec<::ridl_rt::flatbuffers::Pos> =
                                    Vec::with_capacity(__c.len());
                                for __e in __c.iter() {
                                    __offsets.push(#pos);
                                }
                                builder.push_offset_vector(&__offsets)?
                            }
                        }
                    }
                }
            }
            Wire::Map { entry, .. } => {
                let body = self.table_encode_body(entry, &quote! { __e })?;
                quote! {
                    {
                        let __c = #reference;
                        let mut __offsets: Vec<::ridl_rt::flatbuffers::Pos> =
                            Vec::with_capacity(__c.len());
                        for __e in __c.iter() {
                            __offsets.push({ #body }?);
                        }
                        builder.push_offset_vector(&__offsets)?
                    }
                }
            }
        })
    }

    // -----------------------------------------------------------------
    // verify
    // -----------------------------------------------------------------

    fn table_verify_fn(&self, owner: &str, table: &Table) -> Result<TokenStream, GenerateError> {
        let name = verify_ident(owner);
        let body = self.table_verify_body(owner, table)?;
        let doc = format!(" Checks the FlatBuffers table at `table` against `{owner}`'s shape.");
        Ok(quote! {
            #[doc = #doc]
            ///
            /// A total walk of the type's own shape: the structure in full,
            /// an enum and an enum-set discriminant, a collection's declared
            /// element count, and every **named** scalar's own declared
            /// range, length and pattern, checked over a borrow (`check`,
            /// beside `new` on the type itself) against its declared range,
            /// length and pattern.
            ///
            /// This does not make every value `decode` builds satisfy every
            /// typl constraint. Three gaps:
            ///
            /// - a `step` constraint is checked nowhere — not by `new`, by
            ///   `check`, or here (driftsys/ridl#469);
            /// - the pattern check is behind the `validate-pattern` feature,
            ///   so a value violating a `match` pattern passes when that
            ///   feature is off;
            /// - an anonymous inline constraint (a field's own `[..]` or
            ///   `match` written at the field, not through a named scalar)
            ///   is not checked here at all (driftsys/ridl#469).
            #[allow(deprecated)]
            pub(crate) fn #name(
                buf: &[u8],
                table: usize,
            ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                #body
                ::core::result::Result::Ok(())
            }
        })
    }

    fn table_verify_body(&self, owner: &str, table: &Table) -> Result<TokenStream, GenerateError> {
        let mut checks: Vec<TokenStream> = Vec::new();
        for slot in &table.slots {
            let id = Literal::u16_suffixed(slot.id);
            let width = Literal::usize_suffixed(slot.wire.inline_width());
            let present = self.verify_at(owner, &slot.wire, &quote! { __p })?;
            let absent = if slot.optional {
                quote! { ::core::option::Option::None => {} }
            } else {
                quote! {
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::MissingRequired,
                            ),
                        );
                    }
                }
            };
            checks.push(quote! {
                match ::ridl_rt::flatbuffers::field(buf, table, #id, #width)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?
                {
                    ::core::option::Option::Some(__p) => { #present }
                    #absent
                }
            });
        }
        Ok(quote! { #(#checks)* })
    }

    /// The structural walk of one value, plus every typl constraint this
    /// codec checks: an enum and an enum-set discriminant, a collection's
    /// declared element count, and — over a borrow, through the `check`
    /// associated function beside `new` on the type itself — a named
    /// scalar's own range, length and pattern.
    fn verify_at(
        &self,
        owner: &str,
        wire: &Wire,
        at: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        let buf = quote! { buf };
        Ok(match wire {
            Wire::Scalar(scalar) => {
                let read = scalar.read(&buf, at);
                match &scalar.repr {
                    Repr::Enum { name, .. } | Repr::EnumSet { name } => {
                        let ty = type_path(name);
                        let widened = scalar.widen(quote! { __raw });
                        quote! {
                            let __raw = #read
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                            <#ty as ::core::convert::TryFrom<i64>>::try_from(#widened)
                                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                        }
                    }
                    Repr::Named(named) => {
                        let widened = scalar.widen(quote! { __raw });
                        match named_scalar_check(named, quote! { &(#widened) }) {
                            Some(check) => quote! {
                                let __raw = #read
                                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                                #check
                            },
                            None => quote! {
                                #read.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                            },
                        }
                    }
                    _ => quote! {
                        #read.map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    },
                }
            }
            Wire::Text(named) => {
                let check = named
                    .as_ref()
                    .and_then(|named| named_scalar_check(named, quote! { __s }));
                match check {
                    Some(check) => quote! {
                        let __s = ::ridl_rt::flatbuffers::string(buf, #at)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #check
                    },
                    None => quote! {
                        ::ridl_rt::flatbuffers::string(buf, #at)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    },
                }
            }
            Wire::Bytes(named) => {
                let check = named.as_ref().and_then(|named| {
                    named_scalar_check(named, quote! { &buf[__v.first..__v.first + __v.len] })
                });
                match check {
                    Some(check) => quote! {
                        let __v = ::ridl_rt::flatbuffers::vector(buf, #at, 1usize)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #check
                    },
                    None => quote! {
                        ::ridl_rt::flatbuffers::vector(buf, #at, 1usize)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    },
                }
            }
            Wire::Table(name) | Wire::Union(name) => {
                let call = verify_path(name);
                quote! {
                    let __t = ::ridl_rt::flatbuffers::follow(buf, #at)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    #call(buf, __t)?;
                }
            }
            Wire::Vector { element, min, max } => {
                let stride = Literal::usize_suffixed(element.inline_width());
                let inner = self.verify_at(owner, element, &quote! { __at })?;
                let count = count_check(owner, *min, *max);
                quote! {
                    let __v = ::ridl_rt::flatbuffers::vector(buf, #at, #stride)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    #count
                    for __i in 0..__v.len {
                        let __at = __v.element(__i, #stride);
                        #inner
                    }
                }
            }
            Wire::Map { entry, min, max } => {
                let inner = self.table_verify_body(owner, entry)?;
                let count = count_check(owner, *min, *max);
                quote! {
                    let __v = ::ridl_rt::flatbuffers::vector(buf, #at, 4usize)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    #count
                    for __i in 0..__v.len {
                        let table = ::ridl_rt::flatbuffers::follow(buf, __v.element(__i, 4usize))
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #inner
                    }
                }
            }
        })
    }

    // -----------------------------------------------------------------
    // decode
    // -----------------------------------------------------------------

    fn table_decode_fn(
        &self,
        owner: &str,
        ty: &Ident,
        table: &Table,
    ) -> Result<TokenStream, GenerateError> {
        let name = decode_ident(owner);
        let mut fields: Vec<TokenStream> = Vec::new();
        for slot in &table.slots {
            let field = ident(&snake_case(&slot.name));
            let value = self.slot_decode_expr(slot, &quote! { buf }, &quote! { table })?;
            fields.push(quote! { #field: #value });
        }
        let doc = format!(" Builds `{owner}` from the FlatBuffers table at `table`.");
        Ok(quote! {
            #[doc = #doc]
            ///
            /// It cannot fail. A read that could is discharged with the
            /// neutral value of its own type — zero, the empty string or
            /// collection, the first declared enum variant — and `verify` is
            /// what makes those branches unreachable. A named scalar is
            /// built with its unchecked constructor (`new_unchecked`) over a
            /// value `verify` has already range-checked (`check`), so this
            /// never re-checks and never fails.
            #[allow(deprecated)]
            pub(crate) fn #name(buf: &[u8], table: usize) -> #ty {
                #ty { #(#fields),* }
            }
        })
    }

    fn slot_decode_expr(
        &self,
        slot: &Slot,
        buf: &TokenStream,
        table: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        let id = Literal::u16_suffixed(slot.id);
        let width = Literal::usize_suffixed(slot.wire.inline_width());
        let inner = self.decode_expr(&slot.wire, buf, &quote! { __p })?;
        Ok(if slot.optional {
            quote! {
                match ::ridl_rt::flatbuffers::field(#buf, #table, #id, #width) {
                    ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                        ::core::option::Option::Some(#inner)
                    }
                    _ => ::core::option::Option::None,
                }
            }
        } else {
            quote! {
                {
                    let __p = ::ridl_rt::flatbuffers::field(#buf, #table, #id, #width)
                        .unwrap_or(::core::option::Option::None)
                        .unwrap_or(0usize);
                    #inner
                }
            }
        })
    }

    fn decode_expr(
        &self,
        wire: &Wire,
        buf: &TokenStream,
        at: &TokenStream,
    ) -> Result<TokenStream, GenerateError> {
        Ok(match wire {
            Wire::Scalar(scalar) => scalar.decode(buf, at),
            Wire::Text(named) => {
                let text = quote! {
                    String::from(::ridl_rt::flatbuffers::string(#buf, #at).unwrap_or(""))
                };
                match named {
                    Some(named) => {
                        let ty = type_path(&named.name);
                        let ctor = format_ident!("{}", named.ctor);
                        quote! { #ty::#ctor(#text) }
                    }
                    None => text,
                }
            }
            Wire::Bytes(named) => {
                let bytes = quote! {
                    {
                        let __v = ::ridl_rt::flatbuffers::vector(#buf, #at, 1usize)
                            .unwrap_or(::ridl_rt::flatbuffers::Vector { len: 0, first: 0 });
                        #buf.get(__v.first..__v.first + __v.len).unwrap_or(&[]).to_vec()
                    }
                };
                match named {
                    Some(named) => {
                        let ty = type_path(&named.name);
                        let ctor = format_ident!("{}", named.ctor);
                        quote! { #ty::#ctor(#bytes) }
                    }
                    None => bytes,
                }
            }
            Wire::Table(name) | Wire::Union(name) => {
                let call = decode_path(name);
                quote! {
                    #call(#buf, ::ridl_rt::flatbuffers::follow(#buf, #at).unwrap_or(0usize))
                }
            }
            Wire::Vector { element, min, max } => {
                let stride = Literal::usize_suffixed(element.inline_width());
                let inner = self.decode_expr(element, buf, &quote! { __at })?;
                if min == max {
                    quote! {
                        {
                            let __v = ::ridl_rt::flatbuffers::vector(#buf, #at, #stride)
                                .unwrap_or(::ridl_rt::flatbuffers::Vector { len: 0, first: 0 });
                            ::core::array::from_fn(|__i| {
                                let __at = __v.element(__i, #stride);
                                #inner
                            })
                        }
                    }
                } else {
                    quote! {
                        {
                            let __v = ::ridl_rt::flatbuffers::vector(#buf, #at, #stride)
                                .unwrap_or(::ridl_rt::flatbuffers::Vector { len: 0, first: 0 });
                            let mut __out = Vec::with_capacity(__v.len);
                            for __i in 0..__v.len {
                                let __at = __v.element(__i, #stride);
                                __out.push(#inner);
                            }
                            __out
                        }
                    }
                }
            }
            Wire::Map { entry, .. } => {
                let key = self.slot_decode_expr(&entry.slots[0], buf, &quote! { __t })?;
                let value = self.slot_decode_expr(&entry.slots[1], buf, &quote! { __t })?;
                quote! {
                    {
                        let __v = ::ridl_rt::flatbuffers::vector(#buf, #at, 4usize)
                            .unwrap_or(::ridl_rt::flatbuffers::Vector { len: 0, first: 0 });
                        let mut __out = Vec::with_capacity(__v.len);
                        for __i in 0..__v.len {
                            let __t = ::ridl_rt::flatbuffers::follow(
                                #buf,
                                __v.element(__i, 4usize),
                            )
                            .unwrap_or(0usize);
                            __out.push((#key, #value));
                        }
                        __out
                    }
                }
            }
        })
    }

    // -----------------------------------------------------------------
    // Unions (ADR-0019 decisions 1 and 2)
    // -----------------------------------------------------------------

    /// A union's wrapper table: the discriminant at the implicit id 0 and the
    /// value at id 1.
    fn union_layout(&self) -> (u16, u16, usize, usize) {
        let (offsets, size, align) = place(&[1, 4]);
        (offsets[0], offsets[1], size, align)
    }

    fn union_items(
        &self,
        decl: &v2::Decl,
        def: &v2::UnionDef,
    ) -> Result<Vec<TokenStream>, GenerateError> {
        let owner = decl.name.as_str();
        if def.arms.is_empty() {
            return Err(GenerateError {
                message: format!("`{owner}` declares no arm, so it encodes no value"),
            });
        }
        let vis = vis_tokens(decl.visibility);
        let ty = ident(owner);
        let view = view_ident(owner);
        let (disc_offset, value_offset, size, align) = self.union_layout();
        let disc_offset = Literal::u16_suffixed(disc_offset);
        let value_offset = Literal::u16_suffixed(value_offset);
        let size = Literal::usize_suffixed(size);
        let align = Literal::usize_suffixed(align);
        let wrapper_slots = Literal::u16_suffixed(
            u16::try_from(fb_projection::UNION_WRAPPER_VALUE_ID + 1).unwrap(),
        );

        let mut arms: Vec<UnionArm> = Vec::new();
        for arm in &def.arms {
            arms.push(self.union_arm(owner, arm)?);
        }

        let encode_name = encode_ident(owner);
        let verify_name = verify_ident(owner);
        let decode_name = decode_ident(owner);

        let mut encode_arms: Vec<TokenStream> = Vec::new();
        let mut verify_arms: Vec<TokenStream> = Vec::new();
        let mut decode_arms: Vec<TokenStream> = Vec::new();
        for arm in &arms {
            let variant = ident(&camel_case(&arm.name));
            let tag = Literal::u8_suffixed(arm.tag);
            let write = &arm.encode;
            encode_arms.push(quote! { #ty::#variant(__a) => (#tag, #write) });
            let check = &arm.verify;
            verify_arms.push(quote! { #tag => { #check } });
            let build = &arm.decode;
            decode_arms.push(quote! { #tag => #ty::#variant(#build) });
        }
        let fallback = {
            let first = &arms[0];
            let variant = ident(&camel_case(&first.name));
            let build = &first.decode;
            quote! { _ => #ty::#variant(#build) }
        };

        let union_malformed = quote! {
            ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Structure(
                ::ridl_rt::payload::Malformed::Union,
            ))
        };

        let doc =
            format!(" A zero-copy accessor over FlatBuffers bytes `{owner}`'s `verify` accepted.");
        let value_doc = format!(
            " The value the buffer carries. A union's arms have no one view type, so this \
             decodes `{owner}` rather than borrowing it."
        );

        Ok(vec![
            quote! {
                #[doc = #doc]
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                #[allow(deprecated)]
                #vis struct #view<'a> {
                    buf: &'a [u8],
                    table: usize,
                }

                #[allow(deprecated)]
                impl<'a> #view<'a> {
                    /// The verified bytes this view reads.
                    #vis fn bytes(&self) -> &'a [u8] {
                        self.buf
                    }

                    #[doc = #value_doc]
                    #vis fn value(&self) -> #ty {
                        #decode_name(self.buf, self.table)
                    }
                }
            },
            quote! {
                #[allow(deprecated)]
                pub(crate) fn #encode_name(
                    value: &#ty,
                    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
                ) -> ::core::result::Result<
                    ::ridl_rt::flatbuffers::Pos,
                    ::ridl_rt::payload::EncodeError,
                > {
                    let (__d, __v) = match value {
                        #(#encode_arms),*
                    };
                    let __fields = [
                        ::ridl_rt::flatbuffers::TableField {
                            slot: 0u16,
                            offset: #disc_offset,
                            value: ::ridl_rt::flatbuffers::Field::U8(__d),
                        },
                        ::ridl_rt::flatbuffers::TableField {
                            slot: 1u16,
                            offset: #value_offset,
                            value: ::ridl_rt::flatbuffers::Field::Offset(__v),
                        },
                    ];
                    builder.push_table(#size, #align, #wrapper_slots, &__fields)
                }
            },
            quote! {
                #[allow(deprecated)]
                pub(crate) fn #verify_name(
                    buf: &[u8],
                    table: usize,
                ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                    let __dp = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    let __vp = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    let (__d, __value) = match (__dp, __vp) {
                        (
                            ::core::option::Option::Some(__d),
                            ::core::option::Option::Some(__value),
                        ) => (
                            ::ridl_rt::flatbuffers::read_u8(buf, __d)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                            __value,
                        ),
                        // A discriminant with no value, or a value with no
                        // discriminant (design note D-5).
                        _ => return #union_malformed,
                    };
                    match __d {
                        #(#verify_arms)*
                        _ => return #union_malformed,
                    }
                    ::core::result::Result::Ok(())
                }
            },
            quote! {
                #[allow(deprecated)]
                pub(crate) fn #decode_name(buf: &[u8], table: usize) -> #ty {
                    let __d = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
                        .unwrap_or(::core::option::Option::None)
                        .map(|__p| ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8))
                        .unwrap_or(0u8);
                    let __value = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
                        .unwrap_or(::core::option::Option::None)
                        .unwrap_or(0usize);
                    match __d {
                        #(#decode_arms,)*
                        #fallback
                    }
                }
            },
        ])
    }

    /// The three bodies of one box table — the encode, the verify and the
    /// decode — for a value of `wire` held in `table`'s one slot.
    ///
    /// **One box, one implementation.** ADR-0019 decision 8 adopts decision 2's
    /// box idiom rather than minting a second shape, and this is where that is
    /// true of the code rather than only of the records: `union_arm`'s
    /// non-table branch and `root_box_items` both call it, and the slot, the
    /// vtable width, the inline placement and the `MissingRequired` rule are
    /// written once. `table` is the layout the projection hands over —
    /// `union_arm_box_table` for an arm, `root_box_table` for a root — so the
    /// table this writes and the table `max_size` charges cannot be two
    /// different tables.
    ///
    /// `at` is the expression naming the box's own table offset. The two call
    /// sites differ in one thing and nothing else: a root table is already
    /// followed by the time `Payload::verify` reaches it, while an arm's sits
    /// behind the union's value offset and is followed first — which is what
    /// the caller passes in.
    fn box_bodies(
        &self,
        owner: &str,
        wire: &Wire,
        table: &fb_projection::TableLayout,
        value: &Operand,
        at: &TokenStream,
    ) -> Result<BoxBodies, GenerateError> {
        let [slot] = table.slots.as_slice() else {
            return Err(GenerateError {
                message: format!(
                    "the FlatBuffers projection describes a box table with {} slots, and a box \
                     holds exactly one value (ADR-0019 decisions 2 and 8)",
                    table.slots.len()
                ),
            });
        };
        let id = u16::try_from(slot.id).map_err(|_| GenerateError {
            message: format!(
                "the FlatBuffers projection puts a box table's value at id {}, which a vtable \
                 cannot carry",
                slot.id
            ),
        })?;
        let id_lit = Literal::u16_suffixed(id);
        let slots = Literal::u16_suffixed(u16::try_from(table.vtable_slots()).map_err(|_| {
            GenerateError {
                message: "a box table's vtable does not fit a u16".to_string(),
            }
        })?);

        let width = wire.inline_width();
        let (offsets, size, align) = place(&[width]);
        let offset = Literal::u16_suffixed(offsets[0]);
        let size = Literal::usize_suffixed(size);
        let align = Literal::usize_suffixed(align);
        let width_lit = Literal::usize_suffixed(width);

        let field = self.encode_field(wire, value)?;
        let inner_verify = self.verify_at(owner, wire, &quote! { __p })?;
        let inner_decode = self.decode_expr(wire, &quote! { buf }, &quote! { __p })?;

        Ok(BoxBodies {
            encode: quote! {
                {
                    let __box = [::ridl_rt::flatbuffers::TableField {
                        slot: #id_lit,
                        offset: #offset,
                        value: #field,
                    }];
                    builder.push_table(#size, #align, #slots, &__box)?
                }
            },
            // The box's one field is not optional, so a buffer with no slot
            // for it carries no value at all.
            verify: quote! {
                match ::ridl_rt::flatbuffers::field(buf, #at, #id_lit, #width_lit)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?
                {
                    ::core::option::Option::Some(__p) => { #inner_verify }
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::MissingRequired,
                            ),
                        );
                    }
                }
            },
            decode: quote! {
                {
                    let __p = ::ridl_rt::flatbuffers::field(buf, #at, #id_lit, #width_lit)
                        .unwrap_or(::core::option::Option::None)
                        .unwrap_or(0usize);
                    #inner_decode
                }
            },
        })
    }

    /// One arm's three bodies. A struct or a union arm is the referenced
    /// table itself; anything else is isolated in a box table with one value
    /// field (ADR-0019 decision 2).
    fn union_arm(&self, owner: &str, arm: &v2::UnionArm) -> Result<UnionArm, GenerateError> {
        let tag = u8::try_from(arm.ordinal).map_err(|_| GenerateError {
            message: format!(
                "`{owner}.{}` has ordinal {}, and a FlatBuffers union discriminant is a ubyte",
                arm.name, arm.ordinal
            ),
        })?;
        let wire = self.wire(
            &v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(arm.type_ref.clone())),
            },
            "",
        )?;
        match &wire {
            Wire::Table(_) | Wire::Union(_) => Ok(UnionArm {
                name: arm.name.clone(),
                tag,
                encode: self.encode_pos(&wire, &Operand::borrowed(quote! { __a }))?,
                verify: self.verify_at(owner, &wire, &quote! { __value })?,
                decode: self.decode_expr(&wire, &quote! { buf }, &quote! { __value })?,
            }),
            _ => {
                // The arm's box sits behind the union's value offset, so it is
                // followed before its one slot is read; a root's is already
                // followed (`root_box_items`). That is the only difference
                // between the two, and `box_bodies` writes everything else.
                let bodies = self.box_bodies(
                    owner,
                    &wire,
                    &fb_projection::union_arm_box_table(),
                    &Operand::borrowed(quote! { __a }),
                    &quote! { __t },
                )?;
                let verify = &bodies.verify;
                let decode = &bodies.decode;
                Ok(UnionArm {
                    name: arm.name.clone(),
                    tag,
                    encode: bodies.encode.clone(),
                    verify: quote! {
                        let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        #verify
                    },
                    decode: quote! {
                        {
                            let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                                .unwrap_or(0usize);
                            #decode
                        }
                    },
                })
            }
        }
    }

    // -----------------------------------------------------------------
    // The box root (ADR-0019 decision 8)
    // -----------------------------------------------------------------

    /// The view struct and the three functions for a declaration rooted in a
    /// box table: a named scalar, an enum or an enum set.
    ///
    /// A FlatBuffers root is a table, and each of these three kinds inlines to
    /// a bare scalar at a field position, so before ADR-0019 decision 8 none of
    /// them had a root and none of them carried a codec — which is what left
    /// the generated face on its `ReprC` placeholder (driftsys/ridl#470, closed
    /// by stage K9b, which moved the face onto `Wire`). The
    /// box is `table <Name>Box { value: <resolved type> (id: 0); }`, the same
    /// table decision 2 gives a non-table union arm, so the three bodies are
    /// the same three [`Codec::union_arm`] writes for that arm — read at the
    /// root rather than behind a union's value offset, which is the one
    /// difference: the root table is already followed, so nothing here
    /// dereferences an offset first.
    ///
    /// The view hands back the value rather than a borrow: a box holds exactly
    /// one value and decoding it costs a read, so there is nothing a nested
    /// view would save.
    fn root_box_items(&self, decl: &v2::Decl) -> Result<Vec<TokenStream>, GenerateError> {
        let owner = decl.name.as_str();
        let wire = self.wire(
            &v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(owner.to_string())),
            },
            "",
        )?;
        let vis = vis_tokens(decl.visibility);
        let ty = ident(owner);
        let view = view_ident(owner);
        let encode_name = encode_ident(owner);
        let verify_name = verify_ident(owner);
        let decode_name = decode_ident(owner);

        // The root table is already followed by the time `Payload::verify`
        // and `Payload::decode` reach it, so the box's one slot is read from
        // `table` directly. Everything else is `box_bodies`, shared with
        // decision 2's arm box.
        let bodies = self.box_bodies(
            owner,
            &wire,
            &fb_projection::root_box_table(),
            &Operand::borrowed(quote! { value }),
            &quote! { table },
        )?;
        let encode_body = &bodies.encode;
        let verify_body = &bodies.verify;
        let decode_body = &bodies.decode;

        let doc = format!(" An accessor over FlatBuffers bytes `{owner}`'s `verify` accepted.");
        // A box holds one value, and `value()` hands it over rather than
        // borrowing it: for a scalar and an enum that is a read, and nothing
        // a nested view would save; for a string or a bytes backing it is an
        // allocation, where a struct field of the same type is borrowed in
        // place. The doc says which, since a caller in a hot path needs to
        // know.
        let value_doc = if matches!(wire, Wire::Text(_) | Wire::Bytes(_)) {
            format!(
                " The value the box carries. `{owner}` owns its bytes, so this allocates — \
                 unlike a struct field of the same type, which a view borrows in place."
            )
        } else {
            format!(
                " The value the box carries. `{owner}` is one value, so this decodes it rather \
                 than borrowing it, which costs one read."
            )
        };
        let encode_doc = format!(
            " Writes `{owner}` as its box table and returns its position (ADR-0019 decision 8)."
        );

        Ok(vec![
            quote! {
                #[doc = #doc]
                ///
                /// The buffer's root is the box table ADR-0019 decision 8
                /// gives this declaration: one required `value` field. A
                /// buffer carrying no slot for it is `MissingRequired`.
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                #[allow(deprecated)]
                #vis struct #view<'a> {
                    buf: &'a [u8],
                    table: usize,
                }

                #[allow(deprecated)]
                impl<'a> #view<'a> {
                    /// The verified bytes this view reads.
                    #vis fn bytes(&self) -> &'a [u8] {
                        self.buf
                    }

                    #[doc = #value_doc]
                    #vis fn value(&self) -> #ty {
                        #decode_name(self.buf, self.table)
                    }
                }
            },
            quote! {
                #[doc = #encode_doc]
                #[allow(deprecated)]
                pub(crate) fn #encode_name(
                    value: &#ty,
                    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
                ) -> ::core::result::Result<
                    ::ridl_rt::flatbuffers::Pos,
                    ::ridl_rt::payload::EncodeError,
                > {
                    ::core::result::Result::Ok(#encode_body)
                }
            },
            quote! {
                #[allow(deprecated)]
                pub(crate) fn #verify_name(
                    buf: &[u8],
                    table: usize,
                ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
                    #verify_body
                    ::core::result::Result::Ok(())
                }
            },
            quote! {
                #[allow(deprecated)]
                pub(crate) fn #decode_name(buf: &[u8], table: usize) -> #ty {
                    #decode_body
                }
            },
        ])
    }

    // -----------------------------------------------------------------
    // The Payload implementation
    // -----------------------------------------------------------------

    fn payload_impl(&self, decl: &v2::Decl) -> Result<TokenStream, GenerateError> {
        let owner = decl.name.as_str();
        let bound =
            fb_projection::max_size(self.packages(), decl).ok_or_else(|| GenerateError {
                message: format!("`{owner}` has no finite FlatBuffers bound"),
            })?;
        let ty = ident(owner);
        let view = view_ident(owner);
        let encode = encode_ident(owner);
        let verify = verify_ident(owner);
        let decode = decode_ident(owner);
        let max = Literal::usize_suffixed(bound as usize);
        let align = Literal::usize_suffixed(BUFFER_ALIGN);
        let max_doc = format!(
            " The largest FlatBuffers buffer any legal `{owner}` encodes to: {bound} bytes."
        );

        Ok(quote! {
            #[allow(deprecated)]
            impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for #ty {
                #[doc = #max_doc]
                ///
                /// `ridl_ir::projection::flatbuffers::max_size` computed it,
                /// which is the one implementation of the bound (design note
                /// D-6): each table is charged its `soffset`, its inline
                /// fields, its vtable and one alignment event per slot; a
                /// string four bytes per declared character plus a
                /// terminator; a collection its declared maximum. It is a
                /// literal rather than an expression over the field types
                /// because that slack is not expressible in Rust's type
                /// system.
                const MAX_SIZE: usize = #max;

                type View<'a> = #view<'a>;

                fn encode<'o>(
                    &self,
                    out: &'o mut [u8],
                ) -> ::core::result::Result<
                    ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
                    ::ridl_rt::payload::EncodeError,
                > {
                    let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
                    let __root = #encode(self, &mut builder)?;
                    let bytes = builder.finish(__root, #align)?;
                    // The root offset of a buffer this encoder just wrote.
                    let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
                    ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
                        bytes,
                        view: #view { buf: bytes, table },
                    })
                }

                fn verify(
                    buf: &[u8],
                ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
                    if buf.len()
                        > <Self as ::ridl_rt::payload::Payload<
                            ::ridl_rt::encoding::FlatBuffers,
                        >>::MAX_SIZE
                    {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::TooLarge,
                            ),
                        );
                    }
                    let table = ::ridl_rt::flatbuffers::root(buf)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    #verify(buf, table)?;
                    ::core::result::Result::Ok(#view { buf, table })
                }

                fn decode(
                    r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
                ) -> Self {
                    let __view = r.view();
                    #decode(__view.buf, __view.table)
                }
            }
        })
    }
}

/// One box table's three generated bodies ([`Codec::box_bodies`]): the
/// expression that writes it, the statements that verify it, and the
/// expression that decodes it.
struct BoxBodies {
    encode: TokenStream,
    verify: TokenStream,
    decode: TokenStream,
}

/// One union arm's three generated bodies.
struct UnionArm {
    name: String,
    tag: u8,
    encode: TokenStream,
    verify: TokenStream,
    decode: TokenStream,
}

/// The element-count check for a collection, as the statement `verify` runs
/// after it has read the vector header.
///
/// A bound rustc can fold is not emitted: `__v.len < 0` is never true and
/// draws `unused_comparisons` in a consumer's build, which is the same rule
/// `crate::constraint_checks` follows for a length minimum of zero. A fixed
/// array is one equality instead of two comparisons, and it is what keeps
/// `decode`'s `[T; N]` from being built out of a vector of another length.
fn count_check(owner: &str, min: u64, max: u64) -> TokenStream {
    let violation = quote! {
        return ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(
            ::ridl_rt::payload::Violation {
                type_name: #owner,
                rule: ::ridl_rt::payload::Rule::Length,
            },
        ));
    };
    let high = Literal::usize_suffixed(max as usize);
    if min == max {
        return quote! {
            if __v.len != #high {
                #violation
            }
        };
    }
    if min == 0 {
        return quote! {
            if __v.len > #high {
                #violation
            }
        };
    }
    let low = Literal::usize_suffixed(min as usize);
    quote! {
        if __v.len < #low || __v.len > #high {
            #violation
        }
    }
}

fn scalar_repr(named: Option<NamedScalar>, backing: ScalarBacking) -> Repr {
    match named {
        Some(named) => Repr::Named(named),
        None => match backing {
            ScalarBacking::Boolean => Repr::Bool,
            ScalarBacking::Integer => Repr::Int,
            _ => Repr::Float,
        },
    }
}

/// The FlatBuffers scalar one typl integer width writes as
/// (typl Appendix D).
fn int_prim(width: i32) -> Option<Prim> {
    match v2::IntWidth::try_from(width).ok()? {
        v2::IntWidth::U8 => Some(Prim::U8),
        v2::IntWidth::I8 => Some(Prim::I8),
        v2::IntWidth::U16 => Some(Prim::U16),
        v2::IntWidth::I16 => Some(Prim::I16),
        v2::IntWidth::U32 => Some(Prim::U32),
        v2::IntWidth::I32 => Some(Prim::I32),
        v2::IntWidth::U64 => Some(Prim::U64),
        v2::IntWidth::I64 => Some(Prim::I64),
        v2::IntWidth::Unspecified => None,
    }
}
