//! The lowering (rsdl reference §13): the checked system to the IR's `System`
//! message (`crates/ridl-ir/proto/ridl/ir/v2/system.proto`).
//!
//! The lowering runs over the closure once and then once per deployment. It
//! reads the checked model and the lowered IR of the workspace's packages,
//! which carries the inputs rsdl does not own (rsdl §13): each interface's
//! number and provisional flag (`Interface.number`, `Interface.provisional`,
//! from the package's lock) and each member's ordinal (`Decl.ordinal`, ridl
//! §11).
//!
//! **Gating (rsdl §13).** An error in the closure blocks lowering for every
//! deployment, so [`lower_system`] returns `None` when
//! `CheckedSystem::closure_has_errors` is set. An RSDL-7xx error blocks its own
//! deployment only, which `DeploymentPlacement::has_errors` records: that
//! deployment is left out. A warning never blocks: the facts are produced and
//! carry the warned condition.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ridl_ir::v2;

use super::closure::{Closure, ComponentId, InterfaceId};
use super::placement::{DeploymentPlacement, Placement};
use super::{BackendKey, CheckedSystem, DeploymentDecl, DistributionDecl, WrittenValue};

/// Lowers `system` to the IR's `System` message, or returns `None` when the
/// workspace declares no `system` or an error in the closure blocks lowering
/// (rsdl §13).
///
/// `packages` is the lowered IR of the workspace's packages: every package that
/// declares an interface a closure service lists must be among them.
pub fn lower_system(system: &CheckedSystem, packages: &[&v2::Package]) -> Option<v2::System> {
    if system.closure_has_errors {
        return None;
    }
    let closure = system.closure.as_ref()?;
    let decl = system.systems.first()?;
    let lowering = Lowering {
        system,
        closure,
        packages,
    };
    Some(v2::System {
        name: decl.name.name.clone(),
        package: decl.package.clone(),
        labels: decl.attrs.labels.clone(),
        attributes: attributes(&decl.attrs.backend_keys),
        members: lowering.system_members(),
        components: lowering.components(),
        producers: lowering.producers(),
        grants: lowering.grants(),
        regions: lowering.regions(),
        distributions: lowering.distributions(),
        deployments: lowering.deployments(),
    })
}

/// The inputs every part of the lowering reads.
struct Lowering<'a> {
    system: &'a CheckedSystem,
    closure: &'a Closure,
    packages: &'a [&'a v2::Package],
}

impl<'a> Lowering<'a> {
    /// The qualified name of the closure component at `index`: `pkg.Name`,
    /// or the service's dotted name for an implicit component. It equals
    /// `v2::Component::qualified_name` of the lowered component.
    fn component_name(&self, index: usize) -> String {
        match &self.closure.components[index].id {
            ComponentId::Declared { package, name } => format!("{package}.{name}"),
            ComponentId::Implicit { service } => service.clone(),
        }
    }

    /// An interface by identity (rsdl §11): the catalog is the package that
    /// declares the interface, or the package that declares the service for an
    /// inline shape.
    fn interface_ref(&self, interface: &InterfaceId) -> v2::InterfaceRef {
        match interface {
            InterfaceId::Declared { package, name } => v2::InterfaceRef {
                catalog: package.clone(),
                name: name.clone(),
                inline: false,
            },
            InterfaceId::Inline { service } => v2::InterfaceRef {
                catalog: self.closure.services[service].package.clone(),
                name: service.clone(),
                inline: true,
            },
        }
    }

