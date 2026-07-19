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
    Ctx, ScalarBacking, backing_scalar, bool_tokens, camel_case, ident, numeric_tokens, type_path,
};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::v1;

/// The right-hand side of `fn default() -> Self` for a top-level declaration,
/// or `None` when the type is not fully Default-constructible.
pub(crate) fn decl_default_expr(ctx: &Ctx, decl: &v1::Decl) -> Option<TokenStream> {
    match &decl.kind {
        Some(v1::decl::Kind::TypeDef(td)) => type_def_default(&decl.name, td),
        Some(v1::decl::Kind::StructDef(sd)) => struct_default(ctx, &decl.name, sd),
        Some(v1::decl::Kind::EnumDef(ed)) => enum_default(&decl.name, ed),
        Some(v1::decl::Kind::EnumSetDef(_)) => Some(enum_set_default(&decl.name)),
        Some(v1::decl::Kind::UnionDef(ud)) => union_default(ctx, &decl.name, ud),
        Some(v1::decl::Kind::ConstDef(_)) | None => None,
    }
}

/// Reference position: what is known about the slot a value fills. `flag` is the
/// enclosing field's T15 derivability flag (`Some` for a struct field, `None`
/// for a tuple field or a collection element, which carry no `InitValue`);
/// `hint` is the generated tuple-struct name for a tuple in this position.
struct Slot<'a> {
    init_value: Option<&'a str>,
    declared_init: Option<&'a str>,
    flag: Option<bool>,
    hint: &'a str,
}

