//! Compare canonical units at sites that have the same exact name.

use std::collections::BTreeMap;

use ridl_core::diag::{DiagCode, Diagnostic, Label, Span};
use ridl_core::lint::lint_by_name;
use ridl_ir::v2::{self, backing, decl, field_type};

use super::Ctx;

/// Optionality is a flag on the field type, so it does not change its unit.
/// Import aliases are already canonical named references in checked IR.
pub(crate) fn unit_of(ctx: &Ctx<'_>, pkg: &str, ty: &v2::FieldType) -> Option<String> {
    match ty.kind.as_ref()? {
        field_type::Kind::Named(name) => named_unit(ctx, pkg, name),
        field_type::Kind::InlineScalar(scalar) => scalar_unit(scalar),
        field_type::Kind::Primitive(_)
        | field_type::Kind::Tuple(_)
        | field_type::Kind::Array(_)
        | field_type::Kind::Map(_)
        | field_type::Kind::Stream(_) => None,
    }
}

fn named_unit(ctx: &Ctx<'_>, pkg: &str, name: &str) -> Option<String> {
    let (pkg, name) = name.rsplit_once('.').unwrap_or((pkg, name));
    let package = if pkg == "ridl.std" {
        ctx.std_ir
    } else {
        &ctx.checked.iter().find(|p| p.ir.name == pkg)?.ir
    };
    let decl = package.decls.iter().find(|decl| decl.name == name)?;
    match decl.kind.as_ref()? {
        decl::Kind::TypeDef(scalar) => scalar_unit(scalar),
        _ => None,
    }
}

fn scalar_unit(scalar: &v2::TypeDef) -> Option<String> {
    match scalar.backing.as_ref()?.kind.as_ref()? {
        backing::Kind::Unit(unit) => Some(unit.clone()),
        backing::Kind::Primitive(_) => None,
    }
}

pub(crate) fn check(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<Span>>> = BTreeMap::new();
    let mut add = |name: &str, unit: Option<String>, span: Option<Span>| {
        if let (Some(unit), Some(span)) = (unit, span) {
            groups
                .entry(name.into())
                .or_default()
                .entry(unit)
                .or_default()
                .push(span);
        }
    };
    for checked in ctx.checked.iter().filter(|p| p.ir.name != "ridl.std") {
        let pkg = &checked.ir;
        for decl in &pkg.decls {
            if let Some(decl::Kind::StructDef(def)) = &decl.kind {
                for member in &def.members {
                    let Some(v2::struct_member::Member::Field(field)) = &member.member else {
                        continue;
                    };
                    add(
                        &field.name,
                        field
                            .r#type
                            .as_ref()
                            .and_then(|ty| unit_of(ctx, &pkg.name, ty)),
                        ctx.sites.field(&pkg.name, &decl.name, &field.name),
                    );
                }
            }
        }
        for shape in pkg.shapes() {
            for member in &shape.interface.interactions {
                let span = ctx.sites.member(&pkg.name, shape.name, &member.name);
                let params = match &member.kind {
                    Some(decl::Kind::SignalDef(def)) => {
                        add(&member.name, named_unit(ctx, &pkg.name, &def.payload), span);
                        continue;
                    }
                    Some(decl::Kind::EventDef(def)) => {
                        add(&member.name, named_unit(ctx, &pkg.name, &def.payload), span);
                        continue;
                    }
                    Some(decl::Kind::FixedDef(def)) => {
                        add(
                            &member.name,
                            def.payload
                                .as_ref()
                                .and_then(|ty| unit_of(ctx, &pkg.name, ty)),
                            span,
                        );
                        continue;
                    }
                    Some(decl::Kind::CommandDef(def)) => &def.params,
                    Some(decl::Kind::QueryDef(def)) => &def.params,
                    _ => continue,
                };
                for param in params {
                    add(
                        &param.name,
                        param
                            .r#type
                            .as_ref()
                            .and_then(|ty| unit_of(ctx, &pkg.name, ty)),
                        ctx.sites
                            .param(&pkg.name, shape.name, &member.name, &param.name),
                    );
                }
            }
        }
    }
    let mut diagnostics = Vec::new();
    for (name, units) in groups {
        if units.len() < 2 {
            continue;
        }
        let max = units.values().map(Vec::len).max().unwrap_or(0);
        let tied = units.values().filter(|sites| sites.len() == max).count() > 1;
        for (unit, sites) in &units {
            if !tied && sites.len() == max {
                continue;
            }
            let others: Vec<_> = units.iter().filter(|(other, _)| *other != unit).collect();
            let other_units = others
                .iter()
                .map(|(unit, _)| format!("`{unit}`"))
                .collect::<Vec<_>>()
                .join(", ");
            for span in sites {
                diagnostics.push(Diagnostic {
                    code: DiagCode::TYPL_222,
                    severity: lint_by_name("inconsistent-unit")
                        .expect("registered lint")
                        .severity,
                    message: format!(
                        "`{name}` uses `{unit}` here; elsewhere `{name}` uses {other_units}"
                    ),
                    primary: *span,
                    labels: others
                        .iter()
                        .map(|(unit, sites)| Label {
                            span: sites[0],
                            message: format!("`{name}` uses `{unit}` here"),
                        })
                        .collect(),
                    fixits: Vec::new(),
                });
            }
        }
    }
    diagnostics
}
