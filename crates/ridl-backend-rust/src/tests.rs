//! Backend tests: per-construct snapshots, the Appendix B rustc-compile proof,
//! Default derivation behaviour (the leaf-recursion rule), and the C header
//! snapshot.

use super::{Generated, generate};
use ridl_ir::v2;

// ---------------------------------------------------------------------------
// Fixture builders.
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

// ---------------------------------------------------------------------------
// Per-construct snapshots.
// ---------------------------------------------------------------------------

fn rust_for(decls: Vec<v2::Decl>) -> String {
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
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
fn tuple_field_generates_named_struct() {
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
    insta::assert_snapshot!(rust_for(decls));
}

/// A struct field whose type is `kind`, at `ordinal`.
fn shaped_field(name: &str, ordinal: u32, kind: v2::field_type::Kind) -> v2::Field {
    v2::Field {
        ordinal,
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(kind),
        }),
        ..named_field(name, ordinal, "", false, init_value(true, None))
    }
}

/// A tuple type whose fields are the given `(name, type_ref)` pairs.
fn tuple_of(fields: &[(&str, &str)]) -> v2::field_type::Kind {
    v2::field_type::Kind::Tuple(v2::TupleType {
        fields: fields
            .iter()
            .map(|(name, type_ref)| tuple_field(name, type_ref))
            .collect(),
    })
}

/// A tuple under an `internal` declaration generates a **`pub(crate)`** struct,
/// in every typl position that can reach one, and the struct compiles under
/// `-D private-interfaces` (issue #167).
///
/// A tuple is anonymous in source, so the struct it generates has no visibility
/// of its own to declare; before this fix it was fixed at `pub`, which put a
/// `pub(crate)` payload type into a `pub` struct's field. `ridlc check` accepts
/// that source — an `internal` declaration may name `internal` declarations
/// freely (typl §3.3) — so the generated module was the only thing wrong with
/// it, and only rustc could say so.
///
/// **All five typl positions are exercised, not one.** Issue #161 was reported
/// as two positions and turned out to be twenty; a fix verified on the direct
/// field alone would leave the array element, the optional, the map value and
/// the nested tuple emitting `pub`, and every one of them reaches
/// `field_type_tokens` by its own branch. The nested tuple is the one that
/// cannot be found during the declaration walk at all: it is discovered only
/// while emitting the outer tuple's fields, from the flat worklist, which is
/// exactly where the inducing declaration is out of reach.
///
/// The `rustc` run is the proof rather than the string assertions: a snapshot
/// records what was emitted, not that it is valid Rust, and this defect
/// produced output that a plain `rustc` invocation accepts with a warning.
#[test]
fn a_tuple_under_an_internal_declaration_is_package_private() {
    let hidden = v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..public_decl(
            "Hidden",
            primitive_type(
                v2::PrimitiveType::Integer,
                init_value(true, Some("0")),
                Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
            ),
        )
    };
    let holder = v2::StructDef {
        members: vec![
            // 1. a direct tuple field
            field_member(shaped_field("direct", 1, tuple_of(&[("a", "Hidden")]))),
            // 2. a tuple as an array element
            field_member(shaped_field(
                "arr",
                2,
                v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(tuple_of(&[("b", "Hidden")])),
                    })),
                    min: 3,
                    max: 3,
                })),
            )),
            // 3. an optional tuple field
            field_member(v2::Field {
                r#type: Some(v2::FieldType {
                    optional: true,
                    kind: Some(tuple_of(&[("c", "Hidden")])),
                }),
                ..shaped_field("opt", 3, tuple_of(&[("c", "Hidden")]))
            }),
            // 4. a tuple as a map value
            field_member(shaped_field(
                "mp",
                4,
                v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    })),
                    value: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(tuple_of(&[("d", "Hidden")])),
                    })),
                    min: 0,
                    max: 4,
                })),
            )),
            // 5. a tuple nested inside a tuple — reached only by draining the
            //    worklist, after the declaration walk has finished.
            field_member(shaped_field(
                "nested",
                5,
                v2::field_type::Kind::Tuple(v2::TupleType {
                    fields: vec![v2::TupleField {
                        name: "e".to_string(),
                        r#type: Some(v2::FieldType {
                            optional: false,
                            kind: Some(tuple_of(&[("f", "Hidden")])),
                        }),
                    }],
                }),
            )),
        ],
        fixed_layout: false,
    };
    // The regression direction, in the same module: a public struct's tuple
    // stays `pub`. Its payload is the public `Counter`, because a public
    // declaration naming an `internal` one is TYPL-005 and never reaches a
    // backend.
    let shown = v2::StructDef {
        members: vec![field_member(shaped_field(
            "direct",
            1,
            tuple_of(&[("g", "Counter")]),
        ))],
        fixed_layout: false,
    };

    let rust_source = rust_for(vec![
        counter_decl(),
        hidden,
        v2::Decl {
            visibility: v2::Visibility::Internal as i32,
            ..public_decl("Holder", v2::decl::Kind::StructDef(holder))
        },
        public_decl("Shown", v2::decl::Kind::StructDef(shown)),
    ]);

    for item in [
        "pub(crate) struct HolderDirect",
        "pub(crate) struct HolderArrElement",
        "pub(crate) struct HolderOpt",
        "pub(crate) struct HolderMpValue",
        "pub(crate) struct HolderNested",
        "pub(crate) struct HolderNestedE",
    ] {
        assert!(
            rust_source.contains(item),
            "a tuple under an `internal` declaration must emit `{item}`, got:\n{rust_source}"
        );
    }
    // `pub(crate) struct X` does not contain `pub struct X`, so these are
    // genuine negatives rather than restatements of the loop above.
    for leaked in [
        "pub struct HolderDirect",
        "pub struct HolderArrElement",
        "pub struct HolderOpt",
        "pub struct HolderMpValue",
        "pub struct HolderNested",
        "pub struct HolderNestedE",
    ] {
        assert!(
            !rust_source.contains(leaked),
            "a tuple under an `internal` declaration must not emit `{leaked}`, \
             got:\n{rust_source}"
        );
    }
    assert!(
        rust_source.contains("pub struct ShownDirect"),
        "a public declaration's tuple must still be `pub`, got:\n{rust_source}"
    );

    // The proof. `private-interfaces` and `private-bounds` are denied by name
    // rather than with a blanket `-D warnings`, matching `rustc_accepts` in
    // `crates/ridlc/tests/corpus.rs`: the generated code carries by-design
    // naming and dead-code lints that say nothing about visibility.
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("internal_tuple.rs");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "private-interfaces",
            "-D",
            "private-bounds",
        ])
        .arg("-o")
        .arg(dir.path().join("internal_tuple.rmeta"))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "a tuple under an `internal` declaration must compile under \
         `-D private-interfaces`, source:\n{rust_source}"
    );
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
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
    insta::assert_snapshot!(rust_for(decls));
}