    /// The lowered IR of the interface `interface` names: the shape of that
    /// name, declared or inline, in the package of its catalog.
    fn interface_ir(&self, interface: &v2::InterfaceRef) -> &'a v2::Interface {
        self.packages
            .iter()
            .filter(|package| package.name == interface.catalog)
            .flat_map(|package| package.shapes())
            .find(|shape| shape.name == interface.name && shape.is_inline() == interface.inline)
            .map(|shape| shape.interface)
            .expect("a closure interface is declared by a package the lowering reads")
    }

    /// The system's member lines, in source order (rsdl §4, §13).
    fn system_members(&self) -> Vec<v2::MemberLine> {
        let decl = &self.system.systems[0];
        (0..decl.members.len())
            .filter_map(|position| {
                let component = self
                    .closure
                    .components
                    .iter()
                    .position(|component| component.line == position)?;
                Some(v2::MemberLine {
                    component: self.component_name(component),
                    attributes: attributes(&decl.members[position].backend_keys),
                })
            })
            .collect()
    }

    /// The closure components with their resolved lines (rsdl §3.2, §6, §7,
    /// §8).
    fn components(&self) -> Vec<v2::Component> {
        self.closure
            .components
            .iter()
            .enumerate()
            .map(|(index, component)| {
                let (name, package, implicit) = match &component.id {
                    ComponentId::Declared { package, name } => {
                        (name.clone(), package.clone(), false)
                    }
                    ComponentId::Implicit { service } => (service.clone(), String::new(), true),
                };
                let decl = component.decl.map(|decl| &self.system.components[decl]);
                let offers = match component.decl {
                    Some(decl) => self.system.components[decl]
                        .offers
                        .iter()
                        .zip(&self.system.component_lines[decl].offers)
                        .filter_map(|(line, service)| {
                            Some(v2::Offer {
                                service: service.clone()?,
                                attributes: attributes(&line.backend_keys),
                            })
                        })
                        .collect(),
                    None => component
                        .offers
                        .iter()
                        .map(|service| v2::Offer {
                            service: service.clone(),
                            attributes: Vec::new(),
                        })
                        .collect(),
                };
                let requires = self
                    .closure
                    .requires
                    .iter()
                    .filter(|require| require.consumer == index)
                    .map(|require| v2::Require {
                        interface: Some(self.interface_ref(&require.interface)),
                        service: require.service.clone(),
                        producer: self.component_name(require.producer),
                        attributes: decl.map_or_else(Vec::new, |decl| {
                            attributes(&decl.requires[require.line].backend_keys)
                        }),
                    })
                    .collect();
                v2::Component {
                    name,
                    package,
                    implicit,
                    external: component.external,
                    instances: component.instances.clone(),
                    offers,
                    requires,
                    labels: decl.map_or_else(Vec::new, |decl| decl.attrs.labels.clone()),
                    attributes: decl
                        .map_or_else(Vec::new, |decl| attributes(&decl.attrs.backend_keys)),
                }
            })
            .collect()
    }

    /// Every deployment no RSDL-7xx error blocked, in declaration order (rsdl
    /// §13).
    fn deployments(&self) -> Vec<v2::Deployment> {
        self.system
            .deployments
            .iter()
            .zip(&self.system.placements)
            .filter(|(_, placement)| !placement.has_errors)
            .map(|(decl, placement)| self.deployment(decl, placement))
            .collect()
    }

    /// One deployment's facts: its machines, the placement of every closure
    /// instance, the link set with crossing kinds and the surface set (rsdl §9,
    /// §10, §13).
    fn deployment(&self, decl: &DeploymentDecl, placement: &DeploymentPlacement) -> v2::Deployment {
        let at = placed(placement);
        let routes = self.routes(decl, &at);
        let machines = decl
            .machines
            .iter()
            .map(|machine| v2::Machine {
                name: machine.name.name.clone(),
                external: machine.external,
                labels: machine.attrs.labels.clone(),
                attributes: attributes(&machine.attrs.backend_keys),
            })
            .collect();

        let mut placements = Vec::new();
        for (index, component) in self.closure.components.iter().enumerate() {
            for instance in &component.instances {
                let placed = at[&(index, instance.as_str())];
                let line = &decl.machines[placed.machine].members[placed.line];
                placements.push(v2::Placement {
                    component: self.component_name(index),
                    instance: instance.clone(),
                    machine: decl.machines[placed.machine].name.name.clone(),
                    attributes: attributes(&line.backend_keys),
                });
            }
        }

        let mut links = Vec::new();
        let mut surface = Vec::new();
        for require in &self.closure.requires {
            let consumer = &self.closure.components[require.consumer];
            let producer = &self.closure.components[require.producer];
            // rsdl §10: a link with two external endpoints crosses nothing the
            // workspace builds and is not lowered; a link with one enters the
            // surface set, read from the consumer's side (§13).
            let direction = match (consumer.external, producer.external) {
                (true, true) => continue,
                (true, false) => Some(v2::SurfaceDirection::ExternalConsumes),
                (false, true) => Some(v2::SurfaceDirection::ExternalOffers),
                (false, false) => None,
            };
            for consumer_instance in &consumer.instances {
                for producer_instance in &producer.instances {
                    let from = at[&(require.consumer, consumer_instance.as_str())];
                    let to = at[&(require.producer, producer_instance.as_str())];
                    let crossing = if from.machine == to.machine {
                        v2::Crossing::SameMachine
                    } else if decl.machines[from.machine].external
                        || decl.machines[to.machine].external
                    {
                        v2::Crossing::OffBoard
                    } else {
                        v2::Crossing::DifferentMachine
                    };
                    let link = v2::Link {
                        interface: Some(self.interface_ref(&require.interface)),
                        service: require.service.clone(),
                        consumer: Some(self.endpoint(decl, from)),
                        producer: Some(self.endpoint(decl, to)),
                        crossing: crossing as i32,
                    };
                    if let Some(direction) = direction {
                        surface.push(v2::Surface {
                            link: Some(link.clone()),
                            direction: direction as i32,
                        });
                    }
                    links.push(link);
                }
            }
        }

        v2::Deployment {
            name: decl.name.name.clone(),
            package: decl.package.clone(),
            labels: decl.attrs.labels.clone(),
            attributes: attributes(&decl.attrs.backend_keys),
            machines,
            placements,
            links,
            routes,
            surface,
            installations: self.installations(decl, placement),
        }
    }

    /// The routing table of one deployment (rsdl §13): for every member of
    /// every interface a closure service lists, the key (catalog, interface
    /// number, member ordinal) and the owning service's producing instances
    /// with their machines. A `reserved` tombstone is not a member and has no
    /// route.
    fn routes(
        &self,
        decl: &DeploymentDecl,
        at: &HashMap<(usize, &str), &Placement>,
    ) -> Vec<v2::Route> {
        let mut routes = Vec::new();
        for (interface, owners) in &self.closure.interface_owners {
            let service = &owners[0];
            let offerer = self.closure.services[service].offerers[0];
            let producers: Vec<v2::Endpoint> = self.closure.components[offerer]
                .instances
                .iter()
                .map(|instance| self.endpoint(decl, at[&(offerer, instance.as_str())]))
                .collect();
            let reference = self.interface_ref(interface);
            let ir = self.interface_ir(&reference);
            for member in &ir.interactions {
                if matches!(member.kind, None | Some(v2::decl::Kind::ReservedSlot(_))) {
                    continue;
                }
                routes.push(v2::Route {
                    catalog: reference.catalog.clone(),
                    interface_number: ir.number,
                    member_ordinal: member.ordinal,
                    interface: reference.name.clone(),
                    member: member.name.clone(),
                    service: service.clone(),
                    producers: producers.clone(),
                });
            }
        }
        routes.sort_by(|a, b| {
            (&a.catalog, a.interface_number, a.member_ordinal).cmp(&(
                &b.catalog,
                b.interface_number,
                b.member_ordinal,
            ))
        });
        routes
    }

    /// Distribution installation in one deployment (rsdl §13): per
    /// distribution, the machines hosting at least one instance of its
    /// components, in machine declaration order. Empty when the workspace
    /// declares no distribution.
    fn installations(
        &self,
        decl: &DeploymentDecl,
        placement: &DeploymentPlacement,
    ) -> Vec<v2::Installation> {
        let Some(facts) = &self.closure.distribution_facts else {
            return Vec::new();
        };
        self.system
            .distributions
            .iter()
            .enumerate()
            .map(|(index, distribution)| {
                let hosting: BTreeSet<usize> = placement
                    .placements
                    .iter()
                    .filter(|placed| facts.membership[placed.component] == Some(index))
                    .map(|placed| placed.machine)
                    .collect();
                v2::Installation {
                    distribution: distribution_name(distribution),
                    machines: hosting
                        .into_iter()
                        .map(|machine| decl.machines[machine].name.name.clone())
                        .collect(),
                }
            })
            .collect()
    }

    /// One end of a link: the placed instance and its machine.
    fn endpoint(&self, decl: &DeploymentDecl, placement: &Placement) -> v2::Endpoint {
        v2::Endpoint {
            component: self.component_name(placement.component),
            instance: placement.instance.clone(),
            machine: decl.machines[placement.machine].name.name.clone(),
        }
    }

    /// The permission list (rsdl §11, §13): per closure component, the catalog
    /// regions its `requires` lines reach — the catalog of each required
    /// interface. The write side is read from the producers fact.
    fn grants(&self) -> Vec<v2::Grant> {
        self.closure
            .components
            .iter()
            .enumerate()
            .map(|(index, component)| {
                let regions: BTreeSet<String> = self
                    .closure
                    .requires
                    .iter()
                    .filter(|require| require.consumer == index)
                    .map(|require| self.interface_ref(&require.interface).catalog)
                    .collect();
                v2::Grant {
                    component: self.component_name(index),
                    external: component.external,
                    regions: regions.into_iter().collect(),
                }
            })
            .collect()
    }

    /// The region map (rsdl §11, §13): one region per catalog the closure
    /// reaches, holding the interfaces of that catalog that closure services
    /// list, each with its number, provisional flag and owning service.
    fn regions(&self) -> Vec<v2::Region> {
        let mut regions: BTreeMap<String, Vec<v2::RegionInterface>> = BTreeMap::new();
        for (interface, owners) in &self.closure.interface_owners {
            let reference = self.interface_ref(interface);
            let ir = self.interface_ir(&reference);
            regions
                .entry(reference.catalog)
                .or_default()
                .push(v2::RegionInterface {
                    name: reference.name,
                    inline: reference.inline,
                    number: ir.number,
                    provisional: ir.provisional,
                    service: owners[0].clone(),
                });
        }
        regions
            .into_iter()
            .map(|(catalog, mut interfaces)| {
                interfaces.sort_by_key(|interface| interface.number);
                v2::Region {
                    catalog,
                    interfaces,
                }
            })
            .collect()
    }

    /// Every distribution of the workspace with its member lines and its
    /// dependency (rsdl §3.3, §13). Empty when the workspace declares no
    /// distribution.
    fn distributions(&self) -> Vec<v2::Distribution> {
        let Some(facts) = &self.closure.distribution_facts else {
            return Vec::new();
        };
        self.system
            .distributions
            .iter()
            .enumerate()
            .map(|(index, decl)| {
                let members = (0..decl.members.len())
                    .filter_map(|position| {
                        let component = (0..self.closure.components.len()).find(|&component| {
                            facts.membership[component] == Some(index)
                                && facts.member_lines[component] == Some(position)
                        })?;
                        Some(v2::MemberLine {
                            component: self.component_name(component),
                            attributes: attributes(&decl.members[position].backend_keys),
                        })
                    })
                    .collect();
                let depends_on: BTreeSet<String> = facts
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.from == index)
                    .map(|dependency| distribution_name(&self.system.distributions[dependency.to]))
                    .collect();
                v2::Distribution {
                    name: decl.name.name.clone(),
                    package: decl.package.clone(),
                    members,
                    depends_on: depends_on.into_iter().collect(),
                    labels: decl.attrs.labels.clone(),
                    attributes: attributes(&decl.attrs.backend_keys),
                }
            })
            .collect()
    }

    /// For every closure service, its offering component and that component's
    /// instances; more than one instance is a redundant provider set, marked
    /// not yet realizable (rsdl §7, §13).
    fn producers(&self) -> Vec<v2::Producer> {
        self.closure
            .services
            .iter()
            .map(|(service, entry)| {
                let offerer = entry.offerers[0];
                let instances = self.closure.components[offerer].instances.clone();
                v2::Producer {
                    service: service.clone(),
                    component: self.component_name(offerer),
                    not_yet_realizable: instances.len() > 1,
                    instances,
                }
            })
            .collect()
    }
}

