//! Backend tests: per-construct snapshots, init-function derivation (the
//! leaf-recursion rule), totality (`GenerateError`, never a panic), the
//! Appendix B example, and the best-effort tsc strict-compile check.

use super::{GenerateError, generate};
use ridl_ir::v2;

// ---------------------------------------------------------------------------
// Fixture builders (mirroring crates/ridl-backend-rust/src/tests.rs, lifted to v2 —
// package-level declarations carry ordinal 0).
// ---------------------------------------------------------------------------

fn public_decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal: 0,
        kind: Some(kind),
    }
}

fn package(name: &str, decls: Vec<v2::Decl>) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        decls,
        interfaces: Vec::new(),
        services: Vec::new(),
    }
}

fn init_value(derivable: bool, value: Option<&str>) -> v2::InitValue {
    v2::InitValue {
        derivable,
        value: value.map(str::to_string),
    }
}

fn constraint(min: Option<&str>, max: Option<&str>, step: Option<&str>) -> v2::Constraint {
    v2::Constraint {
        min: min.map(str::to_string),
        max: max.map(str::to_string),
        step: step.map(str::to_string),
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    }
}

fn unit_type(unit: &str, min: &str, max: &str, step: &str, init: &str) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit(unit.to_string())),
        }),
        constraint: Some(constraint(Some(min), Some(max), Some(step))),
        declared_init: None,
        init: Some(init_value(true, Some(init))),
        width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
    })
}

fn primitive_type(
    prim: v2::PrimitiveType,
    init: v2::InitValue,
    width: Option<v2::type_def::Width>,
) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(prim as i32)),
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
    init: v2::InitValue,
) -> v2::Field {
    v2::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(v2::FieldType {
            optional,
            kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
        }),
        declared_init: None,
        init: Some(init),
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    }
}

fn field_member(field: v2::Field) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(field)),
    }
}

fn reserved_member(ordinal: u32, name: &str) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        })),
    }
}

fn enum_value(name: &str, value: i64) -> v2::EnumValue {
    v2::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
    }
}

fn warning_bits() -> Vec<v2::EnumValue> {
    vec![
        enum_value("LOW_FUEL", 0),
        enum_value("CHECK_ENGINE", 1),
        enum_value("DOOR_OPEN", 2),
        enum_value("SEATBELT", 3),
    ]
}

/// A `Speed`-like unit type declaration used across the small fixtures.
fn speed_decl() -> v2::Decl {
    v2::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    }
}

fn counter_decl() -> v2::Decl {
    public_decl(
        "Counter",
        primitive_type(
            v2::PrimitiveType::Integer,
            init_value(true, Some("0")),
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
        ),
    )
}

/// `type Odometer : integer [0..2^63-1]` — a U64-width named scalar, the
/// bigint-branding case (exactness beyond 2^53, task 13 mapping).
fn odometer_decl() -> v2::Decl {
    public_decl(
        "Odometer",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            constraint: Some(constraint(Some("0"), Some("18446744073709551615"), None)),
            declared_init: None,
            init: Some(init_value(true, Some("0"))),
            width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U64 as i32)),
        }),
    )
}

fn ts_for(decls: Vec<v2::Decl>) -> String {
    generate(&package("veh.common", decls))
        .expect("generation succeeds")
        .source
}

// ---------------------------------------------------------------------------
// Per-construct snapshots.
// ---------------------------------------------------------------------------

#[test]
fn scalar_unit_type_with_doc() {
    insta::assert_snapshot!(ts_for(vec![speed_decl()]));
}

#[test]
fn named_scalar_backings() {
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl(
            "Enabled",
            primitive_type(
                v2::PrimitiveType::Boolean,
                init_value(true, Some("false")),
                None,
            ),
        ),
        public_decl(
            "Label",
            primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "Blob",
            primitive_type(v2::PrimitiveType::Bytes, init_value(true, Some("")), None),
        ),
    ];
    insta::assert_snapshot!(ts_for(decls));
}

