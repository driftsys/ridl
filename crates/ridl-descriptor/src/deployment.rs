//! The deployment descriptor: the FlatBuffers file per deployment that a
//! runtime reads without decoding (`schema/deployment.fbs`).
//!
//! `generated.rs` holds the accessors planus generates from the schema
//! (`cargo xtask descriptor-codegen`). [`finish`] turns an owned
//! [`Deployment`] into a buffer, and [`verify`] checks a buffer, and every
//! catalog embedded in it, before the first read. This module does no I/O and
//! needs no standard library.

use alloc::boxed::Box;
use alloc::string::ToString as _;
use alloc::vec::Vec;

use planus::ReadAsRoot;

use crate::VerifyError;

pub mod generated;

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
    walk(deployment)?;
    Ok(deployment)
}

/// Touches every field once, and verifies the embedded catalogs.
fn walk(deployment: DeploymentRef<'_>) -> Result<(), VerifyError> {
    walk_fields(deployment).map_err(VerifyError::Invalid)?;
    for catalog in deployment.catalogs().map_err(VerifyError::Invalid)? {
        let catalog = catalog.map_err(VerifyError::Invalid)?;
        let name = catalog.name().map_err(VerifyError::Invalid)?;
        let bytes = catalog.bytes().map_err(VerifyError::Invalid)?;
        crate::verify(bytes).map_err(|source| VerifyError::EmbeddedCatalog {
            catalog: name.to_string(),
            source: Box::new(source),
        })?;
    }
    Ok(())
}

fn walk_fields(deployment: DeploymentRef<'_>) -> planus::Result<()> {
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
        walk_attributes(machine.attributes()?)?;
    }
    for component in deployment.components()? {
        let component = component?;
        component.name()?;
        component.implicit()?;
        component.external()?;
        walk_strings(component.instances()?)?;
        walk_strings(component.labels()?)?;
        walk_attributes(component.attributes()?)?;
    }
    for instance in deployment.instances()? {
        let instance = instance?;
        instance.component()?;
        instance.instance()?;
        instance.machine()?;
        instance.external()?;
        walk_attributes(instance.attributes()?)?;
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
        walk_attributes(link.attributes()?)?;
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
    walk_attributes(deployment.system_attributes()?)?;
    walk_attributes(deployment.attributes()?)?;
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
) -> planus::Result<()> {
    for attribute in attributes {
        let attribute = attribute?;
        attribute.ns()?;
        attribute.key()?;
        if let Some(value) = attribute.value()? {
            walk_value(value)?;
        }
    }
    Ok(())
}

fn walk_value(value: AttributeValueRef<'_>) -> planus::Result<()> {
    value.scalar()?;
    if let Some(list) = value.list()? {
        for item in list {
            walk_value(item?)?;
        }
    }
    Ok(())
}
