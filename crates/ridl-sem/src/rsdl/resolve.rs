//! Resolution over the closure (rsdl reference §8): a `requires` line names an
//! interface, which resolves to the one closure service that lists it and the
//! one closure component that offers that service.
//!
//! Two closure rules make the lookup total: one offering component per closure
//! service (RSDL-502), and one owning closure service per required interface
//! (RSDL-403 for none, RSDL-408 for two). A line those rules leave without one
//! answer is not resolved, so the lowering never reads half a link. Links are
//! per deployment (§8 step 3) and are the lowering's (§13).
//!
//! RSDL-308 is a rule of one component (§3.2). It runs on every declared
//! component with the other line rules (see `closure.rs`), through
//! [`requires_own_service`].

use ridl_core::diag::DiagCode;

use super::closure::{Closure, ComponentLines, InterfaceId, Lookup, service_interfaces};
use super::{CheckedSystem, ComponentDecl, MemberRef, Reporter, Site};

/// One resolved `requires` line of a closure component (rsdl §8 steps 1 and
/// 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRequire {
    /// The consumer, as an index into `Closure::components`.
    pub consumer: usize,
    /// The line, as an index into the consumer's `ComponentDecl::requires`.
    pub line: usize,
    /// The required interface.
    pub interface: InterfaceId,
    /// The dotted name of its owning service.
    pub service: String,
    /// The offering component, as an index into `Closure::components`.
    pub producer: usize,
}

/// RSDL-308 (rsdl §3.2): whether `interface`, which `line` of `component`
/// requires, is listed by a service the component offers — `offers` holds its
/// resolved `offers` lines. Reports it when it is.
pub(super) fn requires_own_service(
    lookup: &Lookup,
    component: &ComponentDecl,
    offers: &[Option<String>],
    line: &MemberRef,
    interface: &InterfaceId,
    reporter: &mut Reporter,
) -> bool {
    let Some(service) = offers.iter().flatten().find(|service| {
        service_interfaces(service, &lookup.catalog.entries[*service]).contains(interface)
    }) else {
        return false;
    };
    reporter.error(
        DiagCode::RSDL_308,
        line.reference.site,
        format!(
            "`{name}` requires `{interface}`, which `{service}` lists, and `{name}` offers \
             `{service}` — a component cannot be in its own provider set (rsdl reference §3.2)",
            name = component.name.name,
            interface = interface.text(),
        ),
    );
    true
}

/// Checks the two closure rules and resolves every `requires` line of every
/// closure component they leave with one answer (rsdl §8).
pub(super) fn resolve(
    lookup: &Lookup,
    system: &CheckedSystem,
    lines: &[ComponentLines],
    closure: &Closure,
    reporter: &mut Reporter,
) -> Vec<ResolvedRequire> {
    for (name, service) in &closure.services {
        let [first, others @ ..] = service.offerers.as_slice() else {
            continue;
        };
        for &other in others {
            reporter.error(
                DiagCode::RSDL_502,
                offers_site(system, lines, closure, other, name),
                format!(
                    "`{name}` is offered by `{}` and by `{}` — one closure component offers a \
                     service; redundancy is instances of that one component (rsdl reference §7, \
                     §8)",
                    closure.components[*first].id.text(),
                    closure.components[other].id.text()
                ),
            );
        }
    }
    for (interface, owners) in &closure.interface_owners {
        let [first, others @ ..] = owners.as_slice() else {
            continue;
        };
        for other in others {
            let offerer = closure.services[other].offerers[0];
            reporter.error(
                DiagCode::RSDL_408,
                offers_site(system, lines, closure, offerer, other),
                format!(
                    "`{}` is listed by the closure services `{first}` and `{other}` — an \
                     interface has one owning service in the closure (rsdl reference §8)",
                    interface.text()
                ),
            );
        }
    }
    let mut resolved = Vec::new();
    for (consumer, component) in closure.components.iter().enumerate() {
        let Some(decl) = component.decl else {
            continue;
        };
        for (line, interface) in lines[decl].requires.iter().enumerate() {
            let Some(interface) = interface else {
                continue;
            };
            let site = system.components[decl].requires[line].reference.site;
            let Some(owners) = closure.interface_owners.get(interface) else {
                let hint = outside_offerer(lookup, system, lines, closure, interface)
                    .map(|(offerer, service)| {
                        format!(
                            "; `{offerer}` offers `{service}`, which lists it, and the system does \
                             not list `{offerer}`"
                        )
                    })
                    .unwrap_or_default();
                reporter.error(
                    DiagCode::RSDL_403,
                    site,
                    format!(
                        "no closure service lists `{}`, which `{}` requires{hint} (rsdl reference \
                         §8)",
                        interface.text(),
                        component.id.text()
                    ),
                );
                continue;
            };
            let [service] = owners.as_slice() else {
                continue;
            };
            let [producer] = closure.services[service].offerers.as_slice() else {
                continue;
            };
            let instances = &closure.components[*producer].instances;
            if instances.len() > 1 {
                reporter.warning(
                    DiagCode::RSDL_409,
                    site,
                    format!(
                        "`{}` requires `{}`, which resolves to `{}`, a redundant provider set of {} \
                         instances — not yet realizable: every link is lowered to each instance, \
                         and the runtime has no arbitration yet (rsdl reference §7)",
                        component.id.text(),
                        interface.text(),
                        closure.components[*producer].id.text(),
                        instances.len()
                    ),
                );
            }
            resolved.push(ResolvedRequire {
                consumer,
                line,
                interface: interface.clone(),
                service: service.clone(),
                producer: *producer,
            });
        }
    }
    resolved
}

/// Where the closure component `component` offers `service`: its `offers`
/// line, or the system member line of an implicit component.
fn offers_site(
    system: &CheckedSystem,
    lines: &[ComponentLines],
    closure: &Closure,
    component: usize,
    service: &str,
) -> Site {
    let component = &closure.components[component];
    component
        .decl
        .and_then(|decl| {
            let line = lines[decl]
                .offers
                .iter()
                .position(|offered| offered.as_deref() == Some(service))?;
            Some(system.components[decl].offers[line].reference.site)
        })
        .unwrap_or(component.site)
}

/// The RSDL-403 hint: the first declared component the closure does not list
/// that offers a service listing `interface`, with that service.
fn outside_offerer<'s>(
    lookup: &Lookup,
    system: &'s CheckedSystem,
    lines: &'s [ComponentLines],
    closure: &Closure,
    interface: &InterfaceId,
) -> Option<(&'s str, &'s str)> {
    system
        .components
        .iter()
        .zip(lines)
        .enumerate()
        .filter(|(decl, _)| {
            !closure
                .components
                .iter()
                .any(|listed| listed.decl == Some(*decl))
        })
        .find_map(|(_, (component, resolved))| {
            let service = resolved.offers.iter().flatten().find(|service| {
                service_interfaces(service, &lookup.catalog.entries[*service]).contains(interface)
            })?;
            Some((component.name.name.as_str(), service.as_str()))
        })
}