#[test]
fn u64_width_brands_bigint() {
    let source = ts_for(vec![odometer_decl()]);
    assert!(
        source.contains("bigint & { readonly __ridl: 'veh.common.Odometer' }"),
        "a U64-width scalar must brand bigint, not number, got:\n{source}"
    );
    assert!(
        source.contains("0n as Odometer"),
        "the bigint init must use an n-suffixed literal, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn constants_all_forms() {
    let decls = vec![
        speed_decl(),
        public_decl(
            "MAX_SPEED",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Speed".to_string()),
                value: "250.0".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "MAX_GEAR",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("integer".to_string()),
                value: "6".to_string(),
                regex: None,
            }),
        ),
        odometer_decl(),
        // A constant of a bigint-branded type must be an n-suffixed literal.
        public_decl(
            "MAX_ODOMETER",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Odometer".to_string()),
                value: "18446744073709551615".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "Greeting",
            primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "BANNER",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Greeting".to_string()),
                value: "hello".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "VIN_PATTERN",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: None,
                value: String::new(),
                regex: Some("/^[A-HJ-NPR-Z0-9]{17}$/".to_string()),
            }),
        ),
    ];
    insta::assert_snapshot!(ts_for(decls));
}

#[test]
fn struct_with_optional_and_reserved() {
    // A struct that mixes a normal field, a reserved tombstone (occupies an
    // ordinal, emits no property), and an optional field.
    let struct_def = v2::StructDef {
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
                v2::PrimitiveType::String,
                // A match-constrained string is not derivable.
                init_value(false, None),
                None,
            ),
        ),
        public_decl("DriverProfile", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        !source.contains("legacyChecksum"),
        "a reserved tombstone must emit no property, got:\n{source}"
    );
    assert!(
        source.contains("override?: Speed;"),
        "an optional field must use the ?: form, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn enum_with_discriminants() {
    let decls = vec![public_decl(
        "GearPosition",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 2),
                enum_value("NEUTRAL", 3),
            ],
            reserved: Vec::new(),
        }),
    )];
    insta::assert_snapshot!(ts_for(decls));
}

#[test]
fn enumset_standalone_form() {
    let decls = vec![public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    )];
    insta::assert_snapshot!(ts_for(decls));
}

#[test]
fn enumset_derived_form() {
    // The derived form carries a backing enum name; the checker copies the
    // backing enum's values into `bits`, so the emitted TypeScript for the
    // enumset itself is identical to the standalone form.
    let decls = vec![
        public_decl(
            "Warning",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values: warning_bits(),
                reserved: Vec::new(),
            }),
        ),
        public_decl(
            "WarningFlags",
            v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                backing_enum: Some("Warning".to_string()),
                bits: warning_bits(),
                width: v2::IntWidth::U8 as i32,
            }),
        ),
    ];
    insta::assert_snapshot!(ts_for(decls));
}

