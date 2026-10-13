//! What a reference in an `.rsdl` file names (rsdl reference §4), for the
//! language server. The reference is read from the checked model, and its
//! target from the checker's own results: an `offers` or `requires` line from
//! the component's resolved lines, a member line and a `for` reference through
//! the lookups the checker resolves them with. The language server does not
//! resolve a name itself, so it cannot navigate to a declaration the checker
//! would not bind. [`reference_at`] binds the one reference under a cursor;
//! [`references`] binds, for one target, the references whose slot and form
//! can name a declaration of its kind, and keeps those bound to it.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, service_catalog};
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::AstNode;
use rowan::{TextRange, TextSize};

use super::closure::{InterfaceId, Lookup};
use super::{CheckedSystem, Reference, ReferenceForm};
use crate::resolve::source_file;

/// The declaration a reference names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A `system`, as an index into [`CheckedSystem::systems`].
    System(usize),
    /// A `component`, as an index into [`CheckedSystem::components`].
    Component(usize),
    /// One declared instance: the component, as an index into
    /// [`CheckedSystem::components`], and the instance, as an index into that
    /// component's `instances` list.
    Instance { component: usize, instance: usize },
    /// A `service`, by its dotted name.
    Service(String),
    /// A declared `interface`, or the inline shape of a service (rsdl §3.2).
    Interface(InterfaceId),
}

/// A reference and the declaration a part of it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTarget {
    /// The reference as the model holds it.
    pub reference: Reference,
    /// The part of the reference that names the target: the instance segment
    /// of `Name.inst` for the instance, the segments before it for the
    /// component, and the whole reference for every other form.
    pub range: TextRange,
    pub target: Target,
}

/// Where a reference is written. Its role depends on the slot (rsdl §4).
#[derive(Clone, Copy)]
enum Slot<'a> {
    /// A member line of a `system`, `distribution` or `machine` in `package`;
    /// `placement` for a machine's, the one member line that may name an
    /// instance (rsdl §4).
    Member { package: &'a str, placement: bool },
    /// The `line`-th `offers` line of the `component`-th component.
    Offers { component: usize, line: usize },
    /// The `line`-th `requires` line of the `component`-th component.
    Requires { component: usize, line: usize },
    /// The `for` reference of a deployment in `package`.
    For { package: &'a str },
}

/// The reference of `system` written at `offset` in `file`, and what it names,
/// or `None` when no reference is there or the checker bound it to nothing.
///
/// `system` is the result of [`check_system`](super::check_system) for `ws`
/// and `std`.
pub fn reference_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    file: InputFile,
    offset: TextSize,
) -> Option<ReferenceTarget> {
    let (reference, slot) = written_references(system)
        .into_iter()
        .find(|(reference, _)| {
            reference.site.file == file && reference.site.range.contains_inclusive(offset)
        })?;
    let catalog = service_catalog(db, ws, std);
    let lookup = Lookup::new(db, ws, std, system, &catalog);
    target_of(db, system, &lookup, reference, slot, offset)
}

/// Whether two targets name the same declaration. A service and the inline
/// shape of that service are one declaration: both are written as the
/// service's dotted name.
pub fn same_declaration(a: &Target, b: &Target) -> bool {
    fn service(target: &Target) -> Option<&str> {
        match target {
            Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name }) => {
                Some(name.as_str())
            }
            _ => None,
        }
    }
    match (service(a), service(b)) {
        (Some(a), Some(b)) => a == b,
        (None, None) => a == b,
        _ => false,
    }
}

