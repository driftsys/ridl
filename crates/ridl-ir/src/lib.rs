//! RIDL intermediate representation.
//!
//! The v1 schema (`proto/ridl/ir/v1/ir.proto`) and the v2 schema
//! (`proto/ridl/ir/v2/ir.proto`) are compiled from their protobuf sources by
//! `build.rs` (protox + prost-build, ADR-0006 decision 3) and exposed as
//! [`v1`] and [`v2`]. v1 is the typl surface with exact decimal values —
//! every numeric value is a canonical decimal string, never a floating-point
//! field (ADR-0007 decision 9). v2 holds every v1 message verbatim plus the
//! ridl interaction layer on the numbers v1 earmarked for it (ADR-0008
//! decision 8); v1 stays until the v2 lowering lands, mirroring the E1 v0→v1
//! transition. The E0 walking-skeleton v0 schema was removed when its last
//! consumer moved to v1 (task 13 of the E1 plan).

pub mod v1 {
    //! IR v1 — the typl surface (typl language reference §3–§12) with exact
    //! decimal values (ADR-0007 decision 9).

    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v1.rs"));

    /// Renders a package as pretty-printed JSON — the debug rendering of the
    /// IR (ADR-0004 §4), used by golden tests and diagnostic output.
    pub fn to_json_pretty(package: &Package) -> String {
        serde_json::to_string_pretty(package)
            .expect("IR serialization to JSON cannot fail: the generated types hold only JSON-representable values")
    }
}

pub mod v2 {
    //! IR v2 — the typl surface plus the ridl interaction layer (ridl
    //! language reference §3–§14) with exact decimal values (ADR-0007
    //! decision 9, ADR-0008 decision 12).

    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v2.rs"));

    /// Derives the synthesized transport identity of an inline `T | E`
    /// result union (ADR-0008 decision 4): the enclosing interface name plus
    /// the interaction ordinal plus the ordered arm references. The single
    /// derivation every consumer — backends and the diff classifier — calls,
    /// so the identity stays stable under compatible evolution.
    pub fn fallible_transport_identity(
        interface: &str,
        ordinal: u32,
        fallible: &FallibleType,
    ) -> String {
        format!(
            "{interface}#{ordinal}:{ok}|{err}",
            ok = fallible.ok,
            err = fallible.err
        )
    }

    /// Renders a package as pretty-printed JSON — the debug rendering of the
    /// IR (ADR-0004 §4), used by golden tests and diagnostic output.
    pub fn to_json_pretty(package: &Package) -> String {
        serde_json::to_string_pretty(package)
            .expect("IR serialization to JSON cannot fail: the generated types hold only JSON-representable values")
    }
}

#[cfg(test)]
mod v1_round_trip {
    use crate::v1;
    use prost::Message;

    /// Wraps a declaration kind in the shared `Decl` envelope with defaults:
    /// public, not an error type, no doc metadata.
    fn decl(name: &str, kind: v1::decl::Kind) -> v1::Decl {
        v1::Decl {
            name: name.to_string(),
            visibility: v1::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            kind: Some(kind),
        }
    }

    fn constraint(min: &str, max: &str, step: Option<&str>) -> v1::Constraint {
        v1::Constraint {
            min: Some(min.to_string()),
            max: Some(max.to_string()),
            step: step.map(str::to_string),
            len_min: None,
            len_max: None,
            pattern: None,
            pattern_const: None,
        }
    }

