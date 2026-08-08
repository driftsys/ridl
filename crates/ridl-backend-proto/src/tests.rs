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
fn a_declared_zero_leads_even_when_it_is_not_declared_first() {
    // typl §8 assigns every enum value explicitly and does not require the
    // zero-valued member to come first in source order — `lower_enum`
    // preserves source order and does not sort. proto3 requires the emitted
    // first value to be zero regardless, so a zero declared later must still
    // be moved to lead.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Mode".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    v2::EnumValue {
                        name: "ON".to_string(),
                        value: 1,
                        doc: String::new(),
                    },
                    v2::EnumValue {
                        name: "OFF".to_string(),
                        value: 0,
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
        generated
            .proto_source
            .contains("enum Mode {\n  MODE_OFF = 0;\n  MODE_ON = 1;\n}"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_retired_zero_slot_synthesizes_unspecified_without_a_conflicting_reserved() {
    // typl allows retiring the zero slot with `reserved 0` instead of giving
    // it a live member. proto3 still requires a live value at 0, so
    // MODE_UNSPECIFIED = 0 is synthesized to fill it — and the matching
    // `reserved 0;` must NOT also be emitted, because protoc rejects the
    // same number claimed as both a live value and reserved.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Mode".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![v2::EnumValue {
                    name: "ON".to_string(),
                    value: 1,
                    doc: String::new(),
                }],
                reserved: vec![v2::Reserved {
                    value: Some(0),
                    ..Default::default()
                }],
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .proto_source
            .contains("enum Mode {\n  MODE_UNSPECIFIED = 0;\n  MODE_ON = 1;\n}"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        !generated.proto_source.contains("reserved 0;"),
        "a retired zero slot filled by the synthetic member must not also be reserved:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_struct_field_typed_by_an_enum_projects_and_compiles() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
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
                    reserved: Vec::new(),
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Status".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("gear", 1, named_type("GearPosition"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated.proto_source.contains("GearPosition gear = 1;"),
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

#[test]
fn a_union_becomes_a_message_wrapping_a_oneof() {
    // "Speed" and "GearIndex" must be declared for their arms to resolve —
    // a union arm references a named type the same way a struct field does
    // (typl §10.1, TYPL-204), and `named_field_type` refuses an
    // undeclared reference. The union body itself carries no constraint
    // comment (a `oneof` arm has no line of its own to hold one), so the
    // two referenced types are given no unit or constraint here — the
    // assertion below would otherwise have to account for one.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Speed".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(speed_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "GearIndex".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(gear_index_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "Payload".to_string(),
                kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![
                        v2::UnionArm {
                            name: "speed".to_string(),
                            ordinal: 1,
                            type_ref: "Speed".to_string(),
                            doc: String::new(),
                        },
                        v2::UnionArm {
                            name: "gearIndex".to_string(),
                            ordinal: 2,
                            type_ref: "GearIndex".to_string(),
                            doc: String::new(),
                        },
                    ],
                    is_result: false,
                    reserved: vec![v2::Reserved {
                        ordinal: 3,
                        ..Default::default()
                    }],
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "message Payload {\n  \
             oneof value {\n    \
             double speed = 1;\n    \
             sint64 gear_index = 2;\n  \
             }\n  \
             reserved 3;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_struct_field_typed_by_a_named_union_projects_and_compiles() {
    // `named_field_type` must resolve a `UnionDef` reference the same way it
    // resolves a struct or an enum: a union is legal wherever data is legal
    // (typl §10), so a field typed by one must project rather than fail with
    // "a declaration kind this backend does not project yet".
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Speed".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(speed_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "GearIndex".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(gear_index_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "Payload".to_string(),
                kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![
                        v2::UnionArm {
                            name: "speed".to_string(),
                            ordinal: 1,
                            type_ref: "Speed".to_string(),
                            doc: String::new(),
                        },
                        v2::UnionArm {
                            name: "gearIndex".to_string(),
                            ordinal: 2,
                            type_ref: "GearIndex".to_string(),
                            doc: String::new(),
                        },
                    ],
                    is_result: false,
                    reserved: Vec::new(),
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("value", 1, named_type("Payload"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains("message Payload {"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("Payload value = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_array_field_is_repeated() {
    let package = struct_package("Trace", "samples", 1, array_of(float64_type()));
    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .proto_source
            .contains("repeated double samples = 1;"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_map_field_becomes_a_proto_map() {
    let package = struct_package(
        "Index",
        "byName",
        1,
        map_of(v2::PrimitiveType::String, float64_type()),
    );
    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .proto_source
            .contains("map<string, double> by_name = 1;"),
        "got:\n{}",
        generated.proto_source
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_map_key_type_proto3_cannot_carry_is_refused() {
    // proto3 restricts a map key to an integral or string type. typl admits
    // a bare `float` at a map key position (TYPL-209 accepts any
    // primitive), so this is a real gap this backend must refuse rather
    // than hand `protoc` a `map<double, ...>` it rejects.
    let package = struct_package(
        "Index",
        "byReading",
        1,
        map_of(v2::PrimitiveType::Float, float64_type()),
    );
    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("map key"), "got: {}", error.message);
}

#[test]
fn a_map_value_that_is_itself_repeated_is_refused() {
    // proto3 does not admit a `repeated` map value.
    let package = struct_package(
        "Index",
        "byName",
        1,
        map_of(v2::PrimitiveType::String, array_of(float64_type())),
    );
    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("repeated"), "got: {}", error.message);
}

#[test]
fn a_map_value_that_is_itself_a_map_is_refused() {
    // proto3 forbids a map value from being another map (the language
    // guide, "Maps": the value type "can be any type except another map").
    let package = struct_package(
        "Index",
        "byName",
        1,
        map_of(
            v2::PrimitiveType::String,
            map_of(v2::PrimitiveType::String, float64_type()),
        ),
    );
    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("another map"),
        "got: {}",
        error.message
    );
}

#[test]
fn an_array_of_arrays_is_refused() {
    // proto3 has no nested `repeated`: `repeated repeated double rows = 1;`
    // is a syntax error (`protoc`: "expected '=', but found 'rows'"), not
    // merely unusual. Nested arrays are legal typl (the checker accepts
    // them, and `ridlc/tests/cli.rs` exercises a 60-level nested-array
    // package through other backends), so this is a real gap to close
    // rather than a theoretical one.
    let package = struct_package("Grid", "rows", 1, array_of(array_of(float64_type())));
    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("array of arrays"),
        "got: {}",
        error.message
    );
}

#[test]
fn an_array_of_maps_is_refused() {
    // A map field is already implicitly repeated on the wire, so applying
    // `repeated` to it is a field-label conflict `protoc` rejects ("Field
    // labels are not allowed on map fields"), not merely unusual.
    let package = struct_package(
        "Index",
        "byName",
        1,
        array_of(map_of(v2::PrimitiveType::String, float64_type())),
    );
    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("array of maps"),
        "got: {}",
        error.message
    );
}

#[test]
fn a_tuple_field_induces_a_positional_message() {
    // proto3 has no tuple, so one is generated, named for its owner and field.
    let package = struct_package(
        "Reading",
        "bounds",
        1,
        tuple_of(vec![float64_type(), float64_type()]),
    );
    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "message ReadingBounds {\n  \
             double field_1 = 1;\n  \
             double field_2 = 2;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated.proto_source.contains("ReadingBounds bounds = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.common.proto", &generated.proto_source);
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

/// A signed 64-bit named scalar with no unit or constraint — enough to
/// resolve to proto3's `sint64` (typl Appendix D) without leaving a
/// constraint comment worth checking.
fn gear_index_type_def() -> v2::TypeDef {
    v2::TypeDef {
        width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::I64 as i32)),
        ..Default::default()
    }
}

/// A package holding one struct `name` with one field `field_name`, at
/// `ordinal`, typed `r#type`.
fn struct_package(
    name: &str,
    field_name: &str,
    ordinal: u32,
    r#type: v2::FieldType,
) -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: name.to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(field_name, ordinal, r#type)],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A bounded array of `element` (typl §12.1). The bound is immaterial to
/// this backend — proto3's `repeated` carries no bound — so an arbitrary
/// non-degenerate one is used.
fn array_of(element: v2::FieldType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(element)),
            min: 0,
            max: 8,
        }))),
    }
}

/// A bounded map from a bare `key` primitive to `value` (typl §12.2). The
/// bound is immaterial here for the same reason as [`array_of`].
fn map_of(key: v2::PrimitiveType, value: v2::FieldType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
            key: Some(Box::new(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Primitive(key as i32)),
            })),
            value: Some(Box::new(value)),
            min: 0,
            max: 8,
        }))),
    }
}

/// An anonymous tuple of `fields`, in order (typl §11). Each tuple field
/// needs a source name of its own (typl §11 — positional access is not
/// permitted), but this backend generates positional `field_N` names
/// regardless of it, so the source name here is arbitrary.
fn tuple_of(fields: Vec<v2::FieldType>) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, r#type)| v2::TupleField {
                    name: format!("f{index}"),
                    r#type: Some(r#type),
                })
                .collect(),
        })),
    }
}
