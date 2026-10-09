// This test uses the half of the crate that builds a descriptor, which the
// `std` feature carries.
#![cfg(feature = "std")]

//! Spec D-4: what the catalog descriptor contains, checked through `verify`.

use ridl_descriptor::lower::LowerError;
use ridl_descriptor::{
    CatalogRef, Encoding, InterfaceRef, Kind, SCHEMA_VERSION, SizeStateTag, UnboundedCause, lower,
    verify,
};
use ridl_ir::projection::flatbuffers::{Packages, max_size};
use ridl_ir::v2::{
    ArrayType, Backing, CommandDef, Constraint, Decl, EventDef, Field, FieldType, FixedDef,
    IntWidth, Interface, MapType, Package, Param, PrimitiveType, QueryDef, Reserved,
    RetiredInterface, ReturnType, Service, ServiceShape, SignalDef, StreamType, StructDef,
    StructMember, Timing, TimingMode, TupleField, TupleType, TypeDef, backing, decl, field_type,
    return_type, service_shape, stream_type, struct_member, type_def, unit_of,
};

/// Lowers the unit of `package` when `package` is its only member.
fn lower_one(package: &Package) -> Result<Vec<u8>, LowerError> {
    lower(unit_of(package), &[package])
}

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

/// A `Range` timing with the given bounds, in microseconds.
fn range(min_us: Option<&str>, max_us: Option<&str>) -> Timing {
    Timing {
        mode: TimingMode::Range as i32,
        min_us: min_us.map(str::to_owned),
        max_us: max_us.map(str::to_owned),
        default_applied: false,
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

/// Coord = integer i16; Point { x: Coord, y: Coord }; interface Vehicle,
/// locked (not provisional), with one member of every kind and one reserved
/// slot. The event, the command and the query `nearest` declare a `Range`
/// timing, so every kind that carries timing carries one here.
fn package() -> Package {
    let field = |name: &str, ordinal: u32| StructMember {
        member: Some(struct_member::Member::Field(Box::new(Field {
            name: name.to_owned(),
            ordinal,
            r#type: Some(named("Coord")),
            ..Default::default()
        }))),
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
            provisional: false,
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
                        timing: Some(range(Some("100000"), Some("1000000"))),
                    }),
                ),
                interaction(
                    "moveTo",
                    3,
                    decl::Kind::CommandDef(CommandDef {
                        params: vec![Param {
                            name: "to".to_owned(),
                            r#type: Some(named("Point")),
                            ..Default::default()
                        }],
                        timing: Some(range(None, Some("50000"))),
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
                            ..Default::default()
                        }],
                        return_type: Some(ReturnType {
                            kind: Some(return_type::Kind::Value(named("Point"))),
                        }),
                        timing: Some(range(Some("20000"), Some("200000"))),
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
                                ..Default::default()
                            },
                            Param {
                                name: "b".to_owned(),
                                r#type: Some(named("Point")),
                                ..Default::default()
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
                            ..Default::default()
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
    let bytes = lower_one(&package()).unwrap();
    let catalog = verify(&bytes).expect("the lowering writes a valid descriptor");
    assert_eq!(catalog.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.name().unwrap(), "veh.cluster");
    assert_eq!(catalog.hash().unwrap().len(), 32);
    assert_eq!(catalog.toolchain().unwrap(), env!("CARGO_PKG_VERSION"));
    assert_eq!(catalog.retired().unwrap().len(), 0);

    let interface = vehicle(catalog);
    assert_eq!(interface.name().unwrap(), "Vehicle");
    assert_eq!(interface.number().unwrap(), 1);
    // The fixture's interface is locked; the inline shape in
    // `an_inline_service_shape_is_an_interface_under_the_service_name` is
    // provisional, so each value of the flag is read back once.
    assert!(!interface.provisional().unwrap());
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

/// The descriptor's first interface: `Vehicle` in most fixtures here.
fn vehicle(catalog: CatalogRef<'_>) -> InterfaceRef<'_> {
    catalog.interfaces().unwrap().get(0).unwrap().unwrap()
}

/// The whole of every row of one payload: (encoding, state, bytes, cause).
/// Every field the schema carries, so a row that changed any one of them
/// fails the assertion.
fn rows(
    payload: ridl_descriptor::PayloadRef<'_>,
) -> Vec<(Encoding, SizeStateTag, u32, UnboundedCause)> {
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
                s.cause().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_named_type_payload_has_a_row_per_sized_encoding_and_no_repr_c() {
    let package = package();
    let bytes = lower_one(&package).unwrap();
    let catalog = verify(&bytes).unwrap();
    let position = vehicle(catalog).members().unwrap().get(0).unwrap().unwrap();
    let payload = position.payloads().unwrap().get(0).unwrap().unwrap();
    assert_eq!(payload.type_name().unwrap(), "Point");
    assert_eq!(
        rows(payload),
        vec![
            // ADR-0017: i16 projects to sint32, at most 5 bytes; (1-byte tag + 5) x 2 fields = 12.
            (
                Encoding::Proto3,
                SizeStateTag::Bounded,
                12,
                UnboundedCause::Unspecified
            ),
            (
                Encoding::FlatBuffers,
                SizeStateTag::Bounded,
                point_fb_bound(&package),
                UnboundedCause::Unspecified
            ),
        ]
    );
}

#[test]
fn a_request_of_one_named_parameter_is_sized_and_of_several_is_absent() {
    let package = package();
    let bytes = lower_one(&package).unwrap();
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
    // Which encodings and which byte counts, not how many rows: the request
    // of one named parameter carries that type's own two rows.
    assert_eq!(
        rows(move_to),
        vec![
            (
                Encoding::Proto3,
                SizeStateTag::Bounded,
                12,
                UnboundedCause::Unspecified
            ),
            (
                Encoding::FlatBuffers,
                SizeStateTag::Bounded,
                point_fb_bound(&package),
                UnboundedCause::Unspecified
            ),
        ]
    );
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
        "several parameters: `docs/design/catalog-descriptor.md`"
    );
}

#[test]
fn a_fallible_reply_is_absent() {
    let bytes = lower_one(&package()).unwrap();
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
        "an inline `T | E`: `docs/design/catalog-descriptor.md`"
    );
}

/// `Open { text: string }` with no length bound on the string, behind the one
/// event of one interface. The FlatBuffers projection cannot bound that
/// member, so this is the package that reaches the lowering's unbounded arm.
fn unbounded_payload_package() -> Package {
    Package {
        name: "veh.open".to_owned(),
        decls: vec![Decl {
            name: "Open".to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: vec![StructMember {
                    member: Some(struct_member::Member::Field(Box::new(Field {
                        name: "text".to_owned(),
                        ordinal: 1,
                        r#type: Some(FieldType {
                            optional: false,
                            kind: Some(field_type::Kind::Primitive(PrimitiveType::String as i32)),
                        }),
                        ..Default::default()
                    }))),
                }],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        interfaces: vec![Interface {
            name: "Opener".to_owned(),
            number: 1,
            provisional: false,
            interactions: vec![interaction(
                "opened",
                1,
                decl::Kind::EventDef(EventDef {
                    payload: "Open".to_owned(),
                    timing: None,
                }),
            )],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn an_unbounded_payload_lowers_to_one_unbounded_flatbuffers_row() {
    // The lowering's unbounded arm, end to end through `lower`: the state,
    // the cause and the byte count of the row it writes. A lowering that
    // wrote `Bounded`, the unspecified cause, or any byte count other than 0
    // fails here. proto3 refuses a member no leaf bounds, so its state is
    // absent and writes no row at all.
    let bytes = lower_one(&unbounded_payload_package()).unwrap();
    let catalog = verify(&bytes).unwrap();
    let payload = payload_at(catalog, 0, 0);
    assert_eq!(payload.type_name().unwrap(), "Open");
    assert_eq!(
        rows(payload),
        vec![(
            Encoding::FlatBuffers,
            SizeStateTag::Unbounded,
            0,
            UnboundedCause::Member
        )]
    );
}

/// A member's lowered timing as `(mode, min_us, max_us)`, `None` when the
/// member carries none.
fn timing_of(
    member: ridl_descriptor::MemberRef<'_>,
) -> Option<(ridl_descriptor::TimingMode, Option<String>, Option<String>)> {
    member.timing().unwrap().map(|t| {
        (
            t.mode().unwrap(),
            t.min_us().unwrap().map(str::to_owned),
            t.max_us().unwrap().map(str::to_owned),
        )
    })
}

#[test]
fn timing_is_carried_when_declared_and_absent_otherwise() {
    use ridl_descriptor::TimingMode::{Range, StrictPeriodic};
    let bytes = lower_one(&package()).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = vehicle(catalog).members().unwrap();
    let us = |v: &str| Some(v.to_owned());
    let timings: Vec<_> = members
        .iter()
        .map(|m| {
            let m = m.unwrap();
            (m.name().unwrap().to_owned(), timing_of(m))
        })
        .collect();
    assert_eq!(
        timings,
        vec![
            (
                "position".to_owned(),
                Some((StrictPeriodic, us("100000"), None))
            ),
            (
                "doorOpened".to_owned(),
                Some((Range, us("100000"), us("1000000")))
            ),
            ("moveTo".to_owned(), Some((Range, None, us("50000")))),
            (
                "nearest".to_owned(),
                Some((Range, us("20000"), us("200000")))
            ),
            // A fixed never carries timing.
            ("vin".to_owned(), None),
            ("trace".to_owned(), None),
            ("moveBoth".to_owned(), None),
            ("tryNearest".to_owned(), None),
        ]
    );
}

#[test]
fn a_stream_response_has_absent_sizes_and_a_spelled_type_name() {
    // Driver §4 answer 10: no `stream` flag, no per-element bound yet (#336).
    let bytes = lower_one(&package()).unwrap();
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
    let catalog_bytes = lower_one(&package).unwrap();
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
    let bytes = lower_one(&package).unwrap();
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
        lower_one(&package),
        Err(LowerError::ZeroNumber("Vehicle".to_owned()))
    );
}

#[test]
fn the_bytes_are_stable_across_runs() {
    assert_eq!(
        lower_one(&package()).unwrap(),
        lower_one(&package()).unwrap()
    );
}

/// One payload of the member at `index` of the fixture's interface.
fn payload_at(
    catalog: CatalogRef<'_>,
    index: usize,
    payload: usize,
) -> ridl_descriptor::PayloadRef<'_> {
    vehicle(catalog)
        .members()
        .unwrap()
        .get(index)
        .unwrap()
        .unwrap()
        .payloads()
        .unwrap()
        .get(payload)
        .unwrap()
        .unwrap()
}

#[test]
fn a_query_response_of_one_named_type_is_sized() {
    let package = package();
    let bytes = lower_one(&package).unwrap();
    let catalog = verify(&bytes).unwrap();
    let response = payload_at(catalog, 3, 1);
    assert_eq!(response.role().unwrap(), "response");
    assert_eq!(response.type_name().unwrap(), "Point");
    assert_eq!(
        rows(response),
        vec![
            (
                Encoding::Proto3,
                SizeStateTag::Bounded,
                12,
                UnboundedCause::Unspecified
            ),
            (
                Encoding::FlatBuffers,
                SizeStateTag::Bounded,
                point_fb_bound(&package),
                UnboundedCause::Unspecified
            ),
        ]
    );
}

#[test]
fn the_event_and_the_fixed_payloads_name_their_types() {
    let bytes = lower_one(&package()).unwrap();
    let catalog = verify(&bytes).unwrap();
    assert_eq!(payload_at(catalog, 1, 0).type_name().unwrap(), "Point");
    assert_eq!(payload_at(catalog, 4, 0).type_name().unwrap(), "Coord");
}

/// `veh.geo` declares `Coord` and `Point`; `veh.cluster` declares no type and
/// names `veh.geo.Point` as its one signal's payload, so sizing the payload
/// and hashing the catalog both need `veh.geo` among `others`.
fn importing_and_imported() -> (Package, Package) {
    let geo = Package {
        name: "veh.geo".to_owned(),
        decls: package().decls,
        ..Default::default()
    };
    let cluster = Package {
        name: "veh.cluster".to_owned(),
        interfaces: vec![Interface {
            name: "Vehicle".to_owned(),
            number: 1,
            interactions: vec![interaction(
                "position",
                1,
                decl::Kind::SignalDef(SignalDef {
                    payload: "veh.geo.Point".to_owned(),
                    ..Default::default()
                }),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    (cluster, geo)
}

#[test]
fn a_payload_from_another_package_is_sized_and_hashed_through_others() {
    let (cluster, geo) = importing_and_imported();
    let with = lower("veh.cluster", &[&cluster, &geo]).unwrap();
    let without = lower_one(&cluster).unwrap();

    let catalog = verify(&with).unwrap();
    let payload = payload_at(catalog, 0, 0);
    assert_eq!(payload.type_name().unwrap(), "veh.geo.Point");
    assert_eq!(
        rows(payload),
        vec![
            (
                Encoding::Proto3,
                SizeStateTag::Bounded,
                12,
                UnboundedCause::Unspecified
            ),
            (
                Encoding::FlatBuffers,
                SizeStateTag::Bounded,
                point_fb_bound(&geo),
                UnboundedCause::Unspecified
            ),
        ]
    );

    // Without `others` the name resolves nowhere: no row is sized, and the
    // hash closure loses `Point`, so the two hashes differ.
    let alone = verify(&without).unwrap();
    assert!(rows(payload_at(alone, 0, 0)).is_empty());
    assert_ne!(catalog.hash().unwrap(), alone.hash().unwrap());
}

/// A field type of `kind`, required.
fn ty(kind: field_type::Kind) -> FieldType {
    FieldType {
        optional: false,
        kind: Some(kind),
    }
}

/// An inline scalar over `backing` with `constraint`.
fn inline(backing: backing::Kind, constraint: Constraint) -> FieldType {
    ty(field_type::Kind::InlineScalar(Box::new(TypeDef {
        backing: Some(Backing {
            kind: Some(backing),
        }),
        constraint: Some(constraint),
        ..Default::default()
    })))
}

#[test]
fn a_structural_type_name_is_spelled_in_the_typl_syntax_over_the_canonical_values() {
    let integer = || ty(field_type::Kind::Primitive(PrimitiveType::Integer as i32));
    let cases: Vec<(FieldType, &str)> = vec![
        (integer(), "integer"),
        (
            ty(field_type::Kind::Primitive(PrimitiveType::Boolean as i32)),
            "boolean",
        ),
        (
            FieldType {
                optional: true,
                kind: Some(field_type::Kind::Named("Coord".to_owned())),
            },
            "Coord?",
        ),
        (
            ty(field_type::Kind::Tuple(TupleType {
                fields: vec![
                    TupleField {
                        name: "lo".to_owned(),
                        r#type: Some(named("Coord")),
                    },
                    TupleField {
                        name: "hi".to_owned(),
                        r#type: Some(named("Coord")),
                    },
                ],
            })),
            "(lo: Coord, hi: Coord)",
        ),
        (
            ty(field_type::Kind::Array(Box::new(ArrayType {
                element: Some(Box::new(named("Coord"))),
                min: 8,
                max: 8,
            }))),
            "[Coord; 8]",
        ),
        (
            ty(field_type::Kind::Array(Box::new(ArrayType {
                element: Some(Box::new(named("Coord"))),
                min: 0,
                max: 32,
            }))),
            "[Coord; 0..32]",
        ),
        (
            ty(field_type::Kind::Map(Box::new(MapType {
                key: Some(Box::new(ty(field_type::Kind::Primitive(
                    PrimitiveType::String as i32,
                )))),
                value: Some(Box::new(named("Coord"))),
                min: 1,
                max: 8,
            }))),
            "[string: Coord; 1..8]",
        ),
        (
            inline(
                backing::Kind::Unit("km/h".to_owned()),
                Constraint {
                    min: Some("0".to_owned()),
                    max: Some("250".to_owned()),
                    step: Some("0.5".to_owned()),
                    ..Default::default()
                },
            ),
            "km/h [0..250 step 0.5]",
        ),
        (
            inline(
                backing::Kind::Primitive(PrimitiveType::Integer as i32),
                Constraint {
                    max: Some("100".to_owned()),
                    ..Default::default()
                },
            ),
            "integer [..100]",
        ),
        (
            inline(
                backing::Kind::Primitive(PrimitiveType::String as i32),
                Constraint {
                    len_min: Some(17),
                    len_max: Some(17),
                    pattern: Some("/^[A-Z]+$/".to_owned()),
                    pattern_const: Some("VIN_PATTERN".to_owned()),
                    ..Default::default()
                },
            ),
            "string [17 match VIN_PATTERN]",
        ),
        (
            inline(
                backing::Kind::Primitive(PrimitiveType::Bytes as i32),
                Constraint {
                    len_min: Some(0),
                    len_max: Some(64),
                    pattern: Some("/^ab/".to_owned()),
                    ..Default::default()
                },
            ),
            "bytes [0..64 match /^ab/]",
        ),
    ];
    let members = cases
        .iter()
        .zip(1..)
        .map(|((field, _), ordinal)| {
            interaction(
                &format!("m{ordinal}"),
                ordinal,
                decl::Kind::FixedDef(FixedDef {
                    payload: Some(field.clone()),
                }),
            )
        })
        .collect();
    let package = Package {
        name: "veh.cluster".to_owned(),
        decls: package().decls,
        interfaces: vec![Interface {
            name: "Vehicle".to_owned(),
            number: 1,
            interactions: members,
            ..Default::default()
        }],
        ..Default::default()
    };
    let bytes = lower_one(&package).unwrap();
    let catalog = verify(&bytes).unwrap();
    let spelled: Vec<String> = (0..cases.len())
        .map(|index| {
            payload_at(catalog, index, 0)
                .type_name()
                .unwrap()
                .to_owned()
        })
        .collect();
    let expected: Vec<&str> = cases.iter().map(|(_, spelling)| *spelling).collect();
    assert_eq!(spelled, expected);
}

/// Unit `u` has the root package `u` (interface `Session`, number 2) and the
/// package `u.cluster` (interface `Speed`, number 1, retired `Old` 3, spelled
/// as its lock key).
#[test]
fn a_unit_of_two_packages_lowers_to_one_descriptor_with_qualified_names() {
    let interface = |name: &str, number: u32| Interface {
        name: name.to_owned(),
        number,
        interactions: vec![interaction(
            "v",
            1,
            decl::Kind::SignalDef(SignalDef {
                payload: "integer".to_owned(),
                ..Default::default()
            }),
        )],
        ..Default::default()
    };
    let root = Package {
        name: "u".to_owned(),
        interfaces: vec![interface("Session", 2)],
        ..Default::default()
    };
    let cluster = Package {
        name: "u.cluster".to_owned(),
        unit: "u".to_owned(),
        interfaces: vec![interface("Speed", 1)],
        retired: vec![RetiredInterface {
            name: "cluster.Old".to_owned(),
            number: 3,
        }],
        ..Default::default()
    };
    let bytes = lower("u", &[&root, &cluster]).unwrap();
    let catalog = verify(&bytes).unwrap();
    assert_eq!(catalog.name().unwrap(), "u");
    let interfaces = catalog.interfaces().unwrap();
    let names: Vec<&str> = interfaces
        .iter()
        .map(|i| i.unwrap().name().unwrap())
        .collect();
    assert_eq!(names, ["cluster.Speed", "Session"]);
    let retired = catalog.retired().unwrap();
    assert_eq!(retired.len(), 1);
    let old = retired.get(0).unwrap().unwrap();
    assert_eq!(
        (old.name().unwrap(), old.number().unwrap()),
        ("cluster.Old", 3)
    );
    // A package of another unit is not part of the descriptor, whether its
    // name is unrelated or extends the unit's (`u.sib` is not in `u`).
    let other = Package {
        name: "w".to_owned(),
        interfaces: vec![interface("Other", 1)],
        ..Default::default()
    };
    let sibling = Package {
        name: "u.sib".to_owned(),
        unit: "u.sib".to_owned(),
        interfaces: vec![interface("Sib", 4)],
        ..Default::default()
    };
    assert_eq!(
        lower("u", &[&root, &cluster, &other, &sibling]).unwrap(),
        bytes
    );
}
