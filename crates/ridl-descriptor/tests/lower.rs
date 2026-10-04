//! Spec D-4: what the catalog descriptor contains, checked through `verify`.

use ridl_descriptor::lower::LowerError;
use ridl_descriptor::{
    CatalogRef, Encoding, InterfaceRef, Kind, SCHEMA_VERSION, SizeStateTag, lower, verify,
};
use ridl_ir::projection::flatbuffers::{Packages, max_size};
use ridl_ir::v2::{
    Backing, CommandDef, Decl, EventDef, Field, FieldType, FixedDef, IntWidth, Interface, Package,
    Param, PrimitiveType, QueryDef, Reserved, RetiredInterface, ReturnType, Service, ServiceShape,
    SignalDef, StreamType, StructDef, StructMember, Timing, TimingMode, TypeDef, backing, decl,
    field_type, return_type, service_shape, stream_type, struct_member, type_def,
};

fn i16_def() -> TypeDef {
    TypeDef {
        backing: Some(Backing {
            kind: Some(backing::Kind::Primitive(PrimitiveType::Integer as i32)),
        }),
        width: Some(type_def::Width::IntWidth(IntWidth::I16 as i32)),
        ..Default::default()
    }
}

fn named(name: &str) -> FieldType {
    FieldType {
        optional: false,
        kind: Some(field_type::Kind::Named(name.to_owned())),
    }
}

fn interaction(name: &str, ordinal: u32, kind: decl::Kind) -> Decl {
    Decl {
        name: name.to_owned(),
        ordinal,
        kind: Some(kind),
        ..Default::default()
    }
}

