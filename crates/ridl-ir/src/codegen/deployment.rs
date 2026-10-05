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
        // A later task fills the binding table the toolchain holds.
        bindings: Vec::new(),
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
                        crossing: link.crossing,
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
                depth: bound.map(|bound| ring_depth(&consumers, bound)),
                consumers,
            });
        }
    }
    channels
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
/// interface number and identity name together — the number is the routing
/// identity and the name tells two shapes apart when a package was compiled
/// before its lock was folded in and every number is still 0. The member is
/// found by ordinal. A route whose interface or member is not found gives
/// `KIND_UNSPECIFIED` and no timing, so the channel is still emitted with its
/// producer and its consumer links and carries no sizing it cannot justify.
fn member_kind<'a>(
    packages: &[&'a v2::Package],
    route: &v2::Route,
) -> (v1::Kind, Option<&'a v2::Timing>) {
    let found = packages
        .iter()
        .copied()
        .filter(|package| package.name == route.catalog)
        .flat_map(v2::Package::shapes)
        .find(|shape| {
            shape.interface.number == route.interface_number && shape.name == route.interface
        });
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
/// A channel with no consumer link takes `bound`, the contract bound of its
/// member: that is the value every consumer link of it would carry.
fn ring_depth(consumers: &[v1::Consumer], bound: v1::Depth) -> v1::Depth {
    if consumers.is_empty() {
        return bound;
    }
    let mut max = 0u32;
    for consumer in consumers {
        match consumer.depth.and_then(|depth| depth.value) {
            Some(value) => max = max.max(value),
            None => {
                return v1::Depth {
                    value: None,
                    source: v1::ValueSource::Underivable as i32,
                };
            }
        }
    }
    v1::Depth {
        value: Some(max),
        source: v1::ValueSource::Derived as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::lower_deployment;
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
        // A later task fills the binding table from the toolchain.
        assert!(section.bindings.is_empty());

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

    #[test]
    fn reordering_placements_in_the_system_does_not_change_the_bytes() {
        // `instances` is in the system IR's placement order by design, so a
        // permuted placement list permutes it; the test sorts that one list
        // in both sections and then holds every other list, and every value,
        // byte-identical.
        let package = package();
        let mut system = system();
        let first = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        system.deployments[0].placements.reverse();
        let second = lower_deployment(&system, "prod", &[&package]).expect("the fixture lowers");
        assert_eq!(first.instances.len(), 5);
        assert_eq!(second.instances.len(), 5);
        assert_ne!(first.instances, second.instances);

        let normalized = |mut section: v1::Deployment| {
            section
                .instances
                .sort_by(|a, b| (&a.component, &a.instance).cmp(&(&b.component, &b.instance)));
            let request = v1::CodegenRequest {
                deployment: Some(section),
                ..Default::default()
            };
            crate::codegen::request_to_json(&request).expect("the request renders")
        };
        assert_eq!(normalized(first), normalized(second));
    }
}