/// Every reference of `system` the checker binds to the declaration `target`
/// names (as [`same_declaration`] compares), with the part of it that names
/// the declaration.
///
/// Only a reference whose slot and form can name a declaration of the
/// target's kind is resolved, at the one offset where that part is: the
/// instance segment of `Name.inst` at its end, every other part at its start.
/// The written name is not compared, because a reference written through an
/// import alias does not spell the declared name.
///
/// `system` is the result of [`check_system`](super::check_system) for `ws`
/// and `std`.
pub fn references(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    target: &Target,
) -> Vec<ReferenceTarget> {
    let catalog = service_catalog(db, ws, std);
    let lookup = Lookup::new(db, ws, std, system, &catalog);
    let mut found = Vec::new();
    for (reference, slot) in written_references(system) {
        let Some(offset) = naming_offset(target, reference, slot) else {
            continue;
        };
        if let Some(bound) = target_of(db, system, &lookup, reference, slot, offset)
            && same_declaration(&bound.target, target)
        {
            found.push(bound);
        }
    }
    found
}

/// The offset of the part of `reference`, written in `slot`, that can name a
/// declaration of `target`'s kind, or `None` when no part can. Mirrors the
/// slots and forms [`target_of`] binds to each kind of target.
fn naming_offset(target: &Target, reference: &Reference, slot: Slot<'_>) -> Option<TextSize> {
    let range = reference.site.range;
    let start = Some(range.start());
    match (target, slot, &reference.form) {
        (Target::System(_), Slot::For { .. }, _) => start,
        (Target::Component(_), Slot::Member { .. }, ReferenceForm::Declared { .. }) => start,
        (
            Target::Component(_),
            Slot::Member {
                placement: true, ..
            },
            ReferenceForm::Instance { .. },
        ) => start,
        (
            Target::Instance { .. },
            Slot::Member {
                placement: true, ..
            },
            ReferenceForm::Instance { .. },
        ) => Some(range.end()),
        (
            Target::Service(_) | Target::Interface(InterfaceId::Inline { .. }),
            Slot::Offers { .. } | Slot::Requires { .. },
            _,
        ) => start,
        (
            Target::Service(_) | Target::Interface(InterfaceId::Inline { .. }),
            Slot::Member { .. },
            ReferenceForm::Service { .. },
        ) => start,
        (Target::Interface(InterfaceId::Declared { .. }), Slot::Requires { .. }, _) => start,
        _ => None,
    }
}

/// What `reference`, written in `slot`, names at `offset`, which lies inside
/// the reference.
///
/// [`naming_offset`] restates which slot and form can bind each kind of
/// target, so it must change with this function.
fn target_of(
    db: &dyn salsa::Database,
    system: &CheckedSystem,
    lookup: &Lookup<'_>,
    reference: &Reference,
    slot: Slot<'_>,
    offset: TextSize,
) -> Option<ReferenceTarget> {
    let reference = reference.clone();
    let whole = reference.site.range;
    let (target, range) = match slot {
        Slot::Offers { component, line } => {
            let service = system.component_lines.get(component)?.offers.get(line)?;
            (Target::Service(service.clone()?), whole)
        }
        Slot::Requires { component, line } => {
            let interface = system.component_lines.get(component)?.requires.get(line)?;
            (Target::Interface(interface.clone()?), whole)
        }
        Slot::For { package } => {
            let ReferenceForm::Declared {
                package: written,
                name,
            } = &reference.form
            else {
                return None;
            };
            let index = lookup.system(package, written.as_deref(), name)?;
            (Target::System(index), whole)
        }
        Slot::Member { package, placement } => {
            match &reference.form {
                ReferenceForm::Declared {
                    package: written,
                    name,
                } => {
                    let index = lookup.component(package, written.as_deref(), name)?;
                    (Target::Component(index), whole)
                }
                // A `system` or `distribution` line naming an instance is
                // RSDL-602 or RSDL-903.
                ReferenceForm::Instance { .. } if !placement => return None,
                ReferenceForm::Instance {
                    package: written,
                    component,
                    instance,
                } => {
                    let index = lookup.component(package, written.as_deref(), component)?;
                    let (component_part, instance_part) = instance_ranges(db, &reference)?;
                    if instance_part.contains_inclusive(offset) {
                        let position = system.components[index]
                            .instances
                            .as_ref()?
                            .iter()
                            .position(|declared| declared.name == *instance)?;
                        let target = Target::Instance {
                            component: index,
                            instance: position,
                        };
                        (target, instance_part)
                    } else {
                        (Target::Component(index), component_part)
                    }
                }
                // A service a declared component offers does not stand for a
                // component (RSDL-504); the checker's `Lookup::record_offers`
                // reads the same resolved `offers` lines.
                ReferenceForm::Service { name }
                    if lookup.catalog.entries.contains_key(name)
                        && !system.component_lines.iter().any(|lines| {
                            lines.offers.iter().flatten().any(|offered| offered == name)
                        }) =>
                {
                    (Target::Service(name.clone()), whole)
                }
                ReferenceForm::Service { .. } | ReferenceForm::Unreadable => return None,
            }
        }
    };
    Some(ReferenceTarget {
        reference,
        range,
        target,
    })
}

