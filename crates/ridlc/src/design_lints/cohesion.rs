//! Group interface members by their directly referenced nominal types.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::diag::{DiagCode, Diagnostic};
use ridl_core::lint::lint_by_name;
use ridl_ir::v2::{FieldType, Interface, Package, decl, field_type, return_type, stream_type};

use super::{Ctx, qualify};

/// The least strict thresholds whose precision on the evaluation corpus meets
/// the Info level; the derivation is recorded in `evals/calibration/summary.md`.
pub(crate) const LOW_COHESION_MIN_GROUPS: usize = 7;
pub(crate) const LOW_COHESION_MIN_GROUP_SIZE: usize = 1;

/// Groups checked interface members linked by a shared named type.
///
/// References inside anonymous containers count, but named type definitions are
/// not expanded. Primitives and types owned by `ridl.std` do not count; members
/// with no remaining types are omitted. Names within each group are sorted,
/// and groups follow their earliest member in declaration order.
pub fn cohesion_groups(pkg: &Package, iface: &Interface) -> Vec<Vec<String>> {
    let mut parents: Vec<_> = (0..iface.interactions.len()).collect();
    let mut typed_members = Vec::new();
    let mut first_by_type = BTreeMap::new();
    for (index, member) in iface.interactions.iter().enumerate() {
        let mut types = BTreeSet::new();
        match &member.kind {
            Some(decl::Kind::SignalDef(signal)) => add_named(pkg, &signal.payload, &mut types),
            Some(decl::Kind::EventDef(event)) => add_named(pkg, &event.payload, &mut types),
            Some(decl::Kind::FixedDef(fixed)) => {
                if let Some(ty) = &fixed.payload {
                    add_types(pkg, ty, &mut types);
                }
            }
            Some(decl::Kind::CommandDef(command)) => {
                for param in &command.params {
                    if let Some(ty) = &param.r#type {
                        add_types(pkg, ty, &mut types);
                    }
                }
            }
            Some(decl::Kind::QueryDef(query)) => {
                for param in &query.params {
                    if let Some(ty) = &param.r#type {
                        add_types(pkg, ty, &mut types);
                    }
                }
                match query.return_type.as_ref().and_then(|ret| ret.kind.as_ref()) {
                    Some(return_type::Kind::Value(ty)) => add_types(pkg, ty, &mut types),
                    Some(return_type::Kind::Fallible(fallible)) => {
                        add_named(pkg, &fallible.ok, &mut types);
                        add_named(pkg, &fallible.err, &mut types);
                    }
                    None => {}
                }
            }
            _ => {}
        }
        if types.is_empty() {
            continue;
        }
        typed_members.push(index);
        for ty in types {
            if let Some(&first) = first_by_type.get(&ty) {
                let a = root(&mut parents, first);
                let b = root(&mut parents, index);
                // The root is always the earliest source member of the group.
                parents[a.max(b)] = a.min(b);
            } else {
                first_by_type.insert(ty, index);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for index in typed_members {
        groups
            .entry(root(&mut parents, index))
            .or_default()
            .push(iface.interactions[index].name.clone());
    }
    groups
        .into_values()
        .map(|mut members| {
            members.sort();
            members
        })
        .collect()
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn add_named(pkg: &Package, name: &str, types: &mut BTreeSet<String>) {
    let mut name = name.to_string();
    qualify(&pkg.name, &mut name);
    if name
        .rsplit_once('.')
        .is_some_and(|(owner, _)| owner != "ridl.std")
    {
        types.insert(name);
    }
}

fn add_types(pkg: &Package, ty: &FieldType, types: &mut BTreeSet<String>) {
    match &ty.kind {
        Some(field_type::Kind::Named(name)) => add_named(pkg, name, types),
        Some(field_type::Kind::Tuple(tuple)) => {
            for field in &tuple.fields {
                if let Some(ty) = &field.r#type {
                    add_types(pkg, ty, types);
                }
            }
        }
        Some(field_type::Kind::Array(array)) => {
            if let Some(ty) = &array.element {
                add_types(pkg, ty, types);
            }
        }
        Some(field_type::Kind::Map(map)) => {
            for ty in [&map.key, &map.value].into_iter().flatten() {
                add_types(pkg, ty, types);
            }
        }
        Some(field_type::Kind::Stream(stream)) => {
            if let Some(stream_type::Element::Named(name)) = &stream.element {
                add_named(pkg, name, types);
            }
        }
        Some(field_type::Kind::Primitive(_) | field_type::Kind::InlineScalar(_)) | None => {}
    }
}

pub(crate) fn check(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut packages: Vec<_> = ctx
        .checked
        .iter()
        .filter(|p| p.ir.name != "ridl.std")
        .collect();
    packages.sort_by(|a, b| a.ir.name.cmp(&b.ir.name));
    for checked in packages {
        let pkg = &checked.ir;
        for iface in &pkg.interfaces {
            let groups = cohesion_groups(pkg, iface);
            if groups.len() < LOW_COHESION_MIN_GROUPS
                || groups
                    .iter()
                    .any(|group| group.len() < LOW_COHESION_MIN_GROUP_SIZE)
            {
                continue;
            }
            let Some(primary) = ctx.sites.decl(&pkg.name, &iface.name) else {
                continue;
            };
            let listed = groups
                .iter()
                .map(|group| format!("[{}]", group.join(", ")))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic {
                code: DiagCode::RIDL_414,
                severity: lint_by_name("low-cohesion-interface")
                    .expect("registered lint")
                    .severity,
                message: format!(
                    "interface `{}` splits into {} groups of members that share no type: {listed}",
                    iface.name,
                    groups.len()
                ),
                primary,
                labels: Vec::new(),
                fixits: Vec::new(),
            });
        }
    }
    diagnostics
}
