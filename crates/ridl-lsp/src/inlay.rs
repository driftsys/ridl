//! Inlay hints (docs/ROADMAP.md epic E1.16, general form §6.3).
//!
//! Two families of hint make hidden semantics visible at the desk, so the
//! author never has to open the IR to read them:
//!
//! - **Ordinal hints** render, beside every struct field, union arm, and
//!   enum/enum-set value, the number that is its wire identity (typl §7.4).
//!   For a struct field or union arm that is the derived declaration-order
//!   ordinal — counting `reserved` tombstones, so a field after a retired slot
//!   shows the higher number and a reorder is visibly a renumbering. For an
//!   enum value or enum-set bit it is the explicit integer value, because the
//!   wire identity of an enum member is the value it declares, not its
//!   position — showing a position there would contradict the `= N` in the
//!   source. Every number is read from the [`CheckedPackage`](ridl_sem::
//!   CheckedPackage) IR, never re-derived from the AST; the struct-field walk
//!   is [`hover::field_ordinal`](crate::hover::field_ordinal), shared with
//!   hover.
//! - **Unit expansion hints** render, after the UCUM code of a unit-typed
//!   `type`, the unit's human reading (`km/h ⟨kilometer per hour⟩`), from
//!   [`UcumExpr::display_name`](ridl_sem::ucum::UcumExpr::display_name) over
//!   the canonical unit the checker stored. When `display_name` cannot read the
//!   canonical form back (a curated-but-undisplayed atom, or a unit that failed
//!   to validate and left its raw source in the IR), no hint is emitted —
//!   never a wrong reading.
//!
//! The request is a range request: only hints whose anchor falls inside the
//! requested window (the editor's visible range) are returned. Each hint
//! carries a byte offset; the server converts it to a UTF-16 LSP position
//! through the file's `LineIndex`.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace};
use ridl_ir::v1;
use ridl_sem::check_package;
use ridl_sem::ucum::UcumExpr;
use ridl_syntax::ast::{
    AstNode, Backing, Definition, EnumDef, EnumSetDef, Name, StructDef, StructMember, TypeDef,
    UnionDef,
};
use rowan::{TextRange, TextSize};

use crate::hover::field_ordinal;
use crate::nav;

/// The kind of an inlay hint — what the server maps onto an LSP
/// `InlayHintKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// A member's wire-identity number, e.g. `#3`.
    Ordinal,
    /// A unit's human reading, e.g. `⟨kilometer per hour⟩`.
    Unit,
}

/// One inlay hint in the compiler's byte-offset world: the offset it anchors
/// after, its label text, and its kind. The server converts the offset to a
/// UTF-16 LSP position and maps the kind onto the LSP `InlayHintKind`.
#[derive(Debug, Clone)]
pub struct InlayHint {
    /// The byte offset the hint sits at — the end of a member name, or the end
    /// of a unit expression.
    pub offset: TextSize,
    /// The rendered label text.
    pub label: String,
    /// Ordinal or unit expansion.
    pub kind: HintKind,
}

/// The inlay hints inside `range` in `file` (a file of `pkg`): the ordinal
/// hints beside every member and the unit-expansion hints beside every
/// unit-typed `type`, filtered to the requested window.
pub fn inlay_hints(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    range: TextRange,
) -> Vec<InlayHint> {
    let source = nav::source_file(db, file);
    let ir = &check_package(db, ws, pkg, std).ir;

    let mut hints = Vec::new();
    for definition in source.definitions() {
        match definition {
            Definition::Struct(struct_def) => struct_hints(ir, &struct_def, &mut hints),
            Definition::Union(union_def) => union_hints(ir, &union_def, &mut hints),
            Definition::Enum(enum_def) => enum_hints(ir, &enum_def, &mut hints),
            Definition::EnumSet(enum_set) => enum_set_hints(ir, &enum_set, &mut hints),
            Definition::Type(type_def) => unit_hint(ir, &type_def, &mut hints),
            Definition::Const(_) => {}
        }
    }
    hints.retain(|hint| range.contains_inclusive(hint.offset));
    hints
}

/// An ordinal hint anchored after `name`, labeled `#N`.
fn ordinal_hint(name: &Name, number: impl std::fmt::Display) -> InlayHint {
    InlayHint {
        offset: name.syntax().text_range().end(),
        label: format!("#{number}"),
        kind: HintKind::Ordinal,
    }
}

/// The identifier text of a member name, if present.
fn name_text(name: &Name) -> Option<String> {
    Some(name.ident_token()?.text().to_string())
}

/// Ordinal hints for a struct's fields. The ordinal is read from the struct's
/// IR by field name through the shared [`field_ordinal`] walk, so tombstones
/// are counted exactly as codegen counts them. Reserved slots carry no field
/// name and are skipped.
fn struct_hints(ir: &v1::Package, struct_def: &StructDef, out: &mut Vec<InlayHint>) {
    let Some(struct_name) = struct_def.name().and_then(|name| name_text(&name)) else {
        return;
    };
    for member in struct_def.members() {
        let StructMember::Field(field) = member else {
            continue;
        };
        let Some(name) = field.name() else {
            continue;
        };
        let Some(field_name) = name_text(&name) else {
            continue;
        };
        if let Some(ordinal) = field_ordinal(ir, &struct_name, &field_name) {
            out.push(ordinal_hint(&name, ordinal));
        }
    }
}