/// Every reference of the model with its slot. References never overlap, so
/// the order does not decide which one a cursor is on.
fn written_references(system: &CheckedSystem) -> Vec<(&Reference, Slot<'_>)> {
    let mut references = Vec::new();
    let members = system
        .systems
        .iter()
        .map(|decl| (&decl.package, &decl.members, false))
        .chain(
            system
                .distributions
                .iter()
                .map(|decl| (&decl.package, &decl.members, false)),
        )
        .chain(system.deployments.iter().flat_map(|decl| {
            decl.machines
                .iter()
                .map(move |machine| (&decl.package, &machine.members, true))
        }));
    for (package, lines, placement) in members {
        for line in lines {
            let slot = Slot::Member { package, placement };
            references.push((&line.reference, slot));
        }
    }
    for (component, decl) in system.components.iter().enumerate() {
        for (line, member) in decl.offers.iter().enumerate() {
            references.push((&member.reference, Slot::Offers { component, line }));
        }
        for (line, member) in decl.requires.iter().enumerate() {
            references.push((&member.reference, Slot::Requires { component, line }));
        }
    }
    for decl in &system.deployments {
        if let Some(reference) = &decl.system {
            let package = &decl.package;
            references.push((reference, Slot::For { package }));
        }
    }
    references
}

