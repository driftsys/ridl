//! The lowering of one deployment of a system into a deployment descriptor.

#![cfg(feature = "std")]

use std::collections::BTreeMap;

use ridl_descriptor::deployment::{DeploymentLowerError, FILE_IDENTIFIER, lower, to_json, verify};
use ridl_descriptor::{Catalog, SCHEMA_VERSION};
use ridl_ir::v2;
use serde_json::json;

const HASH: [u8; 32] = [9; 32];

fn catalog_bytes(hash: &[u8]) -> Vec<u8> {
    ridl_descriptor::finish(&Catalog {
        version: SCHEMA_VERSION,
        name: "veh".to_owned(),
        hash: hash.to_vec(),
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![],
        retired: vec![],
        compatible: None,
    })
}

fn catalogs() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([("veh".to_owned(), catalog_bytes(&HASH))])
}

fn scalar(text: &str) -> v2::AttributeValue {
    v2::AttributeValue {
        kind: Some(v2::attribute_value::Kind::Scalar(text.to_owned())),
    }
}

fn list(items: Vec<v2::AttributeValue>) -> v2::AttributeValue {
    v2::AttributeValue {
        kind: Some(v2::attribute_value::Kind::List(v2::AttributeList { items })),
    }
}

fn attribute(namespace: &str, key: &str, value: Option<v2::AttributeValue>) -> v2::Attribute {
    v2::Attribute {
        namespace: namespace.to_owned(),
        key: key.to_owned(),
        value,
    }
}

fn endpoint(component: &str, instance: &str, machine: &str) -> v2::Endpoint {
    v2::Endpoint {
        component: component.to_owned(),
        instance: instance.to_owned(),
        machine: machine.to_owned(),
    }
}

fn interface_ref() -> v2::InterfaceRef {
    v2::InterfaceRef {
        catalog: "veh".to_owned(),
        name: "Climate".to_owned(),
        inline: false,
    }
}

