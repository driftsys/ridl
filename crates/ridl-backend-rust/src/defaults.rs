//! Default-value derivation with the leaf-recursion rule (typl §5.8).
//!
//! An `impl Default` is emitted for a type only when every field it
//! transitively contains is derivable. The IR carries an `InitValue.derivable`
//! flag on each scalar type and each field, but for a field whose type is a
//! same-package composite that flag is a one-level flag (T15): a struct
//! `S { inner: Inner }` where `Inner` has a non-derivable field records
//! `S.inner.init.derivable == true`. Emitting `Default` for `S` on that basis
//! while `Inner` has no `Default` would not compile. So same-package composite
//! and scalar references are re-checked by recursing into the referenced
//! declaration; the flag is trusted only for cross-package references, which
//! this backend cannot resolve (it generates one package at a time) and which
//! T15 computed with full resolution.

use crate::{
    Ctx, ScalarBacking, bool_tokens, class_backing, declared, ident, numeric_tokens, scalar_ctor,
    snake_of, tuple_name, type_path,
};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::codegen::v1;

/// The right-hand side of `fn default() -> Self` for a top-level declaration,
/// or `None` when the type is not fully Default-constructible.
pub(crate) fn decl_default_expr(ctx: &Ctx, decl: &v1::Declaration) -> Option<TokenStream> {
    let name = declared(decl.name.as_ref());
    match decl.kind.as_ref() {
        Some(v1::declaration::Kind::Scalar(sc)) => type_def_default(name, decl.init.as_ref()?, sc),
        Some(v1::declaration::Kind::Struct(sd)) => struct_default(ctx, name, sd),
        Some(v1::declaration::Kind::Enum(ed)) => enum_default(name, ed),
        Some(v1::declaration::Kind::EnumSet(_)) => Some(enum_set_default(name)),
        Some(v1::declaration::Kind::Union(ud)) => union_default(ctx, name, ud),
        // A constant emits no value to default, and an interaction rides
        // `Interface.interactions` rather than a package declaration. No
        // Default either way.
        Some(v1::declaration::Kind::Constant(_)) | None => None,
    }
}

/// Reference position: what is known about the slot a value fills. `flag` is
/// the enclosing field's T15 derivability flag — the IR's own one-level
/// `InitValue.derivable`, which the model carries as `Init.one_level`:
/// `Some` for a struct field, `None` for a tuple field or a collection
/// element, which carry no `InitValue`. A tuple in this position names itself
/// through the model, so there is no name hint to carry.
struct Slot<'a> {
    init_value: Option<&'a str>,
    declared_init: Option<&'a str>,
    flag: Option<bool>,
}

