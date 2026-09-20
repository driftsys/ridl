//! Derive eligibility for the typl surface (design decision 7).
//!
//! `Debug`, `Clone`, and `PartialEq` are sound on every generated type: every
//! backing has them and every generated type receives them, so the recursion
//! cannot fail on them. The rest are conditional and need the transitive
//! closure.
//!
//! Each conditional rule below is stated in full. A condition on the leaves
//! alone is necessary and **not** sufficient: four positions refuse every
//! conditional derive whatever their leaves are, and they are listed after the
//! rules.
//!
//! - `Copy` — every leaf is `f64`, `i64`, or `bool`; no position in the
//!   closure is an array or a map, because an array emits `Vec<T>` or
//!   `[T; N]` and a map emits `Vec<(K, V)>`; and no position is a refusing
//!   position.
//! - `Eq`, `Hash` — no `f64` appears anywhere in the closure, including
//!   unit-backed types, since a unit backing implies float (typl §5.1); and no
//!   position is a refusing position. An array or a map does not disqualify
//!   these: a `Vec` is `Eq` and `Hash` when its element is.
//! - `PartialOrd`/`Ord` — named scalars over a numeric backing only. Ordering
//!   a struct's fields lexicographically, or a union's arms by declaration
//!   order, is not a property typl states, so deriving it would invent
//!   contract semantics. `Ord` requires `Eq`, so a float-backed scalar takes
//!   `PartialOrd` alone.
//!
//! The four refusing positions, each of which makes `Copy` and the equality
//! pair both false for the whole closure that contains it:
//!
//! 1. A named reference that does not resolve to a declaration of this
//!    package — a dotted cross-package name, an unknown name, or a name that
//!    resolves to a constant rather than a type. See the paragraph below.
//! 2. A named reference that closes a cycle. A cyclic IR is TYPL-206 upstream,
//!    but this pass does not trust that gate; on a repeat visit it refuses
//!    rather than recursing forever.
//! 3. A `Stream`, which is an interaction-position type (ridl §12.3) and emits
//!    `()` in a field position it should never reach.
//! 4. An `Unspecified` field primitive, which also emits `()`. An
//!    `Unspecified` *backing* is not this position: [`backing_scalar`] maps an
//!    unspecified primitive backing to [`ScalarBacking::Bytes`], so such a
//!    type emits `Vec<u8>` and is `Eq` and `Hash` without being `Copy`. An
//!    *absent* backing is different again, and maps to `Float` — see
//!    [`scalar_eligibility`], which this note previously contradicted.
//!
//! `Default` is never derived (design decision 8). [`defaults`] builds it from
//! the typl init value, which may be a declared `= 0.5`; `#[derive(Default)]`
//! would give the backing's `0.0` and silently contradict the contract. There
//! is no branch here that can emit it, for any declaration kind.
//!
//! **Cross-package references are handled conservatively.** [`defaults`] can be
//! optimistic — it emits `path::default()` and lets rustc verify. A derive
//! cannot: `#[derive(Copy)]` on a struct whose cross-package field is not
//! `Copy` is a hard error in the consumer's build, with no line of the
//! consumer's own source to point at. So an unresolvable reference anywhere in
//! the closure disables every conditional derive. The three unconditional ones
//! stay, because every generated type has them and a cross-package reference
//! in checked IR names a generated type.
//!
//! The recursion mirrors [`defaults`]: leaf recursion with a cycle guard. It
//! reads no `derivable` flag — `InitValue.derivable` governs `Default`
//! derivation in [`defaults`] and plays no part in derive eligibility, so
//! there is nothing here to trust or re-check.
//!
//! [`defaults`]: crate::defaults

use crate::{Ctx, ScalarBacking, backing_scalar};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::v2;
use std::collections::HashSet;

/// What the transitive closure of one type permits. The two conditions are
/// independent: a float-backed scalar is `Copy` and not `Eq`, a `String`-backed
/// one is `Eq` and not `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Eligibility {
    copy: bool,
    eq: bool,
}

impl Eligibility {
    const NONE: Self = Self {
        copy: false,
        eq: false,
    };
    const ALL: Self = Self {
        copy: true,
        eq: true,
    };
    const COPY_ONLY: Self = Self {
        copy: true,
        eq: false,
    };
    const EQ_ONLY: Self = Self {
        copy: false,
        eq: true,
    };

    /// The greatest lower bound: a composite is eligible only where every part
    /// of it is.
    fn meet(self, other: Self) -> Self {
        Self {
            copy: self.copy && other.copy,
            eq: self.eq && other.eq,
        }
    }
}