/// Coord = integer i16; Point { x: Coord, y: Coord }; interface Vehicle with
/// one member of every kind and one reserved slot.
fn package() -> Package {
    let field = |name: &str, ordinal: u32| StructMember {
        member: Some(struct_member::Member::Field(Field {
            name: name.to_owned(),
            ordinal,
            r#type: Some(named("Coord")),
            ..Default::default()
        })),
    };
    Package {
        name: "veh.cluster".to_owned(),
        decls: vec![
            Decl {
                name: "Coord".to_owned(),
                kind: Some(decl::Kind::TypeDef(i16_def())),
                ..Default::default()
            },
            Decl {
                name: "Point".to_owned(),
                kind: Some(decl::Kind::StructDef(StructDef {
                    members: vec![field("x", 1), field("y", 2)],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        interfaces: vec![Interface {
            name: "Vehicle".to_owned(),
            number: 1,
            provisional: true,
            interactions: vec![
                interaction(
                    "position",
                    1,
                    decl::Kind::SignalDef(SignalDef {
                        payload: "Point".to_owned(),
                        timing: Some(Timing {
                            mode: TimingMode::StrictPeriodic as i32,
                            min_us: Some("100000".to_owned()),
                            max_us: None,
                            default_applied: false,
                        }),
                        ..Default::default()
                    }),
                ),
                interaction(
                    "doorOpened",
                    2,
                    decl::Kind::EventDef(EventDef {
                        payload: "Point".to_owned(),
                        timing: None,
                    }),
                ),
                interaction(
                    "moveTo",
                    3,
                    decl::Kind::CommandDef(CommandDef {
                        params: vec![Param {
                            name: "to".to_owned(),
                            r#type: Some(named("Point")),
                        }],
                        ..Default::default()
                    }),
                ),
                interaction(
                    "nearest",
                    4,
                    decl::Kind::QueryDef(QueryDef {
                        params: vec![Param {
                            name: "from".to_owned(),
                            r#type: Some(named("Point")),
                        }],
                        return_type: Some(ReturnType {
                            kind: Some(return_type::Kind::Value(named("Point"))),
                        }),
                        ..Default::default()
                    }),
                ),
                interaction(
                    "legacyWheelPhase",
                    5,
                    decl::Kind::ReservedSlot(Reserved {
                        ordinal: 5,
                        ..Default::default()
                    }),
                ),
                interaction(
                    "vin",
                    6,
                    decl::Kind::FixedDef(FixedDef {
                        payload: Some(named("Coord")),
                    }),
                ),
                interaction(
                    "trace",
                    7,
                    decl::Kind::QueryDef(QueryDef {
                        params: vec![],
                        return_type: Some(ReturnType {
                            kind: Some(return_type::Kind::Value(FieldType {
                                optional: false,
                                kind: Some(field_type::Kind::Stream(StreamType {
                                    element: Some(stream_type::Element::Named("Point".to_owned())),
                                })),
                            })),
                        }),
                        ..Default::default()
                    }),
                ),
                interaction(
                    "moveBoth",
                    8,
                    decl::Kind::CommandDef(CommandDef {
                        params: vec![
                            Param {
                                name: "a".to_owned(),
                                r#type: Some(named("Point")),
                            },
                            Param {
                                name: "b".to_owned(),
                                r#type: Some(named("Point")),
                            },
                        ],
                        ..Default::default()
                    }),
                ),
                interaction(
                    "tryNearest",
                    9,
                    decl::Kind::QueryDef(QueryDef {
                        params: vec![Param {
                            name: "from".to_owned(),
                            r#type: Some(named("Point")),
                        }],
                        return_type: Some(ReturnType {
                            kind: Some(return_type::Kind::Fallible(ridl_ir::v2::FallibleType {
                                ok: "Point".to_owned(),
                                err: "Coord".to_owned(),
                            })),
                        }),
                        ..Default::default()
                    }),
                ),
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The FlatBuffers bound the projection gives `Point`, which the descriptor
/// must advertise unchanged (Task 7).
fn point_fb_bound(package: &Package) -> u32 {
    let decl = package.decls.iter().find(|d| d.name == "Point").unwrap();
    u32::try_from(
        max_size(
            Packages {
                package,
                others: &[],
            },
            decl,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn the_descriptor_carries_every_member_of_every_kind() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).expect("the lowering writes a valid descriptor");
    assert_eq!(catalog.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.name().unwrap(), "veh.cluster");
    assert_eq!(catalog.hash().unwrap().len(), 32);
    assert_eq!(catalog.toolchain().unwrap(), env!("CARGO_PKG_VERSION"));
    assert_eq!(catalog.retired().unwrap().len(), 0);

    let interface = vehicle(catalog);
    assert_eq!(interface.name().unwrap(), "Vehicle");
    assert_eq!(interface.number().unwrap(), 1);
    assert!(interface.provisional().unwrap());
    assert_eq!(
        interface
            .reserved_ordinals()
            .unwrap()
            .iter()
            .collect::<Vec<u32>>(),
        vec![5]
    );

    let members = interface.members().unwrap();
    let summary: Vec<(String, u32, Kind, Vec<String>)> = members
        .iter()
        .map(|m| {
            let m = m.unwrap();
            let roles = m
                .payloads()
                .unwrap()
                .iter()
                .map(|p| p.unwrap().role().unwrap().to_owned())
                .collect();
            (
                m.name().unwrap().to_owned(),
                m.ordinal().unwrap(),
                m.kind().unwrap(),
                roles,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "position".to_owned(),
                1,
                Kind::Signal,
                vec!["value".to_owned()]
            ),
            (
                "doorOpened".to_owned(),
                2,
                Kind::Event,
                vec!["occurrence".to_owned()]
            ),
            (
                "moveTo".to_owned(),
                3,
                Kind::Command,
                vec!["request".to_owned()]
            ),
            (
                "nearest".to_owned(),
                4,
                Kind::Query,
                vec!["request".to_owned(), "response".to_owned()]
            ),
            ("vin".to_owned(), 6, Kind::Fixed, vec!["value".to_owned()]),
            (
                "trace".to_owned(),
                7,
                Kind::Query,
                vec!["request".to_owned(), "response".to_owned()]
            ),
            (
                "moveBoth".to_owned(),
                8,
                Kind::Command,
                vec!["request".to_owned()]
            ),
            (
                "tryNearest".to_owned(),
                9,
                Kind::Query,
                vec!["request".to_owned(), "response".to_owned()]
            ),
        ]
    );
}

/// The descriptor's first interface: `Vehicle` in every fixture here.
fn vehicle(catalog: CatalogRef<'_>) -> InterfaceRef<'_> {
    catalog.interfaces().unwrap().get(0).unwrap().unwrap()
}

/// The rows of one payload: (encoding, state, bytes).
fn rows(payload: ridl_descriptor::PayloadRef<'_>) -> Vec<(Encoding, SizeStateTag, u32)> {
    payload
        .max_sizes()
        .unwrap()
        .iter()
        .map(|s| {
            let s = s.unwrap();
            (
                s.encoding().unwrap(),
                s.state().unwrap(),
                s.bytes().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_named_type_payload_has_a_row_per_sized_encoding_and_no_repr_c() {
    let package = package();
    let bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let position = vehicle(catalog).members().unwrap().get(0).unwrap().unwrap();
    let payload = position.payloads().unwrap().get(0).unwrap().unwrap();
    assert_eq!(payload.type_name().unwrap(), "Point");
    assert_eq!(
        rows(payload),
        vec![
            // ADR-0017: i16 projects to sint32, at most 5 bytes; (1-byte tag + 5) x 2 fields = 12.
            (Encoding::Proto3, SizeStateTag::Bounded, 12),
            (
                Encoding::FlatBuffers,
                SizeStateTag::Bounded,
                point_fb_bound(&package)
            ),
        ]
    );
}

#[test]
fn a_request_of_one_named_parameter_is_sized_and_of_several_is_absent() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = vehicle(catalog).members().unwrap();
    let move_to = members
        .get(2)
        .unwrap()
        .unwrap()
        .payloads()
        .unwrap()
        .get(0)
        .unwrap()
        .unwrap();
    assert_eq!(move_to.type_name().unwrap(), "Point");
    assert_eq!(rows(move_to).len(), 2);
    let move_both = members
        .get(6)
        .unwrap()
        .unwrap()
        .payloads()
        .unwrap()
        .get(0)
        .unwrap()
        .unwrap();
    assert_eq!(move_both.type_name().unwrap(), "(a: Point, b: Point)");
    assert!(
        rows(move_both).is_empty(),
        "several parameters: driver §4 answer 6"
    );
}

#[test]
fn a_fallible_reply_is_absent() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = vehicle(catalog).members().unwrap();
    let reply = members
        .get(7)
        .unwrap()
        .unwrap()
        .payloads()
        .unwrap()
        .get(1)
        .unwrap()
        .unwrap();
    assert_eq!(reply.type_name().unwrap(), "Point | Coord");
    assert!(
        rows(reply).is_empty(),
        "an inline `T | E`: driver §4 answer 6"
    );
}

#[test]
fn timing_is_carried_when_declared_and_absent_otherwise() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = vehicle(catalog).members().unwrap();
    let timing = members
        .get(0)
        .unwrap()
        .unwrap()
        .timing()
        .unwrap()
        .expect("the signal declares timing");
    assert_eq!(
        timing.mode().unwrap(),
        ridl_descriptor::TimingMode::StrictPeriodic
    );
    assert_eq!(timing.min_us().unwrap(), Some("100000"));
    assert!(members.get(1).unwrap().unwrap().timing().unwrap().is_none());
    assert!(
        members.get(4).unwrap().unwrap().timing().unwrap().is_none(),
        "a fixed never carries timing"
    );
}

#[test]
fn a_stream_response_has_absent_sizes_and_a_spelled_type_name() {
    // Driver §4 answer 10: no `stream` flag, no per-element bound yet (#336).
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let trace = vehicle(catalog).members().unwrap().get(5).unwrap().unwrap();
    let response = trace.payloads().unwrap().get(1).unwrap().unwrap();
    assert_eq!(response.type_name().unwrap(), "<Point>");
    assert!(rows(response).is_empty());
}

#[test]
fn an_inline_service_shape_is_an_interface_under_the_service_name() {
    let mut package = package();
    package.services.push(Service {
        name: "veh.cluster.hvac".to_owned(),
        shapes: vec![ServiceShape {
            kind: Some(service_shape::Kind::Inline(Interface {
                number: 2,
                provisional: true,
                interactions: vec![interaction(
                    "cabinTemp",
                    1,
                    decl::Kind::SignalDef(SignalDef {
                        payload: "Coord".to_owned(),
                        ..Default::default()
                    }),
                )],
                ..Default::default()
            })),
        }],
        ..Default::default()
    });
    let catalog_bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&catalog_bytes).expect("the lowering writes a valid descriptor");
    let interfaces = catalog.interfaces().unwrap();
    assert_eq!(interfaces.len(), 2);
    let hvac = interfaces.get(1).unwrap().unwrap();
    assert_eq!(hvac.name().unwrap(), "veh.cluster.hvac");
    assert_eq!(hvac.number().unwrap(), 2);
    assert!(hvac.provisional().unwrap());
    assert_eq!(hvac.members().unwrap().len(), 1);
}

#[test]
fn the_retired_list_is_copied_from_the_ir() {
    let mut package = package();
    package.retired.push(RetiredInterface {
        name: "LaneAssist".to_owned(),
        number: 9,
    });
    let bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let retired = catalog.retired().unwrap().get(0).unwrap().unwrap();
    assert_eq!(retired.name().unwrap(), "LaneAssist");
    assert_eq!(retired.number().unwrap(), 9);
}

#[test]
fn a_zero_number_is_an_internal_error() {
    let mut package = package();
    package.interfaces[0].number = 0;
    assert_eq!(
        lower(&package, &[]),
        Err(LowerError::ZeroNumber("Vehicle".to_owned()))
    );
}

#[test]
fn the_bytes_are_stable_across_runs() {
    assert_eq!(
        lower(&package(), &[]).unwrap(),
        lower(&package(), &[]).unwrap()
    );
}
