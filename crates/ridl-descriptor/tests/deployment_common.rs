//! Builders shared by the deployment descriptor tests.

use ridl_descriptor::Catalog;
use ridl_descriptor::deployment::{
    Attribute, AttributeValue, Component, Crossing, DeployedInterface, Deployment, EmbeddedCatalog,
    Endpoint, Instance, InterfaceKey, Link, Machine, Route, SCHEMA_VERSION, Surface,
    SurfaceDirection,
};

/// The bytes of a minimal, valid catalog descriptor.
pub fn catalog_bytes() -> Vec<u8> {
    ridl_descriptor::finish(&Catalog {
        version: ridl_descriptor::SCHEMA_VERSION,
        name: "veh".to_owned(),
        hash: vec![7u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![],
        retired: vec![],
        compatible: None,
    })
}

pub fn endpoint(component: &str) -> Endpoint {
    Endpoint {
        component: component.to_owned(),
        instance: format!("{component}0"),
        machine: "hpc".to_owned(),
    }
}

/// A flag, a scalar and a list that nests a list: two levels deep.
pub fn attributes() -> Vec<Attribute> {
    let scalar = |text: &str| AttributeValue {
        scalar: Some(text.to_owned()),
        list: None,
    };
    vec![
        Attribute {
            ns: "rust".to_owned(),
            key: "flag".to_owned(),
            value: None,
        },
        Attribute {
            ns: "rust".to_owned(),
            key: "id".to_owned(),
            value: Some(Box::new(scalar("42"))),
        },
        Attribute {
            ns: "rust".to_owned(),
            key: "nested".to_owned(),
            value: Some(Box::new(AttributeValue {
                scalar: None,
                list: Some(vec![
                    scalar("a"),
                    AttributeValue {
                        scalar: None,
                        list: Some(vec![scalar("deep")]),
                    },
                ]),
            })),
        },
    ]
}

/// A deployment with one of every table.
pub fn deployment(version: u32, embedded: Vec<u8>) -> Deployment {
    Deployment {
        version,
        system: "veh.Vehicle".to_owned(),
        name: "Prod".to_owned(),
        package: "veh".to_owned(),
        catalogs: vec![EmbeddedCatalog {
            name: "veh".to_owned(),
            hash: vec![7u8; 32],
            bytes: embedded,
        }],
        interfaces: vec![DeployedInterface {
            catalog: "veh".to_owned(),
            number: 3,
            name: "Cabin".to_owned(),
            inline: true,
            provisional: true,
            service: "cabin".to_owned(),
        }],
        machines: vec![Machine {
            name: "hpc".to_owned(),
            external: true,
            labels: vec!["gen3".to_owned()],
            attributes: attributes(),
        }],
        components: vec![Component {
            name: "Climate".to_owned(),
            implicit: true,
            external: true,
            instances: vec!["Climate0".to_owned()],
            labels: vec!["safety".to_owned()],
            attributes: attributes(),
        }],
        instances: vec![Instance {
            component: "Climate".to_owned(),
            instance: "Climate0".to_owned(),
            machine: "hpc".to_owned(),
            external: true,
            attributes: attributes(),
            offers: vec![InterfaceKey {
                catalog: "veh".to_owned(),
                number: 3,
            }],
            maps: vec!["veh".to_owned()],
        }],
        links: vec![Link {
            catalog: "veh".to_owned(),
            number: 3,
            service: "cabin".to_owned(),
            consumer: Box::new(endpoint("Ui")),
            producer: Box::new(endpoint("Climate")),
            crossing: Crossing::OffBoard,
            attributes: attributes(),
        }],
        routes: vec![Route {
            catalog: "veh".to_owned(),
            number: 3,
            ordinal: 2,
            interface: "Cabin".to_owned(),
            member: "temperature".to_owned(),
            service: "cabin".to_owned(),
            producers: vec![endpoint("Climate")],
        }],
        surface: vec![Surface {
            link: 0,
            direction: SurfaceDirection::ExternalOffers,
        }],
        system_attributes: attributes(),
        attributes: attributes(),
    }
}

pub fn valid() -> Deployment {
    deployment(SCHEMA_VERSION, catalog_bytes())
}
