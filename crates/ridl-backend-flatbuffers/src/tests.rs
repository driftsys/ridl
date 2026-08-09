use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as a FlatBuffers schema with planus, panicking with the
/// compiler's own message on failure. This is the story's acceptance check:
/// every test that emits a schema runs it through here.
pub(crate) fn compile_with_planus(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if planus_translation::translate_files(&[path]).is_none() {
        panic!("emitted schema is not a valid FlatBuffers schema:\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn an_empty_package_emits_a_valid_namespace_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(generated.fbs_source, "namespace veh.common;\n");
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
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
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    // FlatBuffers scopes enum values to their enum, so values are NOT
    // prefixed with the enum name — unlike the proto3 backend.
    assert!(
        generated.fbs_source.contains(
            "enum VehicleStatusOrdinal : uint {\n  \
             CURRENT_SPEED = 1,\n  \
             DOOR_OPENED = 2,\n  \
             TYRE_PRESSURE = 4,\n}"
        ),
        "got:\n{}",
        generated.fbs_source
    );

    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn two_enums_may_share_a_value_name() {
    // The scoping difference from proto3, pinned so nobody reintroduces
    // prefixing by copying the proto backend.
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![
            v2::Interface {
                name: "Alpha".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
            v2::Interface {
                name: "Beta".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("enum AlphaOrdinal : uint {"));
    assert!(generated.fbs_source.contains("enum BetaOrdinal : uint {"));
    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
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
            .fbs_source
            .contains("enum CorpusBaselineHvacOrdinal : uint {"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("corpus.baseline.fbs", &generated.fbs_source);
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

#[test]
fn a_struct_emits_a_table_with_ids_one_below_the_ordinal() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "SensorReading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("currentSpeed", 1, float64_type()),
                    field_member("sensorId", 2, int64_type()),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains(
            "table SensorReading {\n  \
             current_speed: double (id: 0);\n  \
             sensor_id: long (id: 1);\n}"
        ),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_fixed_layout_struct_is_still_a_table() {
    // typl Appendix D permits a FlatBuffers `struct` here. The design
    // withdraws that: a FlatBuffers struct fabricates a value from padding
    // after a compatible append, which makes ADR-0016 decision 6 property 3
    // unsatisfiable. The flag must not change the emitted construct.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Packed".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member("a", 1, int64_type())],
                fixed_layout: true,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains("table Packed {"),
        "got:\n{}",
        generated.fbs_source
    );
    assert!(
        !generated.fbs_source.contains("struct Packed"),
        "a fixed-layout struct must not become a FlatBuffers struct:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_retired_field_holds_its_slot_as_a_deprecated_field() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Reading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("value", 1, float64_type()),
                    reserved_member(2),
                    field_member("trim", 3, int64_type()),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    // The tombstone keeps id 1 occupied so `trim` stays at id 2.
    assert!(
        generated.fbs_source.contains("(id: 1, deprecated)"),
        "got:\n{}",
        generated.fbs_source
    );
    assert!(
        generated.fbs_source.contains("trim: long (id: 2);"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn every_width_takes_its_narrow_flatbuffers_type() {
    // The full uint8..uint64 palette is the reason to choose this target
    // (typl Appendix D, ADR-0013 decision 4). A width silently widened here
    // is the defect this test exists to catch.
    let package = widths_package();
    let generated = generate(&package).expect("generate");
    for expected in [
        "u8_field: ubyte (id: 0);",
        "u16_field: ushort (id: 1);",
        "u32_field: uint (id: 2);",
        "u64_field: ulong (id: 3);",
        "i8_field: byte (id: 4);",
        "i16_field: short (id: 5);",
        "i32_field: int (id: 6);",
        "i64_field: long (id: 7);",
        "f32_field: float (id: 8);",
        "f64_field: double (id: 9);",
    ] {
        assert!(
            generated.fbs_source.contains(expected),
            "missing `{expected}` in:\n{}",
            generated.fbs_source
        );
    }
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_const_is_not_emitted() {
    // ADR-0013 decision 5: neither proto3 nor FlatBuffers has a constant
    // declaration, and no instance of a typl constant ever crosses a wire.
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
        !generated.fbs_source.contains("MAX_GEAR"),
        "a const must not be emitted:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_named_scalar_inlines_and_leaves_its_constraint_in_a_comment() {
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
    assert!(
        !generated.fbs_source.contains("table Speed"),
        "a named scalar must inline:\n{}",
        generated.fbs_source
    );
    assert!(
        generated.fbs_source.contains("// Speed"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
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

/// One named scalar per typl width (typl Appendix D), and one struct with a
/// field for each, at ordinals 1 to 10 in the table above. No constraint is
/// declared on any of them: the width is set directly, the same way
/// `ridl-backend-proto`'s `gear_index_type_def` fixture does, so the width
/// table is what each field's type exercises, not constraint derivation.
fn widths_package() -> v2::Package {
    let widths: [(&str, &str, v2::type_def::Width); 10] = [
        (
            "U8Type",
            "u8Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32),
        ),
        (
            "U16Type",
            "u16Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32),
        ),
        (
            "U32Type",
            "u32Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32),
        ),
        (
            "U64Type",
            "u64Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::U64 as i32),
        ),
        (
            "I8Type",
            "i8Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::I8 as i32),
        ),
        (
            "I16Type",
            "i16Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::I16 as i32),
        ),
        (
            "I32Type",
            "i32Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::I32 as i32),
        ),
        (
            "I64Type",
            "i64Field",
            v2::type_def::Width::IntWidth(v2::IntWidth::I64 as i32),
        ),
        (
            "F32Type",
            "f32Field",
            v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32),
        ),
        (
            "F64Type",
            "f64Field",
            v2::type_def::Width::FloatWidth(v2::FloatWidth::F64 as i32),
        ),
    ];

    let mut decls: Vec<v2::Decl> = widths
        .iter()
        .map(|(type_name, _, width)| v2::Decl {
            name: type_name.to_string(),
            kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                width: Some(*width),
                ..Default::default()
            })),
            ..Default::default()
        })
        .collect();

    let members = widths
        .iter()
        .enumerate()
        .map(|(index, (type_name, field_name, _))| {
            field_member(field_name, (index + 1) as u32, named_type(type_name))
        })
        .collect();

    decls.push(v2::Decl {
        name: "Widths".to_string(),
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members,
            fixed_layout: false,
        })),
        ..Default::default()
    });

    v2::Package {
        name: "veh.common".to_string(),
        decls,
        ..Default::default()
    }
}