    fn named_field(name: &str, ordinal: u32, type_name: &str, optional: bool) -> v1::Field {
        v1::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(v1::FieldType {
                kind: Some(v1::field_type::Kind::Named(type_name.to_string())),
                optional,
            }),
            declared_init: None,
            init: Some(v1::InitValue {
                derivable: true,
                value: None,
            }),
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
        }
    }

    fn field_member(field: v1::Field) -> v1::StructMember {
        v1::StructMember {
            member: Some(v1::struct_member::Member::Field(field)),
        }
    }

    fn warning_bits() -> Vec<v1::EnumValue> {
        [
            ("LOW_FUEL", 0),
            ("CHECK_ENGINE", 1),
            ("DOOR_OPEN", 2),
            ("SEATBELT", 3),
        ]
        .into_iter()
        .map(|(name, value)| v1::EnumValue {
            name: name.to_string(),
            value,
            doc: String::new(),
        })
        .collect()
    }

    /// A representative slice of the typl reference Appendix B package: one
    /// unit type with constraint and declared init, one constant, one struct
    /// with a reserved tombstone, an optional field, and an inline scalar
    /// field, one enum with a derived enumset, and one result union.
    fn fixture() -> v1::Package {
        // type Speed : km/h [0.0..250.0 step 0.5] = 0.0
        let speed = v1::TypeDef {
            backing: Some(v1::Backing {
                kind: Some(v1::backing::Kind::Unit("km/h".to_string())),
            }),
            constraint: Some(constraint("0.0", "250.0", Some("0.5"))),
            declared_init: Some("0.0".to_string()),
            init: Some(v1::InitValue {
                derivable: true,
                value: Some("0.0".to_string()),
            }),
            width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
        };

        // const MAX_SPEED : Speed = 250.0
        let max_speed = v1::ConstDef {
            type_ref: Some("Speed".to_string()),
            value: "250.0".to_string(),
            regex: None,
        };

        // struct DriverProfile — reserved tombstone at ordinal 2, optional
        // field, and an inline scalar field `gears : integer [0..6] = 6`.
        let gears = v1::Field {
            name: "gears".to_string(),
            ordinal: 4,
            r#type: Some(v1::FieldType {
                kind: Some(v1::field_type::Kind::InlineScalar(Box::new(v1::TypeDef {
                    backing: Some(v1::Backing {
                        kind: Some(v1::backing::Kind::Primitive(
                            v1::PrimitiveType::Integer as i32,
                        )),
                    }),
                    constraint: Some(constraint("0", "6", None)),
                    // The enclosing Field's declared_init and init are
                    // authoritative for an inline scalar; the nested
                    // TypeDef's init fields stay unset.
                    declared_init: None,
                    init: None,
                    width: Some(v1::type_def::Width::IntWidth(v1::IntWidth::U8 as i32)),
                }))),
                optional: false,
            }),
            declared_init: Some("6".to_string()),
            init: Some(v1::InitValue {
                derivable: true,
                value: Some("6".to_string()),
            }),
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
        };
        let driver_profile = v1::StructDef {
            members: vec![
                field_member(named_field("name", 1, "Name", false)),
                v1::StructMember {
                    member: Some(v1::struct_member::Member::Reserved(v1::Reserved {
                        ordinal: 2,
                        name: Some("legacyChecksum".to_string()),
                        value: None,
                    })),
                },
                field_member(named_field("override", 3, "Speed", true)),
                field_member(gears),
            ],
            fixed_layout: false,
        };

        // enum Warning { … } and the derived enumset WarningFlags : Warning.
        let warning = v1::EnumDef {
            values: warning_bits(),
            reserved: Vec::new(),
        };
        let warning_flags = v1::EnumSetDef {
            backing_enum: Some("Warning".to_string()),
            bits: warning_bits(),
            width: v1::IntWidth::U8 as i32,
        };

        // union SensorResult { ok : SensorReading, reserved legacyErr,
        // err : SensorFault } — a tombstoned arm keeps ordinal 2 occupied.
        let sensor_result = v1::UnionDef {
            arms: vec![
                v1::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "SensorReading".to_string(),
                    doc: "Successful reading".to_string(),
                },
                v1::UnionArm {
                    name: "err".to_string(),
                    ordinal: 3,
                    type_ref: "SensorFault".to_string(),
                    doc: String::new(),
                },
            ],
            is_result: true,
            reserved: vec![v1::Reserved {
                ordinal: 2,
                name: Some("legacyErr".to_string()),
                value: None,
            }],
        };

        v1::Package {
            name: "veh.common".to_string(),
            decls: vec![
                v1::Decl {
                    doc: "Vehicle speed over ground".to_string(),
                    ..decl("Speed", v1::decl::Kind::TypeDef(speed))
                },
                decl("MAX_SPEED", v1::decl::Kind::ConstDef(max_speed)),
                decl("DriverProfile", v1::decl::Kind::StructDef(driver_profile)),
                decl("Warning", v1::decl::Kind::EnumDef(warning)),
                decl("WarningFlags", v1::decl::Kind::EnumSetDef(warning_flags)),
                decl("SensorResult", v1::decl::Kind::UnionDef(sensor_result)),
            ],
        }
    }

    /// A slice of Appendix B's `SensorBounds`: a tuple field, a fixed array,
    /// and a bounded map.
    fn sensor_bounds() -> v1::Package {
        let range = v1::Field {
            r#type: Some(v1::FieldType {
                kind: Some(v1::field_type::Kind::Tuple(v1::TupleType {
                    fields: vec![
                        v1::TupleField {
                            name: "min".to_string(),
                            r#type: Some(v1::FieldType {
                                kind: Some(v1::field_type::Kind::Named("Speed".to_string())),
                                optional: false,
                            }),
                        },
                        v1::TupleField {
                            name: "max".to_string(),
                            r#type: Some(v1::FieldType {
                                kind: Some(v1::field_type::Kind::Named("Speed".to_string())),
                                optional: false,
                            }),
                        },
                    ],
                })),
                optional: false,
            }),
            ..named_field("range", 1, "", false)
        };
        // readings : [Speed; 8] — fixed array, min == max == 8.
        let readings = v1::Field {
            r#type: Some(v1::FieldType {
                kind: Some(v1::field_type::Kind::Array(Box::new(v1::ArrayType {
                    element: Some(Box::new(v1::FieldType {
                        kind: Some(v1::field_type::Kind::Named("Speed".to_string())),
                        optional: false,
                    })),
                    min: 8,
                    max: 8,
                }))),
                optional: false,
            }),
            ..named_field("readings", 2, "", false)
        };
        // meta : [Label : Name; 0..32] — bounded map.
        let meta = v1::Field {
            r#type: Some(v1::FieldType {
                kind: Some(v1::field_type::Kind::Map(Box::new(v1::MapType {
                    key: Some(Box::new(v1::FieldType {
                        kind: Some(v1::field_type::Kind::Named("Label".to_string())),
                        optional: false,
                    })),
                    value: Some(Box::new(v1::FieldType {
                        kind: Some(v1::field_type::Kind::Named("Name".to_string())),
                        optional: false,
                    })),
                    min: 0,
                    max: 32,
                }))),
                optional: false,
            }),
            ..named_field("meta", 3, "", false)
        };

        v1::Package {
            name: "veh.common".to_string(),
            decls: vec![decl(
                "SensorBounds",
                v1::decl::Kind::StructDef(v1::StructDef {
                    members: vec![
                        field_member(range),
                        field_member(readings),
                        field_member(meta),
                    ],
                    fixed_layout: false,
                }),
            )],
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_package() {
        let package = fixture();

        let mut buf = Vec::new();
        package.encode(&mut buf).expect("encode must succeed");
        let decoded = v1::Package::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(package, decoded);
        let Some(v1::decl::Kind::UnionDef(union_def)) = &decoded.decls[5].kind else {
            panic!("SensorResult must decode as a union");
        };
        assert_eq!(
            union_def.reserved[0].name.as_deref(),
            Some("legacyErr"),
            "the tombstoned union arm must survive the round trip"
        );
    }

    #[test]
    fn json_round_trip_preserves_package() {
        let package = fixture();

        let json = v1::to_json_pretty(&package);
        let decoded: v1::Package =
            serde_json::from_str(&json).expect("json deserialization must succeed");

        assert_eq!(package, decoded);
    }

    #[test]
    fn json_renders_range_bounds_as_decimal_strings() {
        let json = v1::to_json_pretty(&fixture());

        // Exactness is visible: bounds and steps are canonical decimal
        // strings, never floating-point numbers (ADR-0007 decision 9).
        assert!(
            json.contains(r#""min": "0.0""#),
            "the min bound must be a JSON string, got: {json}"
        );
        assert!(
            json.contains(r#""step": "0.5""#),
            "the step must be a JSON string, got: {json}"
        );
    }

    #[test]
    fn tuple_field_type_survives_round_trip() {
        let package = sensor_bounds();

        let mut buf = Vec::new();
        package.encode(&mut buf).expect("encode must succeed");
        let decoded = v1::Package::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(package, decoded);
        let Some(v1::decl::Kind::StructDef(struct_def)) = &decoded.decls[0].kind else {
            panic!("SensorBounds must decode as a struct");
        };
        let Some(v1::struct_member::Member::Field(range)) = &struct_def.members[0].member else {
            panic!("the first member must decode as a field");
        };
        let Some(v1::field_type::Kind::Tuple(tuple)) = &range.r#type.as_ref().unwrap().kind else {
            panic!("the range field must decode as a tuple");
        };
        let names: Vec<&str> = tuple.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["min", "max"], "tuple field names must survive");
    }

    #[test]
    fn array_and_map_bounds_survive_round_trip() {
        let package = sensor_bounds();

        let mut buf = Vec::new();
        package.encode(&mut buf).expect("encode must succeed");
        let decoded = v1::Package::decode(buf.as_slice()).expect("decode must succeed");

        let Some(v1::decl::Kind::StructDef(struct_def)) = &decoded.decls[0].kind else {
            panic!("SensorBounds must decode as a struct");
        };
        let field_kind = |index: usize| {
            let Some(v1::struct_member::Member::Field(field)) = &struct_def.members[index].member
            else {
                panic!("member {index} must decode as a field");
            };
            field.r#type.as_ref().unwrap().kind.as_ref().unwrap()
        };
        let v1::field_type::Kind::Array(array) = field_kind(1) else {
            panic!("readings must decode as an array");
        };
        assert_eq!((array.min, array.max), (8, 8), "fixed array bounds");
        let v1::field_type::Kind::Map(map) = field_kind(2) else {
            panic!("meta must decode as a map");
        };
        assert_eq!((map.min, map.max), (0, 32), "bounded map bounds");
    }
}

