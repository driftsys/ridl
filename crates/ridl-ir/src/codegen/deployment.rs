//! The deployment section of a codegen request, emitted from the lowered
//! system.
//!
//! The section carries the facts of one concrete deployment that a plugin
//! needs to lay out memory for it: the regions, the placed instances with
//! what each offers and maps, and one channel per (interface member, producer
//! instance) with its consumer links. The schema and the order of every
//! repeated field are
//! `proto/ridl/codegen/v1/deployment.proto`; the design record is
//! `docs/design/codegen-plugins.md`, section "The deployment section".
//!
//! This is an emitter over the lowered system artifact of `ridl.ir.v2`
//! (`docs/decisions/ADR-0022-rsdl-system-in-the-ir.md` decision 1: a later
//! emitter, never a second lowering), so it reads the system the way the rest
//! of the toolchain does (`docs/technotes/rsdl-implementation.md`).

use super::bindings::bindings;
use super::depth::ceil_ratio;
use super::v1;
use crate::v2;

/// The slot count every call channel starts from, until a deployment declares
/// one (`docs/design/codegen-plugins.md`, the deployment section).
const DEFAULT_SLOTS: u32 = 16;

/// Emits the deployment section for the deployment of `system` named `name`.
///
/// `packages` holds every package of the workspace, `ridl.std` included: the
/// member's kind and its timing are read from the package that declares the
/// route's interface. Returns `None` when no deployment of `system` is named
/// `name`.
pub fn lower_deployment(
    system: &v2::System,
    name: &str,
    packages: &[&v2::Package],
) -> Option<v1::Deployment> {
    lower_with_bindings(system, name, packages, bindings())
}

/// [`lower_deployment`] with the binding table given as `table` rather than
/// the toolchain's own, already rendered.
fn lower_with_bindings(
    system: &v2::System,
    name: &str,
    packages: &[&v2::Package],
    bindings: Vec<v1::Binding>,
) -> Option<v1::Deployment> {
    let deployment = system
        .deployments
        .iter()
        .find(|candidate| candidate.name == name)?;
    Some(v1::Deployment {
        system: system.qualified_name(),
        name: deployment.name.clone(),
        regions: regions(system),
        instances: instances(system, deployment),
        channels: channels(system, deployment, packages),
        bindings,
    })
}

/// The region map, copied in the order the system IR holds it: catalog name
/// order, each catalog's interfaces in interface number order.
fn regions(system: &v2::System) -> Vec<v1::Region> {
    system
        .regions
        .iter()
        .map(|region| v1::Region {
            catalog: region.catalog.clone(),
            hash: region.hash.clone(),
            interfaces: region
                .interfaces
                .iter()
                .map(|interface| v1::RegionInterface {
                    name: interface.name.clone(),
                    number: interface.number,
                    inline: interface.inline,
                    provisional: interface.provisional,
                    service: interface.service.clone(),
                })
                .collect(),
        })
        .collect()
}

/// One entry per placement, in the system IR's placement order.
///
/// `offers` is every region interface whose listing service the component
/// offers, walked in region order and then interface number order. `maps` is
/// the component's grant: the catalogs its `requires` lines reach. A
/// placement whose component is not in the closure, or a component with no
/// grant, contributes an empty list rather than being dropped: the instance is
/// placed either way, and a plugin that lays out its memory needs the
/// placement.
fn instances(system: &v2::System, deployment: &v2::Deployment) -> Vec<v1::Instance> {
    deployment
        .placements
        .iter()
        .map(|placement| {
            let component = system
                .components
                .iter()
                .find(|component| component.qualified_name() == placement.component);
            let offered: Vec<&str> = component
                .map(|component| {
                    component
                        .offers
                        .iter()
                        .map(|offer| offer.service.as_str())
                        .collect()
                })
                .unwrap_or_default();
            let offers = system
                .regions
                .iter()
                .flat_map(|region| {
                    region
                        .interfaces
                        .iter()
                        .filter(|interface| offered.contains(&interface.service.as_str()))
                        .map(|interface| v1::InterfaceKey {
                            catalog: region.catalog.clone(),
                            number: interface.number,
                            name: interface.name.clone(),
                            inline: interface.inline,
                        })
                })
                .collect();
            let maps = system
                .grants
                .iter()
                .find(|grant| grant.component == placement.component)
                .map(|grant| grant.regions.clone())
                .unwrap_or_default();
            v1::Instance {
                component: placement.component.clone(),
                instance: placement.instance.clone(),
                machine: placement.machine.clone(),
                external: component.is_some_and(|component| component.external),
                offers,
                maps,
            }
        })
        .collect()
}

/// One channel per (route, producer instance).
///
/// The routes are already in (catalog, interface number, member ordinal)
/// order in the system IR; each route's producers are sorted by (component,
/// instance), so that the whole list is in the order the schema fixes. A
/// redundant provider set gives one channel per producer instance, each with
/// its own consumer links — the links are never merged across instances.
fn channels(
    system: &v2::System,
    deployment: &v2::Deployment,
    packages: &[&v2::Package],
) -> Vec<v1::Channel> {
    let mut channels = Vec::new();
    for route in &deployment.routes {
        let (kind, timing) = member_kind(packages, route);
        let inline = inline_of(system, route);
        let bound = matches!(kind, v1::Kind::Event).then(|| depth_of(timing));
        let call = matches!(kind, v1::Kind::Command | v1::Kind::Query);
        let mut producers: Vec<&v2::Endpoint> = route.producers.iter().collect();
        producers.sort_by(|left, right| {
            (&left.component, &left.instance).cmp(&(&right.component, &right.instance))
        });
        for producer in producers {
            let mut links: Vec<&v2::Link> = deployment
                .links
                .iter()
                .filter(|link| {
                    link.interface.as_ref().is_some_and(|interface| {
                        interface.catalog == route.catalog
                            && interface.name == route.interface
                            && interface.inline == inline
                    })
                })
                .filter(|link| link.producer.as_ref() == Some(producer))
                .collect();
            links.sort_by_key(|link| {
                link.consumer
                    .as_ref()
                    .map(|consumer| (consumer.component.clone(), consumer.instance.clone()))
            });
            let consumers: Vec<v1::Consumer> = links
                .iter()
                .filter_map(|link| {
                    let consumer = link.consumer.as_ref()?;
                    Some(v1::Consumer {
                        component: consumer.component.clone(),
                        instance: consumer.instance.clone(),
                        machine: consumer.machine.clone(),
                        crossing: crossing_of(link.crossing),
                        encoding: encoding_of(link.crossing),
                        depth: bound,
                        slots: call.then_some(DEFAULT_SLOTS),
                        slots_source: if call {
                            v1::ValueSource::Default as i32
                        } else {
                            v1::ValueSource::Unspecified as i32
                        },
                        budget: None,
                        budget_source: v1::ValueSource::Unspecified as i32,
                    })
                })
                .collect();
            channels.push(v1::Channel {
                catalog: route.catalog.clone(),
                interface_number: route.interface_number,
                interface: route.interface.clone(),
                inline,
                member_ordinal: route.member_ordinal,
                member: route.member.clone(),
                kind: kind as i32,
                producer: Some(v1::Endpoint {
                    component: producer.component.clone(),
                    instance: producer.instance.clone(),
                    machine: producer.machine.clone(),
                }),
                // Every link of this channel was given `bound`, so the
                // aggregation answers `bound` for every input the emitter
                // can build; `ring_depth` is where the rule itself is
                // pinned.
                depth: bound.map(|bound| ring_depth(&consumers, bound)),
                consumers,
            });
        }
    }
    channels
}

/// The crossing of a consumer link, as the section's own enum.
///
/// The two schemas carry the same values and version independently
/// (`deployment.proto`, the `Crossing` comment), so an IR value this schema
/// does not list is written as `CROSSING_UNSPECIFIED` rather than copied
/// through. Copying it through would put a discriminant outside the schema
/// into the message, which the canonical-JSON writer refuses with the one
/// error [`super::SerializeError::Json`] has.
fn crossing_of(crossing: i32) -> i32 {
    let value = match v2::Crossing::try_from(crossing) {
        Ok(v2::Crossing::SameMachine) => v1::Crossing::SameMachine,
        Ok(v2::Crossing::DifferentMachine) => v1::Crossing::DifferentMachine,
        Ok(v2::Crossing::OffBoard) => v1::Crossing::OffBoard,
        Ok(v2::Crossing::Unspecified) | Err(_) => v1::Crossing::Unspecified,
    };
    value as i32
}