/// The `#[derive(...)]` attribute for one declaration.
pub(crate) fn derive_attr(ctx: &Ctx, decl: &v2::Decl) -> TokenStream {
    let mut seen = HashSet::new();
    let eligibility = decl_eligibility(ctx, decl, &mut seen);
    attr(eligibility, numeric_named_scalar(decl))
}

/// The `#[derive(...)]` attribute for one induced tuple struct (typl §11).
///
/// A tuple struct needs one for the same reason every declaration does, and
/// more sharply: the struct that holds it derives `Debug`, `Clone` and
/// `PartialEq` unconditionally, and none of those three compiles unless the
/// generated tuple struct carries them too. A tuple is anonymous, so it is
/// never a named scalar and takes no ordering.
pub(crate) fn tuple_derive_attr(ctx: &Ctx, tuple: &v2::TupleType) -> TokenStream {
    let mut seen = HashSet::new();
    attr(tuple_eligibility(ctx, tuple, &mut seen), false)
}

/// Assembles the attribute. The order is the one Rust is conventionally
/// written in: `Debug`, then `Clone` with `Copy` beside it, then `PartialEq`
/// with the equality pair after it, then the ordering pair. The three
/// unconditional traits are therefore not one block — `Copy` sits between
/// `Clone` and `PartialEq`, which is what every snapshot shows as
/// `Debug, Clone, Copy, PartialEq, …`.
fn attr(eligibility: Eligibility, ordered: bool) -> TokenStream {
    let mut traits = vec![quote! { Debug }, quote! { Clone }];
    if eligibility.copy {
        traits.push(quote! { Copy });
    }
    traits.push(quote! { PartialEq });
    if eligibility.eq {
        traits.push(quote! { Eq });
        traits.push(quote! { Hash });
    }
    if ordered {
        traits.push(quote! { PartialOrd });
        if eligibility.eq {
            traits.push(quote! { Ord });
        }
    }
    quote! { #[derive(#(#traits),*)] }
}

/// True for a `type` declaration over an integer or float backing. A unit
/// backing implies float (typl §5.1), so it qualifies. Nothing else does:
/// ordering is a named-scalar property.
fn numeric_named_scalar(decl: &v2::Decl) -> bool {
    let Some(v2::decl::Kind::TypeDef(td)) = &decl.kind else {
        return false;
    };
    matches!(
        backing_scalar(td),
        ScalarBacking::Float | ScalarBacking::Integer
    )
}

fn decl_eligibility(ctx: &Ctx, decl: &v2::Decl, seen: &mut HashSet<String>) -> Eligibility {
    if !seen.insert(decl.name.clone()) {
        // A cyclic IR is TYPL-206 upstream, but this pass must not trust that
        // gate: on a cycle it refuses the conditional derives rather than
        // recurse forever (the C1b guard `defaults.rs` applies to Default).
        return Eligibility::NONE;
    }
    let result = match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => scalar_eligibility(td),
        // A fieldless `#[repr(i64)]` enum and a `#[repr(transparent)]` newtype
        // over `i64` are both an integer at the leaf.
        Some(v2::decl::Kind::EnumDef(_)) | Some(v2::decl::Kind::EnumSetDef(_)) => Eligibility::ALL,
        Some(v2::decl::Kind::StructDef(sd)) => sd
            .members
            .iter()
            .filter_map(|member| match &member.member {
                Some(v2::struct_member::Member::Field(field)) => Some(field),
                // A reserved tombstone emits no field (typl §7.4), so it
                // constrains nothing.
                Some(v2::struct_member::Member::Reserved(_)) | None => None,
            })
            .fold(Eligibility::ALL, |acc, field| {
                acc.meet(field_eligibility(ctx, field, seen))
            }),
        Some(v2::decl::Kind::UnionDef(ud)) => ud.arms.iter().fold(Eligibility::ALL, |acc, arm| {
            acc.meet(type_ref_eligibility(ctx, &arm.type_ref, seen))
        }),
        // A constant emits no item to derive on, and the interaction kinds
        // ride `Interface.interactions` rather than a package decl. Kept
        // total.
        Some(_) | None => Eligibility::NONE,
    };
    seen.remove(&decl.name);
    result
}

