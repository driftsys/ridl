//! `ridl describe`'s view of a descriptor (the runtime descriptors design,
//! D-9): strict JSON built by walking the checked accessors. There is no
//! JSON emit; this is a rendering of the binary. `flatc --json --strict-json
//! --defaults-json` gives the same view except that it omits an absent
//! `timing`, which is `null` here.

use serde_json::{Value, json};

use crate::{CatalogRef, Encoding, Kind, SizeStateTag, TimingMode, UnboundedCause};

/// Renders `catalog` as JSON: the schema's field names as keys in schema
/// order, enums by member name, `hash` as an array of bytes, an absent
/// `timing` as `null`, and every field of a `MaxSize` row, defaults included.
/// A payload's `type_name` is printed as it is stored.
pub fn to_json(catalog: CatalogRef<'_>) -> planus::Result<Value> {
    let mut interfaces = Vec::new();
    for interface in catalog.interfaces()? {
        let interface = interface?;
        let mut members = Vec::new();
        for member in interface.members()? {
            let member = member?;
            let mut payloads = Vec::new();
            for payload in member.payloads()? {
                let payload = payload?;
                let mut sizes = Vec::new();
                for size in payload.max_sizes()? {
                    let size = size?;
                    sizes.push(json!({
                        "encoding": encoding_name(size.encoding()?),
                        "bytes": size.bytes()?,
                        "state": state_name(size.state()?),
                        "cause": cause_name(size.cause()?),
                    }));
                }
                payloads.push(json!({
                    "role": payload.role()?,
                    "type_name": payload.type_name()?,
                    "max_sizes": sizes,
                }));
            }
            let timing = match member.timing()? {
                Some(t) => json!({
                    "mode": timing_mode_name(t.mode()?),
                    "min_us": t.min_us()?,
                    "max_us": t.max_us()?,
                }),
                None => Value::Null,
            };
            members.push(json!({
                "name": member.name()?,
                "ordinal": member.ordinal()?,
                "kind": kind_name(member.kind()?),
                "payloads": payloads,
                "timing": timing,
            }));
        }
        interfaces.push(json!({
            "name": interface.name()?,
            "number": interface.number()?,
            "provisional": interface.provisional()?,
            "members": members,
            "reserved_ordinals": interface.reserved_ordinals()?.iter().collect::<Vec<u32>>(),
        }));
    }
    let mut retired = Vec::new();
    for entry in catalog.retired()? {
        let entry = entry?;
        retired.push(json!({ "name": entry.name()?, "number": entry.number()? }));
    }
    Ok(json!({
        "version": catalog.version()?,
        "name": catalog.name()?,
        "hash": catalog.hash()?,
        "toolchain": catalog.toolchain()?,
        "interfaces": interfaces,
        "retired": retired,
    }))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Signal => "Signal",
        Kind::Event => "Event",
        Kind::Command => "Command",
        Kind::Query => "Query",
        Kind::Fixed => "Fixed",
    }
}

fn encoding_name(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Proto3 => "Proto3",
        Encoding::FlatBuffers => "FlatBuffers",
        Encoding::ReprC => "ReprC",
    }
}

fn timing_mode_name(mode: TimingMode) -> &'static str {
    match mode {
        TimingMode::Unspecified => "Unspecified",
        TimingMode::StrictPeriodic => "StrictPeriodic",
        TimingMode::Range => "Range",
    }
}

fn state_name(state: SizeStateTag) -> &'static str {
    match state {
        SizeStateTag::Bounded => "Bounded",
        SizeStateTag::Unbounded => "Unbounded",
    }
}

fn cause_name(cause: UnboundedCause) -> &'static str {
    match cause {
        UnboundedCause::Unspecified => "Unspecified",
        UnboundedCause::Member => "Member",
        UnboundedCause::Untyped => "Untyped",
        UnboundedCause::Layout => "Layout",
        UnboundedCause::Aggregate => "Aggregate",
        UnboundedCause::Exempt => "Exempt",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Catalog, Encoding, Interface, Kind, MaxSize, Member, Payload, SCHEMA_VERSION, SizeStateTag,
        UnboundedCause, verify,
    };

    #[test]
    fn the_view_uses_schema_names_and_enum_members() {
        let catalog = Catalog {
            version: SCHEMA_VERSION,
            name: "p".to_owned(),
            hash: vec![1, 2],
            toolchain: "0.0.0".to_owned(),
            interfaces: vec![Interface {
                name: "I".to_owned(),
                number: 1,
                provisional: true,
                members: vec![Member {
                    name: "m".to_owned(),
                    ordinal: 1,
                    kind: Kind::Query,
                    payloads: vec![Payload {
                        role: "request".to_owned(),
                        type_name: "Point".to_owned(),
                        max_sizes: vec![
                            MaxSize {
                                encoding: Encoding::Proto3,
                                bytes: 12,
                                state: SizeStateTag::Bounded,
                                cause: UnboundedCause::Unspecified,
                            },
                            MaxSize {
                                encoding: Encoding::FlatBuffers,
                                bytes: 0,
                                state: SizeStateTag::Unbounded,
                                cause: UnboundedCause::Layout,
                            },
                        ],
                    }],
                    timing: None,
                }],
                reserved_ordinals: vec![],
            }],
            retired: vec![],
        };
        let bytes = crate::finish(&catalog);
        let json = to_json(verify(&bytes).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "version": 1,
                "name": "p",
                "hash": [1, 2],
                "toolchain": "0.0.0",
                "interfaces": [{
                    "name": "I",
                    "number": 1,
                    "provisional": true,
                    "members": [{
                        "name": "m",
                        "ordinal": 1,
                        "kind": "Query",
                        "payloads": [{
                            "role": "request",
                            "type_name": "Point",
                            "max_sizes": [
                                { "encoding": "Proto3", "bytes": 12, "state": "Bounded", "cause": "Unspecified" },
                                { "encoding": "FlatBuffers", "bytes": 0, "state": "Unbounded", "cause": "Layout" }
                            ]
                        }],
                        "timing": null
                    }],
                    "reserved_ordinals": []
                }],
                "retired": []
            })
        );
    }
}
