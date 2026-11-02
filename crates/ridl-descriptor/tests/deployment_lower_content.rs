//! The content of a lowered deployment descriptor, checked against values
//! written out independently of the lowering: several catalogs, several
//! requirements with distinct attributes, links of every crossing and
//! surface entries that name a later link.

#![cfg(feature = "std")]

use std::collections::BTreeMap;

use ridl_descriptor::deployment::{
    Attribute, AttributeValue, Component, Crossing, DeployedInterface, Deployment,
    DeploymentLowerError, EmbeddedCatalog, Endpoint, Instance, InterfaceKey, Link, Machine, Route,
    SCHEMA_VERSION, Surface, SurfaceDirection, lower, to_json, verify,
};
use ridl_descriptor::{Catalog, finish};
use ridl_ir::v2;
use serde_json::json;

const VEH_HASH: [u8; 32] = [9; 32];
const NAV_HASH: [u8; 32] = [5; 32];

fn catalog_bytes(name: &str, hash: &[u8; 32]) -> Vec<u8> {
    finish(&Catalog {
        version: ridl_descriptor::SCHEMA_VERSION,
        name: name.to_owned(),
        hash: hash.to_vec(),
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![],
        retired: vec![],
        compatible: None,
    })
}

fn catalogs() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        ("nav".to_owned(), catalog_bytes("nav", &NAV_HASH)),
        ("veh".to_owned(), catalog_bytes("veh", &VEH_HASH)),
    ])
}

fn scalar(text: &str) -> v2::AttributeValue {
    v2::AttributeValue {
        kind: Some(v2::attribute_value::Kind::Scalar(text.to_owned())),
    }
}

fn attr(namespace: &str, key: &str, value: &str) -> v2::Attribute {
    v2::Attribute {
        namespace: namespace.to_owned(),
        key: key.to_owned(),
        value: Some(scalar(value)),
    }
}

fn owned_attr(ns: &str, key: &str, value: &str) -> Attribute {
    Attribute {
        ns: ns.to_owned(),
        key: key.to_owned(),
        value: Some(Box::new(AttributeValue {
            scalar: Some(value.to_owned()),
            list: None,
        })),
    }
}

fn ep(component: &str, instance: &str, machine: &str) -> v2::Endpoint {
    v2::Endpoint {
        component: component.to_owned(),
        instance: instance.to_owned(),
        machine: machine.to_owned(),
    }
}

fn owned_ep(component: &str, instance: &str, machine: &str) -> Endpoint {
    Endpoint {
        component: component.to_owned(),
        instance: instance.to_owned(),
        machine: machine.to_owned(),
    }
}

fn iface(catalog: &str, name: &str, inline: bool) -> v2::InterfaceRef {
    v2::InterfaceRef {
        catalog: catalog.to_owned(),
        name: name.to_owned(),
        inline,
    }
}

fn region_interface(
    name: &str,
    inline: bool,
    number: u32,
    provisional: bool,
    service: &str,
) -> v2::RegionInterface {
    v2::RegionInterface {
        name: name.to_owned(),
        inline,
        number,
        provisional,
        service: service.to_owned(),
    }
}

fn require(interface: v2::InterfaceRef, service: &str, producer: &str, qos: &str) -> v2::Require {
    v2::Require {
        interface: Some(interface),
        service: service.to_owned(),
        producer: producer.to_owned(),
        attributes: vec![attr("net", "qos", qos)],
        ..Default::default()
    }
}

fn link(
    interface: v2::InterfaceRef,
    service: &str,
    producer: v2::Endpoint,
    crossing: v2::Crossing,
) -> v2::Link {
    v2::Link {
        interface: Some(interface),
        service: service.to_owned(),
        consumer: Some(ep("veh.Consumer", "c0", "ui")),
        producer: Some(producer),
        crossing: crossing as i32,
    }
}

fn component(name: &str, instances: &[&str]) -> v2::Component {
    v2::Component {
        name: name.to_owned(),
        package: "veh".to_owned(),
        instances: instances.iter().map(|i| (*i).to_owned()).collect(),
        ..Default::default()
    }
}

