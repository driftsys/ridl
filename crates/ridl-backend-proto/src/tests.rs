use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as proto3 with protox, panicking with the compiler's own
/// message on failure. This is the story's acceptance check: every test that
/// emits a schema runs it through here.
pub(crate) fn compile_with_protox(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if let Err(error) = protox::compile([file_name], [dir.path()]) {
        panic!("emitted schema is not valid proto3:\n{error}\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn an_empty_package_emits_a_valid_file_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(
        generated.proto_source,
        "syntax = \"proto3\";\n\npackage veh.common;\n"
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_interface_emits_its_ordinal_table() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "VehicleStatus".to_string(),
            interactions: vec![
                signal_decl("currentSpeed", 1),
                signal_decl("doorOpened", 2),
                reserved_decl(3),
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "enum VehicleStatusOrdinal {\n  \
         VEHICLE_STATUS_ORDINAL_UNSPECIFIED = 0;\n  \
         VEHICLE_STATUS_ORDINAL_CURRENT_SPEED = 1;\n  \
         VEHICLE_STATUS_ORDINAL_DOOR_OPENED = 2;\n  \
         reserved 3;\n  \
         VEHICLE_STATUS_ORDINAL_TYRE_PRESSURE = 4;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.cluster.proto", &generated.proto_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
    // Interface.name is "" for an inline shape (ridl §14.5), so the enum takes
    // the service's dotted address instead.
    let package = v2::Package {
        name: "corpus.baseline".to_string(),
        services: vec![v2::Service {
            name: "corpus.baseline.hvac".to_string(),
            shapes: vec![v2::ServiceShape {
                id: 1,
                kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                    name: String::new(),
                    interactions: vec![signal_decl("cabinTemp", 1)],
                    ..Default::default()
                })),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated
            .proto_source
            .contains("enum CorpusBaselineHvacOrdinal {"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated
            .proto_source
            .contains("CORPUS_BASELINE_HVAC_ORDINAL_CABIN_TEMP = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("corpus.baseline.proto", &generated.proto_source);
}

#[test]
fn an_ordinal_in_the_protobuf_reserved_span_is_refused() {
    // Field numbers 19,000 to 19,999 belong to protobuf itself (note §4.2).
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 19_000)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("19000"), "got: {}", error.message);
    assert!(
        error.message.contains("reserved by protobuf"),
        "got: {}",
        error.message
    );
}

#[test]
fn an_ordinal_above_the_proto_ceiling_is_refused() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 536_870_912)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("536870911"),
        "got: {}",
        error.message
    );
}

#[test]
fn a_struct_emits_a_message_numbered_by_typl_ordinals() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "SensorReading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("currentSpeed", 1, float64_type()),
                    field_member("sensorId", 2, int64_type()),
                    reserved_member(3),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "message SensorReading {\n  \
         double current_speed = 1;\n  \
         int64 sensor_id = 2;\n  \
         reserved 3;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_named_scalar_inlines_and_leaves_its_constraint_in_a_comment() {
    // type Speed : km/h [0.0..250.0] — no step, so the checker derives f64
    // (`scalar::derive_float_width` gives f32 only when a step is declared).
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Speed".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(speed_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("value", 1, named_type("Speed"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    // The named type does not become a declaration of its own: it inlines.
    assert!(
        !generated.proto_source.contains("message Speed"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated
            .proto_source
            .contains("// Speed — km/h [0.0..250.0]"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("double value = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_quantized_float_keeps_its_native_width() {
    // type Speed : km/h [0.0..250.0 step 0.5] — the step makes the checker
    // derive f32 (typl §4.3), and a wire backend keeps that native form: the
    // scaled-integer encoding belongs to CAN/DBC and to SOME/IP per
    // deployment, and must not be applied unasked (ADR-0013 decision 4).
    let mut quantized = speed_type_def();
    quantized
        .constraint
        .as_mut()
        .expect("speed_type_def has a constraint")
        .step = Some("0.5".to_string());
    quantized.width = Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32));
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Speed".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(quantized)),
                ..Default::default()
            },
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("value", 1, named_type("Speed"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated
            .proto_source
            .contains("// Speed — km/h [0.0..250.0 step 0.5]"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("float value = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_optional_field_takes_the_proto3_optional_keyword() {
    // ADR-0013 decision 7: proto3 represents absence structurally, so it does.
    let mut ty = float64_type();
    ty.optional = true;
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Reading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member("value", 1, ty)],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .proto_source
            .contains("optional double value = 1;"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_prefixes_its_values_and_gains_a_zero_member() {
    // proto3 scopes enum values as siblings of the enum, so two enums in one
    // package could otherwise both declare OK. And the first value must be 0.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "GearPosition".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    v2::EnumValue {
                        name: "PARK".to_string(),
                        value: 1,
                        doc: String::new(),
                    },
                    v2::EnumValue {
                        name: "DRIVE".to_string(),
                        value: 2,
                        doc: String::new(),
                    },
                ],
                // The retired identity of an enum tombstone lives in `value`,
                // not `ordinal`: `ordinal` is 0 in enum bodies (ir.proto,
                // Reserved.ordinal doc comment).
                reserved: vec![v2::Reserved {
                    value: Some(3),
                    ..Default::default()
                }],
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "enum GearPosition {\n  \
         GEAR_POSITION_UNSPECIFIED = 0;\n  \
         GEAR_POSITION_PARK = 1;\n  \
         GEAR_POSITION_DRIVE = 2;\n  \
         reserved 3;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_that_already_declares_zero_gains_no_synthetic_member() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Mode".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    v2::EnumValue {
                        name: "OFF".to_string(),
                        value: 0,
                        doc: String::new(),
                    },
                    v2::EnumValue {
                        name: "ON".to_string(),
                        value: 1,
                        doc: String::new(),
                    },
                ],
                reserved: Vec::new(),
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        !generated.proto_source.contains("MODE_UNSPECIFIED"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("MODE_OFF = 0;"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_value_outside_int32_is_refused() {
    // proto3 enum values are int32; typl admits int64.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Wide".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![v2::EnumValue {
                    name: "HUGE".to_string(),
                    value: i64::from(i32::MAX) + 1,
                    doc: String::new(),
                }],
                reserved: Vec::new(),
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("int32"), "got: {}", error.message);
}

#[test]
fn a_const_is_not_emitted() {
    // ADR-0013 decision 5: neither proto3 nor FlatBuffers has a constant
    // declaration, and no instance of a typl constant ever crosses a wire.
    // A wire backend may emit one as a comment and must not encode it as an
    // enum, which is the mistake this test exists to catch.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "MAX_GEAR".to_string(),
            kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("integer".to_string()),
                value: "6".to_string(),
                regex: None,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        !generated.proto_source.contains("enum MAX_GEAR"),
        "a const must not become an enum:\n{}",
        generated.proto_source
    );
    assert!(
        !generated.proto_source.contains("message MAX_GEAR"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_set_becomes_an_integer_with_its_bits_in_a_comment() {
    // A proto enum field holds one value, so it cannot represent a
    // combination of bits. Emitting one would imply a guarantee proto3 does
    // not make (ADR-0013 decision 2).
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Warnings".to_string(),
                kind: Some(v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                    backing_enum: None,
                    bits: vec![
                        v2::EnumValue {
                            name: "LOW_FUEL".to_string(),
                            value: 0,
                            doc: String::new(),
                        },
                        v2::EnumValue {
                            name: "DOOR_AJAR".to_string(),
                            value: 1,
                            doc: String::new(),
                        },
                    ],
                    width: v2::IntWidth::U32 as i32,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Status".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("warnings", 1, named_type("Warnings"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        !generated.proto_source.contains("enum Warnings"),
        "an enum set must not become a proto enum:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("uint32 warnings = 1;"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("LOW_FUEL = bit 0"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_struct_field_number_in_the_protobuf_reserved_span_is_refused() {
    // The 19,000–19,999 span and the 536,870,911 ceiling constrain message
    // field numbers, which is what a struct field becomes.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Wide".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member("far", 19_000, float64_type())],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("19000"), "got: {}", error.message);
    assert!(
        error.message.contains("reserved by protobuf"),
        "got: {}",
        error.message
    );
}

/// A signal interaction at `ordinal`. The kind is immaterial to tier 2: the
/// table is interface-wide and kind-blind (ridl §11, ADR-0013 decision 3).
fn signal_decl(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        ordinal,
        kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef::default())),
        ..Default::default()
    }
}

fn reserved_decl(ordinal: u32) -> v2::Decl {
    v2::Decl {
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn field_member(name: &str, ordinal: u32, r#type: v2::FieldType) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(r#type),
            ..Default::default()
        })),
    }
}

fn reserved_member(ordinal: u32) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
            ordinal,
            ..Default::default()
        })),
    }
}

/// A bare `float` field type — a direct primitive use, whose domain is
/// float64 (typl §4).
fn float64_type() -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::Float as i32,
        )),
    }
}

/// A bare `integer` field type — a direct primitive use, whose domain is
/// int64 (typl §4).
fn int64_type() -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::Integer as i32,
        )),
    }
}

fn named_type(name: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(name.to_string())),
    }
}

/// `type Speed : km/h [0.0..250.0]` as the checker lowers it: a unit backing
/// (which implies the float primitive, typl §5.1), the canonical constraint
/// strings, and the derived f64 width — no step is declared, and
/// `scalar::derive_float_width` gives f32 only when one is.
fn speed_type_def() -> v2::TypeDef {
    v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit("km/h".to_string())),
        }),
        constraint: Some(v2::Constraint {
            min: Some("0.0".to_string()),
            max: Some("250.0".to_string()),
            ..Default::default()
        }),
        width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F64 as i32)),
        ..Default::default()
    }
}