/// Ordinal hints for a union's arms, read from the union's IR by arm name so
/// reserved tombstone slots are counted (typl §7.4).
fn union_hints(ir: &v1::Package, union_def: &UnionDef, out: &mut Vec<InlayHint>) {
    let Some(union_name) = union_def.name().and_then(|name| name_text(&name)) else {
        return;
    };
    let Some(def) = union_ir(ir, &union_name) else {
        return;
    };
    for arm in union_def.arms() {
        let Some(name) = arm.name() else {
            continue;
        };
        let Some(arm_name) = name_text(&name) else {
            continue;
        };
        if let Some(ordinal) = def
            .arms
            .iter()
            .find(|arm| arm.name == arm_name)
            .map(|arm| arm.ordinal)
        {
            out.push(ordinal_hint(&name, ordinal));
        }
    }
}

/// Wire-value hints for an enum's values, read from the enum's IR by name. The
/// number is the explicit integer value — an enum member's transport identity
/// — not a declaration position.
fn enum_hints(ir: &v1::Package, enum_def: &EnumDef, out: &mut Vec<InlayHint>) {
    let Some(enum_name) = enum_def.name().and_then(|name| name_text(&name)) else {
        return;
    };
    let Some(def) = enum_ir(ir, &enum_name) else {
        return;
    };
    for value in enum_def.values() {
        let Some(name) = value.name() else {
            continue;
        };
        let Some(value_name) = name_text(&name) else {
            continue;
        };
        if let Some(wire) = def
            .values
            .iter()
            .find(|value| value.name == value_name)
            .map(|value| value.value)
        {
            out.push(ordinal_hint(&name, wire));
        }
    }
}

/// Wire-value hints for an enum set's bits, read from the enum set's IR by
/// name. The derived form (`enumset X : Enum`) has no source bits, so it
/// contributes no hints even though the IR copies the backing enum's values.
fn enum_set_hints(ir: &v1::Package, enum_set: &EnumSetDef, out: &mut Vec<InlayHint>) {
    let Some(set_name) = enum_set.name().and_then(|name| name_text(&name)) else {
        return;
    };
    let Some(def) = enum_set_ir(ir, &set_name) else {
        return;
    };
    for bit in enum_set.bits() {
        let Some(name) = bit.name() else {
            continue;
        };
        let Some(bit_name) = name_text(&name) else {
            continue;
        };
        if let Some(wire) = def
            .bits
            .iter()
            .find(|bit| bit.name == bit_name)
            .map(|bit| bit.value)
        {
            out.push(ordinal_hint(&name, wire));
        }
    }
}

/// The unit-expansion hint for a unit-typed `type`: the human reading of the
/// canonical UCUM unit the checker stored, anchored after the unit expression.
/// A non-unit backing, a type that did not lower, or a unit `display_name`
/// cannot read back contributes no hint.
fn unit_hint(ir: &v1::Package, type_def: &TypeDef, out: &mut Vec<InlayHint>) {
    let Some(Backing::Unit(unit_expr)) = type_def.backing() else {
        return;
    };
    let Some(type_name) = type_def.name().and_then(|name| name_text(&name)) else {
        return;
    };
    let Some(canonical) = type_ir(ir, &type_name).and_then(unit_backing) else {
        return;
    };
    let Some(reading) = (UcumExpr { canonical }).display_name() else {
        return;
    };
    out.push(InlayHint {
        offset: unit_expr.syntax().text_range().end(),
        label: format!("\u{27e8}{reading}\u{27e9}"),
        kind: HintKind::Unit,
    });
}

/// The IR `UnionDef` of a declaration named `name`, if it lowered to one.
fn union_ir<'a>(ir: &'a v1::Package, name: &str) -> Option<&'a v1::UnionDef> {
    match &ir.decls.iter().find(|decl| decl.name == name)?.kind {
        Some(v1::decl::Kind::UnionDef(def)) => Some(def),
        _ => None,
    }
}

/// The IR `EnumDef` of a declaration named `name`, if it lowered to one.
fn enum_ir<'a>(ir: &'a v1::Package, name: &str) -> Option<&'a v1::EnumDef> {
    match &ir.decls.iter().find(|decl| decl.name == name)?.kind {
        Some(v1::decl::Kind::EnumDef(def)) => Some(def),
        _ => None,
    }
}

/// The IR `EnumSetDef` of a declaration named `name`, if it lowered to one.
fn enum_set_ir<'a>(ir: &'a v1::Package, name: &str) -> Option<&'a v1::EnumSetDef> {
    match &ir.decls.iter().find(|decl| decl.name == name)?.kind {
        Some(v1::decl::Kind::EnumSetDef(def)) => Some(def),
        _ => None,
    }
}

/// The IR `TypeDef` of a declaration named `name`, if it lowered to one.
fn type_ir<'a>(ir: &'a v1::Package, name: &str) -> Option<&'a v1::TypeDef> {
    match &ir.decls.iter().find(|decl| decl.name == name)?.kind {
        Some(v1::decl::Kind::TypeDef(def)) => Some(def),
        _ => None,
    }
}

/// The canonical UCUM unit string of a type's backing, if the backing is a
/// unit.
fn unit_backing(type_def: &v1::TypeDef) -> Option<String> {
    match type_def.backing.as_ref()?.kind.as_ref()? {
        v1::backing::Kind::Unit(unit) => Some(unit.clone()),
        v1::backing::Kind::Primitive(_) => None,
    }
}