fn placement(component: &str, instance: &str, machine: &str) -> v2::Placement {
    v2::Placement {
        component: component.to_owned(),
        instance: instance.to_owned(),
        machine: machine.to_owned(),
        ..Default::default()
    }
}

/// Three components over two catalogs. The provider offers two services, so
/// it offers three interfaces across both catalogs; the consumer requires
/// three interfaces with different attributes and maps both catalogs.
fn system() -> v2::System {
    let climate = iface("veh", "Climate", false);
    let map = iface("nav", "Map", false);
    let seat = iface("veh", "Seat", false);
    let links = vec![
        link(
            climate.clone(),
            "veh.ClimateService",
            ep("veh.Provider", "p0", "hpc"),
            v2::Crossing::SameMachine,
        ),
        link(
            map.clone(),
            "nav.MapService",
            ep("veh.Provider", "p0", "hpc"),
            v2::Crossing::OffBoard,
        ),
        link(
            seat.clone(),
            "veh.SeatService",
            ep("veh.Other", "o0", "hpc"),
            v2::Crossing::DifferentMachine,
        ),
    ];
    let mut provider = component("Provider", &["p0"]);
    provider.labels = vec!["core".to_owned(), "safe".to_owned()];
    provider.external = true;
    provider.attributes = vec![attr("c", "role", "provider")];
    provider.offers = ["veh.ClimateService", "nav.MapService"]
        .iter()
        .map(|service| v2::Offer {
            service: (*service).to_owned(),
            ..Default::default()
        })
        .collect();
    let mut other = component("Other", &["o0"]);
    other.implicit = true;
    other.offers = vec![v2::Offer {
        service: "veh.SeatService".to_owned(),
        ..Default::default()
    }];
    let mut consumer = component("Consumer", &["c0"]);
    consumer.requires = vec![
        require(climate, "veh.ClimateService", "veh.Provider", "high"),
        require(map, "nav.MapService", "veh.Provider", "low"),
        require(seat, "veh.SeatService", "veh.Other", "mid"),
    ];
    v2::System {
        name: "Vehicle".to_owned(),
        package: "veh".to_owned(),
        attributes: vec![attr("sys", "tier", "system-value")],
        components: vec![other, consumer, provider],
        grants: vec![v2::Grant {
            component: "veh.Consumer".to_owned(),
            external: false,
            regions: vec!["nav".to_owned(), "veh".to_owned()],
        }],
        regions: vec![
            v2::Region {
                catalog: "nav".to_owned(),
                hash: NAV_HASH.to_vec(),
                interfaces: vec![region_interface("Map", false, 1, false, "nav.MapService")],
            },
            v2::Region {
                catalog: "veh".to_owned(),
                hash: VEH_HASH.to_vec(),
                interfaces: vec![
                    region_interface("Climate", false, 1, false, "veh.ClimateService"),
                    region_interface("Seat", false, 2, true, "veh.SeatService"),
                    region_interface("veh.ClimateService", true, 3, false, "veh.ClimateService"),
                ],
            },
        ],
        deployments: vec![v2::Deployment {
            name: "Prod".to_owned(),
            package: "veh".to_owned(),
            attributes: vec![attr("dep", "site", "deployment-value")],
            machines: vec![
                v2::Machine {
                    name: "hpc".to_owned(),
                    external: true,
                    labels: vec!["fast".to_owned(), "gen3".to_owned()],
                    attributes: vec![attr("m", "cores", "8")],
                    ..Default::default()
                },
                v2::Machine {
                    name: "ui".to_owned(),
                    ..Default::default()
                },
            ],
            placements: vec![
                placement("veh.Provider", "p0", "hpc"),
                placement("veh.Other", "o0", "hpc"),
                placement("veh.Consumer", "c0", "ui"),
            ],
            routes: vec![v2::Route {
                catalog: "veh".to_owned(),
                interface_number: 2,
                member_ordinal: 4,
                interface: "Seat".to_owned(),
                member: "recline".to_owned(),
                service: "veh.SeatService".to_owned(),
                producers: vec![
                    ep("veh.Other", "o0", "hpc"),
                    ep("veh.Provider", "p0", "hpc"),
                ],
            }],
            surface: vec![
                v2::Surface {
                    link: Some(links[2].clone()),
                    direction: v2::SurfaceDirection::ExternalOffers as i32,
                },
                v2::Surface {
                    link: Some(links[1].clone()),
                    direction: v2::SurfaceDirection::ExternalConsumes as i32,
                },
            ],
            links,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn owned_of(bytes: &[u8]) -> Deployment {
    Deployment::try_from(verify(bytes).expect("verifies")).expect("converts")
}

fn lowered() -> Vec<u8> {
    lower(&system(), "Prod", &[], &catalogs()).expect("lowers")
}

fn owned_link(
    catalog: &str,
    number: u32,
    service: &str,
    producer: Endpoint,
    crossing: Crossing,
    qos: &str,
) -> Link {
    Link {
        catalog: catalog.to_owned(),
        number,
        service: service.to_owned(),
        consumer: Box::new(owned_ep("veh.Consumer", "c0", "ui")),
        producer: Box::new(producer),
        crossing,
        attributes: vec![owned_attr("net", "qos", qos)],
    }
}

fn expected() -> Deployment {
    let catalogs = catalogs();
    let interface =
        |catalog: &str, number, name: &str, inline, provisional, service: &str| DeployedInterface {
            catalog: catalog.to_owned(),
            number,
            name: name.to_owned(),
            inline,
            provisional,
            service: service.to_owned(),
        };
    let key = |catalog: &str, number| InterfaceKey {
        catalog: catalog.to_owned(),
        number,
    };
    Deployment {
        version: SCHEMA_VERSION,
        system: "veh.Vehicle".to_owned(),
        name: "Prod".to_owned(),
        package: "veh".to_owned(),
        catalogs: vec![
            EmbeddedCatalog {
                name: "nav".to_owned(),
                hash: NAV_HASH.to_vec(),
                bytes: catalogs["nav"].clone(),
            },
            EmbeddedCatalog {
                name: "veh".to_owned(),
                hash: VEH_HASH.to_vec(),
                bytes: catalogs["veh"].clone(),
            },
        ],
        interfaces: vec![
            interface("nav", 1, "Map", false, false, "nav.MapService"),
            interface("veh", 1, "Climate", false, false, "veh.ClimateService"),
            interface("veh", 2, "Seat", false, true, "veh.SeatService"),
            interface(
                "veh",
                3,
                "veh.ClimateService",
                true,
                false,
                "veh.ClimateService",
            ),
        ],
        machines: vec![
            Machine {
                name: "hpc".to_owned(),
                external: true,
                labels: vec!["fast".to_owned(), "gen3".to_owned()],
                attributes: vec![owned_attr("m", "cores", "8")],
            },
            Machine {
                name: "ui".to_owned(),
                external: false,
                labels: vec![],
                attributes: vec![],
            },
        ],
        components: vec![
            Component {
                name: "veh.Other".to_owned(),
                implicit: true,
                external: false,
                instances: vec!["o0".to_owned()],
                labels: vec![],
                attributes: vec![],
            },
            Component {
                name: "veh.Consumer".to_owned(),
                implicit: false,
                external: false,
                instances: vec!["c0".to_owned()],
                labels: vec![],
                attributes: vec![],
            },
            Component {
                name: "veh.Provider".to_owned(),
                implicit: false,
                external: true,
                instances: vec!["p0".to_owned()],
                labels: vec!["core".to_owned(), "safe".to_owned()],
                attributes: vec![owned_attr("c", "role", "provider")],
            },
        ],
        instances: vec![
            Instance {
                component: "veh.Provider".to_owned(),
                instance: "p0".to_owned(),
                machine: "hpc".to_owned(),
                external: true,
                attributes: vec![],
                offers: vec![key("nav", 1), key("veh", 1), key("veh", 3)],
                maps: vec![],
            },
            Instance {
                component: "veh.Other".to_owned(),
                instance: "o0".to_owned(),
                machine: "hpc".to_owned(),
                external: false,
                attributes: vec![],
                offers: vec![key("veh", 2)],
                maps: vec![],
            },
            Instance {
                component: "veh.Consumer".to_owned(),
                instance: "c0".to_owned(),
                machine: "ui".to_owned(),
                external: false,
                attributes: vec![],
                offers: vec![],
                maps: vec!["nav".to_owned(), "veh".to_owned()],
            },
        ],
        links: vec![
            owned_link(
                "veh",
                1,
                "veh.ClimateService",
                owned_ep("veh.Provider", "p0", "hpc"),
                Crossing::SameMachine,
                "high",
            ),
            owned_link(
                "nav",
                1,
                "nav.MapService",
                owned_ep("veh.Provider", "p0", "hpc"),
                Crossing::OffBoard,
                "low",
            ),
            owned_link(
                "veh",
                2,
                "veh.SeatService",
                owned_ep("veh.Other", "o0", "hpc"),
                Crossing::DifferentMachine,
                "mid",
            ),
        ],
        routes: vec![Route {
            catalog: "veh".to_owned(),
            number: 2,
            ordinal: 4,
            interface: "Seat".to_owned(),
            member: "recline".to_owned(),
            service: "veh.SeatService".to_owned(),
            producers: vec![
                owned_ep("veh.Other", "o0", "hpc"),
                owned_ep("veh.Provider", "p0", "hpc"),
            ],
        }],
        surface: vec![
            Surface {
                link: 2,
                direction: SurfaceDirection::ExternalOffers,
            },
            Surface {
                link: 1,
                direction: SurfaceDirection::ExternalConsumes,
            },
        ],
        system_attributes: vec![owned_attr("sys", "tier", "system-value")],
        attributes: vec![owned_attr("dep", "site", "deployment-value")],
    }
}

#[test]
fn the_whole_descriptor_equals_the_independently_written_value() {
    assert_eq!(owned_of(&lowered()), expected());
}

#[test]
fn offers_and_maps_equal_the_codegen_section_for_every_placement() {
    let system = system();
    let section = ridl_ir::codegen::lower_deployment(&system, "Prod", &[]).expect("section");
    let descriptor = owned_of(&lowered());
    assert_eq!(section.instances.len(), descriptor.instances.len());
    for from_section in &section.instances {
        let instance = descriptor
            .instances
            .iter()
            .find(|candidate| {
                candidate.component == from_section.component
                    && candidate.instance == from_section.instance
            })
            .expect("the placement is in the descriptor");
        let offers: Vec<(String, u32)> = from_section
            .offers
            .iter()
            .map(|key| (key.catalog.clone(), key.number))
            .collect();
        let written: Vec<(String, u32)> = instance
            .offers
            .iter()
            .map(|key| (key.catalog.clone(), key.number))
            .collect();
        assert_eq!(written, offers, "{}", from_section.instance);
        assert_eq!(
            instance.maps, from_section.maps,
            "{}",
            from_section.instance
        );
    }
    // The comparison is not vacuous: one instance offers three interfaces
    // and one maps two catalogs.
    assert!(section.instances.iter().any(|i| i.offers.len() == 3));
    assert!(section.instances.iter().any(|i| i.maps.len() == 2));
}

#[test]
fn an_unspecified_surface_direction_is_an_internal_error() {
    let mut system = system();
    system.deployments[0].surface[0].direction = v2::SurfaceDirection::Unspecified as i32;
    let error = lower(&system, "Prod", &[], &catalogs()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::Internal(text) if text.contains("direction")));
}

#[test]
fn a_surface_link_absent_from_a_nonempty_list_is_an_internal_error() {
    let mut system = system();
    let stranger = link(
        iface("veh", "Stranger", false),
        "veh.Other",
        ep("veh.Other", "o0", "hpc"),
        v2::Crossing::SameMachine,
    );
    system.deployments[0].surface[1].link = Some(stranger);
    let error = lower(&system, "Prod", &[], &catalogs()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::Internal(text) if text.contains("surface")));
}

#[test]
fn the_json_view_equals_the_independently_written_value() {
    let bytes = lowered();
    let value = to_json(verify(&bytes).expect("verifies")).expect("renders");
    let attribute = |ns: &str, key: &str, text: &str| json!({ "ns": ns, "key": key, "value": { "scalar": text, "list": null } });
    let endpoint = |component: &str, instance: &str, machine: &str| json!({ "component": component, "instance": instance, "machine": machine });
    let link = |catalog: &str,
                number: u32,
                service: &str,
                producer: (&str, &str, &str),
                crossing: &str,
                qos: &str| {
        json!({
            "catalog": catalog,
            "number": number,
            "service": service,
            "consumer": endpoint("veh.Consumer", "c0", "ui"),
            "producer": endpoint(producer.0, producer.1, producer.2),
            "crossing": crossing,
            "attributes": [attribute("net", "qos", qos)],
        })
    };
    assert_eq!(value["version"], 1);
    assert_eq!(value["system"], "veh.Vehicle");
    assert_eq!(value["name"], "Prod");
    assert_eq!(value["package"], "veh");
    assert_eq!(
        value["links"],
        json!([
            link(
                "veh",
                1,
                "veh.ClimateService",
                ("veh.Provider", "p0", "hpc"),
                "SameMachine",
                "high"
            ),
            link(
                "nav",
                1,
                "nav.MapService",
                ("veh.Provider", "p0", "hpc"),
                "OffBoard",
                "low"
            ),
            link(
                "veh",
                2,
                "veh.SeatService",
                ("veh.Other", "o0", "hpc"),
                "DifferentMachine",
                "mid"
            ),
        ])
    );
    assert_eq!(
        value["routes"],
        json!([{
            "catalog": "veh",
            "number": 2,
            "ordinal": 4,
            "interface": "Seat",
            "member": "recline",
            "service": "veh.SeatService",
            "producers": [endpoint("veh.Other", "o0", "hpc"), endpoint("veh.Provider", "p0", "hpc")],
        }])
    );
    assert_eq!(
        value["surface"],
        json!([
            { "link": 2, "direction": "ExternalOffers" },
            { "link": 1, "direction": "ExternalConsumes" },
        ])
    );
    assert_eq!(
        value["machines"],
        json!([
            {
                "name": "hpc",
                "external": true,
                "labels": ["fast", "gen3"],
                "attributes": [attribute("m", "cores", "8")],
            },
            { "name": "ui", "external": false, "labels": [], "attributes": [] },
        ])
    );
    assert_eq!(
        value["instances"][0],
        json!({
            "component": "veh.Provider",
            "instance": "p0",
            "machine": "hpc",
            "external": true,
            "attributes": [],
            "offers": [
                { "catalog": "nav", "number": 1 },
                { "catalog": "veh", "number": 1 },
                { "catalog": "veh", "number": 3 },
            ],
            "maps": [],
        })
    );
    assert_eq!(value["instances"][2]["maps"], json!(["nav", "veh"]));
    assert_eq!(
        value["interfaces"][2],
        json!({
            "catalog": "veh",
            "number": 2,
            "name": "Seat",
            "inline": false,
            "provisional": true,
            "service": "veh.SeatService",
        })
    );
    assert_eq!(value["components"][2]["labels"], json!(["core", "safe"]));
    assert_eq!(value["components"][0]["implicit"], true);
    assert_eq!(
        value["system_attributes"],
        json!([attribute("sys", "tier", "system-value")])
    );
    assert_eq!(
        value["attributes"],
        json!([attribute("dep", "site", "deployment-value")])
    );
    assert_eq!(value["catalogs"][0]["name"], "nav");
    assert_eq!(value["catalogs"][1]["name"], "veh");
    assert_eq!(value["catalogs"][1]["catalog"]["name"], "veh");
    assert_eq!(value["catalogs"][0]["hash"], json!(NAV_HASH.to_vec()));
}