fn scalar_eligibility(td: &v2::TypeDef) -> Eligibility {
    match backing_scalar(td) {
        // A unit backing implies float and lands here, as does an absent
        // backing: `backing_scalar` is total and maps both to `Float`.
        ScalarBacking::Float => Eligibility::COPY_ONLY,
        ScalarBacking::Integer | ScalarBacking::Boolean => Eligibility::ALL,
        // `String` and `Vec<u8>` are `Eq` and `Hash` but allocate.
        ScalarBacking::String | ScalarBacking::Bytes => Eligibility::EQ_ONLY,
    }
}

/// One struct field's contribution. The `Field` envelope carries nothing the
/// eligibility depends on, so it unwraps to the `FieldType` twin.
fn field_eligibility(ctx: &Ctx, field: &v2::Field, seen: &mut HashSet<String>) -> Eligibility {
    match field.r#type.as_ref() {
        Some(ft) => field_type_eligibility(ctx, ft, seen),
        None => Eligibility::NONE,
    }
}

/// One type position's contribution.
///
/// `optional` is not read: the emitted Rust is `Option<T>`, and `Option` keeps
/// both `Copy` and `Eq` from `T`, so the position contributes exactly what `T`
/// does.
fn field_type_eligibility(
    ctx: &Ctx,
    ft: &v2::FieldType,
    seen: &mut HashSet<String>,
) -> Eligibility {
    match &ft.kind {
        Some(v2::field_type::Kind::Named(reference)) => type_ref_eligibility(ctx, reference, seen),
        Some(v2::field_type::Kind::Primitive(prim)) => primitive_eligibility(*prim),
        Some(v2::field_type::Kind::InlineScalar(td)) => scalar_eligibility(td),
        Some(v2::field_type::Kind::Tuple(tuple)) => tuple_eligibility(ctx, tuple, seen),
        Some(v2::field_type::Kind::Array(array)) => {
            // A bounded array emits `Vec<T>` and a fixed one `[T; N]`. Only
            // the second is `Copy` when `T` is, and the distinction is not
            // drawn here: the conservative answer is sound for both, and a
            // `Copy` that depends on the array bounds being equal is a rule
            // no reader of the generated crate could predict. Refusing the
            // fixed form is therefore policy, not soundness, and
            // `a_fixed_array_field_loses_copy_by_policy` pins it.
            let inner = array
                .element
                .as_deref()
                .map(|element| field_type_eligibility(ctx, element, seen))
                .unwrap_or(Eligibility::NONE);
            Eligibility {
                copy: false,
                eq: inner.eq,
            }
        }
        Some(v2::field_type::Kind::Map(map)) => {
            // `Vec<(K, V)>`: not `Copy`, `Eq` when both halves are.
            let key = map
                .key
                .as_deref()
                .map(|key| field_type_eligibility(ctx, key, seen))
                .unwrap_or(Eligibility::NONE);
            let value = map
                .value
                .as_deref()
                .map(|value| field_type_eligibility(ctx, value, seen))
                .unwrap_or(Eligibility::NONE);
            Eligibility {
                copy: false,
                eq: key.eq && value.eq,
            }
        }
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct or tuple field in checked IR, and it emits `()`
        // when it does. Kept total and conservative.
        Some(v2::field_type::Kind::Stream(_)) | None => Eligibility::NONE,
    }
}

fn tuple_eligibility(ctx: &Ctx, tuple: &v2::TupleType, seen: &mut HashSet<String>) -> Eligibility {
    tuple.fields.iter().fold(Eligibility::ALL, |acc, field| {
        let inner = field
            .r#type
            .as_ref()
            .map(|ft| field_type_eligibility(ctx, ft, seen))
            .unwrap_or(Eligibility::NONE);
        acc.meet(inner)
    })
}

/// A named reference. A same-package name recurses into its declaration; a
/// dotted or unknown one cannot be proven here and therefore disables every
/// conditional derive.
fn type_ref_eligibility(ctx: &Ctx, reference: &str, seen: &mut HashSet<String>) -> Eligibility {
    match ctx.lookup(reference) {
        Some(decl) => decl_eligibility(ctx, decl, seen),
        None => Eligibility::NONE,
    }
}

fn primitive_eligibility(prim: i32) -> Eligibility {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Float => Eligibility::COPY_ONLY,
        v2::PrimitiveType::Integer | v2::PrimitiveType::Boolean => Eligibility::ALL,
        v2::PrimitiveType::String | v2::PrimitiveType::Bytes => Eligibility::EQ_ONLY,
        // `Unspecified` emits `()`. Kept total and conservative.
        v2::PrimitiveType::Unspecified => Eligibility::NONE,
    }
}