/// A named scalar's own default.
///
/// The model's `Init.derivable` on a scalar declaration is exactly the
/// condition this used to compute — the IR's flag and a constructible init
/// text together (`facts::Inits::type_def`) — so it is read rather than
/// recomputed, and the value it carries is the init text.
fn type_def_default(name: &str, init: &v1::Init, sc: &v1::Scalar) -> Option<TokenStream> {
    if !init.derivable {
        return None;
    }
    let inner = scalar_default_value(class_backing(sc.class), init.value.as_deref())?;
    let name_id = ident(name);
    // A vacuous type has no `new_unchecked`; its `new` is `const` and
    // infallible, so it stands in here (`scalar_ctor`).
    let ctor = scalar_ctor(sc);
    Some(quote! { #name_id::#ctor(#inner) })
}

/// The inner value of a newtype default for a scalar backing. A derivable
/// numeric or unit type carries its init text (`"0"` or `min`). A string type
/// with a declared init emits that init verbatim as a string literal; without a
/// declared init the derivable case admits length 0 and defaults to the empty
/// string (I1). A bytes type with a declared init has no faithful literal form
/// here, so it gets no Default rather than a wrong (empty) one; the derivable
/// zero-length case defaults to the empty vector.
fn scalar_default_value(backing: ScalarBacking, value: Option<&str>) -> Option<TokenStream> {
    match backing {
        ScalarBacking::Float => Some(numeric_tokens(value?, true)),
        ScalarBacking::Integer => Some(numeric_tokens(value?, false)),
        ScalarBacking::Boolean => Some(bool_tokens(value.unwrap_or("false"))),
        ScalarBacking::String => match value {
            Some(text) if !text.is_empty() => Some(quote! { #text.to_string() }),
            _ => Some(quote! { String::new() }),
        },
        ScalarBacking::Bytes => match value {
            Some(text) if !text.is_empty() => None,
            _ => Some(quote! { Vec::new() }),
        },
    }
}

fn struct_default(ctx: &Ctx, name: &str, sd: &v1::Struct) -> Option<TokenStream> {
    let name_id = ident(name);
    let mut inits = Vec::new();
    for slot in &sd.slots {
        if let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_ref() {
            let ft = field.r#type.as_ref()?;
            // The same projection `emit_field` applies, or the initializer
            // names a field the struct does not have (ADR-0016 decision 2).
            let fname = ident(snake_of(field.name.as_ref()));
            let init = field.init.as_ref();
            let slot = Slot {
                init_value: init.and_then(|i| i.value.as_deref()),
                declared_init: field.declared_init.as_deref(),
                flag: Some(init.is_some_and(|i| i.one_level)),
            };
            let expr = slot_default(ctx, ft, &slot)?;
            inits.push(quote! { #fname: #expr });
        }
    }
    Some(quote! { #name_id { #(#inits),* } })
}

/// The `Default` body for one generated tuple struct (typl §11). Tuple fields
/// carry no `InitValue`, so the slot has no flag: cross-package tuple fields
/// cannot be resolved and make the tuple non-constructible.
pub(crate) fn tuple_default_expr(ctx: &Ctx, tuple: &v1::InducedTuple) -> Option<TokenStream> {
    let name_id = ident(tuple_name(tuple));
    let mut inits = Vec::new();
    for field in &tuple.fields {
        let ft = field.r#type.as_ref()?;
        // The same projection `emit_tuple_struct` applies, or the initializer
        // names a field the tuple struct does not have (ADR-0016 decision 2).
        let fname = ident(snake_of(field.name.as_ref()));
        let slot = Slot {
            init_value: None,
            declared_init: None,
            flag: None,
        };
        let expr = slot_default(ctx, ft, &slot)?;
        inits.push(quote! { #fname: #expr });
    }
    Some(quote! { #name_id { #(#inits),* } })
}

fn slot_default(ctx: &Ctx, ft: &v1::Type, slot: &Slot) -> Option<TokenStream> {
    if ft.optional {
        return Some(quote! { None });
    }
    match ft.kind.as_ref() {
        Some(v1::r#type::Kind::Named(reference)) => named_default(ctx, reference, slot),
        Some(v1::r#type::Kind::Primitive(prim)) => primitive_default(*prim, slot),
        Some(v1::r#type::Kind::Inline(sc)) => {
            if slot.flag == Some(false) {
                None
            } else {
                scalar_default_value(class_backing(sc.class), slot.init_value)
            }
        }
        Some(v1::r#type::Kind::Tuple(reference)) => {
            let tuple = ctx.tuple(reference.index)?;
            if tuple_default_expr(ctx, tuple).is_some() {
                let id = ident(tuple_name(tuple));
                Some(quote! { #id::default() })
            } else {
                None
            }
        }
        Some(v1::r#type::Kind::Array(array)) => array_default(ctx, array, slot),
        Some(v1::r#type::Kind::Map(map)) => map_default(ctx, map, slot),
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct or tuple field in checked IR, and it has no
        // Default either way.
        Some(v1::r#type::Kind::Stream(_)) | None => None,
    }
}

fn named_default(ctx: &Ctx, reference: &v1::TypeRef, slot: &Slot) -> Option<TokenStream> {
    if reference.foreign {
        // Cross-package: the remote backing is not resolvable here. A declared
        // init on such a field cannot be faithfully wrapped without that
        // backing, and substituting the referenced type's own default would
        // emit a wrong value — worse than none — so the containing struct gets
        // no Default at all (I2). Without a declared init, trust the enclosing
        // field's T15 flag and use the referenced type's own default.
        if slot.declared_init.is_some() {
            None
        } else if slot.flag == Some(true) {
            let path = type_path(&reference.reference);
            Some(quote! { #path::default() })
        } else {
            None
        }
    } else if let Some(decl) = ctx.local(reference) {
        match decl.kind.as_ref() {
            Some(v1::declaration::Kind::Scalar(sc)) => {
                type_def_default(&reference.reference, decl.init.as_ref()?, sc)?;
                let path = type_path(&reference.reference);
                if let Some(declared) = slot.declared_init {
                    let inner = scalar_default_value(class_backing(sc.class), Some(declared))?;
                    let ctor = scalar_ctor(sc);
                    Some(quote! { #path::#ctor(#inner) })
                } else {
                    Some(quote! { #path::default() })
                }
            }
            // Same-package composite, enum, or enum set: recurse rather than
            // trust a one-level flag.
            _ => named_same_package_default(ctx, reference),
        }
    } else {
        None
    }
}

fn named_same_package_default(ctx: &Ctx, reference: &v1::TypeRef) -> Option<TokenStream> {
    let decl = ctx.local(reference)?;
    // Guard the one recursion point into a same-package declaration's Default.
    // A cyclic composite (`struct S { next: S }`) is TYPL-206 upstream, but the
    // backend must not trust that gate: on a cycle it denies a Default rather
    // than recurse forever and overflow the stack (C1b, defense in depth).
    if !ctx.enter_default(&reference.reference) {
        return None;
    }
    let derivable = decl_default_expr(ctx, decl).is_some();
    ctx.leave_default(&reference.reference);
    if derivable {
        let path = type_path(&reference.reference);
        Some(quote! { #path::default() })
    } else {
        None
    }
}

fn primitive_default(prim: i32, slot: &Slot) -> Option<TokenStream> {
    match v1::PrimitiveType::try_from(prim).unwrap_or(v1::PrimitiveType::Unspecified) {
        v1::PrimitiveType::Integer => Some(numeric_tokens(slot.init_value.unwrap_or("0"), false)),
        v1::PrimitiveType::Float => Some(numeric_tokens(slot.init_value.unwrap_or("0"), true)),
        v1::PrimitiveType::Boolean => Some(bool_tokens(slot.init_value.unwrap_or("false"))),
        v1::PrimitiveType::String if slot.flag != Some(false) => Some(quote! { String::new() }),
        v1::PrimitiveType::Bytes if slot.flag != Some(false) => Some(quote! { Vec::new() }),
        _ => None,
    }
}

fn array_default(ctx: &Ctx, array: &v1::ArrayType, slot: &Slot) -> Option<TokenStream> {
    let element = array.element.as_deref()?;
    let element_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
    };
    if array.min == array.max {
        if array.max == 0 {
            return Some(quote! { [] });
        }
        let elem = slot_default(ctx, element, &element_slot)?;
        Some(quote! { ::core::array::from_fn(|_| #elem) })
    } else if array.min == 0 {
        Some(quote! { Vec::new() })
    } else {
        let elem = slot_default(ctx, element, &element_slot)?;
        let count = count_tokens(array.min);
        Some(quote! { (0..#count).map(|_| #elem).collect() })
    }
}

fn map_default(ctx: &Ctx, map: &v1::MapType, slot: &Slot) -> Option<TokenStream> {
    if map.min == 0 {
        return Some(quote! { Vec::new() });
    }
    let key_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
    };
    let value_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
    };
    let key = slot_default(ctx, map.key.as_deref()?, &key_slot)?;
    let value = slot_default(ctx, map.value.as_deref()?, &value_slot)?;
    let count = count_tokens(map.min);
    Some(quote! { (0..#count).map(|_| (#key, #value)).collect() })
}

fn enum_default(name: &str, ed: &v1::Enum) -> Option<TokenStream> {
    // The value 0 if declared, else the lowest declared value (typl §5.8).
    // The lowering picks the member once, as `Enum.init_member`.
    let chosen = ed.values.get(ed.init_member? as usize)?;
    let name_id = ident(name);
    let variant = ident(declared(chosen.name.as_ref()));
    Some(quote! { #name_id::#variant })
}

fn enum_set_default(name: &str) -> TokenStream {
    // The empty set — no bits set (typl §5.8, §9).
    let name_id = ident(name);
    quote! { #name_id(0) }
}

fn union_default(ctx: &Ctx, name: &str, ud: &v1::Union) -> Option<TokenStream> {
    // The first arm's init (typl §5.8). The arm references a named type.
    let first = ud.arms.first()?;
    let reference = first.r#type.as_ref()?;
    let arm_default = if reference.foreign {
        None
    } else {
        named_same_package_default(ctx, reference)
    }?;
    let name_id = ident(name);
    let variant = ident(crate::camel_of(first.name.as_ref()));
    Some(quote! { #name_id::#variant(#arm_default) })
}

fn count_tokens(value: u64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}
