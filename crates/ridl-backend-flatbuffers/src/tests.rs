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

#[test]
fn an_enum_keeps_its_values_unprefixed_with_an_explicit_underlying_type() {
    let package = enum_package("GearPosition", vec![("PARK", 1), ("DRIVE", 2)]);
    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .fbs_source
            .contains("enum GearPosition : long {\n  PARK = 1,\n  DRIVE = 2,\n}"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_field_typed_by_an_enum_without_a_zero_member_takes_null() {
    // flatc: "default value of `0` for field `g` is not part of enum `Gear`".
    // FlatBuffers cannot mark a scalar or enum field required either, so
    // `= null` is the honest rendering — it never fabricates a reading.
    let package = enum_and_field("Gear", vec![("PARK", 1)], "g");
    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains("g: Gear = null (id: 0);"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_field_typed_by_an_enum_declaring_zero_needs_no_null() {
    let package = enum_and_field("Mode", vec![("OFF", 0), ("ON", 1)], "m");
    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains("m: Mode (id: 0);"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn an_enum_set_becomes_an_integer_with_its_bits_in_a_comment() {
    // A FlatBuffers enum field holds one value, so it cannot represent a
    // combination of bits. Emitting one would imply a guarantee the target
    // does not make (ADR-0013 decision 2).
    let package = enum_set_and_field("Warnings", vec![("LOW_FUEL", 0), ("DOOR_AJAR", 1)]);
    let generated = generate(&package).expect("generate");
    assert!(
        !generated.fbs_source.contains("enum Warnings"),
        "an enum set must not become a FlatBuffers enum:\n{}",
        generated.fbs_source
    );
    assert!(
        generated.fbs_source.contains("LOW_FUEL = bit 0"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_union_is_isolated_in_a_wrapper_table() {
    // A native union owns TWO id slots (a hidden _type plus the value), so
    // one in an ordinal-owned slot shifts every later id. The wrapper takes
    // one slot in the parent and keeps the union's two in its own id space.
    let package = union_and_field(
        "Payload",
        vec![("speed", 1, "Speed"), ("gearIndex", 2, "GearIndex")],
    );
    let generated = generate(&package).expect("generate");

    assert!(
        generated
            .fbs_source
            .contains("union PayloadUnion { Speed, GearIndex }"),
        "got:\n{}",
        generated.fbs_source
    );
    assert!(
        generated
            .fbs_source
            .contains("table Payload {\n  value: PayloadUnion (id: 1);\n}"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_union_field_takes_exactly_one_slot_in_its_parent() {
    let package = struct_with_union_between_scalars();
    let generated = generate(&package).expect("generate");
    // before: id 0, union: id 1, after: id 2 — the mapping is intact.
    assert!(
        generated.fbs_source.contains("after: long (id: 2);"),
        "the union must not consume its neighbour's slot:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn an_array_field_is_a_vector() {
    let package = struct_package("Trace", "samples", 1, array_of(float64_type()));
    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains("samples: [double] (id: 0);"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_map_field_is_a_vector_of_entry_tables_with_no_key_attribute() {
    // FlatBuffers has no map. (key) is deliberately NOT emitted: it obliges
    // the producer to sort, and typl §12.2 gives a map no ordering.
    let package = struct_package(
        "Index",
        "byName",
        1,
        map_of(v2::PrimitiveType::String, float64_type()),
    );
    let generated = generate(&package).expect("generate");

    assert!(
        generated.fbs_source.contains("table IndexByNameEntry {"),
        "got:\n{}",
        generated.fbs_source
    );
    assert!(
        generated
            .fbs_source
            .contains("by_name: [IndexByNameEntry] (id: 0);"),
        "got:\n{}",
        generated.fbs_source
    );
    assert!(
        !generated.fbs_source.contains("(key)"),
        "(key) must not be emitted:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_tuple_field_induces_a_positional_table() {
    let package = struct_package(
        "Reading",
        "bounds",
        1,
        tuple_of(vec![float64_type(), float64_type()]),
    );
    let generated = generate(&package).expect("generate");
    assert!(
        generated.fbs_source.contains(
            "table ReadingBounds {\n  field_1: double (id: 0);\n  field_2: double (id: 1);\n}"
        ),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_generated_name_colliding_with_a_declared_type_is_refused() {
    // Wrapper tables and entry tables mint names into the namespace scope,
    // which FlatBuffers shares across tables, structs, enums and unions —
    // `flatc` rejects a repeat with "datatype already exists".
    //
    // Here a declared struct `ReadingBounds` collides with the table induced
    // by the tuple field `bounds` on struct `Reading`, whose generated name
    // is `<Owner><Field>` = `ReadingBounds`.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member(
                        "bounds",
                        1,
                        tuple_of(vec![float64_type(), float64_type()]),
                    )],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "ReadingBounds".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("x", 1, float64_type())],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("ReadingBounds"),
        "got: {}",
        error.message
    );
}

/// A package with one struct `struct_name` holding one field — the generic
/// single-field fixture the container tests share.
fn struct_package(
    struct_name: &str,
    field_name: &str,
    ordinal: u32,
    ty: v2::FieldType,
) -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: struct_name.to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(field_name, ordinal, ty)],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A bounded array of `element` (typl §12.1). Bounds are mandatory in the IR
/// (TYPL-201) but land nowhere in this projection, so the fixture picks
/// arbitrary ones.
fn array_of(element: v2::FieldType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(element)),
            min: 0,
            max: 16,
        }))),
    }
}

/// A bounded map from primitive `key` to `value` (typl §12.2). Bounds are
/// entry counts, mandatory in the IR (TYPL-202) and unused here.
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
            max: 64,
        }))),
    }
}

/// An inline tuple whose fields take the given types, named `f1`, `f2`, … —
/// the names are immaterial: the generated table is positional (typl §11).
fn tuple_of(types: Vec<v2::FieldType>) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| v2::TupleField {
                    name: format!("f{}", index + 1),
                    r#type: Some(ty),
                })
                .collect(),
        })),
    }
}