// ---------------------------------------------------------------------------
// Default derivation — the leaf-recursion rule.
// ---------------------------------------------------------------------------

#[test]
fn derivable_scalar_gets_default_with_derived_value() {
    // A numeric type whose range excludes 0 defaults to `min`, not 0.
    let ranged = v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit("Cel".to_string())),
        }),
        constraint: Some(constraint(Some("10.0"), Some("40.0"), Some("0.5"))),
        declared_init: None,
        init: Some(init_value(true, Some("10.0"))),
        width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
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
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
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
        v2::decl::Kind::EnumDef(v2::EnumDef {
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

/// The typl reference Appendix B package `veh.common`, built as IR v2 with
/// populated init values (T15). Cross-package references to `ridl.std` are
/// fully qualified, as the resolver leaves them (typl §3.2).
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
fn appendix_b_rust_snapshot() {
    let Generated { rust_source, .. } = generate(&appendix_b()).expect("Appendix B generates");
    insta::assert_snapshot!(rust_source);
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
    let bag = v2::StructDef {
        members: vec![
            field_member(array_field("fixed", "Speed", 3, 3)),
            field_member(v2::Field {
                ordinal: 2,
                ..array_field("bounded", "Speed", 2, 5)
            }),
            field_member(v2::Field {
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
        public_decl("Bag", v2::decl::Kind::StructDef(bag)),
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
    let struct_def = v2::StructDef {
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
        public_decl("Config", v2::decl::Kind::StructDef(struct_def)),
    ]);
    assert!(
        source.contains("r#override"),
        "a keyword field name must be raw-escaped, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Epic E1 whole-epic review — regression fixtures.
// ---------------------------------------------------------------------------

/// C1b: a non-optional self-reference `struct S { next: S }` is a cycle. The
/// checker rejects it (TYPL-206), but the backend must not trust that gate:
/// the Default recursion must terminate rather than overflow the stack.
#[test]
fn recursive_struct_default_terminates() {
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
    let plate = v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
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
    let field = v2::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "veh.other.GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    let source = rust_for(vec![public_decl(
        "Selection",
        v2::decl::Kind::StructDef(struct_def),
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
            v2::PrimitiveType::Integer,
            init_value(true, Some("0")),
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
        ),
    );
    let field = v2::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    let source = rust_for(vec![
        gear_index,
        public_decl("Selection", v2::decl::Kind::StructDef(struct_def)),
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
        v2::decl::Kind::ConstDef(v2::ConstDef {
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

// ---------------------------------------------------------------------------
// Identifier totality.
// ---------------------------------------------------------------------------

/// `ident` is total. A valid typl name can never be empty (typl §2.3), so an
/// empty string only reaches here from malformed IR — but the backend is fed
/// by `ridlc::compile`, whose documented contract is that it never panics, and
/// by the language server, which sees IR lowered from in-progress source. An
/// empty name therefore lowers to `_` instead of reaching `Ident::new_raw`,
/// which panics on the empty string.
#[test]
fn ident_is_total_on_the_empty_name() {
    assert_eq!(super::ident("").to_string(), "_");
}

/// A typl name of `_` keeps its own mangling, so the empty-name placeholder
/// can never be confused with a real declaration.
#[test]
fn ident_maps_the_underscore_name_away_from_the_placeholder() {
    assert_eq!(super::ident("_").to_string(), "__");
}

/// The limit of the `_` placeholder, pinned rather than assumed. syn accepts
/// `_` in *field* position, so the `syn::parse2` gate does not catch an
/// empty-named field. A derived `Default` usually catches it instead, because
/// the struct expression it builds has no valid `Member` — but
/// `defaults::struct_default` returns `None` for a non-constructible field (a
/// cross-package reference carrying a declared init), and then nothing is left
/// to catch it and `generate` returns `Ok`.
///
/// This is unreachable from source today: `Parser::block_body` announces a
/// member only on `SyntaxKind::Ident`, so a nameless `FieldDef` is never
/// built. The test exists so that a change making one reachable shows up here
/// rather than as silently emitted Rust.
#[test]
fn generate_emits_an_empty_field_name_without_a_derivable_default() {
    let field = v2::Field {
        name: String::new(),
        ordinal: 1,
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named("other.pkg.Thing".to_string())),
        }),
        // A declared init on a cross-package reference makes the field
        // non-constructible, so no `Default` is derived.
        declared_init: Some("1".to_string()),
        init: Some(init_value(false, None)),
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    };
    let decls = vec![public_decl(
        "S",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(field)],
            fixed_layout: false,
        }),
    )];
    let generated = generate(&package("app", decls)).expect("the parse gate does not catch `_`");
    assert!(
        generated.rust_source.contains("pub _:"),
        "expected an emitted `_` field, got:\n{}",
        generated.rust_source,
    );
    assert!(
        !generated.rust_source.contains("impl Default for S"),
        "the struct must not derive Default, got:\n{}",
        generated.rust_source,
    );
}

/// An empty-named *declaration* is reported, not emitted. `_` is illegal in a
/// Rust declaration-name position, so the `syn::parse2` gate in `generate`
/// turns it into a `GenerateError` — the codegen totality contract
/// (ADR-0004 §5) holds without the malformed name becoming plausible-looking
/// output.
#[test]
fn generate_reports_an_empty_named_decl_instead_of_panicking() {
    let decls = vec![public_decl(
        "",
        primitive_type(
            v2::PrimitiveType::Boolean,
            init_value(true, Some("false")),
            None,
        ),
    )];
    let error = generate(&package("app", decls)).expect_err("an empty name cannot generate");
    assert!(
        error.message.contains("does not parse"),
        "expected the parse gate to reject `_`, got: {}",
        error.message,
    );
}

// ---------------------------------------------------------------------------
// Interactions and services (E2 task 15) — fixture builders.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Per-kind interaction snapshots.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Appendix A — the ridl reference corpus package `veh.cluster`.
// ---------------------------------------------------------------------------

/// The ridl reference Appendix A package `veh.cluster`, as **`ridl-sem`
/// actually lowers it**.
///
/// The IR is not rebuilt here. It is deserialized from the golden that
/// `ridl-sem` writes for its own `appendix_a_ir` test, so this backend and the
/// lowering that feeds it cannot drift: there is one artifact, and a change in
/// lowering reaches this test the moment that golden is regenerated. A
/// hand-built copy could not do that — it would keep compiling happily while
/// describing an IR the compiler no longer produces, which for the E2 exit
/// criterion's compile proof would make the proof about the fixture rather
/// than about the pipeline.
///
/// Cross-package references to `veh.common` and `ridl.std` stay fully
/// qualified, as the resolver leaves them (typl §3.2).
///
/// One residual is worth recording. The coupling holds as long as the golden is
/// a live artifact: if the `ridl-sem` test that writes it were deleted and the
/// `.snap` file left behind, this loader would keep reading an orphan and keep
/// passing. `cargo insta test --unreferenced` detects exactly that, and it is
/// not in the local gate — adding it is a repo-wide call for the epic
/// close-out, not this backend's to make. Deleting the file outright is caught
/// here immediately, so the gap is narrow: an orphaned snapshot, not a missing
/// one.
fn appendix_a() -> v2::Package {
    // `CARGO_MANIFEST_DIR` is `<workspace>/crates/ridl-backend-rust`.
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ridl-sem/src/snapshots/ridl_sem__check__tests__appendix_a_ir.snap"
    );
    let text = std::fs::read_to_string(golden).unwrap_or_else(|err| {
        panic!(
            "the ridl-sem Appendix A golden must be readable at {golden}: {err}\n\
             this test consumes that crate's lowering rather than a copy of it"
        )
    });

    // An insta snapshot is a YAML header, a `---` line, then the payload.
    let body = text
        .split_once("\n---\n")
        .map(|(_, body)| body)
        .unwrap_or(&text);
    let package = v2::from_json(body)
        .expect("the ridl-sem Appendix A golden deserializes as an IR v2 package");

    // The golden is the whole point, so its shape is asserted rather than
    // assumed: a silently empty or renamed package would turn the compile proof
    // below into a proof about nothing.
    assert_eq!(package.name, "veh.cluster");
    assert_eq!(
        package.interfaces.len(),
        1,
        "Appendix A declares exactly one interface"
    );
    assert!(
        package.services.is_empty(),
        "Appendix A declares no service — services are covered by their own tests"
    );
    package
}

/// The generated Rust for the full Appendix A package compiles with `rustc` —
/// the E2 exit criterion's proof for the Rust half. A minimal prelude stands in
/// for the `veh.common` and `ridl.std` types the package imports, declared in
/// the module path the cross-package references map to.
#[test]
fn appendix_a_compiles_with_rustc() {
    let Generated { rust_source, .. } = generate(&appendix_a()).expect("Appendix A generates");

    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        pub struct Message(pub String);
        pub struct Label(pub String);
        pub struct Version(pub String);
        pub struct Duration(pub i64);
        #[derive(Default)]
        pub struct Timestamp(pub i64);
    }
}
pub mod veh {
    pub mod common {
        pub struct Speed(pub f64);
        pub struct Temperature(pub f64);
        pub struct WarningFlags(pub i64);
        pub enum GearPosition {
            PARK = 0,
            DRIVE = 1,
        }
    }
}
";
    let source = format!("{PRELUDE}\n{rust_source}");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("appendix_a.rs");
    let meta_path = dir.path().join("appendix_a.rmeta");
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
        "generated Rust for Appendix A must compile, source:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Transport identity inside an inline service shape.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Contract data.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Streams, services, and vocabulary collisions.
// ---------------------------------------------------------------------------

/// A package with no interactions is unaffected by the collision check: the
/// vocabulary is not emitted, so the name is free.
#[test]
fn a_typl_only_package_may_declare_a_vocabulary_name() {
    let source = rust_for(vec![public_decl(
        "Provenance",
        primitive_type(
            v2::PrimitiveType::Integer,
            init_value(true, Some("0")),
            None,
        ),
    )]);
    assert!(source.contains("struct Provenance"), "got:\n{source}");
    assert!(
        !source.contains("enum Provenance"),
        "the vocabulary is not emitted for a typl-only package, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// The generated-name classes a package must not collide with.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// What the check must NOT reject: Rust's namespaces genuinely separate these.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Nested tuple names.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Generated names colliding with each other.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// `internal` visibility on the interaction layer (ADR-0008 decision 7).
// ---------------------------------------------------------------------------