/// The two parts of an instance reference `pkg.Name.inst`: from its start to
/// the end of `Name`, and the `inst` segment.
fn instance_ranges(
    db: &dyn salsa::Database,
    reference: &Reference,
) -> Option<(TextRange, TextRange)> {
    let root = source_file(db, reference.site.file);
    let node = root
        .syntax()
        .covering_element(reference.site.range)
        .into_node()?;
    let segments: Vec<TextRange> = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Dot)
        .map(|token| token.text_range())
        .collect();
    let [.., name, instance] = segments.as_slice() else {
        return None;
    };
    Some((
        TextRange::new(reference.site.range.start(), name.end()),
        *instance,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::RidlDatabase;
    use ridl_core::std_package;

    use super::super::tests::{ADAS, COMMON, DIAG, PRODUCTION, SYSTEM, package};
    use super::*;

    /// A package that imports two components of `veh.topology`, one through
    /// an alias, and names them in a distribution, together with a third
    /// component that it does not import, written as the qualified
    /// `veh.topology.Panel`. The distribution also names an instance, which a
    /// distribution line may not (RSDL-602 or RSDL-903), so the checker binds
    /// that line to nothing. The component `Idle` and its instance `unused`
    /// are named by no reference.
    const EXTRA: &str = r#"package veh.extra

import veh.topology.Cruise
import veh.topology.Lane as Track

component Idle [ instances = (unused) ] {}

distribution Spare { Cruise, Track, veh.topology.Panel, Cruise.primary }
"#;

    /// A reference site as `(file path, start, end)`, which sorts.
    type Span = (String, u32, u32);

    fn span(db: &RidlDatabase, file: InputFile, range: TextRange) -> Span {
        (
            file.path(db).clone(),
            range.start().into(),
            range.end().into(),
        )
    }

    /// The kind of a target, for the fixture coverage check.
    fn kind(target: &Target) -> &'static str {
        match target {
            Target::System(_) => "system",
            Target::Component(_) => "component",
            Target::Instance { .. } => "instance",
            Target::Service(_) => "service",
            Target::Interface(InterfaceId::Inline { .. }) => "inline",
            Target::Interface(InterfaceId::Declared { .. }) => "declared",
        }
    }

    /// The packages of the fixture: Appendix A's four, and `veh.extra`.
    fn fixture(db: &RidlDatabase) -> Vec<Package> {
        vec![
            package(db, "veh.common", &[("veh/common/common.typl", COMMON)]),
            package(db, "veh.adas", &[("veh/adas/adas.ridl", ADAS)]),
            package(db, "veh.diag", &[("veh/diag/diag.ridl", DIAG)]),
            package(
                db,
                "veh.topology",
                &[
                    ("veh/topology/system.rsdl", SYSTEM),
                    ("veh/topology/production.rsdl", PRODUCTION),
                ],
            ),
            package(db, "veh.extra", &[("veh/extra/extra.rsdl", EXTRA)]),
        ]
    }

    /// The declaration a target names, for grading `references` without
    /// `same_declaration`: a service and the inline shape of that service are
    /// one declaration, and every other target is its own.
    fn declaration(target: &Target) -> Target {
        match target {
            Target::Interface(InterfaceId::Inline { service }) => Target::Service(service.clone()),
            other => other.clone(),
        }
    }

    /// `references(.., target)` returns exactly the references `reference_at`
    /// binds to the same declaration, for every target some reference names
    /// and for a target none does.
    #[test]
    fn references_match_reference_at_for_every_target() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let ws = Workspace::new(&db, fixture(&db), BTreeMap::new());
        let system = super::super::check_system(&db, ws, std);

        // Every reference of the model, read from the declarations.
        let mut written: Vec<&Reference> = Vec::new();
        for decl in &system.systems {
            written.extend(decl.members.iter().map(|line| &line.reference));
        }
        for decl in &system.distributions {
            written.extend(decl.members.iter().map(|line| &line.reference));
        }
        for decl in &system.components {
            written.extend(decl.offers.iter().map(|line| &line.reference));
            written.extend(decl.requires.iter().map(|line| &line.reference));
        }
        for decl in &system.deployments {
            written.extend(decl.system.iter());
            for machine in &decl.machines {
                written.extend(machine.members.iter().map(|line| &line.reference));
            }
        }
        assert!(written.len() >= 20, "{} references", written.len());

        // The oracle: what `reference_at` binds at the start and at the end
        // of each reference, grouped by the declaration it names.
        let mut oracle: Vec<(Target, Vec<Span>)> = Vec::new();
        // The kind of every target `reference_at` binds, before the service
        // and inline-shape targets of one service are merged into one group.
        let mut kinds: Vec<&str> = Vec::new();
        for reference in &written {
            let range = reference.site.range;
            for offset in [range.start(), range.end()] {
                let Some(found) = reference_at(&db, ws, std, &system, reference.site.file, offset)
                else {
                    continue;
                };
                kinds.push(kind(&found.target));
                let site = span(&db, found.reference.site.file, found.range);
                match oracle
                    .iter_mut()
                    .find(|(target, _)| declaration(target) == declaration(&found.target))
                {
                    Some((_, sites)) => {
                        if !sites.contains(&site) {
                            sites.push(site);
                        }
                    }
                    None => oracle.push((found.target, vec![site])),
                }
            }
        }
        for kind in [
            "system",
            "component",
            "instance",
            "service",
            "inline",
            "declared",
        ] {
            assert!(
                kinds.contains(&kind),
                "the fixture names a {kind}: {kinds:?}"
            );
        }
        // The lone service `veh.diag.access` is bound as a service by a member
        // line and as the inline shape by a `requires` line.
        assert!(
            oracle.iter().any(|(target, _)| matches!(
                target,
                Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name })
                    if name == "veh.diag.access"
            )),
            "{kinds:?}"
        );
        // The alias `Track` in `veh.extra` binds to `Lane`.
        let lane = system
            .components
            .iter()
            .position(|decl| decl.name.name == "Lane")
            .expect("Lane is declared");
        let (_, lane_sites) = oracle
            .iter()
            .find(|(target, _)| *target == Target::Component(lane))
            .expect("Lane is referenced");
        assert!(
            lane_sites
                .iter()
                .any(|(path, _, _)| path == "veh/extra/extra.rsdl"),
            "{lane_sites:?}"
        );

        // The instance reference of the distribution in `veh.extra` binds to
        // nothing at either end.
        let misplaced = system
            .distributions
            .iter()
            .flat_map(|decl| &decl.members)
            .map(|line| &line.reference)
            .find(|reference| matches!(reference.form, ReferenceForm::Instance { .. }))
            .expect("the distribution names an instance");
        let misplaced_range = misplaced.site.range;
        for offset in [misplaced_range.start(), misplaced_range.end()] {
            assert_eq!(
                reference_at(&db, ws, std, &system, misplaced.site.file, offset),
                None
            );
        }
        let misplaced_span = span(&db, misplaced.site.file, misplaced_range);
        assert!(
            oracle.iter().all(|(_, sites)| {
                sites.iter().all(|(path, start, end)| {
                    *path != misplaced_span.0
                        || *end <= misplaced_span.1
                        || *start >= misplaced_span.2
                })
            }),
            "{oracle:?}"
        );

        let actual = |target: &Target| {
            let mut actual: Vec<Span> = references(&db, ws, std, &system, target)
                .into_iter()
                .inspect(|found| {
                    assert_eq!(
                        declaration(&found.target),
                        declaration(target),
                        "{target:?} returned {:?}",
                        found.target
                    );
                })
                .map(|found| span(&db, found.reference.site.file, found.range))
                .collect();
            actual.sort();
            actual
        };
        for (target, expected) in &oracle {
            let mut expected = expected.clone();
            expected.sort();
            // A service and its inline shape are one declaration: the group is
            // the same whichever of the two targets the cursor yields.
            let queried = match target {
                Target::Service(name)
                | Target::Interface(InterfaceId::Inline { service: name }) => {
                    vec![
                        Target::Service(name.clone()),
                        Target::Interface(InterfaceId::Inline {
                            service: name.clone(),
                        }),
                    ]
                }
                other => vec![other.clone()],
            };
            for target in queried {
                assert_eq!(actual(&target), expected, "{target:?}");
            }
        }

        // The references of one service include no site of another service.
        let services = ["veh.adas.cruise", "veh.adas.lane", "veh.diag.access"]
            .map(|name| actual(&Target::Service(name.to_string())));
        for (index, sites) in services.iter().enumerate() {
            assert!(!sites.is_empty(), "service {index} is referenced");
            for other in &services[index + 1..] {
                assert!(
                    sites.iter().all(|site| !other.contains(site)),
                    "{sites:?} and {other:?} share a site"
                );
            }
        }

        // A target no reference names: the component `Idle`, its instance
        // `unused`, which is never placed, and an interface no package
        // declares.
        let idle = system
            .components
            .iter()
            .position(|decl| decl.name.name == "Idle")
            .expect("Idle is declared");
        let unreferenced = [
            Target::Component(idle),
            Target::Instance {
                component: idle,
                instance: 0,
            },
            Target::Interface(InterfaceId::Declared {
                package: "veh.adas".to_string(),
                name: "Nowhere".to_string(),
            }),
        ];
        for target in unreferenced {
            assert!(
                oracle
                    .iter()
                    .all(|(named, _)| declaration(named) != declaration(&target)),
                "{target:?}"
            );
            assert_eq!(actual(&target), Vec::<Span>::new(), "{target:?}");
        }
    }

    /// `naming_offset` offers a reference to a target only in the slots and
    /// forms that can name the target, and at the offset of the naming part.
    /// The test checks every kind of target against every class of reference
    /// that the fixture holds.
    #[test]
    fn naming_offset_resolves_only_where_a_target_can_be_named() {
        /// The part of a reference that names a target.
        #[derive(Debug, Clone, Copy, PartialEq)]
        enum Part {
            Start,
            End,
        }
        use Part::{End, Start};

        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let ws = Workspace::new(&db, fixture(&db), BTreeMap::new());
        let system = super::super::check_system(&db, ws, std);
        let written = written_references(&system);
        let find = |wanted: &dyn Fn(&Reference, Slot<'_>) -> bool| {
            written
                .iter()
                .find(|(reference, slot)| wanted(reference, *slot))
                .map(|(reference, slot)| (*reference, *slot))
                .expect("the fixture has such a reference")
        };
        let declared =
            |reference: &Reference| matches!(reference.form, ReferenceForm::Declared { .. });
        let service =
            |reference: &Reference| matches!(reference.form, ReferenceForm::Service { .. });
        let instance =
            |reference: &Reference| matches!(reference.form, ReferenceForm::Instance { .. });
        let member = |placement: bool, slot: Slot<'_>| matches!(slot, Slot::Member { placement: p, .. } if p == placement);

        // The classes of reference, in the order of the columns below.
        let classes = [
            (
                "member line, declared form",
                find(&|r, slot| member(false, slot) && declared(r)),
            ),
            (
                "member line, service form",
                find(&|r, slot| member(false, slot) && service(r)),
            ),
            (
                "member line, instance form",
                find(&|r, slot| member(false, slot) && instance(r)),
            ),
            (
                "placement line, declared form",
                find(&|r, slot| member(true, slot) && declared(r)),
            ),
            (
                "placement line, service form",
                find(&|r, slot| member(true, slot) && service(r)),
            ),
            (
                "placement line, instance form",
                find(&|r, slot| member(true, slot) && instance(r)),
            ),
            (
                "offers line",
                find(&|_, slot| matches!(slot, Slot::Offers { .. })),
            ),
            (
                "requires line, declared form",
                find(&|r, slot| matches!(slot, Slot::Requires { .. }) && declared(r)),
            ),
            (
                "requires line, service form",
                find(&|r, slot| matches!(slot, Slot::Requires { .. }) && service(r)),
            ),
            (
                "for reference",
                find(&|_, slot| matches!(slot, Slot::For { .. })),
            ),
        ];
        let service_name = "veh.diag.access".to_string();
        // Each target with the expected part for each class above.
        let table: [(Target, [Option<Part>; 10]); 6] = [
            (
                Target::System(0),
                [
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Start),
                ],
            ),
            (
                Target::Component(0),
                [
                    Some(Start),
                    None,
                    None,
                    Some(Start),
                    None,
                    Some(Start),
                    None,
                    None,
                    None,
                    None,
                ],
            ),
            (
                Target::Instance {
                    component: 0,
                    instance: 0,
                },
                [
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(End),
                    None,
                    None,
                    None,
                    None,
                ],
            ),
            (
                Target::Service(service_name.clone()),
                [
                    None,
                    Some(Start),
                    None,
                    None,
                    Some(Start),
                    None,
                    Some(Start),
                    Some(Start),
                    Some(Start),
                    None,
                ],
            ),
            (
                Target::Interface(InterfaceId::Inline {
                    service: service_name,
                }),
                [
                    None,
                    Some(Start),
                    None,
                    None,
                    Some(Start),
                    None,
                    Some(Start),
                    Some(Start),
                    Some(Start),
                    None,
                ],
            ),
            (
                Target::Interface(InterfaceId::Declared {
                    package: "veh.adas".to_string(),
                    name: "CruiseControl".to_string(),
                }),
                [
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Start),
                    Some(Start),
                    None,
                ],
            ),
        ];
        for (target, expected) in &table {
            for ((class, (reference, slot)), expected) in classes.iter().zip(expected) {
                let range = reference.site.range;
                let expected = expected.map(|part| match part {
                    Start => range.start(),
                    End => range.end(),
                });
                assert_eq!(
                    naming_offset(target, reference, *slot),
                    expected,
                    "{target:?} on a {class}"
                );
            }
        }
    }
}