/// A one-field struct declaration named `name`. A FlatBuffers union member
/// must be a table, so union fixtures declare each arm target as a struct.
fn arm_struct(name: &str) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member("value", 1, float64_type())],
            fixed_layout: false,
        })),
        ..Default::default()
    }
}

/// A union declaration named `name`, its arms as `(name, ordinal, type_ref)`
/// triples (typl §10).
fn union_decl(name: &str, arms: &[(&str, u32, &str)]) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: arms
                .iter()
                .map(|(arm_name, ordinal, type_ref)| v2::UnionArm {
                    name: arm_name.to_string(),
                    ordinal: *ordinal,
                    type_ref: type_ref.to_string(),
                    doc: String::new(),
                })
                .collect(),
            is_result: false,
            reserved: Vec::new(),
        })),
        ..Default::default()
    }
}

/// A package with a union named `name`, one struct declaration per arm
/// target so the schema compiles, and one struct holding a field of the
/// union type at ordinal 1 — enough to exercise the wrapper emission and the
/// use-site reference together.
fn union_and_field(name: &str, arms: Vec<(&str, u32, &str)>) -> v2::Package {
    let mut decls: Vec<v2::Decl> = arms
        .iter()
        .map(|(_, _, type_ref)| arm_struct(type_ref))
        .collect();
    decls.push(union_decl(name, &arms));
    decls.push(v2::Decl {
        name: "Holder".to_string(),
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member("payload", 1, named_type(name))],
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

/// A scalar at ordinal 1, a union-typed field at ordinal 2 and a scalar at
/// ordinal 3 — the arrangement that proves a union reference takes exactly
/// one id slot in its parent.
fn struct_with_union_between_scalars() -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            arm_struct("Speed"),
            union_decl("Payload", &[("speed", 1, "Speed")]),
            v2::Decl {
                name: "Frame".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![
                        field_member("before", 1, int64_type()),
                        field_member("payload", 2, named_type("Payload")),
                        field_member("after", 3, int64_type()),
                    ],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
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

/// One `(name, value)` pair per enum value or enum-set bit — `EnumValue` is
/// the shared IR type for both (typl §8, §9.1).
fn enum_values(values: Vec<(&str, i64)>) -> Vec<v2::EnumValue> {
    values
        .into_iter()
        .map(|(name, value)| v2::EnumValue {
            name: name.to_string(),
            value,
            doc: String::new(),
        })
        .collect()
}

/// A package with one top-level enum declaration named `name`, its values in
/// declaration order.
fn enum_package(name: &str, values: Vec<(&str, i64)>) -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: name.to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: enum_values(values),
                reserved: Vec::new(),
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// [`enum_package`] plus one struct with a single field of that enum type at
/// ordinal 1, named `field_name` — enough to exercise the enum-typed field's
/// default-value rule.
fn enum_and_field(enum_name: &str, values: Vec<(&str, i64)>, field_name: &str) -> v2::Package {
    let mut package = enum_package(enum_name, values);
    package.decls.push(v2::Decl {
        name: "Holder".to_string(),
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(field_name, 1, named_type(enum_name))],
            fixed_layout: false,
        })),
        ..Default::default()
    });
    package
}

/// A package with one top-level standalone enum-set declaration named
/// `name`, its bits in declaration order, plus one struct with a single
/// field of that enum-set type at ordinal 1 — enough to exercise the
/// no-declaration, comment-only use-site rule.
fn enum_set_and_field(name: &str, bits: Vec<(&str, i64)>) -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: name.to_string(),
                kind: Some(v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                    backing_enum: None,
                    bits: enum_values(bits),
                    width: v2::IntWidth::U8 as i32,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Holder".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("warnings", 1, named_type(name))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