/// The encoding of a consumer link, derived from its crossing and from
/// nothing else: within one machine the payload is mapped or passed, so it is
/// FlatBuffers; across machines it is serialized into a stream, so it is
/// proto3 (`docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
/// decision 2).
fn encoding_of(crossing: i32) -> i32 {
    let encoding = match v2::Crossing::try_from(crossing) {
        Ok(v2::Crossing::SameMachine) => v1::Encoding::Flatbuffers,
        Ok(v2::Crossing::DifferentMachine | v2::Crossing::OffBoard) => v1::Encoding::Proto3,
        Ok(v2::Crossing::Unspecified) | Err(_) => v1::Encoding::Unspecified,
    };
    encoding as i32
}

/// Whether the route's interface is a service's inline shape, read from the
/// region map, which carries the identity the routing key uses.
///
/// The region map is part of the lowered system, so this answer does not
/// depend on the packages the caller handed in. An interface with no region
/// entry is reported as not inline.
fn inline_of(system: &v2::System, route: &v2::Route) -> bool {
    system
        .regions
        .iter()
        .filter(|region| region.catalog == route.catalog)
        .flat_map(|region| &region.interfaces)
        .find(|interface| {
            interface.number == route.interface_number && interface.name == route.interface
        })
        .is_some_and(|interface| interface.inline)
}

/// The kind and the resolved timing of the route's member.
///
/// The interface is found in the package the route names as its catalog, by
/// its identity name alone — the interface's own name, or the owning
/// service's dotted name for an inline shape. That name is unique within a
/// catalog, so it is the whole key. The interface number is deliberately not
/// part of it: a package set whose numbers differ from the lowered system's,
/// which a stale or differently locked package gives, would then match
/// nothing and the channel would lose its kind and its sizing with no
/// diagnostic.
///
/// The member is found by ordinal. A route whose interface or member is not
/// found gives `KIND_UNSPECIFIED` and no timing, so the channel is still
/// emitted with its key, its producer and its consumer links, and carries no
/// sizing it cannot justify.
fn member_kind<'a>(
    packages: &[&'a v2::Package],
    route: &v2::Route,
) -> (v1::Kind, Option<&'a v2::Timing>) {
    let found = packages
        .iter()
        .copied()
        .filter(|package| package.name == route.catalog)
        .flat_map(v2::Package::shapes)
        .find(|shape| shape.name == route.interface);
    let Some(shape) = found else {
        return (v1::Kind::Unspecified, None);
    };
    let member = shape
        .interface
        .interactions
        .iter()
        .find(|decl| decl.ordinal == route.member_ordinal);
    match member.and_then(|decl| decl.kind.as_ref()) {
        Some(v2::decl::Kind::SignalDef(signal)) => (v1::Kind::Signal, signal.timing.as_ref()),
        Some(v2::decl::Kind::EventDef(event)) => (v1::Kind::Event, event.timing.as_ref()),
        Some(v2::decl::Kind::CommandDef(command)) => (v1::Kind::Command, command.timing.as_ref()),
        Some(v2::decl::Kind::QueryDef(query)) => (v1::Kind::Query, query.timing.as_ref()),
        Some(v2::decl::Kind::FixedDef(_)) => (v1::Kind::Fixed, None),
        Some(
            v2::decl::Kind::TypeDef(_)
            | v2::decl::Kind::ConstDef(_)
            | v2::decl::Kind::StructDef(_)
            | v2::decl::Kind::EnumDef(_)
            | v2::decl::Kind::EnumSetDef(_)
            | v2::decl::Kind::UnionDef(_)
            | v2::decl::Kind::ReservedSlot(_),
        )
        | None => (v1::Kind::Unspecified, None),
    }
}

/// The contract bound of an event, `ceil(max / min)` over its resolved
/// timing (`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md`
/// decision 21).
///
/// Absent, with source `VALUE_SOURCE_UNDERIVABLE`, when the timing has an
/// explicit half-open range or when the ratio is not derivable.
fn depth_of(timing: Option<&v2::Timing>) -> v1::Depth {
    let derived =
        timing.and_then(|timing| ceil_ratio(timing.max_us.as_deref()?, timing.min_us.as_deref()?));
    match derived {
        Some(value) => v1::Depth {
            value: Some(value),
            source: v1::ValueSource::Derived as i32,
        },
        None => v1::Depth {
            value: None,
            source: v1::ValueSource::Underivable as i32,
        },
    }
}

/// The ring depth of an event channel: the maximum over its consumer links,
/// absent when any link's depth is absent.
///
/// The source is the source of the link that supplies the maximum, so a
/// declared depth on one link is reported as declared rather than as derived
/// (`docs/design/codegen-plugins.md`, the deployment section, the depth
/// rule). Every consumer link of one channel is given the member's contract
/// bound, so the maximum over the links of one channel is that bound, and
/// nothing the emitter reads writes the declared source.
///
/// A channel with no consumer link takes `bound`, the contract bound of its
/// member: that is the value every consumer link of it would carry.
fn ring_depth(consumers: &[v1::Consumer], bound: v1::Depth) -> v1::Depth {
    if consumers.is_empty() {
        return bound;
    }
    let mut deepest: Option<v1::Depth> = None;
    for consumer in consumers {
        let Some(depth) = consumer.depth.filter(|depth| depth.value.is_some()) else {
            return v1::Depth {
                value: None,
                source: v1::ValueSource::Underivable as i32,
            };
        };
        if deepest.is_none_or(|held| held.value < depth.value) {
            deepest = Some(depth);
        }
    }
    deepest.unwrap_or(bound)
}

#[cfg(test)]
mod tests {
    use super::{lower_deployment, lower_with_bindings};
    use crate::codegen::bindings::{KNOWN, Known, bindings, render};
    use crate::codegen::v1;
    use crate::v2;

    const EVENT_ORDINAL: u32 = 1;
    const QUERY_ORDINAL: u32 = 2;
    const SIGNAL_ORDINAL: u32 = 3;
    const HALF_OPEN_ORDINAL: u32 = 4;

    const CATALOG: &str = "veh.cabin";
    const INTERFACE: &str = "Climate";
    const SERVICE: &str = "veh.cabin.climate";
    const PROVIDER: &str = "veh.cabin.Provider";
    const DASH: &str = "veh.cabin.Dash";
    const LOGGER: &str = "veh.cabin.Logger";
    const FLEET: &str = "veh.cabin.Fleet";

    /// A range timing over exact-decimal microsecond strings.
    fn timing(min_us: Option<&str>, max_us: Option<&str>) -> v2::Timing {
        v2::Timing {
            mode: v2::TimingMode::Range as i32,
            min_us: min_us.map(str::to_string),
            max_us: max_us.map(str::to_string),
            default_applied: false,
        }
    }

