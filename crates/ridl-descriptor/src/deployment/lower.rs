//! Lowers one deployment of a system, and the catalog descriptors of its
//! closure, into a deployment descriptor buffer.
//!
//! Every list is a direct projection of a field of the system IR, in the
//! order the IR holds it. The two lists of an instance that need a rule,
//! `offers` and `maps`, are read from the codegen deployment section
//! ([`ridl_ir::codegen::lower_deployment`]), so that the two emitters cannot
//! disagree.

use std::collections::BTreeMap;
use std::fmt;

use ridl_ir::v2;

use super::{
    Attribute, AttributeValue, Component, Crossing, DeployedInterface, Deployment, EmbeddedCatalog,
    Endpoint, Instance, InterfaceKey, Link, Machine, Route, SCHEMA_VERSION, Surface,
    SurfaceDirection, finish,
};

/// Why a deployment cannot be lowered.
#[derive(Debug)]
pub enum DeploymentLowerError {
    /// The system has no deployment of this name.
    UnknownDeployment(String),
    /// The caller gave no catalog descriptor for this region.
    MissingCatalog(String),
    /// The catalog descriptor given for this region is not a valid catalog
    /// descriptor.
    InvalidCatalog {
        catalog: String,
        source: crate::VerifyError,
    },
    /// The hash of the catalog descriptor given for this region is not the
    /// region's hash. Both come from one build, so this is an internal error.
    HashMismatch { catalog: String },
    /// The system IR holds a value the deployment schema cannot express. The
    /// text names what was found.
    Internal(String),
}

impl fmt::Display for DeploymentLowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDeployment(name) => write!(f, "the system has no deployment `{name}`"),
            Self::MissingCatalog(name) => {
                write!(f, "no catalog descriptor was given for region `{name}`")
            }
            Self::InvalidCatalog { catalog, source } => write!(
                f,
                "the catalog descriptor of `{catalog}` is refused: {source}"
            ),
            Self::HashMismatch { catalog } => write!(
                f,
                "the catalog descriptor of `{catalog}` does not carry the hash of its region"
            ),
            Self::Internal(what) => write!(f, "internal error: {what}"),
        }
    }
}

