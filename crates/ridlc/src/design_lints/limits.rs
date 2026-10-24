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
//! An interaction is named `Interface.member`: the interface name, or for an
//! inline `service` shape its dotted global name, as the IR records it, with
//! no package prefix of its own, then `.`, then the member name.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::UnitLimits;
use ridl_core::diag::{DiagCode, Diagnostic, Severity, Span};
use ridl_core::manifest::{LimitKind, Limits};
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;
use rowan::{TextRange, TextSize};

use super::Ctx;
use super::ceiling::{self, Entry, Side, Size};

/// The limit that a `[limits]` table gives an interaction: its entry in
/// `[limits.interaction]`, else the entry of its kind, else `default`, or
/// `None` when no entry applies.
fn resolve(limits: &Limits, kind: LimitKind, owner: &str, member: &str) -> Option<u32> {
    let name = format!("{owner}.{member}");
    limits
        .interactions
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.value)
        .or_else(|| limits.kinds.get(&kind).map(|entry| *entry.get_ref()))
        .or_else(|| limits.default.as_ref().map(|entry| *entry.get_ref()))
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

/// Why one payload breaks `limit` under one encoding, or `None` when it does
/// not.
fn exceeds(side: &Side, limit: u32, encoding: &str) -> Option<String> {
    let why = match side.size {
        Size::Bytes(bytes) if bytes > u64::from(limit) => format!("up to {bytes} bytes"),
        Size::Bytes(_) | Size::Other => return None,
        Size::NoBound => "no finite bound".to_string(),
        Size::Overflow => "a bound too large to represent".to_string(),
    };
    Some(match &side.ty {
        Some(ty) => format!("{why} in {encoding} for `{ty}`"),
        None => format!("{why} in {encoding}"),
    })
}

pub(crate) fn check(
    ctx: &Ctx<'_>,
    std_ir: &v2::Package,
    encodings: &[String],
    limits: &BTreeMap<String, UnitLimits>,
) -> Vec<Diagnostic> {
    let (flatbuffers, proto3) = ceiling::checked_encodings(encodings);
    let mut diagnostics = Vec::new();
    for checked in ctx.subjects() {
        let package = &checked.ir;
        let Some(unit) = limits.get(&package.name) else {
            continue;
        };
        let others: Vec<&v2::Package> = ctx
            .checked
            .iter()
            .filter(|other| other.ir.name != package.name)
            .map(|other| &other.ir)
            .chain(std::iter::once(std_ir))
            .collect();
        let mut known = BTreeSet::new();
        // A package with no interface has nothing to lower; its entries are
        // all unmatched.
        let interfaces = if package.shapes().next().is_some() {
            codegen::lower(package, &others).interfaces
        } else {
            Vec::new()
        };
        for (shape, interface) in package.shapes().zip(&interfaces) {
            for (decl, slot) in shape.interface.interactions.iter().zip(&interface.slots) {
                known.insert(format!("{}.{}", shape.name, decl.name));
                let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                    slot.occupant.as_ref()
                else {
                    continue;
                };
                let Some(kind) = kind_of(decl) else {
                    continue;
                };
                let Some(limit) = resolve(&unit.limits, kind, shape.name, &decl.name) else {
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
                    if flatbuffers {
                        found.extend(exceeds(&payload.flatbuffers, limit, "flatbuffers"));
                    }
                    if proto3 {
                        found.extend(exceeds(&payload.proto3, limit, "proto3"));
                    }
                    if found.is_empty() {
                        continue;
                    }
                    diagnostics.push(Diagnostic {
                        code: DiagCode::RIDL_151,
                        severity: Severity::Error,
                        message: format!(
                            "the {} of `{}.{}` has {}; its limit is {limit} bytes",
                            payload.role,
                            shape.name,
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
        for entry in &unit.limits.interactions {
            if known.contains(&entry.name) {
                continue;
            }
            let range = TextRange::new(
                TextSize::from(u32::try_from(entry.key.start).unwrap_or(u32::MAX)),
                TextSize::from(u32::try_from(entry.key.end).unwrap_or(u32::MAX)),
            );
            diagnostics.push(Diagnostic {
                code: DiagCode::RIDL_152,
                severity: Severity::Error,
                message: format!(
                    "`{}` names no interaction of package `{}`; write `Interface.member`, \
                     with the interface or inline service name and no package prefix",
                    entry.name, package.name
                ),
                primary: Span {
                    file: unit.manifest,
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
        assert_eq!(resolve(&limits(), LimitKind::Signal, "I", "other"), None);
    }

    #[test]
    fn a_name_entry_applies_to_its_interaction_only() {
        assert_eq!(resolve(&limits(), LimitKind::Signal, "I", "named"), Some(1));
        assert_eq!(resolve(&limits(), LimitKind::Signal, "J", "named"), None);
    }
}
