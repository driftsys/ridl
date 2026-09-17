//! The lowering (rsdl reference §13): the checked system to the IR's `System`
//! message (`crates/ridl-ir/proto/ridl/ir/v2/system.proto`).
//!
//! The lowering runs over the closure once and then once per deployment, and
//! reads the checked model only.
//!
//! **Gating (rsdl §13).** An error in the closure blocks lowering for every
//! deployment, so [`lower_system`] returns `None` when
//! `CheckedSystem::closure_has_errors` is set. A warning never blocks: the
//! facts are produced and carry the warned condition.

use ridl_ir::v2;

use super::closure::{Closure, ComponentId, InterfaceId};
use super::{BackendKey, CheckedSystem, WrittenValue};

/// Lowers `system` to the IR's `System` message, or returns `None` when the
/// workspace declares no `system` or an error in the closure blocks lowering
/// (rsdl §13).
pub fn lower_system(system: &CheckedSystem) -> Option<v2::System> {
    if system.closure_has_errors {
        return None;
    }
    let closure = system.closure.as_ref()?;
    let decl = system.systems.first()?;
    let lowering = Lowering { system, closure };
    Some(v2::System {
        name: decl.name.name.clone(),
        package: decl.package.clone(),
        labels: decl.attrs.labels.clone(),
        attributes: attributes(&decl.attrs.backend_keys),
        members: lowering.system_members(),
        components: lowering.components(),
        producers: lowering.producers(),
        grants: Vec::new(),
        regions: Vec::new(),
        distributions: Vec::new(),
        deployments: Vec::new(),
    })
}

/// The inputs every part of the lowering reads.
struct Lowering<'a> {
    system: &'a CheckedSystem,
    closure: &'a Closure,
}

impl Lowering<'_> {
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
        let ws = Workspace::new(&db, packages, BTreeMap::new());
        let checked = check_system(&db, ws, std);
        let lowered = lower_system(&checked);
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