    fn member(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            ordinal,
            kind: Some(kind),
            ..Default::default()
        }
    }

    /// One package with one interface carrying the four member kinds the
    /// sizing rules distinguish.
    fn package() -> v2::Package {
        v2::Package {
            name: CATALOG.to_string(),
            interfaces: vec![v2::Interface {
                name: INTERFACE.to_string(),
                number: 1,
                interactions: vec![
                    member(
                        "TempChanged",
                        EVENT_ORDINAL,
                        v2::decl::Kind::EventDef(v2::EventDef {
                            payload: "Temp".to_string(),
                            // @[100ms..1s] gives ceil(1000000 / 100000) = 10.
                            timing: Some(timing(Some("100000"), Some("1000000"))),
                        }),
                    ),
                    member(
                        "GetTemp",
                        QUERY_ORDINAL,
                        v2::decl::Kind::QueryDef(v2::QueryDef::default()),
                    ),
                    member(
                        "Level",
                        SIGNAL_ORDINAL,
                        v2::decl::Kind::SignalDef(v2::SignalDef::default()),
                    ),
                    member(
                        "FanSpeedChanged",
                        HALF_OPEN_ORDINAL,
                        v2::decl::Kind::EventDef(v2::EventDef {
                            payload: "Speed".to_string(),
                            // An explicit half-open range: no derivable depth.
                            timing: Some(timing(Some("100000"), None)),
                        }),
                    ),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn component(name: &str, instances: &[&str], external: bool) -> v2::Component {
        v2::Component {
            name: name.to_string(),
            instances: instances.iter().map(|name| (*name).to_string()).collect(),
            external,
            ..Default::default()
        }
    }

    fn endpoint(component: &str, instance: &str, machine: &str) -> v2::Endpoint {
        v2::Endpoint {
            component: component.to_string(),
            instance: instance.to_string(),
            machine: machine.to_string(),
        }
    }

    fn placement(component: &str, instance: &str, machine: &str) -> v2::Placement {
        v2::Placement {
            component: component.to_string(),
            instance: instance.to_string(),
            machine: machine.to_string(),
            attributes: Vec::new(),
            sizing: None,
        }
    }

    fn interface_ref() -> v2::InterfaceRef {
        v2::InterfaceRef {
            catalog: CATALOG.to_string(),
            name: INTERFACE.to_string(),
            inline: false,
        }
    }

    fn link(consumer: v2::Endpoint, producer: v2::Endpoint, crossing: v2::Crossing) -> v2::Link {
        v2::Link {
            interface: Some(interface_ref()),
            service: SERVICE.to_string(),
            consumer: Some(consumer),
            producer: Some(producer),
            crossing: crossing as i32,
        }
    }

    fn route(member: &str, ordinal: u32) -> v2::Route {
        v2::Route {
            catalog: CATALOG.to_string(),
            interface_number: 1,
            member_ordinal: ordinal,
            interface: INTERFACE.to_string(),
            member: member.to_string(),
            service: SERVICE.to_string(),
            producers: vec![
                endpoint(PROVIDER, "primary", "head"),
                endpoint(PROVIDER, "backup", "zone"),
            ],
        }
    }

    fn grant(component: &str, external: bool, regions: &[&str]) -> v2::Grant {
        v2::Grant {
            component: component.to_string(),
            external,
            regions: regions.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    /// One system with one deployment: a redundant provider set of two
    /// instances on two machines, and three consumers — one on each machine
    /// and one external, on an external machine.
    fn system() -> v2::System {
        let mut provider = component("Provider", &["primary", "backup"], false);
        provider.package = CATALOG.to_string();
        provider.offers = vec![v2::Offer {
            service: SERVICE.to_string(),
            ..Default::default()
        }];
        let consumer = |name: &str, external: bool| {
            let mut component = component(name, &["Unit"], external);
            component.package = CATALOG.to_string();
            component.requires = vec![v2::Require {
                interface: Some(interface_ref()),
                service: SERVICE.to_string(),
                producer: PROVIDER.to_string(),
                ..Default::default()
            }];
            component
        };
        v2::System {
            name: "Cabin".to_string(),
            package: CATALOG.to_string(),
            components: vec![
                provider,
                consumer("Dash", false),
                consumer("Logger", false),
                consumer("Fleet", true),
            ],
            producers: vec![v2::Producer {
                service: SERVICE.to_string(),
                component: PROVIDER.to_string(),
                instances: vec!["primary".to_string(), "backup".to_string()],
                not_yet_realizable: true,
            }],
            grants: vec![
                grant(PROVIDER, false, &[]),
                grant(DASH, false, &[CATALOG]),
                grant(LOGGER, false, &[CATALOG]),
                grant(FLEET, true, &[CATALOG]),
            ],
            regions: vec![v2::Region {
                catalog: CATALOG.to_string(),
                interfaces: vec![v2::RegionInterface {
                    name: INTERFACE.to_string(),
                    inline: false,
                    number: 1,
                    provisional: false,
                    service: SERVICE.to_string(),
                }],
                hash: vec![7; 32],
            }],
            deployments: vec![v2::Deployment {
                name: "prod".to_string(),
                package: CATALOG.to_string(),
                machines: vec![
                    v2::Machine {
                        name: "head".to_string(),
                        ..Default::default()
                    },
                    v2::Machine {
                        name: "zone".to_string(),
                        ..Default::default()
                    },
                    v2::Machine {
                        name: "cloud".to_string(),
                        external: true,
                        ..Default::default()
                    },
                ],
                placements: vec![
                    placement(PROVIDER, "primary", "head"),
                    placement(PROVIDER, "backup", "zone"),
                    placement(DASH, "Unit", "head"),
                    placement(LOGGER, "Unit", "zone"),
                    placement(FLEET, "Unit", "cloud"),
                ],
                links: vec![
                    link(
                        endpoint(DASH, "Unit", "head"),
                        endpoint(PROVIDER, "primary", "head"),
                        v2::Crossing::SameMachine,
                    ),
                    link(
                        endpoint(DASH, "Unit", "head"),
                        endpoint(PROVIDER, "backup", "zone"),
                        v2::Crossing::DifferentMachine,
                    ),
                    link(
                        endpoint(LOGGER, "Unit", "zone"),
                        endpoint(PROVIDER, "primary", "head"),
                        v2::Crossing::DifferentMachine,
                    ),
                    link(
                        endpoint(LOGGER, "Unit", "zone"),
                        endpoint(PROVIDER, "backup", "zone"),
                        v2::Crossing::SameMachine,
                    ),
                    link(
                        endpoint(FLEET, "Unit", "cloud"),
                        endpoint(PROVIDER, "primary", "head"),
                        v2::Crossing::OffBoard,
                    ),
                    link(
                        endpoint(FLEET, "Unit", "cloud"),
                        endpoint(PROVIDER, "backup", "zone"),
                        v2::Crossing::OffBoard,
                    ),
                ],
                routes: vec![
                    route("TempChanged", EVENT_ORDINAL),
                    route("GetTemp", QUERY_ORDINAL),
                    route("Level", SIGNAL_ORDINAL),
                    route("FanSpeedChanged", HALF_OPEN_ORDINAL),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// The section of the fixture's one deployment.
    fn section() -> v1::Deployment {
        let package = package();
        let system = system();
        lower_deployment(&system, "prod", &[&package]).expect("the deployment is named prod")
    }

    /// The channels of one member, in emitted order.
    fn channels_of(section: &v1::Deployment, ordinal: u32) -> Vec<v1::Channel> {
        section
            .channels
            .iter()
            .filter(|channel| channel.member_ordinal == ordinal)
            .cloned()
            .collect()
    }

    /// The one channel of one member and one producer instance.
    fn channel_of(section: &v1::Deployment, ordinal: u32, instance: &str) -> v1::Channel {
        channels_of(section, ordinal)
            .into_iter()
            .find(|channel| {
                channel
                    .producer
                    .as_ref()
                    .is_some_and(|producer| producer.instance == instance)
            })
            .expect("the member has a channel for that producer instance")
    }

    #[test]
    fn the_binding_list_is_the_known_table_in_name_order() {
        // The table is empty until a binding document exists
        // (driftsys/ridl#265).
        assert!(KNOWN.is_empty());
        assert_eq!(section().bindings, bindings());
        assert_eq!(bindings().len(), KNOWN.len());
    }

    #[test]
    fn the_emitter_writes_the_rendered_table_it_is_given() {
        let table = [
            Known {
                name: "websocket",
                version: "1",
                frame_header_max_bytes: Some(14),
                envelope_bytes: None,
            },
            Known {
                name: "tcp",
                version: "2",
                frame_header_max_bytes: None,
                envelope_bytes: Some(8),
            },
        ];
        let package = package();
        let system = system();
        let section = lower_with_bindings(&system, "prod", &[&package], render(&table))
            .expect("the deployment is named prod");
        assert_eq!(section.bindings, render(&table));
        let names: Vec<&str> = section.bindings.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["tcp", "websocket"]);
    }

    #[test]
    fn an_unknown_deployment_name_gives_none() {
        let package = package();
        let system = system();
        assert!(lower_deployment(&system, "staging", &[&package]).is_none());
        assert!(lower_deployment(&system, "", &[&package]).is_none());
    }

    #[test]
    fn each_producer_instance_of_a_route_is_one_channel() {
        let section = section();
        assert_eq!(section.system, "veh.cabin.Cabin");
        assert_eq!(section.name, "prod");
        // Four members, each with two producer instances.
        assert_eq!(section.channels.len(), 8);

        let event = channels_of(&section, EVENT_ORDINAL);
        assert_eq!(event.len(), 2);
        let instances: Vec<&str> = event
            .iter()
            .map(|channel| {
                channel
                    .producer
                    .as_ref()
                    .expect("every channel carries a producer")
                    .instance
                    .as_str()
            })
            .collect();
        assert_eq!(instances, ["backup", "primary"]);
        // The consumer links are not merged: each channel carries its own.
        for channel in &event {
            assert_eq!(channel.catalog, CATALOG);
            assert_eq!(channel.interface_number, 1);
            assert_eq!(channel.interface, INTERFACE);
            assert!(!channel.inline);
            assert_eq!(channel.member, "TempChanged");
            assert_eq!(channel.kind, v1::Kind::Event as i32);
            assert_eq!(channel.consumers.len(), 3);
        }
        let primary = channel_of(&section, EVENT_ORDINAL, "primary");
        assert_eq!(
            primary.producer,
            Some(v1::Endpoint {
                component: PROVIDER.to_string(),
                instance: "primary".to_string(),
                machine: "head".to_string(),
            })
        );
        let machines: Vec<&str> = primary
            .consumers
            .iter()
            .map(|consumer| consumer.machine.as_str())
            .collect();
        assert_eq!(machines, ["head", "cloud", "zone"]);
        let backup = channel_of(&section, EVENT_ORDINAL, "backup");
        let machines: Vec<&str> = backup
            .consumers
            .iter()
            .map(|consumer| consumer.machine.as_str())
            .collect();
        assert_eq!(machines, ["head", "cloud", "zone"]);
    }

    #[test]
    fn a_same_machine_link_is_flatbuffers_and_a_different_machine_link_is_proto3() {
        let channel = channel_of(&section(), EVENT_ORDINAL, "primary");
        let seen: Vec<(&str, i32, i32)> = channel
            .consumers
            .iter()
            .map(|consumer| {
                (
                    consumer.component.as_str(),
                    consumer.crossing,
                    consumer.encoding,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (
                    DASH,
                    v1::Crossing::SameMachine as i32,
                    v1::Encoding::Flatbuffers as i32
                ),
                (
                    FLEET,
                    v1::Crossing::OffBoard as i32,
                    v1::Encoding::Proto3 as i32
                ),
                (
                    LOGGER,
                    v1::Crossing::DifferentMachine as i32,
                    v1::Encoding::Proto3 as i32
                ),
            ]
        );
        // The same-machine consumer of the other producer instance.
        let backup = channel_of(&section(), EVENT_ORDINAL, "backup");
        let logger = backup
            .consumers
            .iter()
            .find(|consumer| consumer.component == LOGGER)
            .expect("the logger consumes from the backup instance");
        assert_eq!(logger.crossing, v1::Crossing::SameMachine as i32);
        assert_eq!(logger.encoding, v1::Encoding::Flatbuffers as i32);
    }

    #[test]
    fn an_event_channel_derives_its_depth_and_the_ring_depth_is_the_max() {
        let derived = v1::Depth {
            value: Some(10),
            source: v1::ValueSource::Derived as i32,
        };
        for instance in ["primary", "backup"] {
            let channel = channel_of(&section(), EVENT_ORDINAL, instance);
            assert_eq!(channel.depth, Some(derived));
            for consumer in &channel.consumers {
                assert_eq!(consumer.depth, Some(derived));
                // An event channel carries no call sizing.
                assert_eq!(consumer.slots, None);
                assert_eq!(consumer.slots_source, v1::ValueSource::Unspecified as i32);
                assert_eq!(consumer.budget, None);
                assert_eq!(consumer.budget_source, v1::ValueSource::Unspecified as i32);
            }
        }
    }

    #[test]
    fn a_half_open_event_has_an_absent_underivable_depth() {
        let underivable = v1::Depth {
            value: None,
            source: v1::ValueSource::Underivable as i32,
        };
        let channel = channel_of(&section(), HALF_OPEN_ORDINAL, "primary");
        assert_eq!(channel.kind, v1::Kind::Event as i32);
        assert_eq!(channel.depth, Some(underivable));
        for consumer in &channel.consumers {
            assert_eq!(consumer.depth, Some(underivable));
        }
    }

    #[test]
    fn a_call_channel_carries_sixteen_default_slots_and_no_budget() {
        let channel = channel_of(&section(), QUERY_ORDINAL, "primary");
        assert_eq!(channel.kind, v1::Kind::Query as i32);
        assert_eq!(channel.member, "GetTemp");
        // A call channel has no ring depth.
        assert_eq!(channel.depth, None);
        assert_eq!(channel.consumers.len(), 3);
        for consumer in &channel.consumers {
            assert_eq!(consumer.depth, None);
            assert_eq!(consumer.slots, Some(16));
            assert_eq!(consumer.slots_source, v1::ValueSource::Default as i32);
            assert_eq!(consumer.budget, None);
            assert_eq!(consumer.budget_source, v1::ValueSource::Unspecified as i32);
        }
    }

    #[test]
    fn a_signal_channel_carries_no_sizing() {
        let channel = channel_of(&section(), SIGNAL_ORDINAL, "primary");
        assert_eq!(channel.kind, v1::Kind::Signal as i32);
        assert_eq!(channel.member, "Level");
        assert_eq!(channel.depth, None);
        assert_eq!(channel.consumers.len(), 3);
        for consumer in &channel.consumers {
            assert_eq!(consumer.depth, None);
            assert_eq!(consumer.slots, None);
            assert_eq!(consumer.slots_source, v1::ValueSource::Unspecified as i32);
            assert_eq!(consumer.budget, None);
            assert_eq!(consumer.budget_source, v1::ValueSource::Unspecified as i32);
        }
    }

    #[test]
    fn instances_list_what_they_offer_and_what_they_map() {
        let section = section();
        assert_eq!(
            section.regions,
            [v1::Region {
                catalog: CATALOG.to_string(),
                hash: vec![7; 32],
                interfaces: vec![v1::RegionInterface {
                    name: INTERFACE.to_string(),
                    number: 1,
                    inline: false,
                    provisional: false,
                    service: SERVICE.to_string(),
                }],
            }]
        );
        let key = v1::InterfaceKey {
            catalog: CATALOG.to_string(),
            number: 1,
            name: INTERFACE.to_string(),
            inline: false,
        };
        let expected = [
            (
                PROVIDER,
                "primary",
                "head",
                false,
                vec![key.clone()],
                vec![],
            ),
            (PROVIDER, "backup", "zone", false, vec![key], vec![]),
            (DASH, "Unit", "head", false, vec![], vec![CATALOG]),
            (LOGGER, "Unit", "zone", false, vec![], vec![CATALOG]),
            (FLEET, "Unit", "cloud", true, vec![], vec![CATALOG]),
        ];
        assert_eq!(section.instances.len(), expected.len());
        for (instance, (component, name, machine, external, offers, maps)) in
            section.instances.iter().zip(expected)
        {
            assert_eq!(instance.component, component);
            assert_eq!(instance.instance, name);
            assert_eq!(instance.machine, machine);
            assert_eq!(instance.external, external);
            assert_eq!(instance.offers, offers);
            assert_eq!(
                instance.maps,
                maps.iter()
                    .map(|name| (*name).to_string())
                    .collect::<Vec<_>>()
            );
        }
    }

    /// The bytes of one section, for a comparison that covers every field.
    fn bytes(section: &v1::Deployment) -> String {
        let request = v1::CodegenRequest {
            deployment: Some(section.clone()),
            ..Default::default()
        };
        crate::codegen::request_to_json(&request).expect("the request renders")
    }

    /// The two lists the emitter sorts — a route's producers and a
    /// deployment's links — reach the section through that sort, so the
    /// section's bytes do not depend on the order the system IR holds them in,
    /// and the emitted order is the order the schema fixes whatever the input
    /// order is.
    ///
    /// `instances` is in the system IR's placement order by design, so a
    /// permuted placement list permutes that one list and changes nothing
    /// else.
    #[test]
    fn the_emitted_order_does_not_depend_on_the_order_of_the_inputs() {
        let package = package();
        let system = system();
        let first = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");

        let mut permuted = system.clone();
        let deployment = &mut permuted.deployments[0];
        deployment.links.reverse();
        for route in &mut deployment.routes {
            route.producers.reverse();
        }
        let second = lower_deployment(&permuted, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(bytes(&first), bytes(&second));

        // The order the permuted section is in is the schema's, not its
        // input's: the producers of a route ascending, the consumer links of
        // a channel ascending.
        let producers: Vec<String> = channels_of(&second, EVENT_ORDINAL)
            .iter()
            .map(|channel| {
                channel
                    .producer
                    .as_ref()
                    .expect("every channel carries a producer")
                    .instance
                    .clone()
            })
            .collect();
        assert_eq!(producers, ["backup", "primary"]);
        let consumers: Vec<String> = channel_of(&second, EVENT_ORDINAL, "primary")
            .consumers
            .iter()
            .map(|consumer| consumer.component.clone())
            .collect();
        assert_eq!(consumers, [DASH, FLEET, LOGGER]);

        // A permuted placement list permutes `instances`; with that one list
        // put back, every other field is unchanged.
        let mut reordered = system.clone();
        reordered.deployments[0].placements.reverse();
        let third = lower_deployment(&reordered, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(third.instances.len(), 5);
        assert_ne!(third.instances, first.instances);
        let rest = v1::Deployment {
            instances: first.instances.clone(),
            ..third
        };
        assert_eq!(bytes(&rest), bytes(&first));
    }

    #[test]
    fn a_route_whose_interface_is_not_among_the_packages_keeps_its_channel() {
        let system = system();
        // The caller hands in no package at all: nothing declares the route's
        // interface, so no member kind and no timing can be read.
        let section = lower_deployment(&system, "prod", &[]).expect("the fixture lowers");
        assert_eq!(section.channels.len(), 8);
        for channel in &section.channels {
            // The key, the producer and the consumer links are still written.
            assert_eq!(channel.catalog, CATALOG);
            assert_eq!(channel.interface_number, 1);
            assert_eq!(channel.interface, INTERFACE);
            assert!(channel.producer.is_some());
            assert_eq!(channel.consumers.len(), 3);
            // The kind is unspecified and no sizing value is invented.
            assert_eq!(channel.kind, v1::Kind::Unspecified as i32);
            assert_eq!(channel.depth, None);
            for consumer in &channel.consumers {
                // The crossing and the encoding do not depend on the member.
                assert_ne!(consumer.encoding, v1::Encoding::Unspecified as i32);
                assert_eq!(consumer.depth, None);
                assert_eq!(consumer.slots, None);
                assert_eq!(consumer.slots_source, v1::ValueSource::Unspecified as i32);
                assert_eq!(consumer.budget, None);
                assert_eq!(consumer.budget_source, v1::ValueSource::Unspecified as i32);
            }
        }
        // `inline` still comes from the region map, which the system carries.
        assert!(section.channels.iter().all(|channel| !channel.inline));
    }

    #[test]
    fn a_route_matches_its_interface_whatever_number_the_package_carries() {
        let mut package = package();
        // A package compiled against another lock, or before its own lock was
        // folded in: the number differs from the route's, the identity name
        // does not.
        package.interfaces[0].number = 0;
        package.interfaces[0].provisional = true;
        let system = system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let channel = channel_of(&section, EVENT_ORDINAL, "primary");
        assert_eq!(channel.kind, v1::Kind::Event as i32);
        // The channel's key stays the route's, not the package's.
        assert_eq!(channel.interface_number, 1);
        assert_eq!(
            channel.depth,
            Some(v1::Depth {
                value: Some(10),
                source: v1::ValueSource::Derived as i32,
            })
        );
    }

    #[test]
    fn a_producer_instance_no_link_names_has_a_channel_with_no_consumer() {
        let package = package();
        let mut system = system();
        // Every link of the backup instance is dropped: the instance is still
        // placed and still produces, so its channels are still emitted.
        system.deployments[0].links.retain(|link| {
            link.producer
                .as_ref()
                .is_some_and(|end| end.instance != "backup")
        });
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(section.channels.len(), 8);

        let event = channel_of(&section, EVENT_ORDINAL, "backup");
        assert!(event.consumers.is_empty());
        // With no consumer link, the ring depth is the member's contract
        // bound, which is what every link of it would carry.
        assert_eq!(
            event.depth,
            Some(v1::Depth {
                value: Some(10),
                source: v1::ValueSource::Derived as i32,
            })
        );
        let half_open = channel_of(&section, HALF_OPEN_ORDINAL, "backup");
        assert!(half_open.consumers.is_empty());
        assert_eq!(
            half_open.depth,
            Some(v1::Depth {
                value: None,
                source: v1::ValueSource::Underivable as i32,
            })
        );
        let query = channel_of(&section, QUERY_ORDINAL, "backup");
        assert!(query.consumers.is_empty());
        assert_eq!(query.depth, None);
        // The other producer instance keeps every link it had.
        assert_eq!(
            channel_of(&section, EVENT_ORDINAL, "primary")
                .consumers
                .len(),
            3
        );
    }

    #[test]
    fn an_inline_service_shape_is_matched_by_its_dotted_name() {
        // The interface moves into the service's inline shape, where
        // `Interface.name` is empty and the identity name is the service's
        // dotted name (ridl §14.5).
        let mut package = package();
        let mut interface = package.interfaces.remove(0);
        interface.name = String::new();
        package.services = vec![v2::Service {
            name: SERVICE.to_string(),
            shapes: vec![v2::ServiceShape {
                kind: Some(v2::service_shape::Kind::Inline(interface)),
            }],
            ..Default::default()
        }];

        let mut system = system();
        let inline_ref = v2::InterfaceRef {
            catalog: CATALOG.to_string(),
            name: SERVICE.to_string(),
            inline: true,
        };
        system.regions[0].interfaces[0].name = SERVICE.to_string();
        system.regions[0].interfaces[0].inline = true;
        for component in &mut system.components {
            for require in &mut component.requires {
                require.interface = Some(inline_ref.clone());
            }
        }
        let deployment = &mut system.deployments[0];
        for link in &mut deployment.links {
            link.interface = Some(inline_ref.clone());
        }
        for route in &mut deployment.routes {
            route.interface = SERVICE.to_string();
        }

        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let channel = channel_of(&section, EVENT_ORDINAL, "primary");
        assert!(channel.inline);
        assert_eq!(channel.interface, SERVICE);
        assert_eq!(channel.kind, v1::Kind::Event as i32);
        // The links are matched on the same triple, so none is lost.
        assert_eq!(channel.consumers.len(), 3);
        assert_eq!(
            section.instances[0].offers,
            [v1::InterfaceKey {
                catalog: CATALOG.to_string(),
                number: 1,
                name: SERVICE.to_string(),
                inline: true,
            }]
        );
    }

    // --- A second catalog shape: two interfaces of one catalog, each with a
    // consumer of its own. The fixtures above have one interface, so they
    // cannot show that a channel lists the links of its own interface only.

    const HORN: &str = "Horn";
    const HORN_SERVICE: &str = "veh.cabin.horn";
    const PANEL: &str = "veh.cabin.Panel";
    const TELEMETRY: &str = "veh.cabin.Telemetry";
    /// The ordinal of `Climate.setLevel`, the command member of the
    /// two-interface fixture. It is not one of the ordinals above, so a
    /// lookup that confuses the two fixtures finds nothing.
    const SET_LEVEL_ORDINAL: u32 = 7;

    fn interface_ref_of(name: &str, inline: bool) -> v2::InterfaceRef {
        v2::InterfaceRef {
            catalog: CATALOG.to_string(),
            name: name.to_string(),
            inline,
        }
    }

    /// One package whose catalog declares two interfaces: `Climate`, number
    /// 1, with an event and a command, and `Horn`, number 2, with one event.
    fn two_interface_package() -> v2::Package {
        v2::Package {
            name: CATALOG.to_string(),
            interfaces: vec![
                v2::Interface {
                    name: INTERFACE.to_string(),
                    number: 1,
                    interactions: vec![
                        member(
                            "TempChanged",
                            EVENT_ORDINAL,
                            v2::decl::Kind::EventDef(v2::EventDef {
                                payload: "Temp".to_string(),
                                // ceil(1000000 / 100000) = 10.
                                timing: Some(timing(Some("100000"), Some("1000000"))),
                            }),
                        ),
                        member(
                            "setLevel",
                            SET_LEVEL_ORDINAL,
                            v2::decl::Kind::CommandDef(v2::CommandDef::default()),
                        ),
                    ],
                    ..Default::default()
                },
                v2::Interface {
                    name: HORN.to_string(),
                    number: 2,
                    interactions: vec![member(
                        "active",
                        EVENT_ORDINAL,
                        v2::decl::Kind::EventDef(v2::EventDef {
                            payload: "Flag".to_string(),
                            // ceil(500000 / 100000) = 5.
                            timing: Some(timing(Some("100000"), Some("500000"))),
                        }),
                    )],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn two_interface_link(consumer: &str, interface: &str, service: &str) -> v2::Link {
        v2::Link {
            interface: Some(interface_ref_of(interface, false)),
            service: service.to_string(),
            consumer: Some(endpoint(consumer, "Unit", "head")),
            producer: Some(endpoint(PROVIDER, "primary", "head")),
            crossing: v2::Crossing::SameMachine as i32,
        }
    }

    fn two_interface_route(
        interface: &str,
        number: u32,
        name: &str,
        ordinal: u32,
        service: &str,
    ) -> v2::Route {
        v2::Route {
            catalog: CATALOG.to_string(),
            interface_number: number,
            member_ordinal: ordinal,
            interface: interface.to_string(),
            member: name.to_string(),
            service: service.to_string(),
            producers: vec![endpoint(PROVIDER, "primary", "head")],
        }
    }

    /// One system whose provider offers two services, one interface each, and
    /// whose two consumers require one interface each: `Panel` requires
    /// `Climate` and `Telemetry` requires `Horn`. `Horn`'s interface number is
    /// provisional and `Climate`'s is not.
    fn two_interface_system() -> v2::System {
        let mut provider = component("Provider", &["primary"], false);
        provider.package = CATALOG.to_string();
        provider.offers = vec![
            v2::Offer {
                service: SERVICE.to_string(),
                ..Default::default()
            },
            v2::Offer {
                service: HORN_SERVICE.to_string(),
                ..Default::default()
            },
        ];
        let consumer = |name: &str, interface: &str, service: &str| {
            let mut component = component(name, &["Unit"], false);
            component.package = CATALOG.to_string();
            component.requires = vec![v2::Require {
                interface: Some(interface_ref_of(interface, false)),
                service: service.to_string(),
                producer: PROVIDER.to_string(),
                ..Default::default()
            }];
            component
        };
        v2::System {
            name: "Cabin".to_string(),
            package: CATALOG.to_string(),
            components: vec![
                provider,
                consumer("Panel", INTERFACE, SERVICE),
                consumer("Telemetry", HORN, HORN_SERVICE),
            ],
            grants: vec![
                grant(PROVIDER, false, &[]),
                grant(PANEL, false, &[CATALOG]),
                grant(TELEMETRY, false, &[CATALOG]),
            ],
            regions: vec![v2::Region {
                catalog: CATALOG.to_string(),
                interfaces: vec![
                    v2::RegionInterface {
                        name: INTERFACE.to_string(),
                        inline: false,
                        number: 1,
                        provisional: false,
                        service: SERVICE.to_string(),
                    },
                    v2::RegionInterface {
                        name: HORN.to_string(),
                        inline: false,
                        number: 2,
                        provisional: true,
                        service: HORN_SERVICE.to_string(),
                    },
                ],
                hash: vec![3; 32],
            }],
            deployments: vec![v2::Deployment {
                name: "prod".to_string(),
                package: CATALOG.to_string(),
                machines: vec![v2::Machine {
                    name: "head".to_string(),
                    ..Default::default()
                }],
                placements: vec![
                    placement(PROVIDER, "primary", "head"),
                    placement(PANEL, "Unit", "head"),
                    placement(TELEMETRY, "Unit", "head"),
                ],
                links: vec![
                    two_interface_link(PANEL, INTERFACE, SERVICE),
                    two_interface_link(TELEMETRY, HORN, HORN_SERVICE),
                ],
                routes: vec![
                    two_interface_route(INTERFACE, 1, "TempChanged", EVENT_ORDINAL, SERVICE),
                    two_interface_route(INTERFACE, 1, "setLevel", SET_LEVEL_ORDINAL, SERVICE),
                    two_interface_route(HORN, 2, "active", EVENT_ORDINAL, HORN_SERVICE),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// `(interface, member ordinal, the consumer components)` of every
    /// channel, in emitted order.
    fn consumer_rows(section: &v1::Deployment) -> Vec<(&str, u32, Vec<&str>)> {
        section
            .channels
            .iter()
            .map(|channel| {
                (
                    channel.interface.as_str(),
                    channel.member_ordinal,
                    channel
                        .consumers
                        .iter()
                        .map(|consumer| consumer.component.as_str())
                        .collect(),
                )
            })
            .collect()
    }

    /// The kind of every channel, in emitted order.
    fn kind_rows(section: &v1::Deployment) -> Vec<(&str, i32)> {
        section
            .channels
            .iter()
            .map(|channel| (channel.member.as_str(), channel.kind))
            .collect()
    }

    /// A channel's consumer links are the links of that channel's own
    /// interface. `Panel` requires `Climate` and `Telemetry` requires `Horn`,
    /// so neither is listed on the other's channels.
    #[test]
    fn a_channel_lists_only_the_links_of_its_own_interface() {
        let package = two_interface_package();
        let system = two_interface_system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(
            consumer_rows(&section),
            [
                (INTERFACE, EVENT_ORDINAL, vec![PANEL]),
                (INTERFACE, SET_LEVEL_ORDINAL, vec![PANEL]),
                (HORN, EVENT_ORDINAL, vec![TELEMETRY]),
            ]
        );
    }

    /// A command channel is sized as a query channel is: sixteen slots from
    /// the default source on each consumer link, and no ring depth.
    #[test]
    fn a_command_channel_carries_sixteen_default_slots() {
        let package = two_interface_package();
        let system = two_interface_system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let channel = section
            .channels
            .iter()
            .find(|channel| channel.member_ordinal == SET_LEVEL_ORDINAL)
            .expect("the command member has a channel");
        assert_eq!(channel.kind, v1::Kind::Command as i32);
        assert_eq!(channel.member, "setLevel");
        assert_eq!(channel.depth, None);
        assert_eq!(channel.consumers.len(), 1);
        let consumer = &channel.consumers[0];
        assert_eq!(consumer.slots, Some(16));
        assert_eq!(consumer.slots_source, v1::ValueSource::Default as i32);
        assert_eq!(consumer.depth, None);
        assert_eq!(consumer.budget, None);
        assert_eq!(consumer.budget_source, v1::ValueSource::Unspecified as i32);
    }

    /// A region interface carries the `provisional` flag the system IR holds
    /// for it, so a plugin is never told that an unlocked interface number is
    /// final.
    #[test]
    fn a_region_interface_carries_its_own_provisional_flag() {
        let package = two_interface_package();
        let system = two_interface_system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let rows: Vec<(&str, u32, bool)> = section.regions[0]
            .interfaces
            .iter()
            .map(|interface| {
                (
                    interface.name.as_str(),
                    interface.number,
                    interface.provisional,
                )
            })
            .collect();
        assert_eq!(rows, [(INTERFACE, 1, false), (HORN, 2, true)]);
    }

    /// `inline` is read for one interface of a region, not for the region as a
    /// whole: with `Horn` moved into its service's inline shape and `Climate`
    /// left declared, each channel carries its own flag and keeps its own
    /// consumer link.
    #[test]
    fn one_inline_interface_does_not_make_its_sibling_inline() {
        let mut package = two_interface_package();
        let mut horn = package.interfaces.remove(1);
        horn.name = String::new();
        package.services = vec![v2::Service {
            name: HORN_SERVICE.to_string(),
            shapes: vec![v2::ServiceShape {
                kind: Some(v2::service_shape::Kind::Inline(horn)),
            }],
            ..Default::default()
        }];

        let mut system = two_interface_system();
        system.regions[0].interfaces[1].name = HORN_SERVICE.to_string();
        system.regions[0].interfaces[1].inline = true;
        let deployment = &mut system.deployments[0];
        deployment.links[1].interface = Some(interface_ref_of(HORN_SERVICE, true));
        deployment.routes[2].interface = HORN_SERVICE.to_string();

        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let rows: Vec<(&str, bool, Vec<&str>)> = section
            .channels
            .iter()
            .map(|channel| {
                (
                    channel.interface.as_str(),
                    channel.inline,
                    channel
                        .consumers
                        .iter()
                        .map(|consumer| consumer.component.as_str())
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (INTERFACE, false, vec![PANEL]),
                (INTERFACE, false, vec![PANEL]),
                (HORN_SERVICE, true, vec![TELEMETRY]),
            ]
        );
        // Each interface is still found, so each channel still has its kind.
        assert_eq!(
            kind_rows(&section),
            [
                ("TempChanged", v1::Kind::Event as i32),
                ("setLevel", v1::Kind::Command as i32),
                ("active", v1::Kind::Event as i32),
            ]
        );
    }

    /// The member's kind and timing are read from the package the route names
    /// as its catalog. An interface of the same name in another package does
    /// not supply them, even when it is the first package the caller hands in.
    #[test]
    fn an_interface_of_the_same_name_in_another_catalog_supplies_nothing() {
        let mut decoy = two_interface_package();
        decoy.name = "veh.other".to_string();
        for interface in &mut decoy.interfaces {
            for interaction in &mut interface.interactions {
                interaction.kind = Some(v2::decl::Kind::SignalDef(v2::SignalDef::default()));
            }
        }
        let package = two_interface_package();
        let system = two_interface_system();
        let section =
            lower_deployment(&system, "prod", &[&decoy, &package]).expect("the fixture lowers");
        assert_eq!(
            kind_rows(&section),
            [
                ("TempChanged", v1::Kind::Event as i32),
                ("setLevel", v1::Kind::Command as i32),
                ("active", v1::Kind::Event as i32),
            ]
        );
        // The timing comes from the same place, so the depth is the one the
        // route's own catalog states.
        let event = &section.channels[0];
        assert_eq!(
            event.depth,
            Some(v1::Depth {
                value: Some(10),
                source: v1::ValueSource::Derived as i32,
            })
        );
    }

    /// The member is found by ordinal; the route's member name is a label the
    /// section carries. A package whose ordinals disagree with the route's
    /// names — a stale package set — gives the kind of the ordinal.
    #[test]
    fn the_member_is_found_by_ordinal_and_not_by_name() {
        let mut package = two_interface_package();
        package.interfaces[0].interactions[0].ordinal = SET_LEVEL_ORDINAL;
        package.interfaces[0].interactions[1].ordinal = EVENT_ORDINAL;
        let system = two_interface_system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(
            kind_rows(&section),
            [
                ("TempChanged", v1::Kind::Command as i32),
                ("setLevel", v1::Kind::Event as i32),
                ("active", v1::Kind::Event as i32),
            ]
        );
    }

    /// A fixed member is a channel of its own kind with no sizing; a member
    /// declaration that is not an interaction gives the unspecified kind.
    #[test]
    fn a_fixed_member_and_a_member_that_is_not_an_interaction_carry_no_sizing() {
        let mut package = two_interface_package();
        package.interfaces[0].interactions = vec![
            member(
                "vin",
                EVENT_ORDINAL,
                v2::decl::Kind::FixedDef(v2::FixedDef::default()),
            ),
            member(
                "Temp",
                SET_LEVEL_ORDINAL,
                v2::decl::Kind::StructDef(v2::StructDef::default()),
            ),
        ];
        let system = two_interface_system();
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(
            kind_rows(&section),
            [
                ("TempChanged", v1::Kind::Fixed as i32),
                ("setLevel", v1::Kind::Unspecified as i32),
                ("active", v1::Kind::Event as i32),
            ]
        );
        for channel in &section.channels[..2] {
            assert_eq!(channel.depth, None);
            for consumer in &channel.consumers {
                assert_eq!(consumer.depth, None);
                assert_eq!(consumer.slots, None);
                assert_eq!(consumer.slots_source, v1::ValueSource::Unspecified as i32);
            }
        }
    }

    /// A crossing that is unspecified, and one carrying a value this schema
    /// does not list, are both written as the unspecified crossing with the
    /// unspecified encoding, so no discriminant outside the schema reaches the
    /// message and the request still renders.
    #[test]
    fn an_unspecified_or_unknown_crossing_is_written_as_unspecified() {
        let package = two_interface_package();
        let mut system = two_interface_system();
        system.deployments[0].links[0].crossing = v2::Crossing::Unspecified as i32;
        system.deployments[0].links[1].crossing = 99;
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let rows: Vec<(i32, i32)> = section
            .channels
            .iter()
            .flat_map(|channel| &channel.consumers)
            .map(|consumer| (consumer.crossing, consumer.encoding))
            .collect();
        assert_eq!(
            rows,
            [(
                v1::Crossing::Unspecified as i32,
                v1::Encoding::Unspecified as i32
            ); 3]
        );
        bytes(&section);
    }

    /// `Instance.external` is the component's flag and not the machine's: an
    /// external component on an inboard machine is still external, and a local
    /// component on an external machine is still local.
    #[test]
    fn an_instances_external_flag_is_the_components_and_not_the_machines() {
        let package = package();
        let mut system = system();
        for placed in &mut system.deployments[0].placements {
            if placed.component == FLEET {
                placed.machine = "head".to_string();
            } else if placed.component == DASH {
                placed.machine = "cloud".to_string();
            }
        }
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let rows: Vec<(&str, &str, bool)> = section
            .instances
            .iter()
            .map(|instance| {
                (
                    instance.component.as_str(),
                    instance.machine.as_str(),
                    instance.external,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (PROVIDER, "head", false),
                (PROVIDER, "zone", false),
                (DASH, "cloud", false),
                (LOGGER, "zone", false),
                (FLEET, "head", true),
            ]
        );
    }

    /// A placement whose component the system does not carry, and a component
    /// with no grant, each contribute an entry with empty lists rather than
    /// being left out: the instance is placed either way, and a plugin laying
    /// out its memory needs the placement.
    #[test]
    fn a_placement_outside_the_closure_and_a_component_with_no_grant_are_listed() {
        let package = package();
        let mut system = system();
        system.grants.retain(|grant| grant.component != DASH);
        system.deployments[0]
            .placements
            .push(placement("veh.cabin.Ghost", "Unit", "head"));
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(section.instances.len(), 6);
        let dash = section
            .instances
            .iter()
            .find(|instance| instance.component == DASH)
            .expect("a component with no grant is still listed");
        assert!(dash.maps.is_empty());
        assert!(dash.offers.is_empty());
        let ghost = section
            .instances
            .iter()
            .find(|instance| instance.component == "veh.cabin.Ghost")
            .expect("a placement outside the closure is still listed");
        assert!(!ghost.external);
        assert!(ghost.offers.is_empty());
        assert!(ghost.maps.is_empty());
    }

    /// The consumer links are sorted by component and then by instance, so two
    /// instances of one consuming component are in instance order and not in
    /// the order the link list happens to hold them.
    #[test]
    fn consumer_links_of_one_component_are_sorted_by_instance() {
        let package = package();
        let mut system = system();
        let dash = system
            .components
            .iter_mut()
            .find(|component| component.qualified_name() == DASH)
            .expect("Dash is a component of the fixture");
        dash.instances = vec!["right".to_string(), "left".to_string()];
        let deployment = &mut system.deployments[0];
        deployment
            .placements
            .retain(|placed| placed.component != DASH);
        deployment.placements.push(placement(DASH, "right", "head"));
        deployment.placements.push(placement(DASH, "left", "head"));
        deployment.links.retain(|link| {
            link.consumer
                .as_ref()
                .is_some_and(|consumer| consumer.component != DASH)
        });
        // `right` is written before `left`, so a sort by component alone keeps
        // that order.
        deployment.links.insert(
            0,
            link(
                endpoint(DASH, "right", "head"),
                endpoint(PROVIDER, "primary", "head"),
                v2::Crossing::SameMachine,
            ),
        );
        deployment.links.insert(
            1,
            link(
                endpoint(DASH, "left", "head"),
                endpoint(PROVIDER, "primary", "head"),
                v2::Crossing::SameMachine,
            ),
        );
        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        let channel = channel_of(&section, EVENT_ORDINAL, "primary");
        let rows: Vec<(&str, &str)> = channel
            .consumers
            .iter()
            .map(|consumer| (consumer.component.as_str(), consumer.instance.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (DASH, "left"),
                (DASH, "right"),
                (FLEET, "Unit"),
                (LOGGER, "Unit"),
            ]
        );
    }

    /// The ring depth is the maximum over the consumer links, with the source
    /// of the link that supplies it, and absent as soon as one link's depth is
    /// absent.
    ///
    /// No input of the emitter makes two links of one channel differ: every
    /// link is given the member's contract bound. The rule is therefore
    /// pinned on the function, and the aggregation cannot be told apart from
    /// the bound itself through [`lower_deployment`].
    #[test]
    fn the_ring_depth_is_the_deepest_link_and_absent_when_any_link_is() {
        let derived = |value: u32| v1::Depth {
            value: Some(value),
            source: v1::ValueSource::Derived as i32,
        };
        let underivable = v1::Depth {
            value: None,
            source: v1::ValueSource::Underivable as i32,
        };
        let declared = v1::Depth {
            value: Some(9),
            source: v1::ValueSource::Declared as i32,
        };
        let at = |depth: v1::Depth| v1::Consumer {
            depth: Some(depth),
            ..Default::default()
        };
        let bound = derived(4);
        assert_eq!(super::ring_depth(&[], bound), bound);
        assert_eq!(
            super::ring_depth(&[at(derived(2)), at(derived(7)), at(derived(5))], bound),
            derived(7)
        );
        assert_eq!(
            super::ring_depth(&[at(derived(7)), at(derived(2))], bound),
            derived(7)
        );
        assert_eq!(
            super::ring_depth(&[at(derived(2)), at(underivable), at(derived(7))], bound),
            underivable
        );
        assert_eq!(super::ring_depth(&[at(underivable)], bound), underivable);
        // The deepest link states where its value came from.
        assert_eq!(
            super::ring_depth(&[at(derived(3)), at(declared)], bound),
            declared
        );
        // A link with no depth at all is a link whose depth is absent.
        assert_eq!(
            super::ring_depth(&[v1::Consumer::default()], bound),
            underivable
        );
    }

    /// A derived depth needs both bounds, whichever one a half-open range
    /// leaves out, and a lower bound of zero is not a divisor. `@[..50ms]`
    /// and `@[100ms..]` are both source syntax, so both spellings reach this
    /// function; neither bound has a default here.
    #[test]
    fn a_depth_is_derived_only_from_two_bounds_neither_of_which_defaults() {
        let underivable = v1::Depth {
            value: None,
            source: v1::ValueSource::Underivable as i32,
        };
        let derived = v1::Depth {
            value: Some(10),
            source: v1::ValueSource::Derived as i32,
        };
        // `@[100ms..1s]`: both bounds.
        let both = timing(Some("100000"), Some("1000000"));
        assert_eq!(super::depth_of(Some(&both)), derived);
        // `@[100ms..]`: no upper bound. An upper bound defaulted to the lower
        // one would answer 1 here.
        let no_max = timing(Some("100000"), None);
        assert_eq!(super::depth_of(Some(&no_max)), underivable);
        // `@[..1s]`: no lower bound. A lower bound defaulted to 1 microsecond
        // would answer 1000000 here.
        let no_min = timing(None, Some("1000000"));
        assert_eq!(super::depth_of(Some(&no_min)), underivable);
        // Neither bound, and no timing at all.
        let neither = timing(None, None);
        assert_eq!(super::depth_of(Some(&neither)), underivable);
        assert_eq!(super::depth_of(None), underivable);
        // A lower bound of zero: no quotient, and not treated as one.
        let zero_min = timing(Some("0"), Some("1000000"));
        assert_eq!(super::depth_of(Some(&zero_min)), underivable);
    }

    /// `inline` is read from the region entry whose catalog, number and name
    /// all match the route's.
    ///
    /// The catalog term is the one a lowered system exercises: two catalogs
    /// can hold an interface of the same number and the same name, and only
    /// one of them is the route's. The number and the name each identify an
    /// interface on their own within one catalog, so a pair that matches
    /// neither entry reaches this lookup only in a system built by hand,
    /// which is what the last two cases are.
    #[test]
    fn inline_is_read_from_the_entry_matching_catalog_number_and_name() {
        let mut system = two_interface_system();
        // `Horn`, number 2 of `veh.cabin`, is the one inline entry.
        system.regions[0].interfaces[1].inline = true;
        // Another catalog, walked first, whose entry carries the number and
        // the name of `veh.cabin`'s `Climate` and is inline.
        system.regions.insert(
            0,
            v2::Region {
                catalog: "veh.aaa".to_string(),
                interfaces: vec![v2::RegionInterface {
                    name: INTERFACE.to_string(),
                    inline: true,
                    number: 1,
                    provisional: false,
                    service: "veh.aaa.climate".to_string(),
                }],
                hash: vec![9; 32],
            },
        );
        let at = |interface: &str, number: u32| {
            let route = two_interface_route(interface, number, "m", EVENT_ORDINAL, SERVICE);
            super::inline_of(&system, &route)
        };
        // The route's catalog is `veh.cabin`, whose `Climate` is declared.
        assert!(!at(INTERFACE, 1));
        assert!(at(HORN, 2));
        // A pair that matches no entry of the catalog is not inline.
        assert!(!at(INTERFACE, 2));
        assert!(!at(HORN, 1));
    }

    /// What an instance offers is walked in region order and then in
    /// interface number order, which is neither the order the component
    /// writes its `offers` lines in nor the reverse of the walk.
    #[test]
    fn what_an_instance_offers_is_in_region_order_then_interface_number_order() {
        const AUX: &str = "Aux";
        const AUX_CATALOG: &str = "veh.zzz";
        const AUX_SERVICE: &str = "veh.zzz.aux";

        let package = two_interface_package();
        let mut system = two_interface_system();
        // A second catalog, after `veh.cabin` in the region map, with one
        // interface the provider also offers.
        system.regions.push(v2::Region {
            catalog: AUX_CATALOG.to_string(),
            interfaces: vec![v2::RegionInterface {
                name: AUX.to_string(),
                inline: false,
                number: 1,
                provisional: false,
                service: AUX_SERVICE.to_string(),
            }],
            hash: vec![5; 32],
        });
        // The offer lines are written in the reverse of the emitted order, so
        // the list below is the walk's order and not the lines'.
        let provider = &mut system.components[0];
        provider.offers = [AUX_SERVICE, HORN_SERVICE, SERVICE]
            .iter()
            .map(|service| v2::Offer {
                service: (*service).to_string(),
                ..Default::default()
            })
            .collect();

        let section = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(
            section.instances[0].offers,
            [
                v1::InterfaceKey {
                    catalog: CATALOG.to_string(),
                    number: 1,
                    name: INTERFACE.to_string(),
                    inline: false,
                },
                v1::InterfaceKey {
                    catalog: CATALOG.to_string(),
                    number: 2,
                    name: HORN.to_string(),
                    inline: false,
                },
                v1::InterfaceKey {
                    catalog: AUX_CATALOG.to_string(),
                    number: 1,
                    name: AUX.to_string(),
                    inline: false,
                },
            ]
        );
    }
}