fn type_def_default(name: &str, td: &v1::TypeDef) -> Option<TokenStream> {
    let init = td.init.as_ref()?;
    if !init.derivable {
        return None;
    }
    let inner = scalar_default_value(backing_scalar(td), init.value.as_deref())?;
    let name_id = ident(name);
    Some(quote! { #name_id(#inner) })
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

fn struct_default(ctx: &Ctx, name: &str, sd: &v1::StructDef) -> Option<TokenStream> {
    let name_id = ident(name);
    let mut inits = Vec::new();
    for member in &sd.members {
        if let Some(v1::struct_member::Member::Field(field)) = &member.member {
            let ft = field.r#type.as_ref()?;
            let fname = ident(&field.name);
            let hint = format!("{}{}", camel_case(name), camel_case(&field.name));
            let init = field.init.as_ref();
            let slot = Slot {
                init_value: init.and_then(|i| i.value.as_deref()),
                declared_init: field.declared_init.as_deref(),
                flag: Some(init.map(|i| i.derivable).unwrap_or(false)),
                hint: &hint,
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
pub(crate) fn tuple_default_expr(
    ctx: &Ctx,
    name: &str,
    tuple: &v1::TupleType,
) -> Option<TokenStream> {
    let name_id = ident(name);
    let mut inits = Vec::new();
    for field in &tuple.fields {
        let ft = field.r#type.as_ref()?;
        let fname = ident(&field.name);
        let hint = format!("{}{}", name, camel_case(&field.name));
        let slot = Slot {
            init_value: None,
            declared_init: None,
            flag: None,
            hint: &hint,
        };
        let expr = slot_default(ctx, ft, &slot)?;
        inits.push(quote! { #fname: #expr });
    }
    Some(quote! { #name_id { #(#inits),* } })
}

fn slot_default(ctx: &Ctx, ft: &v1::FieldType, slot: &Slot) -> Option<TokenStream> {
    if ft.optional {
        return Some(quote! { None });
    }
    match &ft.kind {
        Some(v1::field_type::Kind::Named(reference)) => named_default(ctx, reference, slot),
        Some(v1::field_type::Kind::Primitive(prim)) => primitive_default(*prim, slot),
        Some(v1::field_type::Kind::InlineScalar(td)) => {
            if slot.flag == Some(false) {
                None
            } else {
                scalar_default_value(backing_scalar(td), slot.init_value)
            }
        }
        Some(v1::field_type::Kind::Tuple(tuple)) => {
            if tuple_default_expr(ctx, slot.hint, tuple).is_some() {
                let id = ident(slot.hint);
                Some(quote! { #id::default() })
            } else {
                None
            }
        }
        Some(v1::field_type::Kind::Array(array)) => array_default(ctx, array, slot),
        Some(v1::field_type::Kind::Map(map)) => map_default(ctx, map, slot),
        None => None,
    }
}

fn named_default(ctx: &Ctx, reference: &str, slot: &Slot) -> Option<TokenStream> {
    if reference.contains('.') {
        // Cross-package: the remote backing is not resolvable here. A declared
        // init on such a field cannot be faithfully wrapped without that
        // backing, and substituting the referenced type's own default would
        // emit a wrong value — worse than none — so the containing struct gets
        // no Default at all (I2). Without a declared init, trust the enclosing
        // field's T15 flag and use the referenced type's own default.
        if slot.declared_init.is_some() {
            None
        } else if slot.flag == Some(true) {
            let path = type_path(reference);
            Some(quote! { #path::default() })
        } else {
            None
        }
    } else if let Some(decl) = ctx.lookup(reference) {
        match &decl.kind {
            Some(v1::decl::Kind::TypeDef(td)) => {
                type_def_default(reference, td)?;
                let path = type_path(reference);
                if let Some(declared) = slot.declared_init {
                    let inner = scalar_default_value(backing_scalar(td), Some(declared))?;
                    Some(quote! { #path(#inner) })
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

fn named_same_package_default(ctx: &Ctx, reference: &str) -> Option<TokenStream> {
    let decl = ctx.lookup(reference)?;
    // Guard the one recursion point into a same-package declaration's Default.
    // A cyclic composite (`struct S { next: S }`) is TYPL-206 upstream, but the
    // backend must not trust that gate: on a cycle it denies a Default rather
    // than recurse forever and overflow the stack (C1b, defense in depth).
    if !ctx.enter_default(reference) {
        return None;
    }
    let derivable = decl_default_expr(ctx, decl).is_some();
    ctx.leave_default(reference);
    if derivable {
        let path = type_path(reference);
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
    let element = array.element.as_ref()?;
    let hint = format!("{}Element", slot.hint);
    let element_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
        hint: &hint,
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
    let key_hint = format!("{}Key", slot.hint);
    let value_hint = format!("{}Value", slot.hint);
    let key_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
        hint: &key_hint,
    };
    let value_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
        hint: &value_hint,
    };
    let key = slot_default(ctx, map.key.as_ref()?, &key_slot)?;
    let value = slot_default(ctx, map.value.as_ref()?, &value_slot)?;
    let count = count_tokens(map.min);
    Some(quote! { (0..#count).map(|_| (#key, #value)).collect() })
}

fn enum_default(name: &str, ed: &v1::EnumDef) -> Option<TokenStream> {
    // The value 0 if declared, else the lowest declared value (typl §5.8).
    let chosen = ed
        .values
        .iter()
        .find(|value| value.value == 0)
        .or_else(|| ed.values.iter().min_by_key(|value| value.value))?;
    let name_id = ident(name);
    let variant = ident(&chosen.name);
    Some(quote! { #name_id::#variant })
}

fn enum_set_default(name: &str) -> TokenStream {
    // The empty set — no bits set (typl §5.8, §9).
    let name_id = ident(name);
    quote! { #name_id(0) }
}

fn union_default(ctx: &Ctx, name: &str, ud: &v1::UnionDef) -> Option<TokenStream> {
    // The first arm's init (typl §5.8). The arm references a named type.
    let first = ud.arms.first()?;
    let arm_default = if first.type_ref.contains('.') {
        None
    } else {
        named_same_package_default(ctx, &first.type_ref)
    }?;
    let name_id = ident(name);
    let variant = ident(&camel_case(&first.name));
    Some(quote! { #name_id::#variant(#arm_default) })
}

fn count_tokens(value: u64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}