#[test]
fn enumset_u64_width_brands_bigint() {
    // A 64-bit enumset cannot ride JS number bitwise operators (they truncate
    // to 32 bits), so U64/I64 widths brand bigint and the bit constants are
    // n-suffixed.
    let decls = vec![public_decl(
        "WideFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: vec![enum_value("LOW", 0), enum_value("TOP", 63)],
            width: v2::IntWidth::U64 as i32,
        }),
    )];
    let source = ts_for(decls);
    assert!(
        source.contains("bigint & { readonly __ridl: 'veh.common.WideFlags' }"),
        "a U64-width enumset must brand bigint, got:\n{source}"
    );
    assert!(
        source.contains("TOP: 9223372036854775808n"),
        "bit 63 must be an exact n-suffixed constant, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn result_union() {
    let reading = v2::StructDef {
        members: vec![field_member(named_field(
            "value",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let fault = v2::StructDef {
        members: vec![field_member(named_field(
            "code",
            1,
            "Counter",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let union = v2::UnionDef {
        arms: vec![
            v2::UnionArm {
                name: "ok".to_string(),
                ordinal: 1,
                type_ref: "SensorReading".to_string(),
                doc: "Successful reading".to_string(),
            },
            v2::UnionArm {
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
        public_decl("SensorReading", v2::decl::Kind::StructDef(reading)),
        v2::Decl {
            is_error: true,
            ..public_decl("SensorFault", v2::decl::Kind::StructDef(fault))
        },
        public_decl("SensorResult", v2::decl::Kind::UnionDef(union)),
    ];
    insta::assert_snapshot!(ts_for(decls));
}

fn tuple_field(name: &str, type_ref: &str) -> v2::TupleField {
    v2::TupleField {
        name: name.to_string(),
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
        }),
    }
}

#[test]
fn tuple_field_becomes_inline_object_type() {
    let range = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(range)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("SensorBounds", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains("range: { min: Speed; max: Speed };"),
        "a tuple field must become an inline object type, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

fn array_field(name: &str, element: &str, min: u64, max: u64) -> v2::Field {
    v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named(element.to_string())),
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
    let struct_def = v2::StructDef {
        members: vec![
            field_member(array_field("readings", "Speed", 8, 8)),
            field_member(v2::Field {
                ordinal: 2,
                ..array_field("history", "Speed", 2, 8)
            }),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("Samples", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains("readonly Speed[]"),
        "arrays must map to readonly T[], got:\n{source}"
    );
    assert!(
        source.contains("@bounds 8") && source.contains("@bounds 2..8"),
        "array bounds must ride a JSDoc @bounds tag, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// An array whose element is a tuple is the one shape that puts an object
/// literal in the concise arrow body of the generated `Array.from` init. A
/// concise arrow body is parsed as a **block**, so the literal has to be
/// parenthesised: unparenthesised, the two-field spelling below is a syntax
/// error, and a one-field tuple parses as a labelled statement whose callback
/// returns `undefined` (issue #177).
///
/// The direct tuple field beside it is the control — it is emitted as a plain
/// object literal, was correct throughout, and must stay unparenthesised.
#[test]
fn array_of_tuple_init_parenthesizes_the_arrow_body() {
    let tuple_element = || v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
        })),
    };
    let range = v2::Field {
        r#type: Some(tuple_element()),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let spans = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(tuple_element())),
                min: 2,
                max: 2,
            }))),
        }),
        ..named_field("spans", 2, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(range), field_member(spans)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("SpanTable", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains(
            "spans: Array.from({ length: 2 }, () => ({ min: initSpeed(), max: initSpeed() })),"
        ),
        "an array-of-tuple init must parenthesise its arrow body, got:\n{source}"
    );
    assert!(
        source.contains("range: { min: initSpeed(), max: initSpeed() },"),
        "a tuple in direct field position must stay an unparenthesised object \
         literal, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn bounded_map() {
    let map_field = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Counter".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Speed".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 1, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(map_field)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Table", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains("ReadonlyArray<readonly [Counter, Speed]>"),
        "maps must be deterministic readonly entry lists, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// The map init is the other site that emits a concise arrow body, and its
/// body is an array literal — `[k, v] as const` — so the parenthesisation rule
/// of [`array_of_tuple_init_parenthesizes_the_arrow_body`] must leave it
/// alone. Asserted rather than assumed: no snapshot covered a map with a
/// non-zero minimum, so nothing else pins this form.
#[test]
fn a_map_init_with_a_non_zero_minimum_keeps_its_array_literal_arrow_body() {
    let map_field = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Counter".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Speed".to_string())),
                })),
                min: 3,
                max: 32,
            }))),
        }),
        ..named_field("meta", 1, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(map_field)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Table", v2::decl::Kind::StructDef(struct_def)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains(
            "meta: Array.from({ length: 3 }, () => [initCounter(), initSpeed()] as const),"
        ),
        "a map init must build its entries as `[k, v] as const`, got:\n{source}"
    );
}