/// A provider that offers one service, a consumer that requires it, and a
/// third component with no line, on two machines.
fn system() -> v2::System {
    let link = v2::Link {
        interface: Some(interface_ref()),
        service: "veh.ClimateService".to_owned(),
        consumer: Some(endpoint("veh.Consumer", "c0", "ui")),
        producer: Some(endpoint("veh.Provider", "p0", "hpc")),
        crossing: v2::Crossing::DifferentMachine as i32,
    };
    v2::System {
        name: "Vehicle".to_owned(),
        package: "veh".to_owned(),
        attributes: vec![attribute("sys", "tier", Some(scalar("1")))],
        components: vec![
            v2::Component {
                name: "Provider".to_owned(),
                package: "veh".to_owned(),
                instances: vec!["p0".to_owned()],
                offers: vec![v2::Offer {
                    service: "veh.ClimateService".to_owned(),
                    ..Default::default()
                }],
                labels: vec!["core".to_owned()],
                ..Default::default()
            },
            v2::Component {
                name: "Consumer".to_owned(),
                package: "veh".to_owned(),
                instances: vec!["c0".to_owned()],
                requires: vec![v2::Require {
                    interface: Some(interface_ref()),
                    service: "veh.ClimateService".to_owned(),
                    producer: "veh.Provider".to_owned(),
                    attributes: vec![attribute("net", "qos", Some(scalar("high")))],
                    ..Default::default()
                }],
                ..Default::default()
            },
            v2::Component {
                name: "Idle".to_owned(),
                package: "veh".to_owned(),
                instances: vec!["i0".to_owned()],
                ..Default::default()
            },
        ],
        grants: vec![v2::Grant {
            component: "veh.Consumer".to_owned(),
            external: false,
            regions: vec!["veh".to_owned()],
        }],
        regions: vec![v2::Region {
            catalog: "veh".to_owned(),
            hash: HASH.to_vec(),
            interfaces: vec![v2::RegionInterface {
                name: "Climate".to_owned(),
                inline: false,
                number: 1,
                provisional: false,
                service: "veh.ClimateService".to_owned(),
            }],
        }],
        deployments: vec![v2::Deployment {
            name: "Prod".to_owned(),
            package: "veh".to_owned(),
            attributes: vec![attribute("dep", "site", None)],
            machines: vec![
                v2::Machine {
                    name: "hpc".to_owned(),
                    labels: vec!["fast".to_owned()],
                    ..Default::default()
                },
                v2::Machine {
                    name: "ui".to_owned(),
                    ..Default::default()
                },
            ],
            placements: vec![
                v2::Placement {
                    component: "veh.Provider".to_owned(),
                    instance: "p0".to_owned(),
                    machine: "hpc".to_owned(),
                    attributes: vec![
                        attribute("shm", "service", Some(scalar("3"))),
                        attribute("shm", "pinned", None),
                        attribute(
                            "linux",
                            "cpuset",
                            Some(list(vec![scalar("2"), scalar("3")])),
                        ),
                    ],
                    sizing: None,
                },
                v2::Placement {
                    component: "veh.Consumer".to_owned(),
                    instance: "c0".to_owned(),
                    machine: "ui".to_owned(),
                    ..Default::default()
                },
                v2::Placement {
                    component: "veh.Idle".to_owned(),
                    instance: "i0".to_owned(),
                    machine: "ui".to_owned(),
                    ..Default::default()
                },
            ],
            links: vec![link.clone()],
            routes: vec![v2::Route {
                catalog: "veh".to_owned(),
                interface_number: 1,
                member_ordinal: 2,
                interface: "Climate".to_owned(),
                member: "temperature".to_owned(),
                service: "veh.ClimateService".to_owned(),
                producers: vec![endpoint("veh.Provider", "p0", "hpc")],
            }],
            surface: vec![v2::Surface {
                link: Some(link),
                direction: v2::SurfaceDirection::ExternalOffers as i32,
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn lowered() -> Vec<u8> {
    lower(&system(), "Prod", &[], &catalogs()).expect("lowers")
}

fn attribute_of<'a>(
    attributes: &'a [ridl_descriptor::deployment::Attribute],
    key: &str,
) -> &'a ridl_descriptor::deployment::Attribute {
    attributes
        .iter()
        .find(|attribute| attribute.key == key)
        .unwrap_or_else(|| panic!("no attribute {key}"))
}

fn owned() -> ridl_descriptor::deployment::Deployment {
    let bytes = lowered();
    let view = verify(&bytes).expect("verifies");
    ridl_descriptor::deployment::Deployment::try_from(view).expect("converts")
}

#[test]
fn the_descriptor_verifies_and_carries_the_identifier() {
    let bytes = lowered();
    assert_eq!(bytes[4..8], FILE_IDENTIFIER);
    let deployment = owned();
    assert_eq!(deployment.system, "veh.Vehicle");
    assert_eq!(deployment.name, "Prod");
    assert_eq!(deployment.package, "veh");
    assert_eq!(deployment.machines.len(), 2);
    assert_eq!(deployment.components.len(), 3);
    assert_eq!(deployment.interfaces.len(), 1);
    assert_eq!(deployment.routes.len(), 1);
    assert_eq!(deployment.routes[0].ordinal, 2);
    assert_eq!(deployment.system_attributes.len(), 1);
    assert_eq!(deployment.attributes.len(), 1);
}

#[test]
fn the_instances_carry_offers_maps_and_placement_keys() {
    let deployment = owned();
    let provider = &deployment.instances[0];
    assert_eq!(provider.component, "veh.Provider");
    assert_eq!(provider.offers.len(), 1);
    assert_eq!(provider.offers[0].catalog, "veh");
    assert_eq!(provider.offers[0].number, 1);
    assert!(provider.maps.is_empty());
    let service = attribute_of(&provider.attributes, "service");
    assert_eq!(service.ns, "shm");
    assert_eq!(service.value.as_ref().unwrap().scalar.as_deref(), Some("3"));

    let consumer = &deployment.instances[1];
    assert!(consumer.offers.is_empty());
    assert_eq!(consumer.maps, vec!["veh".to_owned()]);
}

#[test]
fn an_instance_with_no_lines_is_listed_with_empty_lists() {
    let deployment = owned();
    let idle = &deployment.instances[2];
    assert_eq!(idle.component, "veh.Idle");
    assert!(idle.attributes.is_empty());
    assert!(idle.offers.is_empty());
    assert!(idle.maps.is_empty());
}

#[test]
fn flag_and_list_keys_are_carried() {
    let deployment = owned();
    let attributes = &deployment.instances[0].attributes;
    assert!(attribute_of(attributes, "pinned").value.is_none());
    let cpuset = attribute_of(attributes, "cpuset");
    let items = cpuset.value.as_ref().unwrap().list.as_ref().unwrap();
    let scalars: Vec<_> = items.iter().map(|item| item.scalar.clone()).collect();
    assert_eq!(scalars, vec![Some("2".to_owned()), Some("3".to_owned())]);
}

#[test]
fn links_and_surface_are_projected() {
    let deployment = owned();
    let link = &deployment.links[0];
    assert_eq!(link.catalog, "veh");
    assert_eq!(link.number, 1);
    assert_eq!(
        link.crossing,
        ridl_descriptor::deployment::Crossing::DifferentMachine
    );
    assert_eq!(link.consumer.component, "veh.Consumer");
    assert_eq!(link.attributes.len(), 1);
    assert_eq!(link.attributes[0].key, "qos");
    assert_eq!(deployment.surface[0].link, 0);
}

#[test]
fn the_embedded_catalog_is_the_given_bytes() {
    let deployment = owned();
    assert_eq!(deployment.catalogs.len(), 1);
    assert_eq!(deployment.catalogs[0].name, "veh");
    assert_eq!(deployment.catalogs[0].hash, HASH.to_vec());
    assert_eq!(deployment.catalogs[0].bytes, catalogs()["veh"]);
}

#[test]
fn an_unknown_deployment_is_an_error() {
    let error = lower(&system(), "Nope", &[], &catalogs()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::UnknownDeployment(name) if name == "Nope"));
}

#[test]
fn a_missing_catalog_is_an_error() {
    let error = lower(&system(), "Prod", &[], &BTreeMap::new()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::MissingCatalog(name) if name == "veh"));
}

#[test]
fn a_catalog_whose_hash_differs_from_the_region_is_an_error() {
    let wrong = BTreeMap::from([("veh".to_owned(), catalog_bytes(&[1; 32]))]);
    let error = lower(&system(), "Prod", &[], &wrong).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::HashMismatch { catalog } if catalog == "veh"));
}

