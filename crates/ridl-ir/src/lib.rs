//! RIDL intermediate representation.
//!
//! Two schema versions are compiled from protobuf sources by `build.rs`
//! (protox + prost-build, ADR-0006 decision 3):
//!
//! - `v0` — the walking-skeleton subset (`proto/ridl/ir/v0/ir.proto`): named
//!   scalar types with optional units and ranges, and value constants,
//!   grouped into a `Module`. Re-exported at the crate root until task 13 of
//!   the E1 plan retires its last consumer.
//! - `v1` — the typl surface with exact decimal values
//!   (`proto/ridl/ir/v1/ir.proto`), exposed as `ridl_ir::v1`. Every numeric
//!   value is a canonical decimal string, never a floating-point field
//!   (ADR-0007 decision 9).

mod v0 {
    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v0.rs"));
}

pub use v0::{ConstDef, Module, Range, TypeDef};

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

#[cfg(test)]
mod round_trip {
    use crate::{ConstDef, Module, Range, TypeDef};
    use prost::Message;

    fn fixture() -> Module {
        Module {
            name: "vehicle".to_string(),
            types: vec![TypeDef {
                name: "Speed".to_string(),
                unit: "km/h".to_string(),
                range: Some(Range {
                    min: 0.0,
                    max: 250.0,
                    step: 0.5,
                }),
            }],
            consts: vec![ConstDef {
                name: "MAX_SPEED".to_string(),
                type_name: "Speed".to_string(),
                value: 250.0,
            }],
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_module() {
        let module = fixture();

        let mut buf = Vec::new();
        module.encode(&mut buf).expect("encode must succeed");
        let decoded = Module::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(module, decoded);
    }

    #[test]
    fn json_rendering_contains_type_name() {
        let module = fixture();

        let json = serde_json::to_string(&module).expect("json serialization must succeed");

        assert!(
            json.contains(r#""name":"Speed""#),
            "json debug rendering must include the Speed type name, got: {json}"
        );
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
                    declared_init: Some("6".to_string()),
                    init: Some(v1::InitValue {
                        derivable: true,
                        value: Some("6".to_string()),
                    }),
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

        // union SensorResult { ok : SensorReading, err : SensorFault }
        let sensor_result = v1::UnionDef {
            arms: vec![
                v1::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "SensorReading".to_string(),
                },
                v1::UnionArm {
                    name: "err".to_string(),
                    ordinal: 2,
                    type_ref: "SensorFault".to_string(),
                },
            ],
            is_result: true,
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