#[test]
fn deprecated_and_internal_visibility() {
    let decls = vec![
        v2::Decl {
            deprecated: Some("use Velocity".to_string()),
            ..speed_decl()
        },
        v2::Decl {
            deprecated: Some(String::new()),
            ..public_decl(
                "OldFlag",
                primitive_type(
                    v2::PrimitiveType::Boolean,
                    init_value(true, Some("false")),
                    None,
                ),
            )
        },
        v2::Decl {
            visibility: v2::Visibility::Internal as i32,
            ..public_decl(
                "RawTicks",
                primitive_type(
                    v2::PrimitiveType::Integer,
                    init_value(true, Some("0")),
                    Some(v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32)),
                ),
            )
        },
    ];
    let source = ts_for(decls);
    assert!(
        source.contains("@deprecated use Velocity"),
        "a deprecation reason must ride the JSDoc tag, got:\n{source}"
    );
    assert!(
        !source.contains("export type RawTicks"),
        "an internal declaration must stay module-local, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

// ---------------------------------------------------------------------------
// Init-function derivation — the leaf-recursion rule.
// ---------------------------------------------------------------------------

#[test]
fn init_functions_derive_recursively() {
    // Outer { inner: Inner { speed: Speed } } — the derivation must recurse
    // through the same-package composite and call the inner init function.
    let inner = v2::StructDef {
        members: vec![field_member(named_field(
            "speed",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let outer = v2::StructDef {
        members: vec![field_member(named_field(
            "inner",
            1,
            "Inner",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("Inner", v2::decl::Kind::StructDef(inner)),
        public_decl("Outer", v2::decl::Kind::StructDef(outer)),
    ];
    let source = ts_for(decls);
    assert!(
        source.contains("export function initInner(): Inner"),
        "a derivable struct must get an init function, got:\n{source}"
    );
    assert!(
        source.contains("inner: initInner()"),
        "a composite field init must call the member's init function, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn leaf_recursion_denies_init_through_a_composite_field() {
    // The trap: `Outer { inner: Inner }` where `Inner` holds a non-derivable
    // leaf. T15 marks `Outer.inner.init.derivable == true` (a one-level flag),
    // yet neither `Inner` nor `Outer` is init-constructible, so neither may
    // receive an init function.
    let inner = v2::StructDef {
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
    let outer = v2::StructDef {
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
            primitive_type(v2::PrimitiveType::String, init_value(false, None), None),
        ),
        public_decl("Inner", v2::decl::Kind::StructDef(inner)),
        public_decl("Outer", v2::decl::Kind::StructDef(outer)),
    ];
    let source = ts_for(decls);
    assert!(
        !source.contains("function initInner"),
        "Inner has a non-derivable leaf; it must not get an init function, got:\n{source}"
    );
    assert!(
        !source.contains("function initOuter"),
        "Outer transitively contains a non-derivable leaf; it must not get an init function despite the one-level flag, got:\n{source}"
    );
}

/// C1b defense in depth: a cyclic IR (`struct S { next: S }`) is TYPL-206
/// upstream, but the backend must terminate rather than overflow the stack.
#[test]
fn recursive_struct_init_terminates() {
    let recursive = v2::StructDef {
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
        vec![public_decl("S", v2::decl::Kind::StructDef(recursive))],
    ))
    .expect("a cyclic struct's init derivation must terminate, not overflow");
    assert!(
        !generated.source.contains("function initS"),
        "a cyclic struct must get no init function, got:\n{}",
        generated.source
    );
}

#[test]
fn enum_init_prefers_the_zero_discriminant() {
    // Values start at 5; none is 0, so the init is the lowest declared.
    let source = ts_for(vec![public_decl(
        "Mode",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![enum_value("SLOW", 5), enum_value("FAST", 9)],
            reserved: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("return Mode.SLOW;"),
        "with no zero value the init is the lowest discriminant, got:\n{source}"
    );
}

#[test]
fn enumset_init_is_the_empty_set() {
    let source = ts_for(vec![public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    )]);
    assert!(
        source.contains("return 0 as WarningFlags;"),
        "the enumset init must be the empty-set sentinel 0, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Totality — GenerateError, never a panic.
// ---------------------------------------------------------------------------

#[test]
fn enum_discriminant_beyond_2_53_is_unrepresentable() {
    // TypeScript enums are number-valued; a discriminant beyond
    // Number.MAX_SAFE_INTEGER has no exact representation.
    let result = generate(&package(
        "veh.common",
        vec![public_decl(
            "Huge",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![enum_value("TOO_BIG", 9_007_199_254_740_993)],
                reserved: Vec::new(),
            }),
        )],
    ));
    let Err(GenerateError::Unrepresentable(message)) = result else {
        panic!("an unsafe enum discriminant must be Unrepresentable, got {result:?}");
    };
    assert!(
        message.contains("Huge"),
        "the error must name the declaration, got: {message}"
    );
}

#[test]
fn enumset_bit_beyond_31_needs_a_64_bit_width() {
    // A number-branded enumset rides JS 32-bit bitwise operators; a bit
    // position past 31 under a narrow width is an IR inconsistency and must
    // surface as an error, not a silently wrong mask.
    let result = generate(&package(
        "veh.common",
        vec![public_decl(
            "BadFlags",
            v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                backing_enum: None,
                bits: vec![enum_value("WAY_UP", 40)],
                width: v2::IntWidth::U8 as i32,
            }),
        )],
    ));
    assert!(
        matches!(result, Err(GenerateError::Unrepresentable(_))),
        "a bit past 31 under a number brand must be Unrepresentable, got {result:?}"
    );
}

#[test]
fn narrow_width_const_beyond_2_53_is_unrepresentable() {
    // Defense in depth: a narrow (U32) width claiming a value past 2^53 is a
    // malformed IR — emitting it as a number literal would silently round,
    // and TypeScript would compile the wrong value without complaint.
    let result = generate(&package(
        "veh.common",
        vec![
            counter_decl(),
            public_decl(
                "BAD_COUNT",
                v2::decl::Kind::ConstDef(v2::ConstDef {
                    type_ref: Some("Counter".to_string()),
                    value: "9007199254740993".to_string(),
                    regex: None,
                }),
            ),
        ],
    ));
    let Err(GenerateError::Unrepresentable(message)) = result else {
        panic!("a narrow-width const past 2^53 must be Unrepresentable, got {result:?}");
    };
    assert!(
        message.contains("BAD_COUNT"),
        "the error must name the constant, got: {message}"
    );
}

#[test]
fn narrow_width_init_beyond_2_53_is_not_derivable() {
    // The same malformed shape in init position: the init function is
    // omitted rather than emitting a rounding literal; the type itself
    // still emits.
    let source = ts_for(vec![public_decl(
        "Sus",
        primitive_type(
            v2::PrimitiveType::Integer,
            init_value(true, Some("9007199254740993")),
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32)),
        ),
    )]);
    assert!(
        source.contains("export type Sus"),
        "the branded type must still emit, got:\n{source}"
    );
    assert!(
        !source.contains("function initSus"),
        "an init value past 2^53 under a narrow width must not derive an init function, got:\n{source}"
    );
}

#[test]
fn stream_field_in_typl_surface_is_unrepresentable() {
    // The stream container is an interaction shape (ridl §12); meeting one
    // in a struct field is an IR inconsistency, reported rather than
    // guessed around.
    let stream_field = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                element: Some(v2::stream_type::Element::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            })),
        }),
        ..named_field("tail", 1, "", false, init_value(false, None))
    };
    let result = generate(&package(
        "veh.common",
        vec![public_decl(
            "Bad",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(stream_field)],
                fixed_layout: false,
            }),
        )],
    ));
    assert!(
        matches!(result, Err(GenerateError::Unrepresentable(_))),
        "a stream in a typl-surface field must be Unrepresentable, got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Appendix B — the full typl example.
// ---------------------------------------------------------------------------

/// The typl reference Appendix B package `veh.common`, built as IR v2 with
/// populated init values. Cross-package references to `ridl.std` are fully
/// qualified, as the resolver leaves them (typl §3.2).
#[allow(clippy::vec_init_then_push)]
fn appendix_b() -> v2::Package {
    let mut decls = Vec::new();

    // ---- Units and scalars ----
    decls.push(v2::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    });
    decls.push(v2::Decl {
        doc: "Coolant / ambient temperature".to_string(),
        ..public_decl(
            "Temperature",
            unit_type("Cel", "-40.0", "125.0", "0.1", "0.0"),
        )
    });
    decls.push(v2::Decl {
        doc: "Engine crankshaft speed".to_string(),
        ..public_decl("RPM", unit_type("/min", "0.0", "8000.0", "10.0", "0.0"))
    });
    decls.push(v2::Decl {
        doc: "Normalised ratio".to_string(),
        ..public_decl("Ratio", unit_type("%", "0.0", "100.0", "0.1", "0.0"))
    });
    decls.push(counter_decl());
    decls.push(public_decl(
        "Gain",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Float as i32,
                )),
            }),
            constraint: Some(constraint(Some("0.0"), Some("1.0"), Some("0.01"))),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
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
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some(type_ref.to_string()),
                value: value.to_string(),
                regex: None,
            }),
        ));
    }
    decls.push(public_decl(
        "MAX_GEAR",
        v2::decl::Kind::ConstDef(v2::ConstDef {
            type_ref: Some("integer".to_string()),
            value: "6".to_string(),
            regex: None,
        }),
    ));

    // ---- Enums ----
    decls.push(public_decl(
        "GearPosition",
        v2::decl::Kind::EnumDef(v2::EnumDef {
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
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: warning_bits(),
            reserved: Vec::new(),
        }),
    ));
    decls.push(public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: Some("Warning".to_string()),
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    ));

    // ---- Composites ----
    decls.push(public_decl(
        "SpeedLimitPayload",
        v2::decl::Kind::StructDef(v2::StructDef {
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
    let gears = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: Some(constraint(Some("0"), Some("6"), None)),
                declared_init: None,
                init: None,
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            }))),
        }),
        declared_init: Some("6".to_string()),
        ..named_field("gears", 4, "", false, init_value(true, Some("6")))
    };
    decls.push(public_decl(
        "DriverProfile",
        v2::decl::Kind::StructDef(v2::StructDef {
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
    let range = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let readings = v2::Field {
        ordinal: 2,
        ..array_field("readings", "Speed", 8, 8)
    };
    // labels : [Label; 1..16] — Label is ridl.std, not derivable; min 1 makes
    // the field non-derivable.
    let labels = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                min: 1,
                max: 16,
            }))),
        }),
        ..named_field("labels", 3, "", false, init_value(false, None))
    };
    let meta = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Name".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 4, "", false, init_value(true, None))
    };
    decls.push(public_decl(
        "SensorBounds",
        v2::decl::Kind::StructDef(v2::StructDef {
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
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: vec![
                v2::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "SensorReading".to_string(),
                    doc: String::new(),
                },
                v2::UnionArm {
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
        v2::decl::Kind::StructDef(v2::StructDef {
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

    decls.push(v2::Decl {
        is_error: true,
        ..public_decl(
            "SensorFault",
            v2::decl::Kind::StructDef(v2::StructDef {
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
    let frame = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Bytes as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
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
    decls.push(v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..public_decl(
            "RawWheelFrame",
            v2::decl::Kind::StructDef(v2::StructDef {
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
fn appendix_b_ts_snapshot() {
    let generated = generate(&appendix_b()).expect("Appendix B generates");
    insta::assert_snapshot!(generated.source);
}

/// The generated TypeScript for the full Appendix B package compiles with
/// `tsc --noEmit --strict` when a tsc binary is discoverable; otherwise the
/// check is skipped with a printed notice — the snapshot tests are the gate,
/// this is best-effort local evidence (network-free, mirroring the E1
/// rustc-compile precedent).
#[test]
fn appendix_b_compiles_with_tsc_strict() {
    let Some(tsc) = discover_tsc() else {
        println!(
            "SKIPPED: no tsc binary discoverable (`tsc` on PATH or `npx --no-install tsc`); \
             the snapshot tests remain the gate"
        );
        return;
    };

    let generated = generate(&appendix_b()).expect("Appendix B generates");

    // A minimal module stands in for the `ridl.std` types the package
    // imports, matching the module specifier the cross-package references
    // map to.
    const PRELUDE: &str = "\
export type Name = string & { readonly __ridl: 'ridl.std.Name' };
export type Label = string & { readonly __ridl: 'ridl.std.Label' };
export type Message = string & { readonly __ridl: 'ridl.std.Message' };
export type Timestamp = bigint & { readonly __ridl: 'ridl.std.Timestamp' };
export function initTimestamp(): Timestamp {
  return 0n as Timestamp;
}
";

    let dir = tempfile::tempdir().expect("a temp dir is created");
    std::fs::write(dir.path().join("ridl.std.ts"), PRELUDE).expect("the prelude is written");
    let module_path = dir.path().join("veh.common.ts");
    std::fs::write(&module_path, &generated.source).expect("the generated source is written");

    let status = std::process::Command::new(&tsc.0)
        .args(&tsc.1)
        .args([
            "--noEmit", "--strict", "--target", "es2020", "--module", "commonjs",
        ])
        .arg(&module_path)
        .status()
        .expect("the discovered tsc must be runnable");
    assert!(
        status.success(),
        "generated TypeScript for Appendix B must compile strict, source:\n{}",
        generated.source
    );
}

/// Finds a runnable tsc: the `tsc` binary on PATH first, then
/// `npx --no-install tsc` (network-free). Returns the program and its
/// leading arguments, or `None` when neither responds to `--version`.
pub(crate) fn discover_tsc() -> Option<(String, Vec<String>)> {
    let candidates: [(&str, &[&str]); 2] = [("tsc", &[]), ("npx", &["--no-install", "tsc"])];
    for (program, prefix) in candidates {
        let probe = std::process::Command::new(program)
            .args(prefix)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if matches!(probe, Ok(status) if status.success()) {
            return Some((
                program.to_string(),
                prefix.iter().map(|s| s.to_string()).collect(),
            ));
        }
    }
    None
}
