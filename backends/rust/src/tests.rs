//! Backend tests: per-construct snapshots, the Appendix B rustc-compile proof,
//! Default derivation behaviour (the leaf-recursion rule), and the C header
//! snapshot.

use super::{Generated, generate};
use ridl_ir::v1;

// ---------------------------------------------------------------------------
// Fixture builders.
// ---------------------------------------------------------------------------

fn public_decl(name: &str, kind: v1::decl::Kind) -> v1::Decl {
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

fn package(name: &str, decls: Vec<v1::Decl>) -> v1::Package {
    v1::Package {
        name: name.to_string(),
        decls,
    }
}

fn init_value(derivable: bool, value: Option<&str>) -> v1::InitValue {
    v1::InitValue {
        derivable,
        value: value.map(str::to_string),
    }
}

fn constraint(min: Option<&str>, max: Option<&str>, step: Option<&str>) -> v1::Constraint {
    v1::Constraint {
        min: min.map(str::to_string),
        max: max.map(str::to_string),
        step: step.map(str::to_string),
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    }
}

fn unit_type(unit: &str, min: &str, max: &str, step: &str, init: &str) -> v1::decl::Kind {
    v1::decl::Kind::TypeDef(v1::TypeDef {
        backing: Some(v1::Backing {
            kind: Some(v1::backing::Kind::Unit(unit.to_string())),
        }),
        constraint: Some(constraint(Some(min), Some(max), Some(step))),
        declared_init: None,
        init: Some(init_value(true, Some(init))),
        width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
    })
}

fn primitive_type(
    prim: v1::PrimitiveType,
    init: v1::InitValue,
    width: Option<v1::type_def::Width>,
) -> v1::decl::Kind {
    v1::decl::Kind::TypeDef(v1::TypeDef {
        backing: Some(v1::Backing {
            kind: Some(v1::backing::Kind::Primitive(prim as i32)),
        }),
        constraint: None,
        declared_init: None,
        init: Some(init),
        width,
    })
}

fn named_field(
    name: &str,
    ordinal: u32,
    type_ref: &str,
    optional: bool,
    init: v1::InitValue,
) -> v1::Field {
    v1::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(v1::FieldType {
            optional,
            kind: Some(v1::field_type::Kind::Named(type_ref.to_string())),
        }),
        declared_init: None,
        init: Some(init),
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

fn reserved_member(ordinal: u32, name: &str) -> v1::StructMember {
    v1::StructMember {
        member: Some(v1::struct_member::Member::Reserved(v1::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        })),
    }
}

fn enum_value(name: &str, value: i64) -> v1::EnumValue {
    v1::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
    }
}

fn warning_bits() -> Vec<v1::EnumValue> {
    vec![
        enum_value("LOW_FUEL", 0),
        enum_value("CHECK_ENGINE", 1),
        enum_value("DOOR_OPEN", 2),
        enum_value("SEATBELT", 3),
    ]
}

/// A `Speed`-like unit type declaration used across the small fixtures.
fn speed_decl() -> v1::Decl {
    v1::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    }
}

fn counter_decl() -> v1::Decl {
    public_decl(
        "Counter",
        primitive_type(
            v1::PrimitiveType::Integer,
            init_value(true, Some("0")),
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::U16 as i32)),
        ),
    )
}

// ---------------------------------------------------------------------------
// Per-construct snapshots.
// ---------------------------------------------------------------------------

fn rust_for(decls: Vec<v1::Decl>) -> String {
    generate(&package("veh.common", decls))
        .expect("generation succeeds")
        .rust_source
}

#[test]
fn scalar_unit_type_with_doc() {
    insta::assert_snapshot!(rust_for(vec![speed_decl()]));
}

