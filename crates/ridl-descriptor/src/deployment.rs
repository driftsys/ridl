//! The deployment descriptor: the FlatBuffers file per deployment that a
//! runtime reads without decoding (`schema/deployment.fbs`).
//!
//! `generated.rs` holds the accessors planus generates from the schema
//! (`cargo xtask descriptor-codegen`). [`finish`] turns an owned
//! [`Deployment`] into a buffer, and [`verify`] checks a buffer, and every
//! catalog embedded in it, before the first read. This module does no I/O and
//! needs no standard library.

use alloc::boxed::Box;
use alloc::string::{String, ToString as _};
use alloc::vec::Vec;
use core::fmt;

use planus::ReadAsRoot;

#[cfg(feature = "std")]
mod describe;
pub mod generated;
#[cfg(feature = "std")]
mod lower;

#[cfg(feature = "std")]
pub use describe::to_json;
#[cfg(feature = "std")]
pub use lower::{DeploymentLowerError, lower};

pub use generated::ridl::deployment::{
    Attribute, AttributeRef, AttributeValue, AttributeValueRef, Component, ComponentRef, Crossing,
    DeployedInterface, DeployedInterfaceRef, Deployment, DeploymentRef, EmbeddedCatalog,
    EmbeddedCatalogRef, Endpoint, EndpointRef, Instance, InstanceRef, InterfaceKey,
    InterfaceKeyRef, Link, LinkRef, Machine, MachineRef, Route, RouteRef, Surface,
    SurfaceDirection, SurfaceRef,
};

/// The schema version this toolchain writes and accepts.
pub const SCHEMA_VERSION: u32 = 1;

/// The FlatBuffers file identifier, at bytes 4..8 of every deployment
/// descriptor.
pub const FILE_IDENTIFIER: [u8; 4] = *b"RDLD";

/// The artifact suffix: `<deployment>.deployment.binfb`.
pub const FILE_SUFFIX: &str = ".deployment.binfb";

/// The deepest nesting of `AttributeValue` lists the verifier accepts.
pub const MAX_ATTRIBUTE_DEPTH: usize = 16;

/// Why a buffer is not a deployment descriptor this toolchain reads.
#[derive(Debug)]
pub enum VerifyError {
    /// Fewer bytes than the root offset and the identifier need.
    TooShort(usize),
    /// Bytes 4..8 are not `FILE_IDENTIFIER`: another kind of file.
    WrongIdentifier([u8; 4]),
    /// A schema version this toolchain does not know.
    WrongVersion(u32),
    /// The FlatBuffers structure is not sound: an offset or a length points
    /// outside the buffer, a required field is missing, an enum tag is unknown.
    Invalid(planus::Error),
    /// A catalog embedded in the deployment failed the catalog verifier.
    /// `catalog` is the name the deployment gives it.
    EmbeddedCatalog {
        catalog: String,
        source: Box<crate::VerifyError>,
    },
    /// The hash stored beside an embedded catalog is not the hash inside the
    /// catalog. `catalog` is the name the deployment gives it.
    CatalogHashMismatch { catalog: String },
    /// An `AttributeValue` list nests deeper than [`MAX_ATTRIBUTE_DEPTH`].
    TooDeep,
    /// The attribute values, counted with every repeated visit of a shared
    /// table, outnumber one for every 4 bytes of the buffer.
    TooManyAttributeValues,
}

impl From<planus::Error> for VerifyError {
    fn from(err: planus::Error) -> Self {
        Self::Invalid(err)
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort(len) => write!(
                f,
                "{len} bytes is shorter than a deployment descriptor header"
            ),
            Self::WrongIdentifier(id) => write!(
                f,
                "not a deployment descriptor: file identifier {:?}, expected {:?}",
                String::from_utf8_lossy(id),
                String::from_utf8_lossy(&FILE_IDENTIFIER),
            ),
            Self::WrongVersion(v) => write!(
                f,
                "deployment descriptor version {v}; this toolchain reads version {SCHEMA_VERSION}"
            ),
            Self::Invalid(err) => write!(f, "deployment descriptor is malformed: {err}"),
            Self::EmbeddedCatalog { catalog, source } => {
                write!(f, "embedded catalog `{catalog}` is refused: {source}")
            }
            Self::CatalogHashMismatch { catalog } => write!(
                f,
                "the hash stored for embedded catalog `{catalog}` is not the hash inside the catalog"
            ),
            Self::TooDeep => write!(
                f,
                "attribute values nest deeper than {MAX_ATTRIBUTE_DEPTH} levels"
            ),
            Self::TooManyAttributeValues => {
                write!(f, "the attribute values outnumber the size of the buffer")
            }
        }
    }
}

impl core::error::Error for VerifyError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::EmbeddedCatalog { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Builds the descriptor buffer for `deployment`, with the header layout of
/// [`crate::finish`]: the root offset at bytes 0..4, [`FILE_IDENTIFIER`] at
/// bytes 4..8.
pub fn finish(deployment: &Deployment) -> Vec<u8> {
    crate::finish_with_identifier(deployment, FILE_IDENTIFIER)
}

/// Checks the identifier and the version, walks every table, vector and
/// string once through the checked accessors, and verifies each embedded
/// catalog with [`crate::verify`], so that a later read of the returned view
/// cannot fail on a malformed offset.
pub fn verify(bytes: &[u8]) -> Result<DeploymentRef<'_>, VerifyError> {
    if bytes.len() < 8 {
        return Err(VerifyError::TooShort(bytes.len()));
    }
    let id: [u8; 4] = bytes[4..8].try_into().expect("four bytes");
    if id != FILE_IDENTIFIER {
        return Err(VerifyError::WrongIdentifier(id));
    }
    let deployment = DeploymentRef::read_as_root(bytes).map_err(VerifyError::Invalid)?;
    let version = deployment.version().map_err(VerifyError::Invalid)?;
    if version != SCHEMA_VERSION {
        return Err(VerifyError::WrongVersion(version));
    }
    walk(deployment, bytes.len() / 4)?;
    Ok(deployment)
}