/// A distribution's qualified name, `pkg.Name`: it equals
/// `v2::Distribution::qualified_name` of the lowered distribution.
fn distribution_name(decl: &DistributionDecl) -> String {
    format!("{}.{}", decl.package, decl.name.name)
}

/// The placement of each closure instance in one deployment, by (closure
/// component, instance name). A deployment no RSDL-7xx error blocked places
/// every instance exactly once (rsdl §9), so every lookup finds one.
fn placed(placement: &DeploymentPlacement) -> HashMap<(usize, &str), &Placement> {
    placement
        .placements
        .iter()
        .map(|placed| ((placed.component, placed.instance.as_str()), placed))
        .collect()
}

/// Backend keys as declared (rsdl §5): carried, never interpreted.
fn attributes(keys: &[BackendKey]) -> Vec<v2::Attribute> {
    keys.iter()
        .map(|key| v2::Attribute {
            namespace: key.namespace.clone(),
            key: key.key.clone(),
            value: key.value.as_ref().map(attribute_value),
        })
        .collect()
}

fn attribute_value(value: &WrittenValue) -> v2::AttributeValue {
    let kind = match value {
        WrittenValue::Scalar(text) => v2::attribute_value::Kind::Scalar(text.clone()),
        WrittenValue::List(items) => v2::attribute_value::Kind::List(v2::AttributeList {
            items: items.iter().map(attribute_value).collect(),
        }),
    };
    v2::AttributeValue { kind: Some(kind) }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::RidlDatabase;
    use ridl_core::package::Workspace;
    use ridl_core::std_package;

    use super::*;
    use crate::check_package;
    use crate::rsdl::check_system;
    use crate::rsdl::tests::{ADAS, BENCH, COMMON, DIAG, PRODUCTION, SYSTEM, package};

    /// Checks and lowers Appendix A's three contract packages beside
    /// `topology`, the files of the package `veh.topology`. Returns the
    /// checked model with the lowering.
    fn lower_topology(topology: &[(&str, &str)]) -> (CheckedSystem, Option<v2::System>) {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let packages = vec![
            package(&db, "veh.common", &[("veh/common/common.typl", COMMON)]),
            package(&db, "veh.adas", &[("veh/adas/adas.ridl", ADAS)]),
            package(&db, "veh.diag", &[("veh/diag/diag.ridl", DIAG)]),
            package(&db, "veh.topology", topology),
        ];
        let ws = Workspace::new(&db, packages.clone(), BTreeMap::new());
        let checked = check_system(&db, ws, std);
        let irs: Vec<v2::Package> = packages
            .iter()
            .map(|package| {
                let checked = check_package(&db, ws, *package, std);
                let errors: Vec<&str> = checked
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error)
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect();
                assert_eq!(
                    errors,
                    Vec::<&str>::new(),
                    "the contract packages check clean"
                );
                checked.ir
            })
            .collect();
        let irs: Vec<&v2::Package> = irs.iter().collect();
        let lowered = lower_system(&checked, &irs);
        (checked, lowered)
    }

    /// Appendix A with both deployments, lowered.
    fn appendix_a() -> v2::System {
        let (checked, lowered) = lower_topology(&[
            ("veh/topology/system.rsdl", SYSTEM),
            ("veh/topology/production.rsdl", PRODUCTION),
            ("veh/topology/bench.rsdl", BENCH),
        ]);
        assert!(!checked.closure_has_errors, "{:?}", checked.diagnostics);
        lowered.expect("Appendix A lowers")
    }

    fn names(values: &[String]) -> Vec<&str> {
        values.iter().map(String::as_str).collect()
    }

    /// Appendix A's closure facts: the system, its member lines, the five
    /// closure components with their lines, and the producers (rsdl §13, the
    /// "Closure" and "Instances" items after the example).
    #[test]
    fn appendix_a_lowers_its_closure_facts() {
        let system = appendix_a();
        assert_eq!(system.qualified_name(), "veh.topology.Vehicle");
        let members: Vec<&str> = system
            .members
            .iter()
            .map(|line| line.component.as_str())
            .collect();
        let closure = [
            "veh.topology.Cruise",
            "veh.topology.Lane",
            "veh.topology.Panel",
            "veh.topology.Backend",
            "veh.diag.access",
        ];
        assert_eq!(members, closure);

        let components: Vec<(String, bool, bool, Vec<&str>)> = system
            .components
            .iter()
            .map(|component| {
                (
                    component.qualified_name(),
                    component.implicit,
                    component.external,
                    names(&component.instances),
                )
            })
            .collect();
        assert_eq!(
            components,
            [
                (
                    closure[0].to_string(),
                    false,
                    false,
                    vec!["primary", "backup"]
                ),
                (closure[1].to_string(), false, false, vec!["Unit"]),
                (closure[2].to_string(), false, false, vec!["Unit"]),
                (closure[3].to_string(), false, true, vec!["Unit"]),
                (closure[4].to_string(), true, false, vec!["Unit"]),
            ]
        );
        assert_eq!(system.components[0].labels, ["ASIL_B"]);
        assert_eq!(system.components[4].package, "");

        let offers: Vec<Vec<&str>> = system
            .components
            .iter()
            .map(|component| {
                component
                    .offers
                    .iter()
                    .map(|offer| offer.service.as_str())
                    .collect()
            })
            .collect();
        assert_eq!(
            offers,
            [
                vec!["veh.adas.cruise"],
                vec!["veh.adas.lane"],
                vec![],
                vec![],
                vec!["veh.diag.access"],
            ]
        );

        let requires: Vec<(&str, String, bool, &str, &str)> = system
            .components
            .iter()
            .flat_map(|component| {
                let consumer = component.name.as_str();
                component.requires.iter().map(move |require| {
                    let interface = require.interface.as_ref().expect("a resolved interface");
                    (
                        consumer,
                        format!("{}/{}", interface.catalog, interface.name),
                        interface.inline,
                        require.service.as_str(),
                        require.producer.as_str(),
                    )
                })
            })
            .collect();
        assert_eq!(
            requires,
            [
                (
                    "Cruise",
                    "veh.adas/LaneAssist".to_string(),
                    false,
                    "veh.adas.lane",
                    "veh.topology.Lane"
                ),
                (
                    "Panel",
                    "veh.adas/CruiseControl".to_string(),
                    false,
                    "veh.adas.cruise",
                    "veh.topology.Cruise"
                ),
                (
                    "Panel",
                    "veh.adas/LaneAssist".to_string(),
                    false,
                    "veh.adas.lane",
                    "veh.topology.Lane"
                ),
                (
                    "Backend",
                    "veh.adas/CruiseControl".to_string(),
                    false,
                    "veh.adas.cruise",
                    "veh.topology.Cruise"
                ),
                (
                    "Backend",
                    "veh.diag/veh.diag.access".to_string(),
                    true,
                    "veh.diag.access",
                    "veh.diag.access"
                ),
            ]
        );

        let producers: Vec<(&str, &str, Vec<&str>, bool)> = system
            .producers
            .iter()
            .map(|producer| {
                (
                    producer.service.as_str(),
                    producer.component.as_str(),
                    names(&producer.instances),
                    producer.not_yet_realizable,
                )
            })
            .collect();
        assert_eq!(
            producers,
            [
                (
                    "veh.adas.cruise",
                    "veh.topology.Cruise",
                    vec!["primary", "backup"],
                    true
                ),
                ("veh.adas.lane", "veh.topology.Lane", vec!["Unit"], false),
                ("veh.diag.access", "veh.diag.access", vec!["Unit"], false),
            ]
        );
    }

    /// Every closure attribute site carries its backend keys as declared — the
    /// `system` declaration, a system member line, a `component` declaration,
    /// an `offers` line and a `requires` line — and `labels` ride beside the
    /// map (rsdl §5, §13).
    #[test]
    fn closure_attribute_sites_carry_their_backend_keys() {
        let text = "package veh.topology\n\
                    import veh.adas.LaneAssist\n\
                    component Lane [ rust.crate = \"lane\", labels = (QM) ] {\n\
                    \x20 offers veh.adas.lane [ someip.serviceId = 4097 ]\n\
                    }\n\
                    component Panel { requires LaneAssist [ someip.reliable ] }\n\
                    system Vehicle [ owner.teams = (hmi, adas) ] { Lane [ linux.nice = 5 ], Panel }\n";
        let (checked, lowered) = lower_topology(&[("veh/topology/x.rsdl", text)]);
        assert!(!checked.closure_has_errors, "{:?}", checked.diagnostics);
        let system = lowered.expect("the closure lowers");

        let attribute =
            |namespace: &str, key: &str, value: Option<v2::AttributeValue>| v2::Attribute {
                namespace: namespace.to_string(),
                key: key.to_string(),
                value,
            };
        let scalar = |text: &str| v2::AttributeValue {
            kind: Some(v2::attribute_value::Kind::Scalar(text.to_string())),
        };
        let list = |items: Vec<v2::AttributeValue>| v2::AttributeValue {
            kind: Some(v2::attribute_value::Kind::List(v2::AttributeList { items })),
        };

        assert_eq!(
            system.attributes,
            [attribute(
                "owner",
                "teams",
                Some(list(vec![scalar("hmi"), scalar("adas")]))
            )]
        );
        assert_eq!(
            system.members[0].attributes,
            [attribute("linux", "nice", Some(scalar("5")))]
        );
        assert!(system.members[1].attributes.is_empty());
        let lane = &system.components[0];
        assert_eq!(
            lane.attributes,
            [attribute("rust", "crate", Some(scalar("\"lane\"")))]
        );
        assert_eq!(lane.labels, ["QM"]);
        assert_eq!(
            lane.offers[0].attributes,
            [attribute("someip", "serviceId", Some(scalar("4097")))]
        );
        assert_eq!(
            system.components[1].requires[0].attributes,
            [attribute("someip", "reliable", None)]
        );
    }

    /// The link set of `deployment` as `(consumer instance, producer
    /// instance, crossing)` rows.
    fn link_rows(deployment: &v2::Deployment) -> Vec<(String, String, &str)> {
        deployment
            .links
            .iter()
            .map(|link| {
                let end = |endpoint: &Option<v2::Endpoint>| {
                    let endpoint = endpoint.as_ref().expect("a link has two endpoints");
                    format!("{}.{}", endpoint.component, endpoint.instance)
                };
                let crossing = match v2::Crossing::try_from(link.crossing) {
                    Ok(v2::Crossing::SameMachine) => "same machine",
                    Ok(v2::Crossing::DifferentMachine) => "different machine",
                    Ok(v2::Crossing::OffBoard) => "off-board",
                    Ok(v2::Crossing::Unspecified) | Err(_) => "unspecified",
                };
                (end(&link.consumer), end(&link.producer), crossing)
            })
            .collect()
    }

    fn rows(expected: &[(&str, &str, &'static str)]) -> Vec<(String, String, &'static str)> {
        expected
            .iter()
            .map(|(consumer, producer, crossing)| {
                (consumer.to_string(), producer.to_string(), *crossing)
            })
            .collect()
    }

    /// Appendix A's two deployments: the machines, every instance's placement,
    /// the link set with its crossing kinds and the surface set (rsdl §9, §10,
    /// §13, the "Links, Production" and "Links, Bench" items after the
    /// example).
    #[test]
    fn appendix_a_lowers_its_deployments() {
        let system = appendix_a();
        let deployments: Vec<&str> = system
            .deployments
            .iter()
            .map(|deployment| deployment.name.as_str())
            .collect();
        assert_eq!(deployments, ["Production", "Bench"]);
        let production = &system.deployments[0];
        let bench = &system.deployments[1];

        let machines: Vec<(&str, bool, Vec<&str>)> = production
            .machines
            .iter()
            .map(|machine| {
                (
                    machine.name.as_str(),
                    machine.external,
                    names(&machine.labels),
                )
            })
            .collect();
        assert_eq!(
            machines,
            [
                ("AdasHpc", false, vec!["ASIL_B"]),
                ("Cockpit", false, vec![]),
                ("Cloud", true, vec![]),
            ]
        );

        let placements: Vec<(String, &str)> = production
            .placements
            .iter()
            .map(|placement| {
                (
                    format!("{}.{}", placement.component, placement.instance),
                    placement.machine.as_str(),
                )
            })
            .collect();
        let expected = [
            ("veh.topology.Cruise.primary", "AdasHpc"),
            ("veh.topology.Cruise.backup", "Cockpit"),
            ("veh.topology.Lane.Unit", "AdasHpc"),
            ("veh.topology.Panel.Unit", "Cockpit"),
            ("veh.topology.Backend.Unit", "Cloud"),
            ("veh.diag.access.Unit", "AdasHpc"),
        ];
        assert_eq!(
            placements,
            expected.map(|(instance, machine)| (instance.to_string(), machine))
        );
        let panel = &production.placements[3];
        assert_eq!(panel.attributes[0].namespace, "linux");
        assert_eq!(panel.attributes[0].key, "cpuset");

        assert_eq!(
            link_rows(production),
            rows(&[
                (
                    "veh.topology.Cruise.primary",
                    "veh.topology.Lane.Unit",
                    "same machine"
                ),
                (
                    "veh.topology.Cruise.backup",
                    "veh.topology.Lane.Unit",
                    "different machine"
                ),
                (
                    "veh.topology.Panel.Unit",
                    "veh.topology.Cruise.primary",
                    "different machine"
                ),
                (
                    "veh.topology.Panel.Unit",
                    "veh.topology.Cruise.backup",
                    "same machine"
                ),
                (
                    "veh.topology.Panel.Unit",
                    "veh.topology.Lane.Unit",
                    "different machine"
                ),
                (
                    "veh.topology.Backend.Unit",
                    "veh.topology.Cruise.primary",
                    "off-board"
                ),
                (
                    "veh.topology.Backend.Unit",
                    "veh.topology.Cruise.backup",
                    "off-board"
                ),
                (
                    "veh.topology.Backend.Unit",
                    "veh.diag.access.Unit",
                    "off-board"
                ),
            ])
        );
        let link = &production.links[7];
        let interface = link.interface.as_ref().expect("a link names its interface");
        assert_eq!(
            (
                interface.catalog.as_str(),
                interface.name.as_str(),
                interface.inline
            ),
            ("veh.diag", "veh.diag.access", true)
        );
        assert_eq!(link.service, "veh.diag.access");

        assert!(
            link_rows(bench)
                .iter()
                .all(|(_, _, crossing)| *crossing == "same machine")
        );
        for deployment in [production, bench] {
            let surface: Vec<(String, v2::SurfaceDirection)> = deployment
                .surface
                .iter()
                .map(|entry| {
                    let link = entry.link.as_ref().expect("a surface entry holds its link");
                    let consumer = link.consumer.as_ref().expect("a consumer");
                    let producer = link.producer.as_ref().expect("a producer");
                    (
                        format!("{} -> {}", consumer.component, producer.component),
                        v2::SurfaceDirection::try_from(entry.direction).expect("a known direction"),
                    )
                })
                .collect();
            let consumes = v2::SurfaceDirection::ExternalConsumes;
            assert_eq!(
                surface,
                [
                    (
                        "veh.topology.Backend -> veh.topology.Cruise".to_string(),
                        consumes
                    ),
                    (
                        "veh.topology.Backend -> veh.topology.Cruise".to_string(),
                        consumes
                    ),
                    (
                        "veh.topology.Backend -> veh.diag.access".to_string(),
                        consumes
                    ),
                ]
            );
        }
    }

    /// rsdl §10 and §13: a link with two external endpoints is absent, a link
    /// with an external producer enters the surface set as the external side
    /// offering, and an instance with no link is still placed.
    #[test]
    fn external_endpoints_shape_the_link_and_surface_sets() {
        let text = "package veh.topology\n\
                    import veh.adas.LaneAssist\n\
                    component Lane [ external ] { offers veh.adas.lane }\n\
                    component Probe [ external ] { requires LaneAssist }\n\
                    component Panel { requires LaneAssist }\n\
                    component Idle {}\n\
                    system Vehicle { Lane, Probe, Panel, Idle }\n\
                    deployment Bench for Vehicle {\n\
                    \x20 machine Box { Panel, Idle }\n\
                    \x20 machine Cloud [ external ] { Lane, Probe }\n\
                    }\n";
        let (checked, lowered) = lower_topology(&[("veh/topology/x.rsdl", text)]);
        assert!(!checked.closure_has_errors, "{:?}", checked.diagnostics);
        let system = lowered.expect("the closure lowers");
        let [bench] = system.deployments.as_slice() else {
            panic!("one deployment, got {:?}", system.deployments);
        };
        assert_eq!(
            link_rows(bench),
            rows(&[(
                "veh.topology.Panel.Unit",
                "veh.topology.Lane.Unit",
                "off-board"
            )])
        );
        let [surface] = bench.surface.as_slice() else {
            panic!("one surface entry, got {:?}", bench.surface);
        };
        assert_eq!(surface.link.as_ref(), bench.links.first());
        assert_eq!(
            surface.direction,
            v2::SurfaceDirection::ExternalOffers as i32
        );
        let idle: Vec<&str> = bench
            .placements
            .iter()
            .filter(|placement| placement.component == "veh.topology.Idle")
            .map(|placement| placement.machine.as_str())
            .collect();
        assert_eq!(idle, ["Box"]);
    }

    /// rsdl §13: an RSDL-7xx error blocks lowering for its own deployment
    /// only.
    #[test]
    fn a_placement_error_leaves_out_its_own_deployment() {
        let text = "package veh.topology\n\
                    component Lane { offers veh.adas.lane }\n\
                    system Vehicle { Lane }\n\
                    deployment Good for Vehicle { machine A { Lane } }\n\
                    deployment Bad for Vehicle { machine A {} }\n";
        let (checked, lowered) = lower_topology(&[("veh/topology/x.rsdl", text)]);
        let codes: Vec<&str> = checked
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert_eq!(codes, ["RSDL-701"]);
        let system = lowered.expect("an RSDL-7xx error does not block the closure");
        let deployments: Vec<&str> = system
            .deployments
            .iter()
            .map(|deployment| deployment.name.as_str())
            .collect();
        assert_eq!(deployments, ["Good"]);
    }

    /// A route as `(catalog, interface number, member ordinal, interface,
    /// member, service, producers)`, each producer as `component.instance@machine`.
    type RouteRow<'a> = (&'a str, u32, u32, &'a str, &'a str, &'a str, Vec<String>);

    fn route_rows(deployment: &v2::Deployment) -> Vec<RouteRow<'_>> {
        deployment
            .routes
            .iter()
            .map(|route| {
                (
                    route.catalog.as_str(),
                    route.interface_number,
                    route.member_ordinal,
                    route.interface.as_str(),
                    route.member.as_str(),
                    route.service.as_str(),
                    route
                        .producers
                        .iter()
                        .map(|producer| {
                            format!(
                                "{}.{}@{}",
                                producer.component, producer.instance, producer.machine
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }

    fn grant_rows(system: &v2::System) -> Vec<(&str, bool, Vec<&str>)> {
        system
            .grants
            .iter()
            .map(|grant| {
                (
                    grant.component.as_str(),
                    grant.external,
                    names(&grant.regions),
                )
            })
            .collect()
    }

    /// A region interface as `(catalog, name, inline, number, provisional,
    /// service)`. `Region` is destructured: `xtask/tests/shape_walk.rs` counts
    /// every line that spells a field access to `interfaces`, and this is not a
    /// read of `Package.interfaces`.
    fn region_rows(system: &v2::System) -> Vec<(&str, &str, bool, u32, bool, &str)> {
        system
            .regions
            .iter()
            .flat_map(
                |v2::Region {
                     catalog,
                     interfaces,
                     ..
                 }| {
                    interfaces.iter().map(move |interface| {
                        (
                            catalog.as_str(),
                            interface.name.as_str(),
                            interface.inline,
                            interface.number,
                            interface.provisional,
                            interface.service.as_str(),
                        )
                    })
                },
            )
            .collect()
    }

    /// Appendix A's routing table in each deployment, its region map and its
    /// permission list (rsdl §11, §13, the "Grants" item after the example).
    /// No package has a lock, so every interface number is provisional.
    #[test]
    fn appendix_a_lowers_its_routes_regions_and_grants() {
        let system = appendix_a();
        let cruise = |primary: &str, backup: &str| {
            vec![
                format!("veh.topology.Cruise.primary@{primary}"),
                format!("veh.topology.Cruise.backup@{backup}"),
            ]
        };
        let one = |instance: &str| vec![instance.to_string()];
        assert_eq!(
            route_rows(&system.deployments[0]),
            [
                (
                    "veh.adas",
                    1,
                    1,
                    "CruiseControl",
                    "engaged",
                    "veh.adas.cruise",
                    cruise("AdasHpc", "Cockpit")
                ),
                (
                    "veh.adas",
                    1,
                    2,
                    "CruiseControl",
                    "target",
                    "veh.adas.cruise",
                    cruise("AdasHpc", "Cockpit")
                ),
                (
                    "veh.adas",
                    1,
                    3,
                    "CruiseControl",
                    "setLever",
                    "veh.adas.cruise",
                    cruise("AdasHpc", "Cockpit")
                ),
                (
                    "veh.adas",
                    2,
                    1,
                    "LaneAssist",
                    "active",
                    "veh.adas.lane",
                    one("veh.topology.Lane.Unit@AdasHpc")
                ),
                (
                    "veh.diag",
                    1,
                    1,
                    "veh.diag.access",
                    "readFaults",
                    "veh.diag.access",
                    one("veh.diag.access.Unit@AdasHpc")
                ),
            ]
        );
        let bench = route_rows(&system.deployments[1]);
        assert_eq!(bench.len(), 5);
        assert_eq!(bench[0].6, cruise("DevBox", "DevBox"));

        assert_eq!(
            region_rows(&system),
            [
                (
                    "veh.adas",
                    "CruiseControl",
                    false,
                    1,
                    true,
                    "veh.adas.cruise"
                ),
                ("veh.adas", "LaneAssist", false, 2, true, "veh.adas.lane"),
                (
                    "veh.diag",
                    "veh.diag.access",
                    true,
                    1,
                    true,
                    "veh.diag.access"
                ),
            ]
        );

        assert_eq!(
            grant_rows(&system),
            [
                ("veh.topology.Cruise", false, vec!["veh.adas"]),
                ("veh.topology.Lane", false, vec![]),
                ("veh.topology.Panel", false, vec!["veh.adas"]),
                ("veh.topology.Backend", true, vec!["veh.adas", "veh.diag"]),
                ("veh.diag.access", false, vec![]),
            ]
        );
    }

    /// rsdl §11: the region an interface reaches is the catalog of the package
    /// that declares the interface, whichever package declares the owning
    /// service; the region map holds only the interfaces closure services list;
    /// a `reserved` tombstone keeps its ordinal and has no route (ridl §11).
    #[test]
    fn a_region_is_the_catalog_that_declares_the_interface() {
        let contracts = "package veh.topology\n\
                         \n\
                         import veh.adas.LaneAssist\n\
                         \n\
                         type Flag: boolean\n\
                         \n\
                         interface Status {\n\
                         \x20 signal ready: Flag @[100ms..1s]\n\
                         \x20 reserved legacy\n\
                         \x20 signal fault: Flag @[100ms..1s]\n\
                         }\n\
                         \n\
                         service veh.topology.hub : Status, LaneAssist\n";
        let topology = "package veh.topology\n\
                        component Hub { offers veh.topology.hub }\n\
                        component Screen { requires Status, requires LaneAssist }\n\
                        system Vehicle { Hub, Screen }\n\
                        deployment Desk for Vehicle { machine Top { Hub, Screen } }\n";
        let (checked, lowered) = lower_topology(&[
            ("veh/topology/status.ridl", contracts),
            ("veh/topology/x.rsdl", topology),
        ]);
        assert!(!checked.closure_has_errors, "{:?}", checked.diagnostics);
        let system = lowered.expect("the closure lowers");

        assert_eq!(
            region_rows(&system),
            [
                ("veh.adas", "LaneAssist", false, 2, true, "veh.topology.hub"),
                ("veh.topology", "Status", false, 1, true, "veh.topology.hub"),
            ]
        );
        assert_eq!(
            grant_rows(&system),
            [
                ("veh.topology.Hub", false, vec![]),
                (
                    "veh.topology.Screen",
                    false,
                    vec!["veh.adas", "veh.topology"]
                ),
            ]
        );
        let hub = || vec!["veh.topology.Hub.Unit@Top".to_string()];
        assert_eq!(
            route_rows(&system.deployments[0]),
            [
                (
                    "veh.adas",
                    2,
                    1,
                    "LaneAssist",
                    "active",
                    "veh.topology.hub",
                    hub()
                ),
                (
                    "veh.topology",
                    1,
                    1,
                    "Status",
                    "ready",
                    "veh.topology.hub",
                    hub()
                ),
                (
                    "veh.topology",
                    1,
                    3,
                    "Status",
                    "fault",
                    "veh.topology.hub",
                    hub()
                ),
            ]
        );
    }

    /// Appendix A's distributions with their member lines and dependency, and
    /// their installation in each deployment (rsdl §3.3, §13, the
    /// "Distributions" item after the example).
    #[test]
    fn appendix_a_lowers_its_distributions() {
        let system = appendix_a();
        let distributions: Vec<(String, Vec<&str>, Vec<&str>)> = system
            .distributions
            .iter()
            .map(|distribution| {
                (
                    distribution.qualified_name(),
                    distribution
                        .members
                        .iter()
                        .map(|line| line.component.as_str())
                        .collect(),
                    names(&distribution.depends_on),
                )
            })
            .collect();
        assert_eq!(
            distributions,
            [
                (
                    "veh.topology.Adas".to_string(),
                    vec![
                        "veh.topology.Cruise",
                        "veh.topology.Lane",
                        "veh.diag.access"
                    ],
                    vec![]
                ),
                (
                    "veh.topology.Hmi".to_string(),
                    vec!["veh.topology.Panel"],
                    vec!["veh.topology.Adas"]
                ),
            ]
        );

        let installations = |deployment: &v2::Deployment| -> Vec<(String, Vec<String>)> {
            deployment
                .installations
                .iter()
                .map(|installation| {
                    (
                        installation.distribution.clone(),
                        installation.machines.clone(),
                    )
                })
                .collect()
        };
        let row = |distribution: &str, machines: &[&str]| {
            (
                distribution.to_string(),
                machines.iter().map(|machine| machine.to_string()).collect(),
            )
        };
        assert_eq!(
            installations(&system.deployments[0]),
            [
                row("veh.topology.Adas", &["AdasHpc", "Cockpit"]),
                row("veh.topology.Hmi", &["Cockpit"]),
            ]
        );
        assert_eq!(
            installations(&system.deployments[1]),
            [
                row("veh.topology.Adas", &["DevBox"]),
                row("veh.topology.Hmi", &["DevBox"]),
            ]
        );
    }

    /// A distribution's member lines keep their source order and their backend
    /// keys, and its `labels` ride beside its map (rsdl §5, §13); a workspace
    /// with no distribution lowers no distribution and no installation (rsdl
    /// §3.3).
    #[test]
    fn distribution_lines_keep_their_order_and_backend_keys() {
        let closure = "package veh.topology\n\
                       import veh.adas.LaneAssist\n\
                       component Lane { offers veh.adas.lane }\n\
                       component Panel { requires LaneAssist }\n\
                       system Vehicle { Lane, Panel }\n\
                       deployment Desk for Vehicle { machine Top { Lane, Panel } }\n";
        let (_, lowered) = lower_topology(&[("veh/topology/x.rsdl", closure)]);
        let system = lowered.expect("the closure lowers");
        assert!(system.distributions.is_empty());
        assert!(system.deployments[0].installations.is_empty());

        let distribution = "package veh.topology\n\
                            distribution All [ labels = (QM), deb.section = net ] {\n\
                            \x20 Panel\n\
                            \x20 Lane [ deb.priority = optional ]\n\
                            }\n";
        let (checked, lowered) = lower_topology(&[
            ("veh/topology/x.rsdl", closure),
            ("veh/topology/y.rsdl", distribution),
        ]);
        assert!(!checked.closure_has_errors, "{:?}", checked.diagnostics);
        let system = lowered.expect("the closure lowers");
        let [all] = system.distributions.as_slice() else {
            panic!("one distribution, got {:?}", system.distributions);
        };
        let members: Vec<(&str, usize)> = all
            .members
            .iter()
            .map(|line| (line.component.as_str(), line.attributes.len()))
            .collect();
        assert_eq!(
            members,
            [("veh.topology.Panel", 0), ("veh.topology.Lane", 1)]
        );
        assert_eq!(all.members[1].attributes[0].key, "priority");
        assert_eq!(all.labels, ["QM"]);
        assert_eq!(all.attributes[0].namespace, "deb");
        assert!(all.depends_on.is_empty());
    }

    /// rsdl §13: an error in the closure blocks lowering, a workspace with no
    /// `system` lowers nothing, and a warning never blocks — Appendix A lowers
    /// with its two RSDL-409 warnings.
    #[test]
    fn the_lowering_is_blocked_by_a_closure_error_only() {
        let (_, lowered) = lower_topology(&[(
            "veh/topology/x.rsdl",
            "package veh.topology\ncomponent Lane { offers veh.adas.lane }\n",
        )]);
        assert_eq!(lowered, None);

        let (checked, lowered) = lower_topology(&[(
            "veh/topology/x.rsdl",
            "package veh.topology\n\
             component Lane { offers veh.adas.lane }\n\
             system Vehicle { Lane, Lane }\n",
        )]);
        assert!(checked.closure_has_errors);
        assert_eq!(lowered, None);

        let (checked, lowered) = lower_topology(&[("veh/topology/system.rsdl", SYSTEM)]);
        let warnings: Vec<&str> = checked
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert_eq!(warnings, ["RSDL-409", "RSDL-409"]);
        assert!(lowered.is_some());
    }
}