#[cfg(test)]
mod v2_round_trip {
    use crate::v2;
    use prost::Message;

    /// Wraps an interaction kind in the shared `Decl` envelope. Visibility
    /// and `is_error` stay unset on interactions (ridl §14.1); the ordinal is
    /// the 1-based declaration order across all interactions of the
    /// enclosing interface (ridl §11).
    fn interaction(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            visibility: v2::Visibility::Unspecified as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal,
            kind: Some(kind),
        }
    }

    fn named_type(name: &str) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(name.to_string())),
        }
    }

    fn stream_of(element: v2::stream_type::Element) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                element: Some(element),
            })),
        }
    }

    /// A representative ridl package: one interface holding all five
    /// interaction kinds plus a reserved tombstone (ordinals 1–6, the
    /// tombstone counted, ridl §11), a strict-periodic and a defaulted
    /// range timing, a fallible query, and two services — a named
    /// reference and an inline shape holding a stream query.
    fn fixture() -> v2::Package {
        // signal speed : Speed @10ms — strict periodic stores the period
        // in both bounds (ADR-0008 decision 12).
        let speed = v2::SignalDef {
            payload: "Speed".to_string(),
            declared_init: None,
            init: Some(v2::InitValue {
                derivable: true,
                value: Some("0.0".to_string()),
            }),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("10000".to_string()),
                max_us: Some("10000".to_string()),
                default_applied: false,
            }),
        };

        // event doorOpened : DoorEvent — untimed in source, so the
        // configured default range is resolved at compile time (ridl §9.1).
        let door_opened = v2::EventDef {
            payload: "DoorEvent".to_string(),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("20000".to_string()),
                max_us: Some("500000".to_string()),
                default_applied: true,
            }),
        };

        // command setTarget(target : Speed) [ require target >= speed ]
        let set_target = v2::CommandDef {
            params: vec![v2::Param {
                name: "target".to_string(),
                r#type: Some(named_type("Speed")),
            }],
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Require as i32,
                source: "target >= speed".to_string(),
                signal_refs: vec!["speed".to_string()],
                param_refs: vec!["target".to_string()],
                uses_result: false,
                observer_id: "VehicleStatus.setTarget.require[0]".to_string(),
            }],
        };

        // query fetchFaults(page : PageSpec) : FaultPage | DiagError
        //   [ ensure result.count <= page.limit ]
        let fetch_faults = v2::QueryDef {
            params: vec![v2::Param {
                name: "page".to_string(),
                r#type: Some(named_type("PageSpec")),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Ensure as i32,
                source: "result.count <= page.limit".to_string(),
                signal_refs: Vec::new(),
                param_refs: vec!["page".to_string()],
                uses_result: true,
                observer_id: "VehicleStatus.fetchFaults.ensure[0]".to_string(),
            }],
        };

        // final vin : Vin
        let vin = v2::FinalDef {
            payload: Some(named_type("Vin")),
        };

        let vehicle_status = v2::Interface {
            name: "VehicleStatus".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: "Vehicle status contract".to_string(),
            labels: Vec::new(),
            deprecated: None,
            interactions: vec![
                interaction("speed", 1, v2::decl::Kind::SignalDef(speed)),
                interaction("doorOpened", 2, v2::decl::Kind::EventDef(door_opened)),
                // reserved legacyMode — the tombstone keeps ordinal 3
                // occupied in the one interaction sequence (ridl §11).
                v2::Decl {
                    ordinal: 3,
                    kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
                        ordinal: 3,
                        name: Some("legacyMode".to_string()),
                        value: None,
                    })),
                    ..interaction("", 3, v2::decl::Kind::ReservedSlot(v2::Reserved::default()))
                },
                interaction("setTarget", 4, v2::decl::Kind::CommandDef(set_target)),
                interaction("fetchFaults", 5, v2::decl::Kind::QueryDef(fetch_faults)),
                interaction("vin", 6, v2::decl::Kind::FinalDef(vin)),
            ],
        };

        // query tailLogs(pattern : <string>) : <LogLine> — a stream param
        // and a stream return (ridl §12), inside the inline service shape.
        let tail_logs = v2::QueryDef {
            params: vec![v2::Param {
                name: "pattern".to_string(),
                r#type: Some(stream_of(v2::stream_type::Element::Primitive(
                    v2::PrimitiveType::String as i32,
                ))),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(stream_of(
                    v2::stream_type::Element::Named("LogLine".to_string()),
                ))),
            }),
            contracts: Vec::new(),
        };

        // service veh.adas.status : VehicleStatus — named reference.
        let status_service = v2::Service {
            name: "veh.adas.status".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::InterfaceRef(
                "VehicleStatus".to_string(),
            )),
        };
        // service veh.adas.logs { … } — inline shape, Interface.name == ""
        // (ridl §14.5).
        let logs_service = v2::Service {
            name: "veh.adas.logs".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::Inline(v2::Interface {
                name: String::new(),
                visibility: v2::Visibility::Unspecified as i32,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                interactions: vec![interaction(
                    "tailLogs",
                    1,
                    v2::decl::Kind::QueryDef(tail_logs),
                )],
            })),
        };

        v2::Package {
            name: "veh.adas".to_string(),
            // One typl declaration proves the verbatim v1 surface rides
            // along unchanged in v2; package-level declarations carry
            // ordinal 0.
            decls: vec![v2::Decl {
                name: "Speed".to_string(),
                visibility: v2::Visibility::Public as i32,
                is_error: false,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                ordinal: 0,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Unit("km/h".to_string())),
                    }),
                    constraint: None,
                    declared_init: None,
                    init: None,
                    width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
                })),
            }],
            interfaces: vec![vehicle_status],
            services: vec![status_service, logs_service],
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_package() {
        let package = fixture();

        let mut buf = Vec::new();
        package.encode(&mut buf).expect("encode must succeed");
        let decoded = v2::Package::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(package, decoded);

        let interface = &decoded.interfaces[0];
        let ordinals: Vec<u32> = interface.interactions.iter().map(|d| d.ordinal).collect();
        assert_eq!(
            ordinals,
            [1, 2, 3, 4, 5, 6],
            "one ordinal sequence, tombstone counted (ridl §11)"
        );
        let Some(v2::decl::Kind::ReservedSlot(tombstone)) = &interface.interactions[2].kind else {
            panic!("ordinal 3 must decode as a reserved tombstone");
        };
        assert_eq!(tombstone.name.as_deref(), Some("legacyMode"));
        let Some(v2::service::Shape::Inline(inline)) = &decoded.services[1].shape else {
            panic!("veh.adas.logs must decode as an inline shape");
        };
        assert_eq!(inline.name, "", "an inline shape carries no name");
    }

    #[test]
    fn json_round_trip_preserves_package() {
        let package = fixture();

        let json = v2::to_json_pretty(&package);
        let decoded: v2::Package =
            serde_json::from_str(&json).expect("json deserialization must succeed");

        assert_eq!(package, decoded);
    }

    #[test]
    fn json_renders_timing_bounds_and_fallible_arms_exactly() {
        let json = v2::to_json_pretty(&fixture());

        // Exactness is visible: timing bounds are exact-decimal microsecond
        // strings, never floating-point numbers (ADR-0008 decision 12).
        assert!(
            json.contains(r#""min_us": "10000""#),
            "the timing bound must be a JSON string, got: {json}"
        );
        // Both arms of the inline T | E return are visible by name.
        assert!(
            json.contains(r#""ok": "FaultPage""#),
            "the ok arm must render, got: {json}"
        );
        assert!(
            json.contains(r#""err": "DiagError""#),
            "the err arm must render, got: {json}"
        );
    }

    #[test]
    fn fallible_transport_identity_follows_the_derivation_rule() {
        // The ADR-0008 decision 4 rule: interface + interaction ordinal +
        // both arm references, in that order.
        let fallible = v2::FallibleType {
            ok: "FaultPage".to_string(),
            err: "DiagError".to_string(),
        };
        assert_eq!(
            v2::fallible_transport_identity("VehicleStatus", 9, &fallible),
            "VehicleStatus#9:FaultPage|DiagError"
        );

        // Derived from the fixture: the fallible query sits at ordinal 5.
        let package = fixture();
        let interface = &package.interfaces[0];
        let query_decl = &interface.interactions[4];
        let Some(v2::decl::Kind::QueryDef(query)) = &query_decl.kind else {
            panic!("ordinal 5 must be the fallible query");
        };
        let Some(v2::return_type::Kind::Fallible(arms)) = &query.return_type.as_ref().unwrap().kind
        else {
            panic!("fetchFaults must return a fallible type");
        };
        assert_eq!(
            v2::fallible_transport_identity(&interface.name, query_decl.ordinal, arms),
            "VehicleStatus#5:FaultPage|DiagError"
        );
    }
}