/// Touches every field once, verifies the embedded catalogs, and checks the
/// stored hash of each against the hash inside it.
fn walk(deployment: DeploymentRef<'_>, budget: usize) -> Result<(), VerifyError> {
    walk_fields(deployment, &mut Budget(budget))?;
    for catalog in deployment.catalogs().map_err(VerifyError::Invalid)? {
        let catalog = catalog.map_err(VerifyError::Invalid)?;
        let name = catalog.name().map_err(VerifyError::Invalid)?;
        let bytes = catalog.bytes().map_err(VerifyError::Invalid)?;
        let embedded = crate::verify(bytes).map_err(|source| VerifyError::EmbeddedCatalog {
            catalog: name.to_string(),
            source: Box::new(source),
        })?;
        let stored = catalog.hash().map_err(VerifyError::Invalid)?;
        if embedded.hash().map_err(VerifyError::Invalid)? != stored {
            return Err(VerifyError::CatalogHashMismatch {
                catalog: name.to_string(),
            });
        }
    }
    Ok(())
}

/// How many more `AttributeValue` tables the walk may visit. A table that
/// several lists share is visited once per reference, so without a bound a
/// small buffer can demand exponential work.
struct Budget(usize);

fn walk_fields(deployment: DeploymentRef<'_>, budget: &mut Budget) -> Result<(), VerifyError> {
    deployment.system()?;
    deployment.name()?;
    deployment.package()?;
    for catalog in deployment.catalogs()? {
        let catalog = catalog?;
        catalog.name()?;
        catalog.hash()?;
        catalog.bytes()?;
    }
    for interface in deployment.interfaces()? {
        let interface = interface?;
        interface.catalog()?;
        interface.number()?;
        interface.name()?;
        interface.inline()?;
        interface.provisional()?;
        interface.service()?;
    }
    for machine in deployment.machines()? {
        let machine = machine?;
        machine.name()?;
        machine.external()?;
        walk_strings(machine.labels()?)?;
        walk_attributes(machine.attributes()?, budget)?;
    }
    for component in deployment.components()? {
        let component = component?;
        component.name()?;
        component.implicit()?;
        component.external()?;
        walk_strings(component.instances()?)?;
        walk_strings(component.labels()?)?;
        walk_attributes(component.attributes()?, budget)?;
    }
    for instance in deployment.instances()? {
        let instance = instance?;
        instance.component()?;
        instance.instance()?;
        instance.machine()?;
        instance.external()?;
        walk_attributes(instance.attributes()?, budget)?;
        for key in instance.offers()? {
            let key = key?;
            key.catalog()?;
            key.number()?;
        }
        walk_strings(instance.maps()?)?;
    }
    for link in deployment.links()? {
        let link = link?;
        link.catalog()?;
        link.number()?;
        link.service()?;
        walk_endpoint(link.consumer()?)?;
        walk_endpoint(link.producer()?)?;
        link.crossing()?;
        walk_attributes(link.attributes()?, budget)?;
    }
    for route in deployment.routes()? {
        let route = route?;
        route.catalog()?;
        route.number()?;
        route.ordinal()?;
        route.interface()?;
        route.member()?;
        route.service()?;
        for producer in route.producers()? {
            walk_endpoint(producer?)?;
        }
    }
    for surface in deployment.surface()? {
        let surface = surface?;
        surface.link()?;
        surface.direction()?;
    }
    walk_attributes(deployment.system_attributes()?, budget)?;
    walk_attributes(deployment.attributes()?, budget)?;
    Ok(())
}

fn walk_endpoint(endpoint: EndpointRef<'_>) -> planus::Result<()> {
    endpoint.component()?;
    endpoint.instance()?;
    endpoint.machine()?;
    Ok(())
}

fn walk_strings(strings: planus::Vector<'_, planus::Result<&str>>) -> planus::Result<()> {
    for string in strings {
        string?;
    }
    Ok(())
}

fn walk_attributes(
    attributes: planus::Vector<'_, planus::Result<AttributeRef<'_>>>,
    budget: &mut Budget,
) -> Result<(), VerifyError> {
    for attribute in attributes {
        let attribute = attribute?;
        attribute.ns()?;
        attribute.key()?;
        if let Some(value) = attribute.value()? {
            walk_value(value, 1, budget)?;
        }
    }
    Ok(())
}

fn walk_value(
    value: AttributeValueRef<'_>,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), VerifyError> {
    if depth > MAX_ATTRIBUTE_DEPTH {
        return Err(VerifyError::TooDeep);
    }
    budget.0 = budget
        .0
        .checked_sub(1)
        .ok_or(VerifyError::TooManyAttributeValues)?;
    value.scalar()?;
    if let Some(list) = value.list()? {
        for item in list {
            walk_value(item?, depth + 1, budget)?;
        }
    }
    Ok(())
}