impl std::error::Error for DeploymentLowerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCatalog { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Builds the descriptor of the deployment of `system` named `deployment`.
///
/// `packages` is what [`ridl_ir::codegen::lower_deployment`] takes.
/// `catalogs` maps each region's catalog name to the bytes of its catalog
/// descriptor; the bytes are embedded unchanged, after a check that the
/// catalog's hash is the region's hash.
pub fn lower(
    system: &v2::System,
    deployment: &str,
    packages: &[&v2::Package],
    catalogs: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, DeploymentLowerError> {
    let declared = system
        .deployments
        .iter()
        .find(|candidate| candidate.name == deployment)
        .ok_or_else(|| DeploymentLowerError::UnknownDeployment(deployment.to_owned()))?;
    let section = ridl_ir::codegen::lower_deployment(system, deployment, packages)
        .ok_or_else(|| DeploymentLowerError::UnknownDeployment(deployment.to_owned()))?;

    let mut embedded = Vec::new();
    let mut interfaces = Vec::new();
    for region in &system.regions {
        let bytes = catalogs
            .get(&region.catalog)
            .ok_or_else(|| DeploymentLowerError::MissingCatalog(region.catalog.clone()))?;
        let mismatch = || DeploymentLowerError::HashMismatch {
            catalog: region.catalog.clone(),
        };
        let view = crate::verify(bytes).map_err(|source| DeploymentLowerError::InvalidCatalog {
            catalog: region.catalog.clone(),
            source,
        })?;
        if view.hash().map_err(|_| mismatch())? != region.hash.as_slice() {
            return Err(mismatch());
        }
        embedded.push(EmbeddedCatalog {
            name: region.catalog.clone(),
            hash: region.hash.clone(),
            bytes: bytes.clone(),
        });
        for interface in &region.interfaces {
            interfaces.push(DeployedInterface {
                catalog: region.catalog.clone(),
                number: interface.number,
                name: interface.name.clone(),
                inline: interface.inline,
                provisional: interface.provisional,
                service: interface.service.clone(),
            });
        }
    }

    let instances = section
        .instances
        .iter()
        .zip(&declared.placements)
        .map(|(instance, placement)| Instance {
            component: instance.component.clone(),
            instance: instance.instance.clone(),
            machine: instance.machine.clone(),
            external: instance.external,
            attributes: attributes(&placement.attributes),
            offers: instance
                .offers
                .iter()
                .map(|key| InterfaceKey {
                    catalog: key.catalog.clone(),
                    number: key.number,
                })
                .collect(),
            maps: instance.maps.clone(),
        })
        .collect();

    let descriptor = Deployment {
        version: SCHEMA_VERSION,
        system: section.system,
        name: declared.name.clone(),
        package: declared.package.clone(),
        catalogs: embedded,
        interfaces,
        machines: declared
            .machines
            .iter()
            .map(|machine| Machine {
                name: machine.name.clone(),
                external: machine.external,
                labels: machine.labels.clone(),
                attributes: attributes(&machine.attributes),
            })
            .collect(),
        components: system
            .components
            .iter()
            .map(|component| Component {
                name: component.qualified_name(),
                implicit: component.implicit,
                external: component.external,
                instances: component.instances.clone(),
                labels: component.labels.clone(),
                attributes: attributes(&component.attributes),
            })
            .collect(),
        instances,
        links: declared
            .links
            .iter()
            .map(|link| lower_link(system, link))
            .collect::<Result<_, _>>()?,
        routes: declared
            .routes
            .iter()
            .map(|route| Route {
                catalog: route.catalog.clone(),
                number: route.interface_number,
                ordinal: route.member_ordinal,
                interface: route.interface.clone(),
                member: route.member.clone(),
                service: route.service.clone(),
                producers: route.producers.iter().map(endpoint).collect(),
            })
            .collect(),
        surface: declared
            .surface
            .iter()
            .map(|surface| lower_surface(declared, surface))
            .collect::<Result<_, _>>()?,
        system_attributes: attributes(&system.attributes),
        attributes: attributes(&declared.attributes),
    };
    Ok(finish(&descriptor))
}

fn endpoint(endpoint: &v2::Endpoint) -> Endpoint {
    Endpoint {
        component: endpoint.component.clone(),
        instance: endpoint.instance.clone(),
        machine: endpoint.machine.clone(),
    }
}

/// The link's catalog and name come from its interface reference; the number
/// is read from the region map, which carries the identity the routing key
/// uses. The attributes are those of the consumer's `requires` line, which is
/// the link's attribute site.
fn lower_link(system: &v2::System, link: &v2::Link) -> Result<Link, DeploymentLowerError> {
    let interface = link.interface.clone().unwrap_or_default();
    let number = system
        .regions
        .iter()
        .filter(|region| region.catalog == interface.catalog)
        .flat_map(|region| &region.interfaces)
        .find(|candidate| candidate.name == interface.name && candidate.inline == interface.inline)
        .map_or(0, |candidate| candidate.number);
    let consumer = link.consumer.clone().unwrap_or_default();
    let producer = link.producer.clone().unwrap_or_default();
    let line = system
        .components
        .iter()
        .find(|component| component.qualified_name() == consumer.component)
        .into_iter()
        .flat_map(|component| &component.requires)
        .find(|require| {
            require.interface.as_ref() == link.interface.as_ref()
                && require.service == link.service
                && (require.producer.is_empty() || require.producer == producer.component)
        });
    let crossing = match v2::Crossing::try_from(link.crossing) {
        Ok(v2::Crossing::SameMachine) => Crossing::SameMachine,
        Ok(v2::Crossing::DifferentMachine) => Crossing::DifferentMachine,
        Ok(v2::Crossing::OffBoard) => Crossing::OffBoard,
        Ok(v2::Crossing::Unspecified) | Err(_) => {
            return Err(DeploymentLowerError::Internal(format!(
                "the link of `{}` from `{}` to `{}` has the crossing value {}, which the \
                 deployment schema cannot express",
                link.service, consumer.component, producer.component, link.crossing
            )));
        }
    };
    Ok(Link {
        catalog: interface.catalog,
        number,
        service: link.service.clone(),
        consumer: Box::new(endpoint(&consumer)),
        producer: Box::new(endpoint(&producer)),
        crossing,
        attributes: line.map_or_else(Vec::new, |line| attributes(&line.attributes)),
    })
}

/// `link` is the index of the equal link in the deployment's link list.
fn lower_surface(
    deployment: &v2::Deployment,
    surface: &v2::Surface,
) -> Result<Surface, DeploymentLowerError> {
    let index = surface
        .link
        .as_ref()
        .and_then(|link| {
            deployment
                .links
                .iter()
                .position(|candidate| candidate == link)
        })
        .ok_or_else(|| {
            DeploymentLowerError::Internal(format!(
                "a surface entry of deployment `{}` names a link that is not in its link list",
                deployment.name
            ))
        })?;
    let link = u32::try_from(index).map_err(|_| {
        DeploymentLowerError::Internal(format!(
            "link index {index} of deployment `{}` does not fit in 32 bits",
            deployment.name
        ))
    })?;
    let direction = match v2::SurfaceDirection::try_from(surface.direction) {
        Ok(v2::SurfaceDirection::ExternalConsumes) => SurfaceDirection::ExternalConsumes,
        Ok(v2::SurfaceDirection::ExternalOffers) => SurfaceDirection::ExternalOffers,
        Ok(v2::SurfaceDirection::Unspecified) | Err(_) => {
            return Err(DeploymentLowerError::Internal(format!(
                "a surface entry of deployment `{}` has the direction value {}, which the \
                 deployment schema cannot express",
                deployment.name, surface.direction
            )));
        }
    };
    Ok(Surface { link, direction })
}

fn attributes(attributes: &[v2::Attribute]) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|attribute| Attribute {
            ns: attribute.namespace.clone(),
            key: attribute.key.clone(),
            value: attribute
                .value
                .as_ref()
                .map(|value| Box::new(attribute_value(value))),
        })
        .collect()
}

fn attribute_value(value: &v2::AttributeValue) -> AttributeValue {
    match &value.kind {
        Some(v2::attribute_value::Kind::Scalar(text)) => AttributeValue {
            scalar: Some(text.clone()),
            list: None,
        },
        Some(v2::attribute_value::Kind::List(list)) => AttributeValue {
            scalar: None,
            list: Some(list.items.iter().map(attribute_value).collect()),
        },
        None => AttributeValue {
            scalar: None,
            list: None,
        },
    }
}
