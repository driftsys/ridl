//! The size pass: RIDL-151, RIDL-152 and RIDL-157, and the MANI-022 of a
//! `[limits.interaction]` entry that raises a member above what its kind may
//! be given (ADR-0030 decisions 12, 14, 17 and 18).
//!
//! Every interaction of every examined package is sized once, in each
//! encoding that the workspace manifest enables. The limit of an interaction
//! is, first found: its entry in `[limits.interaction]`, the entry of its kind
//! in `[limits.kind]`, `default` clamped to the default ceiling of its kind,
//! and, when no entry applies, the default ceiling of its kind. Where an entry
//! gives the limit, a payload above it, or with no finite bound, is RIDL-151;
//! where none does, a payload above the default ceiling of the kind, or with no
//! finite bound, is RIDL-157. An interaction draws at most one of the two.
//! RIDL-152 reports a `[limits.interaction]` key that names no interaction of
//! its unit. A `[limits.interaction]` entry above 16,128 bytes that names a
//! member other than a fixed is MANI-022 on the value, and the entry is
//! dropped, as the manifest reader drops an entry above 65,280.
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
use ridl_core::limits::{RAISE_MAX, default_ceiling};
use ridl_core::manifest::{LimitEntry, LimitKind, Limits};
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;
use ridl_sem::CheckedPackage;
use rowan::{TextRange, TextSize};

use super::Ctx;
use super::ceiling::{self, Entry};

/// The `[limits.interaction]` entry of an interaction, or `None` when the
/// table has none for it. `owner` is the short name of the shape, or `None`
/// when another shape of the unit has the same short name, so that no entry
/// can apply.
fn name_entry<'a>(limits: &'a Limits, owner: Option<&str>, member: &str) -> Option<&'a LimitEntry> {
    let name = format!("{}.{member}", owner?);
    limits.interactions.iter().find(|entry| entry.name == name)
}

/// Whether a `[limits.interaction]` entry of `value` bytes may apply to a
/// member of `kind` (ADR-0030 decision 18): a fixed may be given up to its
/// default ceiling, which the manifest reader already bounds, and every other
/// kind up to [`RAISE_MAX`].
fn raise_allowed(kind: LimitKind, value: u32) -> bool {
    kind == LimitKind::Fixed || value <= RAISE_MAX
}

/// The limit that a `[limits]` table gives an interaction: its entry in
/// `[limits.interaction]`, else the entry of its kind, else `default` clamped
/// to the default ceiling of the kind, or `None` when no entry applies
/// (ADR-0030 decision 12). An interaction entry that [`raise_allowed`]
/// refuses is dropped, so the next step applies. `owner` is as for
/// [`name_entry`].
fn resolve(limits: &Limits, kind: LimitKind, owner: Option<&str>, member: &str) -> Option<u32> {
    name_entry(limits, owner, member)
        .map(|entry| entry.value)
        .filter(|value| raise_allowed(kind, *value))
        .or_else(|| limits.kinds.get(&kind).map(|entry| *entry.get_ref()))
        .or_else(|| {
            limits
                .default
                .as_ref()
                .map(|entry| (*entry.get_ref()).min(default_ceiling(kind)))
        })
}

/// The kind with its indefinite article, for a message: `a signal`, `an
/// event`.
fn a_kind(kind: LimitKind) -> String {
    let name = kind.name();
    let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
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

/// The short names that more than one of `shape_names` has. No key can name a
/// shape with such a short name.
fn ambiguous_short_names<'a>(shape_names: impl Iterator<Item = &'a str>) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut ambiguous = BTreeSet::new();
    for name in shape_names {
        if !seen.insert(short_name(name)) {
            ambiguous.insert(short_name(name));
        }
    }
    ambiguous
}

