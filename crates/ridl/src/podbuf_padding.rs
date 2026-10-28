//! RIDL-156 (`podbuf-padding-order`): a struct whose declared field order lays
//! out in a larger podbuf head than another order of the same fields, reported
//! only for a struct that no published baseline declares.
//!
//! The podbuf layout places each field at the lowest offset where it fits
//! (`ridl_ir::projection::podbuf`), so the order of the fields changes the
//! padding. A published struct keeps its order, because the order is part of
//! its wire format; a struct that is not published yet can still change it.
//! The suggested order is the fields sorted by descending alignment, with
//! declaration order kept among fields of equal alignment. It is reported only
//! when it is strictly smaller than the declared order.

use std::collections::BTreeSet;

use ridl_core::diag::{DiagCode, Diagnostic, Severity, SourceMap, Span};
use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::podbuf::{Source, struct_head};
use ridl_ir::v2::{self, decl, struct_member};

/// One RIDL-156 per struct of `subjects` that is not in `released` and whose
/// saving order is smaller than its declared order.
///
/// `all` holds every package of the workspace, remote units included, because a
/// field type can name a declaration of any of them; `std` is `ridl.std`.
/// `released` holds the `(package, struct name)` pairs that a baseline
/// snapshot declares. `site` gives the span of a struct's name.
pub(crate) fn check(
    all: &[&v2::Package],
    std: &v2::Package,
    subjects: &[&v2::Package],
    released: &BTreeSet<(String, String)>,
    sources: &mut SourceMap,
    mut site: impl FnMut(&str, &str, &mut SourceMap) -> Span,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for package in subjects {
        let others: Vec<&v2::Package> = all
            .iter()
            .copied()
            .filter(|other| other.name != package.name)
            .chain(std::iter::once(std))
            .collect();
        let packages = Packages {
            package,
            others: &others,
        };
        for declaration in &package.decls {
            let Some(decl::Kind::StructDef(def)) = declaration.kind.as_ref() else {
                continue;
            };
            if released.contains(&(package.name.clone(), declaration.name.clone())) {
                continue;
            }
            let Some((declared, saving, order)) = compare(packages, &declaration.name, def) else {
                continue;
            };
            diagnostics.push(Diagnostic {
                code: DiagCode::RIDL_156,
                severity: Severity::Info,
                message: format!(
                    "the struct `{}.{}` declares its fields in an order whose podbuf head takes \
                     {declared} bytes; the order `{order}` takes {saving} bytes. Reorder the \
                     fields before the first release, because the order is part of the wire \
                     format once published",
                    package.name, declaration.name,
                ),
                primary: site(&package.name, &declaration.name, sources),
                labels: Vec::new(),
                fixits: Vec::new(),
            });
        }
    }
    diagnostics
}

/// The head size of the declared order, the head size of the saving order and
/// the saving order's field names, when the saving order is strictly smaller.
/// `None` when it is not, and when either layout is an error: the IR of a
/// workspace with no error has no other defect, and RIDL-153 reports a head
/// that a `u32` cannot address.
fn compare(packages: Packages<'_>, name: &str, def: &v2::StructDef) -> Option<(u32, u32, String)> {
    let declared = struct_head(packages, name, def).ok()?;
    let alignment = |field: &str| {
        declared
            .fields
            .iter()
            .find(|placement| matches!(&placement.source, Source::Field { name, .. } if name == field))
            .map_or(1, |placement| placement.align)
    };
    let mut fields: Vec<&v2::Field> = def
        .members
        .iter()
        .filter_map(|member| match &member.member {
            Some(struct_member::Member::Field(field)) => Some(&**field),
            _ => None,
        })
        .collect();
    // `sort_by_key` is stable, so fields of equal alignment keep their
    // declaration order.
    fields.sort_by_key(|field| std::cmp::Reverse(alignment(&field.name)));
    let reordered = v2::StructDef {
        members: fields
            .iter()
            .map(|field| v2::StructMember {
                member: Some(struct_member::Member::Field(Box::new((*field).clone()))),
            })
            .collect(),
        ..def.clone()
    };
    let saving = struct_head(packages, name, &reordered).ok()?;
    if saving.size >= declared.size {
        return None;
    }
    let order = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Some((declared.size, saving.size, order))
}
