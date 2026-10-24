//! RIDL-151 and RIDL-152: the payload limits of a unit's `[limits]` table
//! (ADR-0030 decisions 12 and 14).
//!
//! RIDL-151 reports a payload whose largest encoded size, in an encoding that
//! the workspace manifest enables, is above the limit that a `[limits]` entry
//! gives its interaction, or has no finite bound. It runs only where an entry
//! gives the limit: where the limit is the payload ceiling because no entry
//! applies, only RIDL-150 runs. RIDL-152 reports a `[limits.interaction]` key
//! that names no interaction of its unit.
//!
//! An interaction is named `Interface.member`: the short name of its shape,
//! then `.`, then the member name. The short name is the interface name, or for
//! an inline `service` shape the last segment of its dotted global name, so
//! neither carries a package. When two shapes of one unit share a short name,
//! no key can name either of them, and a key that uses the name is reported as
//! ambiguous.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::UnitLimits;
use ridl_core::diag::{DiagCode, Diagnostic, Severity, Span};
use ridl_core::manifest::{LimitKind, Limits};
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;
use rowan::{TextRange, TextSize};

use super::Ctx;
use super::ceiling::{self, Entry};

/// The limit that a `[limits]` table gives an interaction: its entry in
/// `[limits.interaction]`, else the entry of its kind, else `default`, or
/// `None` when no entry applies.
///
/// `owner` is the short name of the shape, or `None` when another shape of the
/// unit has the same short name, so that no `[limits.interaction]` entry can
/// apply.
fn resolve(limits: &Limits, kind: LimitKind, owner: Option<&str>, member: &str) -> Option<u32> {
    let name = owner.map(|owner| format!("{owner}.{member}"));
    limits
        .interactions
        .iter()
        .find(|entry| Some(&entry.name) == name.as_ref())
        .map(|entry| entry.value)
        .or_else(|| limits.kinds.get(&kind).map(|entry| *entry.get_ref()))
        .or_else(|| limits.default.as_ref().map(|entry| *entry.get_ref()))
}

/// The last segment of the name of a shape.
fn short_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn kind_of(decl: &v2::Decl) -> Option<LimitKind> {
    match decl.kind.as_ref()? {
        v2::decl::Kind::SignalDef(_) => Some(LimitKind::Signal),
        v2::decl::Kind::FixedDef(_) => Some(LimitKind::Fixed),
        v2::decl::Kind::EventDef(_) => Some(LimitKind::Event),
        v2::decl::Kind::CommandDef(_) => Some(LimitKind::Command),
        v2::decl::Kind::QueryDef(_) => Some(LimitKind::Query),
        _ => None,
    }
}

/// The packages of one unit, as indices of `Ctx::checked`.
struct Unit<'a> {
    table: &'a UnitLimits,
    packages: Vec<usize>,
}