/// Records on each interaction the limit that its unit's `[limits]` table gives
/// it (ADR-0030 decision 13), so that the IR, and the catalog hash over it,
/// carries the limit. `units` holds the unit of each entry of `checked`;
/// `limits` holds the table of each unit by unit name. An interaction that no
/// entry reaches keeps `limit` unset, and a unit with no table is not touched.
/// The resolution is the one RIDL-151 applies, so a short name that two shapes
/// of the unit share gives no name entry to either.
pub(crate) fn annotate_limits(
    checked: &mut [CheckedPackage],
    units: &[String],
    limits: &BTreeMap<String, UnitLimits>,
) {
    for (unit, table) in limits {
        let members: Vec<usize> = (0..checked.len())
            .filter(|index| units.get(*index) == Some(unit))
            .collect();
        let ambiguous: BTreeSet<String> = ambiguous_short_names(
            members
                .iter()
                .flat_map(|index| checked[*index].ir.shapes().map(|shape| shape.name)),
        )
        .into_iter()
        .map(str::to_owned)
        .collect();
        let table = &table.limits;
        for index in members {
            let package = &mut checked[index].ir;
            let inline = package.services.iter_mut().flat_map(|service| {
                let name = service.name.clone();
                service
                    .shapes
                    .iter_mut()
                    .filter_map(move |slot| match slot.kind.as_mut()? {
                        v2::service_shape::Kind::Inline(interface) => {
                            Some((name.clone(), interface))
                        }
                        v2::service_shape::Kind::InterfaceRef(_) => None,
                    })
            });
            let named = package
                .interfaces
                .iter_mut()
                .map(|interface| (interface.name.clone(), interface));
            for (name, interface) in named.chain(inline) {
                let short = short_name(&name);
                let owner = (!ambiguous.contains(short)).then_some(short);
                for decl in &mut interface.interactions {
                    decl.limit =
                        kind_of(decl).and_then(|kind| resolve(table, kind, owner, &decl.name));
                }
            }
        }
    }
}

/// The packages of one unit, as indices of `Ctx::checked`, with the unit's
/// `[limits]` table when it has one.
struct Unit<'a> {
    table: Option<&'a UnitLimits>,
    packages: Vec<usize>,
}

/// The size pass: RIDL-151, RIDL-152, RIDL-157 and the MANI-022 of an
/// interaction entry that raises a member above what its kind may be given.
/// `units` holds the unit of each entry of `ctx.checked`; `limits` holds the
/// `[limits]` table of each unit by unit name. A table applies to every
/// package of its unit, so an entry may name an interface of any of them. The
/// pass runs over every unit, with or without a table.
///
/// This function computes short-name ambiguity over the examined packages
/// only, while `annotate_limits` computes it over all packages of the unit.
/// The difference matters only for a build that already has errors.
pub(crate) fn check(
    ctx: &Ctx<'_>,
    std_ir: &v2::Package,
    encodings: &[String],
    limits: &BTreeMap<String, UnitLimits>,
    units: &[String],
) -> Vec<Diagnostic> {
    let (flatbuffers, proto3, podbuf) = ceiling::checked_encodings(encodings);
    let mut by_unit: BTreeMap<&str, Unit<'_>> = BTreeMap::new();
    for (index, unit) in units.iter().enumerate() {
        by_unit
            .entry(unit.as_str())
            .or_insert_with(|| Unit {
                table: limits.get(unit),
                packages: Vec::new(),
            })
            .packages
            .push(index);
    }
    let mut diagnostics = Vec::new();
    for unit in by_unit.values() {
        let table = unit.table.map(|table| &table.limits);
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
        let ambiguous = ambiguous_short_names(
            examined
                .iter()
                .flat_map(|index| ctx.checked[*index].ir.shapes().map(|shape| shape.name)),
        );
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
                    let name = format!("{short}.{}", decl.name);
                    if let Some(unit_limits) = unit.table
                        && let Some(entry) = name_entry(&unit_limits.limits, owner, &decl.name)
                        && !raise_allowed(kind, entry.value)
                    {
                        diagnostics.push(Diagnostic {
                            code: DiagCode::MANI_022,
                            severity: Severity::Error,
                            message: format!(
                                "`{name}` is {}, above what the entry may give {} \
                                 ({RAISE_MAX} bytes); only a `fixed` may be given more",
                                entry.value,
                                a_kind(kind),
                            ),
                            primary: Span {
                                file: unit_limits.manifest,
                                range: text_range(&entry.value_span),
                            },
                            labels: Vec::new(),
                            fixits: Vec::new(),
                        });
                    }
                    let declared = table.and_then(|table| resolve(table, kind, owner, &decl.name));
                    let limit = declared.unwrap_or_else(|| default_ceiling(kind));
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
                        if podbuf {
                            found.extend(ceiling::describe_above(
                                &payload.podbuf,
                                "podbuf",
                                limit_bytes,
                            ));
                        }
                        if found.is_empty() {
                            continue;
                        }
                        let found = found.join(" and ");
                        let (code, message) = match declared {
                            Some(limit) => (
                                DiagCode::RIDL_151,
                                format!(
                                    "the {} of `{name}` has {found}; its limit is {limit} bytes",
                                    payload.role,
                                ),
                            ),
                            None => (
                                DiagCode::RIDL_157,
                                format!(
                                    "the {} of `{name}` has {found}; the default ceiling of {} \
                                     is {limit} bytes{}",
                                    payload.role,
                                    a_kind(kind),
                                    raise_hint(kind),
                                ),
                            ),
                        };
                        diagnostics.push(Diagnostic {
                            code,
                            severity: Severity::Error,
                            message,
                            primary: span,
                            labels: Vec::new(),
                            fixits: Vec::new(),
                        });
                    }
                }
            }
        }
        let Some(table) = table else {
            continue;
        };
        if !complete {
            continue;
        }
        for entry in &table.interactions {
            if known.contains(&entry.name) {
                continue;
            }
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
                    file: unit.table.expect("the table of the entry").manifest,
                    range: text_range(&entry.key),
                },
                labels: Vec::new(),
                fixits: Vec::new(),
            });
        }
    }
    diagnostics
}