#[test]
fn named_scalar_backings() {
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl(
            "Enabled",
            primitive_type(
                v1::PrimitiveType::Boolean,
                init_value(true, Some("false")),
                None,
            ),
        ),
        public_decl(
            "Label",
            primitive_type(v1::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "Blob",
            primitive_type(v1::PrimitiveType::Bytes, init_value(true, Some("")), None),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn constants_all_forms() {
    let decls = vec![
        speed_decl(),
        public_decl(
            "MAX_SPEED",
            v1::decl::Kind::ConstDef(v1::ConstDef {
                type_ref: Some("Speed".to_string()),
                value: "250.0".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "MAX_GEAR",
            v1::decl::Kind::ConstDef(v1::ConstDef {
                type_ref: Some("integer".to_string()),
                value: "6".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "Greeting",
            primitive_type(v1::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "BANNER",
            v1::decl::Kind::ConstDef(v1::ConstDef {
                type_ref: Some("Greeting".to_string()),
                value: "hello".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "VIN_PATTERN",
            v1::decl::Kind::ConstDef(v1::ConstDef {
                type_ref: None,
                value: String::new(),
                regex: Some("^[A-HJ-NPR-Z0-9]{17}$".to_string()),
            }),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn struct_with_optional_and_reserved() {
    // A struct that mixes a normal field, a reserved tombstone (occupies an
    // ordinal, emits no field), and an optional field.
    let struct_def = v1::StructDef {
        members: vec![
            field_member(named_field(
                "name",
                1,
                "Label",
                false,
                init_value(false, None),
            )),
            reserved_member(2, "legacyChecksum"),
            field_member(named_field(
                "speed",
                3,
                "Speed",
                false,
                init_value(true, None),
            )),
            field_member(named_field(
                "override",
                4,
                "Speed",
                true,
                init_value(true, None),
            )),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl(
            "Label",
            primitive_type(
                v1::PrimitiveType::String,
                // A match-constrained string is not derivable.
                init_value(false, None),
                None,
            ),
        ),
        public_decl("DriverProfile", v1::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn enum_with_discriminants() {
    let decls = vec![public_decl(
        "GearPosition",
        v1::decl::Kind::EnumDef(v1::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 2),
                enum_value("NEUTRAL", 3),
            ],
            reserved: Vec::new(),
        }),
    )];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn enumset_standalone_form() {
    let decls = vec![public_decl(
        "WarningFlags",
        v1::decl::Kind::EnumSetDef(v1::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v1::IntWidth::U8 as i32,
        }),
    )];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn enumset_derived_form() {
    // The derived form carries a backing enum name; the checker copies the
    // backing enum's values into `bits`, so the emitted Rust is identical to
    // the standalone form.
    let decls = vec![
        public_decl(
            "Warning",
            v1::decl::Kind::EnumDef(v1::EnumDef {
                values: warning_bits(),
                reserved: Vec::new(),
            }),
        ),
        public_decl(
            "WarningFlags",
            v1::decl::Kind::EnumSetDef(v1::EnumSetDef {
                backing_enum: Some("Warning".to_string()),
                bits: warning_bits(),
                width: v1::IntWidth::U8 as i32,
            }),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn result_union() {
    let reading = v1::StructDef {
        members: vec![field_member(named_field(
            "value",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let fault = v1::StructDef {
        members: vec![field_member(named_field(
            "code",
            1,
            "Counter",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let union = v1::UnionDef {
        arms: vec![
            v1::UnionArm {
                name: "ok".to_string(),
                ordinal: 1,
                type_ref: "SensorReading".to_string(),
                doc: "Successful reading".to_string(),
            },
            v1::UnionArm {
                name: "err".to_string(),
                ordinal: 2,
                type_ref: "SensorFault".to_string(),
                doc: String::new(),
            },
        ],
        is_result: true,
        reserved: Vec::new(),
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("SensorReading", v1::decl::Kind::StructDef(reading)),
        v1::Decl {
            is_error: true,
            ..public_decl("SensorFault", v1::decl::Kind::StructDef(fault))
        },
        public_decl("SensorResult", v1::decl::Kind::UnionDef(union)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

fn tuple_field(name: &str, type_ref: &str) -> v1::TupleField {
    v1::TupleField {
        name: name.to_string(),
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Named(type_ref.to_string())),
        }),
    }
}

#[test]
fn tuple_field_generates_named_struct() {
    let range = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Tuple(v1::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let struct_def = v1::StructDef {
        members: vec![field_member(range)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("SensorBounds", v1::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

fn array_field(name: &str, element: &str, min: u64, max: u64) -> v1::Field {
    v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Array(Box::new(v1::ArrayType {
                element: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named(element.to_string())),
                })),
                min,
                max,
            }))),
        }),
        ..named_field(name, 1, "", false, init_value(true, None))
    }
}

#[test]
fn fixed_and_bounded_arrays() {
    let struct_def = v1::StructDef {
        members: vec![
            field_member(array_field("readings", "Speed", 8, 8)),
            field_member(v1::Field {
                ordinal: 2,
                ..array_field("history", "Speed", 2, 8)
            }),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("Samples", v1::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn bounded_map() {
    let map_field = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Map(Box::new(v1::MapType {
                key: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named("Counter".to_string())),
                })),
                value: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named("Speed".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 1, "", false, init_value(true, None))
    };
    let struct_def = v1::StructDef {
        members: vec![field_member(map_field)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Table", v1::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn deprecated_and_internal_visibility() {
    let decls = vec![
        v1::Decl {
            deprecated: Some("use Velocity".to_string()),
            ..speed_decl()
        },
        v1::Decl {
            deprecated: Some(String::new()),
            ..public_decl(
                "OldFlag",
                primitive_type(
                    v1::PrimitiveType::Boolean,
                    init_value(true, Some("false")),
                    None,
                ),
            )
        },
        v1::Decl {
            visibility: v1::Visibility::Internal as i32,
            ..public_decl(
                "RawTicks",
                primitive_type(
                    v1::PrimitiveType::Integer,
                    init_value(true, Some("0")),
                    Some(v1::type_def::Width::IntWidth(v1::IntWidth::U32 as i32)),
                ),
            )
        },
    ];
    insta::assert_snapshot!(rust_for(decls));
}

// ---------------------------------------------------------------------------
// Default derivation — the leaf-recursion rule.
// ---------------------------------------------------------------------------

#[test]
fn derivable_scalar_gets_default_with_derived_value() {
    // A numeric type whose range excludes 0 defaults to `min`, not 0.
    let ranged = v1::decl::Kind::TypeDef(v1::TypeDef {
        backing: Some(v1::Backing {
            kind: Some(v1::backing::Kind::Unit("Cel".to_string())),
        }),
        constraint: Some(constraint(Some("10.0"), Some("40.0"), Some("0.5"))),
        declared_init: None,
        init: Some(init_value(true, Some("10.0"))),
        width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
    });
    let source = rust_for(vec![public_decl("Warm", ranged)]);
    assert!(
        source.contains("impl Default for Warm"),
        "a derivable scalar must get a Default impl, got:\n{source}"
    );
    assert!(
        source.contains("Warm(10.0)"),
        "the derived init must be the range minimum 10.0, got:\n{source}"
    );
}

#[test]
fn leaf_recursion_denies_default_through_a_composite_field() {
    // The trap: `Outer { inner: Inner }` where `Inner` holds a non-derivable
    // leaf. T15 marks `Outer.inner.init.derivable == true` (a one-level flag),
    // yet neither `Inner` nor `Outer` is Default-constructible, so neither may
    // receive an `impl Default`.
    let inner = v1::StructDef {
        members: vec![field_member(named_field(
            "pattern",
            1,
            "Vin",
            false,
            // A match-constrained string leaf: not derivable.
            init_value(false, None),
        ))],
        fixed_layout: false,
    };
    let outer = v1::StructDef {
        members: vec![field_member(named_field(
            "inner",
            1,
            "Inner",
            false,
            // The one-level composite flag reads derivable despite the leaf.
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let decls = vec![
        public_decl(
            "Vin",
            primitive_type(v1::PrimitiveType::String, init_value(false, None), None),
        ),
        public_decl("Inner", v1::decl::Kind::StructDef(inner)),
        public_decl("Outer", v1::decl::Kind::StructDef(outer)),
    ];
    let source = rust_for(decls);
    assert!(
        !source.contains("impl Default for Inner"),
        "Inner has a non-derivable leaf; it must not get a Default, got:\n{source}"
    );
    assert!(
        !source.contains("impl Default for Outer"),
        "Outer transitively contains a non-derivable leaf; it must not get a Default despite the one-level flag, got:\n{source}"
    );
}

#[test]
fn enumset_default_is_the_empty_set() {
    let source = rust_for(vec![public_decl(
        "WarningFlags",
        v1::decl::Kind::EnumSetDef(v1::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v1::IntWidth::U8 as i32,
        }),
    )]);
    assert!(
        source.contains("WarningFlags(0)"),
        "the enumset default must be the empty-set sentinel 0, got:\n{source}"
    );
}

#[test]
fn enum_default_prefers_the_zero_discriminant() {
    // Values start at 5; none is 0, so the default is the lowest declared.
    let source = rust_for(vec![public_decl(
        "Mode",
        v1::decl::Kind::EnumDef(v1::EnumDef {
            values: vec![enum_value("SLOW", 5), enum_value("FAST", 9)],
            reserved: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("Mode::SLOW"),
        "with no zero value the default is the lowest discriminant, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Appendix B — the full typl example.
// ---------------------------------------------------------------------------

/// The typl reference Appendix B package `veh.common`, built as IR v1 with
/// populated init values (T15). Cross-package references to `ridl.std` are
/// fully qualified, as the resolver leaves them (typl §3.2).
#[allow(clippy::vec_init_then_push)]
fn appendix_b() -> v1::Package {
    let mut decls = Vec::new();

    // ---- Units and scalars ----
    decls.push(v1::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    });
    decls.push(v1::Decl {
        doc: "Coolant / ambient temperature".to_string(),
        ..public_decl(
            "Temperature",
            unit_type("Cel", "-40.0", "125.0", "0.1", "0.0"),
        )
    });
    decls.push(v1::Decl {
        doc: "Engine crankshaft speed".to_string(),
        ..public_decl("RPM", unit_type("/min", "0.0", "8000.0", "10.0", "0.0"))
    });
    decls.push(v1::Decl {
        doc: "Normalised ratio".to_string(),
        ..public_decl("Ratio", unit_type("%", "0.0", "100.0", "0.1", "0.0"))
    });
    decls.push(counter_decl());
    decls.push(public_decl(
        "Gain",
        v1::decl::Kind::TypeDef(v1::TypeDef {
            backing: Some(v1::Backing {
                kind: Some(v1::backing::Kind::Primitive(
                    v1::PrimitiveType::Float as i32,
                )),
            }),
            constraint: Some(constraint(Some("0.0"), Some("1.0"), Some("0.01"))),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
        }),
    ));

    // ---- Constants ----
    for (name, type_ref, value) in [
        ("MAX_SPEED", "Speed", "250.0"),
        ("SPEED_LIMIT_EU", "Speed", "130.0"),
        ("IDLE_RPM", "RPM", "800.0"),
    ] {
        decls.push(public_decl(
            name,
            v1::decl::Kind::ConstDef(v1::ConstDef {
                type_ref: Some(type_ref.to_string()),
                value: value.to_string(),
                regex: None,
            }),
        ));
    }
    decls.push(public_decl(
        "MAX_GEAR",
        v1::decl::Kind::ConstDef(v1::ConstDef {
            type_ref: Some("integer".to_string()),
            value: "6".to_string(),
            regex: None,
        }),
    ));

    // ---- Enums ----
    decls.push(public_decl(
        "GearPosition",
        v1::decl::Kind::EnumDef(v1::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 2),
                enum_value("NEUTRAL", 3),
            ],
            reserved: Vec::new(),
        }),
    ));
    decls.push(public_decl(
        "Warning",
        v1::decl::Kind::EnumDef(v1::EnumDef {
            values: warning_bits(),
            reserved: Vec::new(),
        }),
    ));
    decls.push(public_decl(
        "WarningFlags",
        v1::decl::Kind::EnumSetDef(v1::EnumSetDef {
            backing_enum: Some("Warning".to_string()),
            bits: warning_bits(),
            width: v1::IntWidth::U8 as i32,
        }),
    ));

    // ---- Composites ----
    decls.push(public_decl(
        "SpeedLimitPayload",
        v1::decl::Kind::StructDef(v1::StructDef {
            members: vec![
                field_member(named_field(
                    "limit",
                    1,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                field_member(named_field(
                    "actual",
                    2,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: true,
        }),
    ));

    // gears : integer [0..MAX_GEAR] = 6 (inline scalar with declared init)
    let gears = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::InlineScalar(Box::new(v1::TypeDef {
                backing: Some(v1::Backing {
                    kind: Some(v1::backing::Kind::Primitive(
                        v1::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: Some(constraint(Some("0"), Some("6"), None)),
                declared_init: None,
                init: None,
                width: Some(v1::type_def::Width::IntWidth(v1::IntWidth::U8 as i32)),
            }))),
        }),
        declared_init: Some("6".to_string()),
        ..named_field("gears", 4, "", false, init_value(true, Some("6")))
    };
    decls.push(public_decl(
        "DriverProfile",
        v1::decl::Kind::StructDef(v1::StructDef {
            members: vec![
                // name : Name — ridl.std, string+match, not derivable.
                field_member(named_field(
                    "name",
                    1,
                    "ridl.std.Name",
                    false,
                    init_value(false, None),
                )),
                field_member(named_field(
                    "speed",
                    2,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                field_member(named_field(
                    "override",
                    3,
                    "Speed",
                    true,
                    init_value(true, None),
                )),
                field_member(gears),
            ],
            fixed_layout: false,
        }),
    ));

    // SensorBounds
    let range = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Tuple(v1::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let readings = v1::Field {
        ordinal: 2,
        ..array_field("readings", "Speed", 8, 8)
    };
    // labels : [Label; 1..16] — Label is ridl.std, not derivable; min 1 makes
    // the field non-derivable.
    let labels = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Array(Box::new(v1::ArrayType {
                element: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                min: 1,
                max: 16,
            }))),
        }),
        ..named_field("labels", 3, "", false, init_value(false, None))
    };
    let meta = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::Map(Box::new(v1::MapType {
                key: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                value: Some(Box::new(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named("ridl.std.Name".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 4, "", false, init_value(true, None))
    };
    decls.push(public_decl(
        "SensorBounds",
        v1::decl::Kind::StructDef(v1::StructDef {
            members: vec![
                field_member(range),
                field_member(readings),
                field_member(labels),
                field_member(meta),
            ],
            fixed_layout: false,
        }),
    ));

    decls.push(public_decl(
        "SensorResult",
        v1::decl::Kind::UnionDef(v1::UnionDef {
            arms: vec![
                v1::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "SensorReading".to_string(),
                    doc: String::new(),
                },
                v1::UnionArm {
                    name: "err".to_string(),
                    ordinal: 2,
                    type_ref: "SensorFault".to_string(),
                    doc: String::new(),
                },
            ],
            is_result: true,
            reserved: Vec::new(),
        }),
    ));

    decls.push(public_decl(
        "SensorReading",
        v1::decl::Kind::StructDef(v1::StructDef {
            members: vec![
                field_member(named_field(
                    "value",
                    1,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                // timestamp : Timestamp — ridl.std integer, derivable.
                field_member(named_field(
                    "timestamp",
                    2,
                    "ridl.std.Timestamp",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: true,
        }),
    ));

    decls.push(v1::Decl {
        is_error: true,
        ..public_decl(
            "SensorFault",
            v1::decl::Kind::StructDef(v1::StructDef {
                members: vec![
                    field_member(named_field(
                        "code",
                        1,
                        "Counter",
                        false,
                        init_value(true, None),
                    )),
                    field_member(named_field(
                        "message",
                        2,
                        "ridl.std.Message",
                        false,
                        init_value(false, None),
                    )),
                ],
                fixed_layout: false,
            }),
        )
    });

    // internal struct RawWheelFrame { ticks : Counter, frame : bytes [8] }
    let frame = v1::Field {
        r#type: Some(v1::FieldType {
            optional: false,
            kind: Some(v1::field_type::Kind::InlineScalar(Box::new(v1::TypeDef {
                backing: Some(v1::Backing {
                    kind: Some(v1::backing::Kind::Primitive(
                        v1::PrimitiveType::Bytes as i32,
                    )),
                }),
                constraint: Some(v1::Constraint {
                    len_min: Some(8),
                    len_max: Some(8),
                    ..constraint(None, None, None)
                }),
                declared_init: None,
                init: None,
                width: None,
            }))),
        }),
        ..named_field("frame", 2, "", false, init_value(false, None))
    };
    decls.push(v1::Decl {
        visibility: v1::Visibility::Internal as i32,
        ..public_decl(
            "RawWheelFrame",
            v1::decl::Kind::StructDef(v1::StructDef {
                members: vec![
                    field_member(named_field(
                        "ticks",
                        1,
                        "Counter",
                        false,
                        init_value(true, None),
                    )),
                    field_member(frame),
                ],
                fixed_layout: false,
            }),
        )
    });

    package("veh.common", decls)
}

#[test]
fn appendix_b_rust_snapshot() {
    let Generated { rust_source, .. } = generate(&appendix_b()).expect("Appendix B generates");
    insta::assert_snapshot!(rust_source);
}

#[test]
fn appendix_b_c_header_snapshot() {
    let Generated { c_header, .. } = generate(&appendix_b()).expect("Appendix B generates");
    insta::assert_snapshot!(c_header);
}

/// The generated Rust for the full Appendix B package compiles with `rustc`.
/// A minimal prelude stands in for the `ridl.std` types the package imports,
/// declared in the module path the cross-package references map to. The temp
/// directory is removed on drop, closing the #102 temp-file leak.
#[test]
fn appendix_b_compiles_with_rustc() {
    let Generated { rust_source, .. } = generate(&appendix_b()).expect("Appendix B generates");

    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        pub struct Name(pub String);
        pub struct Message(pub String);
        pub struct Label(pub String);
        pub struct Timestamp(pub i64);
        impl Default for Timestamp {
            fn default() -> Self {
                Timestamp(0)
            }
        }
    }
}
";
    let source = format!("{PRELUDE}\n{rust_source}");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("appendix_b.rs");
    let meta_path = dir.path().join("appendix_b.rmeta");
    std::fs::write(&source_path, &source).expect("the generated source is written");

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    assert!(
        status.success(),
        "generated Rust for Appendix B must compile, source:\n{source}"
    );
}

/// A package whose collection fields are Default-constructible, so the array
/// and map default forms (`core::array::from_fn`, range-map collect) are
/// emitted and compiled.
#[test]
fn constructible_collections_compile() {
    let bag = v1::StructDef {
        members: vec![
            field_member(array_field("fixed", "Speed", 3, 3)),
            field_member(v1::Field {
                ordinal: 2,
                ..array_field("bounded", "Speed", 2, 5)
            }),
            field_member(v1::Field {
                r#type: Some(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Map(Box::new(v1::MapType {
                        key: Some(Box::new(v1::FieldType {
                            optional: false,
                            kind: Some(v1::field_type::Kind::Named("Counter".to_string())),
                        })),
                        value: Some(Box::new(v1::FieldType {
                            optional: false,
                            kind: Some(v1::field_type::Kind::Named("Speed".to_string())),
                        })),
                        min: 1,
                        max: 4,
                    }))),
                }),
                ..named_field("pairs", 3, "", false, init_value(true, None))
            }),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Bag", v1::decl::Kind::StructDef(bag)),
    ];
    let Generated { rust_source, .. } = generate(&package("veh.common", decls)).expect("generates");

    assert!(
        rust_source.contains("impl Default for Bag"),
        "Bag with derivable collection fields must get a Default, got:\n{rust_source}"
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("bag.rs");
    let meta_path = dir.path().join("bag.rmeta");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg(&source_path)
        .status()
        .expect("rustc runs");
    assert!(
        status.success(),
        "the collection default forms must compile, source:\n{rust_source}"
    );
}

#[test]
fn keyword_field_name_is_raw_escaped() {
    // `override` is a reserved Rust keyword; it must be emitted as a raw
    // identifier rather than panicking the emitter.
    let struct_def = v1::StructDef {
        members: vec![field_member(named_field(
            "override",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let source = rust_for(vec![
        speed_decl(),
        public_decl("Config", v1::decl::Kind::StructDef(struct_def)),
    ]);
    assert!(
        source.contains("r#override"),
        "a keyword field name must be raw-escaped, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Epic E1 whole-epic review — regression fixtures.
// ---------------------------------------------------------------------------

/// `type RawFrame : bytes [8]` — a bytes-backed named scalar type with a fixed
/// length. It has no fixed C ABI: the C header refuses its typedef and the
/// Rust backend realizes it as `Vec<u8>`.
fn bytes_frame_decl() -> v1::Decl {
    public_decl(
        "RawFrame",
        v1::decl::Kind::TypeDef(v1::TypeDef {
            backing: Some(v1::Backing {
                kind: Some(v1::backing::Kind::Primitive(
                    v1::PrimitiveType::Bytes as i32,
                )),
            }),
            constraint: Some(v1::Constraint {
                len_min: Some(8),
                len_max: Some(8),
                ..constraint(None, None, None)
            }),
            declared_init: None,
            init: Some(init_value(true, Some(""))),
            width: None,
        }),
    )
}

/// C1b: a non-optional self-reference `struct S { next: S }` is a cycle. The
/// checker rejects it (TYPL-206), but the backend must not trust that gate:
/// the Default recursion must terminate rather than overflow the stack.
#[test]
fn recursive_struct_default_terminates() {
    let recursive = v1::StructDef {
        members: vec![field_member(named_field(
            "next",
            1,
            "S",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let generated = generate(&package(
        "veh.common",
        vec![public_decl("S", v1::decl::Kind::StructDef(recursive))],
    ))
    .expect("a cyclic struct's Default derivation must terminate, not overflow");
    assert!(
        !generated.rust_source.contains("impl Default for S"),
        "a cyclic struct must get no Default, got:\n{}",
        generated.rust_source
    );
}

/// I1: `type Plate : string [8..9 match P] = "AA-000-AA"` — the declared init
/// makes the type derivable and IS its default, so the backend emits the
/// declared value, never an empty string.
#[test]
fn declared_string_init_becomes_the_default() {
    let plate = v1::decl::Kind::TypeDef(v1::TypeDef {
        backing: Some(v1::Backing {
            kind: Some(v1::backing::Kind::Primitive(
                v1::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v1::Constraint {
            len_min: Some(8),
            len_max: Some(9),
            ..constraint(None, None, None)
        }),
        declared_init: Some("AA-000-AA".to_string()),
        init: Some(init_value(true, Some("AA-000-AA"))),
        width: None,
    });
    let source = rust_for(vec![public_decl("Plate", plate)]);
    assert!(
        source.contains("impl Default for Plate"),
        "a declared-init string type gets a Default, got:\n{source}"
    );
    assert!(
        source.contains("\"AA-000-AA\"") && source.contains("to_string"),
        "the Default must be the declared init, not an empty string, got:\n{source}"
    );
    assert!(
        !source.contains("String::new()"),
        "the empty-string form is only for the derived case, got:\n{source}"
    );
}

/// I2: a cross-package field with a declared init cannot be faithfully wrapped
/// without the remote backing; emitting the referenced type's own default would
/// be a wrong value, so the whole struct gets no Default.
#[test]
fn cross_package_declared_init_omits_the_default() {
    let field = v1::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "veh.other.GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v1::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    let source = rust_for(vec![public_decl(
        "Selection",
        v1::decl::Kind::StructDef(struct_def),
    )]);
    assert!(
        !source.contains("impl Default for Selection"),
        "a struct with a cross-package declared-init field must get no Default, got:\n{source}"
    );
}

/// I2: the same-package equivalent CAN be wrapped: the backend knows
/// `GearIndex`'s integer backing, so the declared init 1 wraps to `GearIndex(1)`.
#[test]
fn same_package_declared_init_gets_the_correct_default() {
    let gear_index = public_decl(
        "GearIndex",
        primitive_type(
            v1::PrimitiveType::Integer,
            init_value(true, Some("0")),
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::U8 as i32)),
        ),
    );
    let field = v1::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v1::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    let source = rust_for(vec![
        gear_index,
        public_decl("Selection", v1::decl::Kind::StructDef(struct_def)),
    ]);
    assert!(
        source.contains("impl Default for Selection"),
        "the same-package equivalent gets a Default, got:\n{source}"
    );
    assert!(
        source.contains("GearIndex(1)"),
        "the declared init 1 must wrap to GearIndex(1), got:\n{source}"
    );
}

/// M1: the IR stores a regex constant's source with its `/…/` delimiters; the
/// emitted `&str` must hold the pattern only.
#[test]
fn regex_const_strips_its_delimiters() {
    let source = rust_for(vec![public_decl(
        "PLATE_PATTERN",
        v1::decl::Kind::ConstDef(v1::ConstDef {
            type_ref: None,
            value: String::new(),
            regex: Some("/^[A-Z]{2}-[0-9]{3}$/".to_string()),
        }),
    )]);
    assert!(
        source.contains("PLATE_PATTERN: &str = \"^[A-Z]{2}-[0-9]{3}$\""),
        "the regex const must emit its pattern without delimiters, got:\n{source}"
    );
    assert!(
        !source.contains("/^[A-Z]"),
        "the surrounding slash delimiters must be stripped, got:\n{source}"
    );
}

/// C2: `c_field_type` never emits a dangling type name. A `fixed_layout` struct
/// whose field references a bytes-backed named type (a defensive case the
/// checker's C2 rule normally prevents) is downgraded to the not-representable
/// block rather than emitted as a repr(C) struct naming an undeclared typedef.
#[test]
fn c_field_type_guards_a_dangling_named_reference() {
    let frame = v1::StructDef {
        members: vec![field_member(named_field(
            "data",
            1,
            "RawFrame",
            false,
            init_value(false, None),
        ))],
        // Set true to exercise the guard directly, simulating a malformed IR
        // the checker's C2 rule would not produce.
        fixed_layout: true,
    };
    let Generated { c_header, .. } = generate(&package(
        "veh.common",
        vec![
            bytes_frame_decl(),
            public_decl("Frame", v1::decl::Kind::StructDef(frame)),
        ],
    ))
    .expect("generation succeeds");
    assert!(
        !c_header.contains("veh_common_raw_frame data"),
        "the guard must keep the undeclared bytes typedef out of the struct, got:\n{c_header}"
    );
    assert!(
        c_header.contains("struct Frame"),
        "Frame must fall into the not-representable block, got:\n{c_header}"
    );
}

/// C2: the generated C header for a package whose struct holds a bytes-backed
/// named field compiles under `cc -std=c11 -fsyntax-only`. The bytes-backed type
/// and the struct that holds it have no fixed C ABI, so they land in the
/// not-representable comment block; an all-scalar struct is still a repr(C)
/// struct. Skipped when `cc` is not installed.
#[test]
fn bytes_named_field_c_header_compiles_with_cc() {
    let reading = v1::StructDef {
        members: vec![field_member(named_field(
            "value",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    // A bytes-backed named field makes the struct non-fixed (the checker's C2
    // rule); it is not emitted as a repr(C) struct.
    let frame = v1::StructDef {
        members: vec![field_member(named_field(
            "data",
            1,
            "RawFrame",
            false,
            init_value(false, None),
        ))],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        bytes_frame_decl(),
        public_decl("Reading", v1::decl::Kind::StructDef(reading)),
        public_decl("Frame", v1::decl::Kind::StructDef(frame)),
    ];
    let Generated { c_header, .. } =
        generate(&package("veh.common", decls)).expect("generation succeeds");

    assert!(
        c_header.contains("} veh_common_reading;"),
        "the all-scalar struct is emitted as a repr(C) struct, got:\n{c_header}"
    );
    assert!(
        c_header.contains("not representable in C ABI"),
        "the bytes-backed shapes are listed as not representable, got:\n{c_header}"
    );
    assert!(
        !c_header.contains("veh_common_raw_frame data"),
        "the C header must not reference the undeclared bytes typedef, got:\n{c_header}"
    );

    // The header must parse as C11. A missing C compiler skips the check.
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let header_path = dir.path().join("veh_common.h");
    let unit_path = dir.path().join("veh_common.c");
    std::fs::write(&header_path, &c_header).expect("the header is written");
    std::fs::write(
        &unit_path,
        format!("#include \"{}\"\n", header_path.display()),
    )
    .expect("the translation unit is written");
    match std::process::Command::new("cc")
        .args(["-std=c11", "-fsyntax-only"])
        .arg(&unit_path)
        .status()
    {
        Ok(status) => assert!(
            status.success(),
            "the generated C header must pass cc -std=c11 -fsyntax-only:\n{c_header}"
        ),
        Err(_) => eprintln!("skipping cc syntax check: cc is not available"),
    }
}