#[test]
fn a_catalog_that_fails_the_verifier_is_an_error() {
    let invalid = BTreeMap::from([("veh".to_owned(), vec![0u8; 16])]);
    let error = lower(&system(), "Prod", &[], &invalid).unwrap_err();
    assert!(
        matches!(error, DeploymentLowerError::InvalidCatalog { catalog, .. } if catalog == "veh")
    );
}

#[test]
fn an_unspecified_crossing_is_an_internal_error() {
    let mut system = system();
    system.deployments[0].links[0].crossing = v2::Crossing::Unspecified as i32;
    let error = lower(&system, "Prod", &[], &catalogs()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::Internal(text) if text.contains("crossing")));
}

#[test]
fn a_surface_link_missing_from_the_list_is_an_internal_error() {
    let mut system = system();
    system.deployments[0].links.clear();
    let error = lower(&system, "Prod", &[], &catalogs()).unwrap_err();
    assert!(matches!(error, DeploymentLowerError::Internal(text) if text.contains("surface")));
}

#[test]
fn two_lowerings_write_the_same_bytes() {
    assert_eq!(lowered(), lowered());
}

#[test]
fn to_json_names_every_field() {
    let bytes = lowered();
    let view = verify(&bytes).expect("verifies");
    let value = to_json(view).expect("renders");
    let object = value.as_object().unwrap();
    for key in [
        "version",
        "system",
        "name",
        "package",
        "catalogs",
        "interfaces",
        "machines",
        "components",
        "instances",
        "links",
        "routes",
        "surface",
        "system_attributes",
        "attributes",
    ] {
        assert!(object.contains_key(key), "missing key {key}");
    }
    let catalog = &value["catalogs"][0];
    assert_eq!(catalog["name"], "veh");
    assert_eq!(catalog["hash"].as_array().unwrap().len(), 32);
    assert_eq!(catalog["bytes_len"], catalogs()["veh"].len());
    assert!(catalog.get("bytes").is_none());
    assert_eq!(catalog["catalog"]["name"], "veh");
    assert_eq!(value["instances"][0]["attributes"][0]["ns"], "shm");
    assert_eq!(
        value["instances"][0]["attributes"][2]["value"],
        json!({ "scalar": null, "list": [{ "scalar": "2", "list": null }, { "scalar": "3", "list": null }] })
    );
    assert_eq!(value["instances"][0]["attributes"][1]["value"], json!(null));
    assert_eq!(value["links"][0]["crossing"], "DifferentMachine");
    assert_eq!(value["surface"][0]["direction"], "ExternalOffers");
}
