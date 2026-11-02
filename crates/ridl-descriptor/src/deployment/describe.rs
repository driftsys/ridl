//! The JSON view of a deployment descriptor, in the conventions of
//! [`crate::describe`]: the schema's field names as keys, enums by member
//! name, a hash as an array of bytes, an absent field as `null`. An embedded
//! catalog prints `bytes_len` and `catalog` in place of `bytes` (see
//! [`to_json`]). The
//! serialized object prints its keys in alphabetical order, as
//! [`crate::describe`] does.

use serde_json::{Value, json};

use super::{
    AttributeRef, AttributeValueRef, Crossing, DeploymentRef, EndpointRef, SurfaceDirection,
};

/// Renders `deployment` as JSON.
///
/// An embedded catalog is printed as its `name`, its `hash`, the length of
/// its buffer as `bytes_len` and its own JSON view as `catalog`; the buffer
/// itself is not printed. A buffer that does not pass the catalog verifier
/// has `catalog` as `null`.
pub fn to_json(deployment: DeploymentRef<'_>) -> planus::Result<Value> {
    let mut catalogs = Vec::new();
    for catalog in deployment.catalogs()? {
        let catalog = catalog?;
        let bytes = catalog.bytes()?;
        let view = match crate::verify(bytes) {
            Ok(view) => crate::describe::to_json(view)?,
            Err(_) => Value::Null,
        };
        catalogs.push(json!({
            "name": catalog.name()?,
            "hash": catalog.hash()?,
            "bytes_len": bytes.len(),
            "catalog": view,
        }));
    }
    let mut interfaces = Vec::new();
    for interface in deployment.interfaces()? {
        let interface = interface?;
        interfaces.push(json!({
            "catalog": interface.catalog()?,
            "number": interface.number()?,
            "name": interface.name()?,
            "inline": interface.inline()?,
            "provisional": interface.provisional()?,
            "service": interface.service()?,
        }));
    }
    let mut machines = Vec::new();
    for machine in deployment.machines()? {
        let machine = machine?;
        machines.push(json!({
            "name": machine.name()?,
            "external": machine.external()?,
            "labels": strings(machine.labels()?)?,
            "attributes": attributes(machine.attributes()?)?,
        }));
    }
    let mut components = Vec::new();
    for component in deployment.components()? {
        let component = component?;
        components.push(json!({
            "name": component.name()?,
            "implicit": component.implicit()?,
            "external": component.external()?,
            "instances": strings(component.instances()?)?,
            "labels": strings(component.labels()?)?,
            "attributes": attributes(component.attributes()?)?,
        }));
    }
    let mut instances = Vec::new();
    for instance in deployment.instances()? {
        let instance = instance?;
        let mut offers = Vec::new();
        for key in instance.offers()? {
            let key = key?;
            offers.push(json!({ "catalog": key.catalog()?, "number": key.number()? }));
        }
        instances.push(json!({
            "component": instance.component()?,
            "instance": instance.instance()?,
            "machine": instance.machine()?,
            "external": instance.external()?,
            "attributes": attributes(instance.attributes()?)?,
            "offers": offers,
            "maps": strings(instance.maps()?)?,
        }));
    }
    let mut links = Vec::new();
    for link in deployment.links()? {
        let link = link?;
        links.push(json!({
            "catalog": link.catalog()?,
            "number": link.number()?,
            "service": link.service()?,
            "consumer": endpoint(link.consumer()?)?,
            "producer": endpoint(link.producer()?)?,
            "crossing": crossing_name(link.crossing()?),
            "attributes": attributes(link.attributes()?)?,
        }));
    }
    let mut routes = Vec::new();
    for route in deployment.routes()? {
        let route = route?;
        let mut producers = Vec::new();
        for producer in route.producers()? {
            producers.push(endpoint(producer?)?);
        }
        routes.push(json!({
            "catalog": route.catalog()?,
            "number": route.number()?,
            "ordinal": route.ordinal()?,
            "interface": route.interface()?,
            "member": route.member()?,
            "service": route.service()?,
            "producers": producers,
        }));
    }
    let mut surface = Vec::new();
    for entry in deployment.surface()? {
        let entry = entry?;
        surface.push(json!({
            "link": entry.link()?,
            "direction": direction_name(entry.direction()?),
        }));
    }
    Ok(json!({
        "version": deployment.version()?,
        "system": deployment.system()?,
        "name": deployment.name()?,
        "package": deployment.package()?,
        "catalogs": catalogs,
        "interfaces": interfaces,
        "machines": machines,
        "components": components,
        "instances": instances,
        "links": links,
        "routes": routes,
        "surface": surface,
        "system_attributes": attributes(deployment.system_attributes()?)?,
        "attributes": attributes(deployment.attributes()?)?,
    }))
}

fn strings<'a>(
    items: impl IntoIterator<Item = planus::Result<&'a str>>,
) -> planus::Result<Vec<&'a str>> {
    items.into_iter().collect()
}

fn endpoint(endpoint: EndpointRef<'_>) -> planus::Result<Value> {
    Ok(json!({
        "component": endpoint.component()?,
        "instance": endpoint.instance()?,
        "machine": endpoint.machine()?,
    }))
}

fn attributes<'a>(
    items: impl IntoIterator<Item = planus::Result<AttributeRef<'a>>>,
) -> planus::Result<Vec<Value>> {
    let mut rows = Vec::new();
    for attribute in items {
        let attribute = attribute?;
        let value = match attribute.value()? {
            Some(value) => attribute_value(value)?,
            None => Value::Null,
        };
        rows.push(json!({
            "ns": attribute.ns()?,
            "key": attribute.key()?,
            "value": value,
        }));
    }
    Ok(rows)
}

fn attribute_value(value: AttributeValueRef<'_>) -> planus::Result<Value> {
    let list = match value.list()? {
        Some(items) => {
            let mut rows = Vec::new();
            for item in items {
                rows.push(attribute_value(item?)?);
            }
            Value::Array(rows)
        }
        None => Value::Null,
    };
    Ok(json!({ "scalar": value.scalar()?, "list": list }))
}

fn crossing_name(crossing: Crossing) -> &'static str {
    match crossing {
        Crossing::SameMachine => "SameMachine",
        Crossing::DifferentMachine => "DifferentMachine",
        Crossing::OffBoard => "OffBoard",
    }
}

fn direction_name(direction: SurfaceDirection) -> &'static str {
    match direction {
        SurfaceDirection::ExternalConsumes => "ExternalConsumes",
        SurfaceDirection::ExternalOffers => "ExternalOffers",
    }
}