/// What a RIDL-157 message adds after the default ceiling of `kind`: how a
/// `[limits.interaction]` entry may raise a member of the kind, when the
/// kind's default ceiling is below [`RAISE_MAX`] (ADR-0030 decision 18).
fn raise_hint(kind: LimitKind) -> String {
    if default_ceiling(kind) < RAISE_MAX {
        format!(", and a `[limits.interaction]` entry may raise it up to {RAISE_MAX}")
    } else {
        String::new()
    }
}

/// A byte range of a manifest as a span range.
fn text_range(range: &std::ops::Range<usize>) -> TextRange {
    TextRange::new(
        TextSize::from(u32::try_from(range.start).unwrap_or(u32::MAX)),
        TextSize::from(u32::try_from(range.end).unwrap_or(u32::MAX)),
    )
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
            value_span: 0..0,
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

    /// `default` applies to each kind up to the default ceiling of that kind
    /// (ADR-0030 decision 12).
    #[test]
    fn default_is_clamped_to_the_ceiling_of_the_kind() {
        let mut limits = Limits {
            default: Some(toml::Spanned::new(0..0, 65_280)),
            ..Limits::default()
        };
        assert_eq!(
            resolve(&limits, LimitKind::Signal, Some("I"), "s"),
            Some(3_840)
        );
        assert_eq!(
            resolve(&limits, LimitKind::Query, Some("I"), "q"),
            Some(16_128)
        );
        assert_eq!(
            resolve(&limits, LimitKind::Fixed, Some("I"), "f"),
            Some(65_280)
        );
        limits.default = Some(toml::Spanned::new(0..0, 100));
        assert_eq!(
            resolve(&limits, LimitKind::Signal, Some("I"), "s"),
            Some(100)
        );
    }

    /// An interaction entry above 16,128 applies to a fixed only; for another
    /// kind it is dropped and the next step applies.
    #[test]
    fn a_raise_above_the_largest_applies_to_a_fixed_only() {
        let mut limits = limits();
        limits.interactions[0].value = 16_129;
        assert_eq!(
            resolve(&limits, LimitKind::Fixed, Some("I"), "named"),
            Some(16_129)
        );
        assert_eq!(
            resolve(&limits, LimitKind::Command, Some("I"), "named"),
            None
        );
        limits
            .kinds
            .insert(LimitKind::Command, toml::Spanned::new(0..0, 7));
        assert_eq!(
            resolve(&limits, LimitKind::Command, Some("I"), "named"),
            Some(7)
        );
        limits.interactions[0].value = 16_128;
        assert_eq!(
            resolve(&limits, LimitKind::Command, Some("I"), "named"),
            Some(16_128)
        );
    }

    #[test]
    fn the_article_of_a_kind_fits_its_name() {
        assert_eq!(a_kind(LimitKind::Signal), "a signal");
        assert_eq!(a_kind(LimitKind::Event), "an event");
        assert_eq!(a_kind(LimitKind::Fixed), "a fixed");
    }

    #[test]
    fn the_raise_hint_names_the_largest_raise_below_it_only() {
        assert!(raise_hint(LimitKind::Signal).contains("16128"));
        assert!(raise_hint(LimitKind::Event).contains("16128"));
        assert!(raise_hint(LimitKind::Command).is_empty());
        assert!(raise_hint(LimitKind::Query).is_empty());
        assert!(raise_hint(LimitKind::Fixed).is_empty());
    }
}