/// RIDL-151 and RIDL-152. `units` holds the unit of each entry of
/// `ctx.checked`; `limits` holds the `[limits]` table of each unit by unit
/// name. A table applies to every package of its unit, so an entry may name an
/// interface of any of them.
pub(crate) fn check(
    ctx: &Ctx<'_>,
    std_ir: &v2::Package,
    encodings: &[String],
    limits: &BTreeMap<String, UnitLimits>,
    units: &[String],
) -> Vec<Diagnostic> {
    let (flatbuffers, proto3) = ceiling::checked_encodings(encodings);
    let mut by_unit: BTreeMap<&str, Unit<'_>> = BTreeMap::new();
    for (index, unit) in units.iter().enumerate() {
        if let Some(table) = limits.get(unit) {
            by_unit
                .entry(unit.as_str())
                .or_insert_with(|| Unit {
                    table,
                    packages: Vec::new(),
                })
                .packages
                .push(index);
        }
    }
    let mut diagnostics = Vec::new();
    for unit in by_unit.values() {
        let table = &unit.table.limits;
        // The packages that the checks examine. When one package of the unit is
        // skipped, its interfaces are unknown, so an entry cannot be called
        // unmatched.
        let examined: Vec<usize> = unit
            .packages
            .iter()
            .copied()
            .filter(|index| ctx.is_subject(*index))
            .collect();
        let complete = examined.len() == unit.packages.len();
        let mut known = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut ambiguous = BTreeSet::new();
        for index in &examined {
            for shape in ctx.checked[*index].ir.shapes() {
                if !seen.insert(short_name(shape.name)) {
                    ambiguous.insert(short_name(shape.name));
                }
            }
        }
        for index in examined {
            let package = &ctx.checked[index].ir;
            let others: Vec<&v2::Package> = ctx
                .checked
                .iter()
                .filter(|other| other.ir.name != package.name)
                .map(|other| &other.ir)
                .chain(std::iter::once(std_ir))
                .collect();
            // A package with no interface has nothing to lower.
            let interfaces = if package.shapes().next().is_some() {
                codegen::lower(package, &others).interfaces
            } else {
                Vec::new()
            };
            for (shape, interface) in package.shapes().zip(&interfaces) {
                let short = short_name(shape.name);
                let owner = (!ambiguous.contains(short)).then_some(short);
                for (decl, slot) in shape.interface.interactions.iter().zip(&interface.slots) {
                    let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                        slot.occupant.as_ref()
                    else {
                        continue;
                    };
                    if owner.is_some() {
                        known.insert(format!("{short}.{}", decl.name));
                    }
                    let Some(kind) = kind_of(decl) else {
                        continue;
                    };
                    let Some(limit) = resolve(table, kind, owner, &decl.name) else {
                        continue;
                    };
                    let Some(span) = ctx.sites.member(&package.name, shape.name, &decl.name) else {
                        continue;
                    };
                    let entries: Vec<Entry> = match decl.kind.as_ref() {
                        Some(v2::decl::Kind::FixedDef(fixed)) => {
                            ceiling::fixed_entry(package, &others, fixed)
                                .into_iter()
                                .collect()
                        }
                        _ => ceiling::payloads(interaction, decl, package, &others),
                    };
                    for payload in entries {
                        let mut found = Vec::new();
                        let limit_bytes = u64::from(limit);
                        if flatbuffers {
                            found.extend(ceiling::describe_above(
                                &payload.flatbuffers,
                                "flatbuffers",
                                limit_bytes,
                            ));
                        }
                        if proto3 {
                            found.extend(ceiling::describe_above(
                                &payload.proto3,
                                "proto3",
                                limit_bytes,
                            ));
                        }
                        if found.is_empty() {
                            continue;
                        }
                        diagnostics.push(Diagnostic {
                            code: DiagCode::RIDL_151,
                            severity: Severity::Error,
                            message: format!(
                                "the {} of `{short}.{}` has {}; its limit is {limit} bytes",
                                payload.role,
                                decl.name,
                                found.join(" and "),
                            ),
                            primary: span,
                            labels: Vec::new(),
                            fixits: Vec::new(),
                        });
                    }
                }
            }
        }
        if !complete {
            continue;
        }
        for entry in &table.interactions {
            if known.contains(&entry.name) {
                continue;
            }
            let range = TextRange::new(
                TextSize::from(u32::try_from(entry.key.start).unwrap_or(u32::MAX)),
                TextSize::from(u32::try_from(entry.key.end).unwrap_or(u32::MAX)),
            );
            let message = match entry.name.split_once('.') {
                Some((name, _)) if ambiguous.contains(name) => format!(
                    "`{}` is ambiguous: more than one interface or inline service of the \
                     unit is named `{name}`",
                    entry.name
                ),
                _ => format!(
                    "`{}` names no interaction of its unit; write `Interface.member`, \
                     with the interface or inline service name without its package",
                    entry.name
                ),
            };
            diagnostics.push(Diagnostic {
                code: DiagCode::RIDL_152,
                severity: Severity::Error,
                message,
                primary: Span {
                    file: unit.table.manifest,
                    range,
                },
                labels: Vec::new(),
                fixits: Vec::new(),
            });
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::manifest::LimitEntry;

    fn limits() -> Limits {
        let mut limits = Limits::default();
        limits.interactions.push(LimitEntry {
            name: "I.named".to_string(),
            key: 0..0,
            value: 1,
        });
        limits
    }

    #[test]
    fn no_entry_resolves_to_none() {
        assert_eq!(
            resolve(&limits(), LimitKind::Signal, Some("I"), "other"),
            None
        );
    }

    #[test]
    fn a_name_entry_applies_to_its_interaction_only() {
        assert_eq!(
            resolve(&limits(), LimitKind::Signal, Some("I"), "named"),
            Some(1)
        );
        assert_eq!(
            resolve(&limits(), LimitKind::Signal, Some("J"), "named"),
            None
        );
        assert_eq!(resolve(&limits(), LimitKind::Signal, None, "named"), None);
    }
}
