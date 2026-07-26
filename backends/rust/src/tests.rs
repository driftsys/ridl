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

/// A tuple reached through a `final` payload is package-private with the
/// `internal interface` that declares it.
///
/// **This position was argued away and had no coverage.** FORM-102 narrows a
/// `final` payload to "a named type **or an array**" and does not descend into
/// the element, so `final bounds : [(lo : Hidden, hi : Hidden); 2]` is accepted
/// — `ridlc check` exits 0 — and once inside the array element the whole of
/// `field_type_tokens` is reachable again. A query's return is therefore not
/// the only interaction position a tuple can reach, and this was the one
/// position in the set with no corpus entry and no unit test.
///
/// All four element shapes are exercised, because each is a separate branch:
/// a bare tuple element, an array of arrays, an optional element, and a tuple
/// nested in the element (which induces two structs). A map is admitted inside
/// the element too, though not as the payload itself.
#[test]
fn a_tuple_under_a_final_payload_is_package_private() {
    // `[element; 2]`, with `element` supplied by the caller.
    fn array_of(element: v2::field_type::Kind) -> v2::field_type::Kind {
        v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(v2::FieldType {
                optional: false,
                kind: Some(element),
            })),
            min: 2,
            max: 2,
        }))
    }
    let final_of = |name: &str, ordinal: u32, payload: v2::field_type::Kind| {
        interaction(
            name,
            ordinal,
            "",
            v2::decl::Kind::FinalDef(v2::FinalDef {
                payload: Some(v2::FieldType {
                    optional: false,
                    kind: Some(payload),
                }),
            }),
        )
    };

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
    let panel = v2::Interface {
        visibility: v2::Visibility::Internal as i32,
        ..interface(
            "Panel",
            "",
            vec![
                // 1. the array element is a tuple
                final_of("bounds", 1, array_of(tuple_of(&[("lo", "Hidden")]))),
                // 2. an array of arrays of tuples
                final_of(
                    "nested",
                    2,
                    array_of(array_of(tuple_of(&[("a", "Hidden")]))),
                ),
                // 3. the element is an OPTIONAL tuple
                final_of(
                    "opt",
                    3,
                    v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                        element: Some(Box::new(v2::FieldType {
                            optional: true,
                            kind: Some(tuple_of(&[("b", "Hidden")])),
                        })),
                        min: 2,
                        max: 2,
                    })),
                ),
                // 4. a tuple nested inside the element tuple — two structs
                final_of(
                    "deep",
                    4,
                    array_of(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![v2::TupleField {
                            name: "d".to_string(),
                            r#type: Some(v2::FieldType {
                                optional: false,
                                kind: Some(tuple_of(&[("e", "Hidden")])),
                            }),
                        }],
                    })),
                ),
            ],
        )
    };
    // The regression direction: the same shape on a PUBLIC interface stays
    // `pub`. Its payload is `Counter`, because a public declaration naming an
    // `internal` one is TYPL-005 and never reaches a backend.
    let shown = interface(
        "Shown",
        "",
        vec![final_of(
            "bounds",
            1,
            array_of(tuple_of(&[("g", "Counter")])),
        )],
    );

    let Generated { rust_source, .. } = generate(&interaction_package(
        vec![counter_decl(), hidden],
        vec![panel, shown],
        Vec::new(),
    ))
    .expect("the package generates");

    for item in [
        "pub(crate) struct PanelBoundsElement",
        "pub(crate) struct PanelNestedElementElement",
        "pub(crate) struct PanelOptElement",
        "pub(crate) struct PanelDeepElement",
        "pub(crate) struct PanelDeepElementD",
    ] {
        assert!(
            rust_source.contains(item),
            "a tuple under an `internal` final must emit `{item}`, got:\n{rust_source}"
        );
    }
    // `pub(crate) struct X` does not contain `pub struct X`, so these are
    // genuine negatives.
    for leaked in [
        "pub struct PanelBoundsElement",
        "pub struct PanelNestedElementElement",
        "pub struct PanelOptElement",
        "pub struct PanelDeepElement",
        "pub struct PanelDeepElementD",
    ] {
        assert!(
            !rust_source.contains(leaked),
            "a tuple under an `internal` final must not emit `{leaked}`, got:\n{rust_source}"
        );
    }
    assert!(
        rust_source.contains("pub struct ShownBoundsElement"),
        "a public interface's final tuple must still be `pub`, got:\n{rust_source}"
    );

    // The proof, denying the two lints by name for the reason `rustc_accepts`
    // records: the generated code carries by-design naming and dead-code lints.
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("final_tuple.rs");
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
        .arg(dir.path().join("final_tuple.rmeta"))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "a tuple under an `internal` final must compile under \
         `-D private-interfaces`, source:\n{rust_source}"
    );
}

/// Two tuples whose paths mangle to one struct name are **refused**, in either
/// layer, rather than deduplicated.
///
/// `struct AB { c : … }` and `struct A { bC : … }` both reach the name `ABC`,
/// and nothing upstream keeps them apart — `ridlc check` exits 0. The worklist
/// used to keep the first discovery, which gave the second declaration the
/// first one's shape: a module that compiles and states the wrong contract.
///
/// It is also the corner that makes carrying an inducing declaration's
/// visibility (issue #167) unsafe on its own. With one declaration `internal`
/// and the other public — **and no `internal` payload type anywhere**, so
/// TYPL-005 has nothing to see — first-wins would put a `pub(crate)` struct in a
/// `pub` struct's field and rustc would report `private_interfaces` on a program
/// that built before. Keeping the widest visibility instead would publish the
/// package-private declaration's shape, which is the defect #167 fixed. Neither
/// dedup rule is sound; only rejection is.
///
/// The mixed-visibility pair is the fixture on purpose: a same-visibility pair
/// would be refused by a check that ignored visibility entirely, so it would not
/// show that the rejection is what keeps the visibility rule safe.
#[test]
fn two_tuples_mangling_to_one_name_are_refused() {
    let hidden = v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..public_decl(
            "A",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(shaped_field(
                    "bC",
                    1,
                    tuple_of(&[("y", "Counter")]),
                ))],
                fixed_layout: false,
            }),
        )
    };
    let shown = public_decl(
        "AB",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(shaped_field(
                "c",
                1,
                tuple_of(&[("x", "Counter")]),
            ))],
            fixed_layout: false,
        }),
    );

    let error = generate(&package("veh.common", vec![counter_decl(), hidden, shown]))
        .expect_err("two tuples generating one name are refused");
    assert!(
        error.message.contains("ABC"),
        "the refusal names the generated name, got: {}",
        error.message,
    );
    // The mangled name cannot tell the two apart — that is the defect — so the
    // message names both field lists, which is what a reader can search for.
    assert!(
        error.message.contains("(x)") && error.message.contains("(y)"),
        "the refusal names both tuple shapes, got: {}",
        error.message,
    );

    // The interaction layer reaches the same collision through two query
    // returns, and is refused identically. It was NOT refused before:
    // `check_name_collisions` treats a repeated tuple name as the worklist's own
    // deduplication and skips it.
    let error = generate(&interaction_package(
        vec![counter_decl()],
        vec![
            interface(
                "AB",
                "",
                vec![interaction(
                    "c",
                    1,
                    "",
                    v2::decl::Kind::QueryDef(v2::QueryDef {
                        params: Vec::new(),
                        return_type: Some(v2::ReturnType {
                            kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                                optional: false,
                                kind: Some(tuple_of(&[("x", "Counter")])),
                            })),
                        }),
                        contracts: Vec::new(),
                    }),
                )],
            ),
            interface(
                "A",
                "",
                vec![interaction(
                    "bC",
                    1,
                    "",
                    v2::decl::Kind::QueryDef(v2::QueryDef {
                        params: Vec::new(),
                        return_type: Some(v2::ReturnType {
                            kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                                optional: false,
                                kind: Some(tuple_of(&[("y", "Counter")])),
                            })),
                        }),
                        contracts: Vec::new(),
                    }),
                )],
            ),
        ],
        Vec::new(),
    ))
    .expect_err("two interaction tuples generating one name are refused");
    assert!(
        error.message.contains("ABCResult"),
        "the refusal names the generated name, got: {}",
        error.message,
    );
}

/// The regression direction for the refusal: one tuple **legitimately**
/// discovered twice is not a collision.
///
/// This is not hypothetical — it is how the worklist runs. `interact::emit`
/// walks its own slice to pre-discover the names of tuples nested inside
/// tuples, because the collision check cannot wait for the caller's drain; the
/// drain then finds each of them a second time while emitting the outer tuple's
/// fields. A refusal that compared names alone would reject every package with
/// a tuple inside a tuple in an interaction position.
#[test]
fn one_tuple_discovered_twice_is_not_a_collision() {
    let nested = v2::TupleType {
        fields: vec![v2::TupleField {
            name: "outer".to_string(),
            r#type: Some(v2::FieldType {
                optional: false,
                kind: Some(tuple_of(&[("inner", "Counter")])),
            }),
        }],
    };
    let Generated { rust_source, .. } = generate(&interaction_package(
        vec![counter_decl()],
        vec![interface(
            "Panel",
            "",
            vec![interaction(
                "read",
                1,
                "",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: Vec::new(),
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Tuple(nested)),
                        })),
                    }),
                    contracts: Vec::new(),
                }),
            )],
        )],
        Vec::new(),
    ))
    .expect("a tuple nested in a tuple generates");

    assert!(
        rust_source.contains("pub struct PanelReadResult"),
        "the outer tuple must be emitted, got:\n{rust_source}"
    );
    assert_eq!(
        rust_source.matches("struct PanelReadResultOuter").count(),
        1,
        "the nested tuple is discovered twice and emitted once, got:\n{rust_source}"
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

/// `type RawFrame : bytes [8]` — a bytes-backed named scalar type with a fixed
/// length. It has no fixed C ABI: the C header refuses its typedef and the
/// Rust backend realizes it as `Vec<u8>`.
fn bytes_frame_decl() -> v2::Decl {
    public_decl(
        "RawFrame",
        v2::decl::Kind::TypeDef(v2::TypeDef {
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

/// C2: `c_field_type` never emits a dangling type name. A `fixed_layout` struct
/// whose field references a bytes-backed named type (a defensive case the
/// checker's C2 rule normally prevents) is downgraded to the not-representable
/// block rather than emitted as a repr(C) struct naming an undeclared typedef.
#[test]
fn c_field_type_guards_a_dangling_named_reference() {
    let frame = v2::StructDef {
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
            public_decl("Frame", v2::decl::Kind::StructDef(frame)),
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
    // A bytes-backed named field makes the struct non-fixed (the checker's C2
    // rule); it is not emitted as a repr(C) struct.
    let frame = v2::StructDef {
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
        public_decl("Reading", v2::decl::Kind::StructDef(reading)),
        public_decl("Frame", v2::decl::Kind::StructDef(frame)),
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

/// An interaction declaration: the same `Decl` envelope as a package
/// declaration, with `ordinal` set and `visibility` left unspecified — an
/// interaction is not separately visible (ridl §3).
fn interaction(name: &str, ordinal: u32, doc: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: doc.to_string(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(kind),
    }
}

fn timing(mode: v2::TimingMode, min_us: Option<&str>, max_us: Option<&str>) -> v2::Timing {
    v2::Timing {
        mode: mode as i32,
        min_us: min_us.map(str::to_string),
        max_us: max_us.map(str::to_string),
        default_applied: false,
    }
}

fn named_type(reference: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(reference.to_string())),
    }
}

fn stream_of(element: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
            element: Some(v2::stream_type::Element::Named(element.to_string())),
        })),
    }
}

fn param(name: &str, ty: v2::FieldType) -> v2::Param {
    v2::Param {
        name: name.to_string(),
        r#type: Some(ty),
    }
}

fn contract(
    kind: v2::ContractKind,
    source: &str,
    signal_refs: &[&str],
    param_refs: &[&str],
    uses_result: bool,
    observer_id: &str,
) -> v2::Contract {
    v2::Contract {
        kind: kind as i32,
        source: source.to_string(),
        signal_refs: signal_refs.iter().map(|s| s.to_string()).collect(),
        param_refs: param_refs.iter().map(|s| s.to_string()).collect(),
        uses_result,
        observer_id: observer_id.to_string(),
    }
}

fn interface(name: &str, doc: &str, interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: doc.to_string(),
        labels: Vec::new(),
        deprecated: None,
        interactions,
    }
}

fn service(name: &str, shape: v2::service::Shape) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shape: Some(shape),
    }
}

fn interaction_package(
    decls: Vec<v2::Decl>,
    interfaces: Vec<v2::Interface>,
    services: Vec<v2::Service>,
) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls,
        interfaces,
        services,
    }
}

/// The Rust source for one `VehicleStatus` interface holding `interactions`.
fn rust_for_interaction(interactions: Vec<v2::Decl>) -> String {
    generate(&interaction_package(
        Vec::new(),
        vec![interface("VehicleStatus", "", interactions)],
        Vec::new(),
    ))
    .expect("generation succeeds")
    .rust_source
}

// ---------------------------------------------------------------------------
// Per-kind interaction snapshots.
// ---------------------------------------------------------------------------

#[test]
fn signal_carries_init_and_provenance() {
    let source = rust_for_interaction(vec![interaction(
        "currentSpeed",
        1,
        "Current vehicle speed",
        v2::decl::Kind::SignalDef(v2::SignalDef {
            payload: "veh.common.Speed".to_string(),
            declared_init: Some("MAX_SPEED".to_string()),
            init: Some(init_value(true, Some("250"))),
            timing: Some(timing(
                v2::TimingMode::StrictPeriodic,
                Some("10000"),
                Some("10000"),
            )),
        }),
    )]);
    assert!(
        source.contains("SignalHandle<crate::veh::common::Speed>"),
        "a signal is a SignalHandle over its payload, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn event_is_a_subscribe_only_handle() {
    let source = rust_for_interaction(vec![interaction(
        "doorOpened",
        4,
        "Raised on every door state change",
        v2::decl::Kind::EventDef(v2::EventDef {
            payload: "DoorPayload".to_string(),
            timing: Some(timing(v2::TimingMode::Range, Some("50000"), Some("500000"))),
        }),
    )]);
    assert!(
        source.contains("EventHandle<DoorPayload>"),
        "an event is an EventHandle over its payload, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn command_returns_unit_and_records_its_require() {
    let source = rust_for_interaction(vec![interaction(
        "setGear",
        5,
        "Request a gear change",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![param("position", named_type("veh.common.GearPosition"))],
            contracts: vec![contract(
                v2::ContractKind::Require,
                "position != GearPosition.PARK || currentSpeed == 0.0",
                &["VehicleStatus.currentSpeed"],
                &["position"],
                false,
                "VehicleStatus.setGear.require[0]",
            )],
        }),
    )]);
    assert!(
        source.contains("infrastructure failure — detected, undeclared"),
        "the gf §6.4 transport wording is used verbatim, got:\n{source}"
    );
    assert!(
        source.contains("VehicleStatus.setGear.require[0]"),
        "the observer stub id is emitted as data, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn fallible_query_carries_the_transport_identity() {
    let source = rust_for_interaction(vec![interaction(
        "calibrate",
        9,
        "Run an axle calibration",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: vec![param("target", named_type("Axle"))],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "CalReport".to_string(),
                    err: "CalError".to_string(),
                })),
            }),
            contracts: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("transport identity: VehicleStatus#9:CalReport|CalError"),
        "the identity comes from fallible_transport_identity, got:\n{source}"
    );
    assert!(
        source.contains("Result<CalReport, CalError>"),
        "a fallible return is a native Result, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn tuple_return_query_generates_a_named_struct() {
    let source = rust_for_interaction(vec![interaction(
        "getBounds",
        3,
        "Observed speed envelope",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
                    })),
                })),
            }),
            contracts: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("struct VehicleStatusGetBoundsResult"),
        "a tuple return generates a named struct, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn bidirectional_stream_query_uses_ridl_stream_on_both_sides() {
    let source = rust_for_interaction(vec![interaction(
        "replayFaults",
        8,
        "Stream faults in, stream faults out",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: vec![param("window", stream_of("FaultWindow"))],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(stream_of("FaultEvent"))),
            }),
            contracts: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("impl RidlStream<Item = FaultWindow>"),
        "a stream parameter takes impl RidlStream, got:\n{source}"
    );
    assert!(
        source.contains("impl RidlStream<Item = FaultEvent>"),
        "a stream return is impl RidlStream, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn final_with_an_array_is_a_read_only_accessor() {
    let source = rust_for_interaction(vec![interaction(
        "capabilities",
        11,
        "",
        v2::decl::Kind::FinalDef(v2::FinalDef {
            payload: Some(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(named_type("ridl.std.Label"))),
                    min: 0,
                    max: 32,
                }))),
            }),
        }),
    )]);
    let provider = source
        .split("VehicleStatusProvider")
        .nth(1)
        .expect("a provider face is emitted");
    assert!(
        !provider.contains("capabilities"),
        "a final has no provider entry — it is provisioned externally (ridl §8), got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn services_both_forms() {
    let package = interaction_package(
        Vec::new(),
        vec![interface(
            "CruiseControl",
            "Adaptive cruise",
            vec![interaction(
                "engaged",
                1,
                "",
                v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Engagement".to_string(),
                    declared_init: None,
                    init: Some(init_value(true, Some("0"))),
                    timing: Some(timing(
                        v2::TimingMode::Range,
                        Some("100000"),
                        Some("1000000"),
                    )),
                }),
            )],
        )],
        vec![
            service(
                "veh.adas.cruise",
                v2::service::Shape::InterfaceRef("CruiseControl".to_string()),
            ),
            service(
                "veh.hvac.cabin",
                v2::service::Shape::Inline(interface(
                    "",
                    "Cabin climate",
                    vec![interaction(
                        "setTarget",
                        1,
                        "",
                        v2::decl::Kind::CommandDef(v2::CommandDef {
                            params: vec![param("target", named_type("Temperature"))],
                            contracts: Vec::new(),
                        }),
                    )],
                )),
            ),
        ],
    );
    let source = generate(&package).expect("generation succeeds").rust_source;
    assert!(
        source.contains("ServiceVehHvacCabin"),
        "an inline shape names its generated anonymous interface, got:\n{source}"
    );
    assert!(
        source.contains(r#"("veh.adas.cruise", "CruiseControl")"#),
        "the service address maps to its interface, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

#[test]
fn a_fractional_microsecond_bound_is_refused_rather_than_rounded() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![interface(
            "VehicleStatus",
            "",
            vec![interaction(
                "tick",
                1,
                "",
                v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Speed".to_string(),
                    declared_init: None,
                    init: None,
                    timing: Some(timing(v2::TimingMode::Range, Some("1.5"), Some("100"))),
                }),
            )],
        )],
        Vec::new(),
    ))
    .expect_err("a fractional microsecond bound cannot be represented exactly");
    assert!(
        error.message.contains("1.5"),
        "the refusal names the bound it will not round, got: {}",
        error.message
    );
}

#[test]
fn an_unspecified_timing_mode_is_a_generate_error() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![interface(
            "VehicleStatus",
            "",
            vec![interaction(
                "tick",
                1,
                "",
                v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Speed".to_string(),
                    declared_init: None,
                    init: None,
                    timing: Some(timing(v2::TimingMode::Unspecified, Some("10"), Some("10"))),
                }),
            )],
        )],
        Vec::new(),
    ))
    .expect_err("an unresolved timing mode is refused");
    assert!(
        error.message.contains("timing mode"),
        "the refusal names the unresolved mode, got: {}",
        error.message
    );
}

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
    // `CARGO_MANIFEST_DIR` is `<workspace>/backends/rust`.
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/ridl-sem/src/snapshots/ridl_sem__check__tests__appendix_a_ir.snap"
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
    let package: v2::Package = serde_json::from_str(body)
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

#[test]
fn appendix_a_rust_snapshot() {
    let Generated { rust_source, .. } = generate(&appendix_a()).expect("Appendix A generates");
    insta::assert_snapshot!(rust_source);
}

#[test]
fn appendix_a_c_header_snapshot() {
    let Generated { c_header, .. } = generate(&appendix_a()).expect("Appendix A generates");
    assert!(
        c_header.contains("interface VehicleStatus"),
        "the header records the interface as outside the C ABI, got:\n{c_header}"
    );
    insta::assert_snapshot!(c_header);
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

/// A fallible query inside a service's inline shape takes its transport
/// identity from the service's DOTTED global name, not from the Rust type name
/// this backend generates for it.
///
/// `ServiceVehAdasLogs` is a spelling invented here to satisfy Rust's
/// identifier rules; it means nothing outside this module. Emitting it as a
/// transport identity would disagree with two things that already exist: the
/// observer stubs lowered into this very module are scoped to the dotted name
/// (`ridl-sem`, E2.5), and `ridl diff` keys a service's interactions on the
/// dotted name (`tools/diff/src/walk.rs`). One value, three consumers — a
/// disagreement here is the E2 exit criterion failing, since the whole claim is
/// that two backends emit one contract from one IR.
#[test]
fn inline_service_fallible_query_uses_the_dotted_service_name() {
    let fetch = interaction(
        "fetchPage",
        3,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: vec![param("filter", named_type("DiagFilter"))],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: Vec::new(),
        }),
    );
    let source = generate(&interaction_package(
        Vec::new(),
        Vec::new(),
        vec![service(
            "veh.adas.logs",
            v2::service::Shape::Inline(interface("", "", vec![fetch])),
        )],
    ))
    .expect("generation succeeds")
    .rust_source;

    // The emitted string is exactly what the one IR derivation produces — the
    // rule is not spelled a second time in this backend.
    let expected = v2::fallible_transport_identity(
        "veh.adas.logs",
        3,
        &v2::FallibleType {
            ok: "FaultPage".to_string(),
            err: "DiagError".to_string(),
        },
    );
    assert!(
        source.contains(&format!("transport identity: {expected}")),
        "the identity must match the IR helper ({expected}), got:\n{source}"
    );
    // The regression named directly: the mangled type name is not an identity.
    assert!(
        !source.contains("transport identity: ServiceVehAdasLogs"),
        "the generated type name must never be emitted as a transport identity, got:\n{source}"
    );
    // The generated type name is still what names the Rust items, and the two
    // therefore appear side by side in one module without being confused.
    assert!(
        source.contains("pub trait ServiceVehAdasLogsConsumer"),
        "the generated face keeps the mangled type name, got:\n{source}"
    );
    assert!(
        source.contains(r#"("veh.adas.logs", "ServiceVehAdasLogs")"#),
        "the service table maps the dotted address to the generated type, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// An observer stub inside an inline shape is scoped to the dotted name too, so
/// the identity and the observer address agree inside one generated module.
#[test]
fn inline_service_observer_ids_and_identity_agree() {
    let fetch = interaction(
        "fetchPage",
        3,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: vec![contract(
                v2::ContractKind::Ensure,
                "result.ok",
                &[],
                &[],
                true,
                "veh.adas.logs.fetchPage.ensure[0]",
            )],
        }),
    );
    let source = generate(&interaction_package(
        Vec::new(),
        Vec::new(),
        vec![service(
            "veh.adas.logs",
            v2::service::Shape::Inline(interface("", "", vec![fetch])),
        )],
    ))
    .expect("generation succeeds")
    .rust_source;

    assert!(
        source.contains("transport identity: veh.adas.logs#3:FaultPage|DiagError"),
        "got:\n{source}"
    );
    assert!(
        source.contains(r#"id: "veh.adas.logs.fetchPage.ensure[0]""#),
        "got:\n{source}"
    );
}

/// A named interface keeps using its own name, so threading the identity apart
/// from the generated type name did not disturb the common case.
#[test]
fn named_interface_identity_is_the_interface_name() {
    let source = rust_for_interaction(vec![interaction(
        "getFaultPage",
        9,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("transport identity: VehicleStatus#9:FaultPage|DiagError"),
        "got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Contract data.
// ---------------------------------------------------------------------------

/// `uses_result` is carried from the IR rather than inferred.
///
/// It cannot be recovered from `source`: a parameter named `resultCode`, a
/// field access `.result`, or a string literal all contain the substring, and
/// an `ensure` clause does not always read the result. An `ensure` observer
/// that reads the result cannot be scheduled before the result exists, so the
/// flag decides when the observer runs.
#[test]
fn contract_stubs_carry_uses_result() {
    let source = rust_for_interaction(vec![interaction(
        "getAverageSpeed",
        7,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: vec![param("resultCode", named_type("Code"))],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(named_type("Speed"))),
            }),
            contracts: vec![
                contract(
                    v2::ContractKind::Require,
                    "resultCode > 0",
                    &[],
                    &["resultCode"],
                    false,
                    "VehicleStatus.getAverageSpeed.require[0]",
                ),
                contract(
                    v2::ContractKind::Ensure,
                    "result >= 0.0",
                    &[],
                    &[],
                    true,
                    "VehicleStatus.getAverageSpeed.ensure[0]",
                ),
            ],
        }),
    )]);

    // The require clause mentions `resultCode` but does not read the result:
    // the flag and the text disagree on purpose, which is why the flag is
    // carried rather than recovered from `source`.
    assert!(
        source.contains("uses_result: false"),
        "a clause naming resultCode must still report uses_result: false, got:\n{source}"
    );
    assert!(
        source.contains("uses_result: true"),
        "a clause reading the result must report uses_result: true, got:\n{source}"
    );
    // Two stubs, one flag each. The vocabulary's own field declaration reads
    // `pub uses_result:`, so it is excluded by matching the emitted values.
    assert_eq!(
        source.matches("uses_result: false").count() + source.matches("uses_result: true").count(),
        2,
        "every stub carries the flag, got:\n{source}"
    );
}

/// A contract with no kind is refused rather than silently filed as `require`.
/// A `require` is checked before the call and an `ensure` after, so guessing
/// installs the observer at the wrong moment.
#[test]
fn a_contract_without_a_kind_is_a_generate_error() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![interface(
            "VehicleStatus",
            "",
            vec![interaction(
                "setGear",
                1,
                "",
                v2::decl::Kind::CommandDef(v2::CommandDef {
                    params: Vec::new(),
                    contracts: vec![contract(
                        v2::ContractKind::Unspecified,
                        "position != PARK",
                        &[],
                        &[],
                        false,
                        "VehicleStatus.setGear.require[0]",
                    )],
                }),
            )],
        )],
        Vec::new(),
    ))
    .expect_err("a kindless contract is refused");
    assert!(
        error.message.contains("no kind"),
        "the refusal names the missing kind, got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------------
// Streams, services, and vocabulary collisions.
// ---------------------------------------------------------------------------

/// A stream carries `string` or `bytes` only (ridl §12.2, RIDL-202); any other
/// primitive is an inconsistent IR, refused rather than emitted as a type the
/// contract never admitted.
#[test]
fn a_stream_of_a_non_stream_primitive_is_a_generate_error() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![interface(
            "VehicleStatus",
            "",
            vec![interaction(
                "streamTicks",
                1,
                "",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: Vec::new(),
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                                element: Some(v2::stream_type::Element::Primitive(
                                    v2::PrimitiveType::Integer as i32,
                                )),
                            })),
                        })),
                    }),
                    contracts: Vec::new(),
                }),
            )],
        )],
        Vec::new(),
    ))
    .expect_err("an integer stream element is refused");
    assert!(
        error.message.contains("RIDL-202"),
        "the refusal cites the rule, got: {}",
        error.message
    );
}

/// A stream of `string` is admitted, so the check refuses the illegal element
/// without also rejecting the legal ones.
#[test]
fn a_stream_of_string_is_admitted() {
    let source = rust_for_interaction(vec![interaction(
        "streamLines",
        1,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                        element: Some(v2::stream_type::Element::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    })),
                })),
            }),
            contracts: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("impl RidlStream<Item = String>"),
        "got:\n{source}"
    );
}

/// A service with no shape names an address that nothing answers at, so it is
/// refused rather than emitted with an empty interface column.
#[test]
fn a_service_without_a_shape_is_a_generate_error() {
    let error = generate(&interaction_package(
        Vec::new(),
        Vec::new(),
        vec![v2::Service {
            shape: None,
            ..service(
                "veh.adas.cruise",
                v2::service::Shape::InterfaceRef(String::new()),
            )
        }],
    ))
    .expect_err("a shapeless service is refused");
    assert!(
        error.message.contains("no shape"),
        "the refusal names the missing shape, got: {}",
        error.message
    );
}

/// A typl declaration colliding with a generated vocabulary type is refused.
///
/// Without the check the module emits `pub struct Provenance` and
/// `pub enum Provenance` and rustc rejects it with `error[E0428]` — a broken
/// file handed downstream instead of a diagnostic naming the declaration.
#[test]
fn a_declaration_colliding_with_the_vocabulary_is_a_generate_error() {
    for name in ["Provenance", "SignalHandle", "RidlStream", "ContractStub"] {
        let error = generate(&interaction_package(
            vec![public_decl(
                name,
                primitive_type(
                    v2::PrimitiveType::Integer,
                    init_value(true, Some("0")),
                    None,
                ),
            )],
            vec![interface("VehicleStatus", "", Vec::new())],
            Vec::new(),
        ))
        .expect_err("a vocabulary collision is refused");
        assert!(
            error.message.contains(name) && error.message.contains("collides"),
            "the refusal names the colliding declaration, got: {}",
            error.message
        );
    }
}

/// A declaration colliding with a generated face name is refused for the same
/// reason.
#[test]
fn a_declaration_colliding_with_a_face_name_is_a_generate_error() {
    let error = generate(&interaction_package(
        vec![public_decl(
            "VehicleStatusConsumer",
            primitive_type(
                v2::PrimitiveType::Integer,
                init_value(true, Some("0")),
                None,
            ),
        )],
        vec![interface("VehicleStatus", "", Vec::new())],
        Vec::new(),
    ))
    .expect_err("a face-name collision is refused");
    assert!(
        error.message.contains("Consumer face"),
        "the refusal names the face, got: {}",
        error.message
    );
}

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

/// `@deprecated` reaches every interaction kind and both faces (typl §14.2).
/// Asserted here because deleting the attribute entirely left the per-kind
/// snapshots green.
#[test]
fn deprecated_reaches_interactions_and_both_faces() {
    fn deprecate(mut decl: v2::Decl, reason: &str) -> v2::Decl {
        decl.deprecated = Some(reason.to_string());
        decl
    }

    let source = rust_for_interaction(vec![
        deprecate(
            interaction(
                "oldSpeed",
                1,
                "",
                v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Speed".to_string(),
                    declared_init: None,
                    init: Some(init_value(true, Some("0"))),
                    timing: Some(timing(
                        v2::TimingMode::StrictPeriodic,
                        Some("10000"),
                        Some("10000"),
                    )),
                }),
            ),
            "use currentSpeed",
        ),
        deprecate(
            interaction(
                "oldDoor",
                2,
                "",
                v2::decl::Kind::EventDef(v2::EventDef {
                    payload: "DoorPayload".to_string(),
                    timing: Some(timing(v2::TimingMode::Range, Some("50000"), Some("500000"))),
                }),
            ),
            "use doorOpened",
        ),
        deprecate(
            interaction(
                "oldSetGear",
                3,
                "",
                v2::decl::Kind::CommandDef(v2::CommandDef {
                    params: Vec::new(),
                    contracts: Vec::new(),
                }),
            ),
            "use setGear",
        ),
        deprecate(
            interaction(
                "oldAverage",
                4,
                "",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: Vec::new(),
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(named_type("Speed"))),
                    }),
                    contracts: Vec::new(),
                }),
            ),
            "use getAverageSpeed",
        ),
        deprecate(
            interaction(
                "oldVersion",
                5,
                "",
                v2::decl::Kind::FinalDef(v2::FinalDef {
                    payload: Some(named_type("Version")),
                }),
            ),
            "use softwareVersion",
        ),
    ]);

    for reason in [
        "use currentSpeed",
        "use doorOpened",
        "use setGear",
        "use getAverageSpeed",
        "use softwareVersion",
    ] {
        assert!(
            source.contains(&format!("note = \"{reason}\"")),
            "the deprecation reason must reach the generated item, got:\n{source}"
        );
    }
    // Four of the five have a provider-side counterpart (a `final` does not),
    // so each of those reasons appears twice.
    assert_eq!(
        source.matches("note = \"use currentSpeed\"").count(),
        2,
        "the attribute reaches both faces, got:\n{source}"
    );
    assert_eq!(
        source.matches("note = \"use softwareVersion\"").count(),
        1,
        "a final has no provider entry, got:\n{source}"
    );
}

/// A tuple in either position inside a service's inline shape generates a
/// named struct from the **generated type name**, not from the dotted identity.
///
/// The two names an interface carries are not interchangeable: the identity is
/// a dotted address (`veh.adas.logs`) and is not a Rust identifier, so using it
/// as a struct-name hint produces `Veh.adas.logsGetPairResult` — which is not a
/// name at all. The transport identity in the same generated module must still
/// be the dotted form, so this test pins both halves at once.
///
/// The two positions are not equally reachable from source, on purpose. A tuple
/// **return** is legal ridl and compiles end to end through `ridlc build`. A
/// tuple **parameter** is rejected upstream by FORM-102 ("command parameter
/// must be a named type or a stream"), so it is exercised here at the IR level
/// only. That is deliberate rather than an oversight: this backend's contract
/// is with the IR, not with the surface grammar, and a hint built from the
/// identity would panic on a parameter exactly as it did on a return. Keeping
/// the case means a future grammar relaxation cannot reintroduce the crash
/// silently — do not "simplify" it away because no `.ridl` file can express it
/// today.
#[test]
fn tuples_inside_an_inline_service_use_the_generated_type_name() {
    let get_pair = interaction(
        "getPair",
        1,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![tuple_field("a", "Speed"), tuple_field("b", "Speed")],
                    })),
                })),
            }),
            contracts: Vec::new(),
        }),
    );
    let send_pair = interaction(
        "sendPair",
        2,
        "",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![param(
                "bounds",
                v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![tuple_field("lo", "Speed"), tuple_field("hi", "Speed")],
                    })),
                },
            )],
            contracts: Vec::new(),
        }),
    );
    let fetch = interaction(
        "fetchPage",
        3,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: Vec::new(),
        }),
    );

    let source = generate(&interaction_package(
        Vec::new(),
        Vec::new(),
        vec![service(
            "veh.adas.logs",
            v2::service::Shape::Inline(interface("", "", vec![get_pair, send_pair, fetch])),
        )],
    ))
    .expect("an inline shape carrying tuples generates")
    .rust_source;

    assert!(
        source.contains("pub struct ServiceVehAdasLogsGetPairResult"),
        "a tuple return names its struct from the generated type name, got:\n{source}"
    );
    assert!(
        source.contains("pub struct ServiceVehAdasLogsSendPairBounds"),
        "a tuple parameter names its struct from the generated type name, got:\n{source}"
    );
    // The dotted address is not a Rust identifier; it must never reach a name.
    assert!(
        !source.contains("Veh.adas.logs"),
        "the dotted address must never be used as a name, got:\n{source}"
    );
    // The other half of the split: the identity is still the dotted address.
    assert!(
        source.contains("transport identity: veh.adas.logs#3:FaultPage|DiagError"),
        "the transport identity keeps the dotted address, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// The generated-name classes a package must not collide with.
// ---------------------------------------------------------------------------

/// A typl constant declaration, for the value-namespace collision cases.
fn int_const(name: &str) -> v2::Decl {
    public_decl(
        name,
        v2::decl::Kind::ConstDef(v2::ConstDef {
            type_ref: Some("integer".to_string()),
            value: "5".to_string(),
            regex: None,
        }),
    )
}

/// A typl struct declaration, for the type-namespace collision cases.
fn empty_struct(name: &str) -> v2::Decl {
    public_decl(
        name,
        v2::decl::Kind::StructDef(v2::StructDef {
            members: Vec::new(),
            fixed_layout: false,
        }),
    )
}

fn vehicle_status(interactions: Vec<v2::Decl>) -> v2::Interface {
    interface("VehicleStatus", "", interactions)
}

/// A constant colliding with a generated timing table is refused. Both are
/// `const`, so rustc would reject the module with `error[E0428]`.
#[test]
fn a_const_colliding_with_the_timing_table_is_refused() {
    let error = generate(&interaction_package(
        vec![int_const("VEHICLE_STATUS_TIMING")],
        vec![vehicle_status(Vec::new())],
        Vec::new(),
    ))
    .expect_err("a timing-table collision is refused");
    assert!(
        error.message.contains("VEHICLE_STATUS_TIMING") && error.message.contains("timing table"),
        "got: {}",
        error.message
    );
}

/// The same for the contract table.
#[test]
fn a_const_colliding_with_the_contract_table_is_refused() {
    let error = generate(&interaction_package(
        vec![int_const("VEHICLE_STATUS_CONTRACTS")],
        vec![vehicle_status(Vec::new())],
        Vec::new(),
    ))
    .expect_err("a contract-table collision is refused");
    assert!(
        error.message.contains("contract table"),
        "got: {}",
        error.message
    );
}

/// The same for the service table, which is generated whenever the package
/// declares any service at all.
#[test]
fn a_const_colliding_with_the_service_table_is_refused() {
    let error = generate(&interaction_package(
        vec![int_const("SERVICES")],
        Vec::new(),
        vec![service(
            "veh.adas.cruise",
            v2::service::Shape::InterfaceRef("CruiseControl".to_string()),
        )],
    ))
    .expect_err("a service-table collision is refused");
    assert!(
        error.message.contains("service table"),
        "got: {}",
        error.message
    );
}

/// A declaration colliding with the face of an interface generated for an
/// **inline service shape** is refused.
///
/// This is the case the first version of the check missed: it iterated
/// `package.interfaces`, which is not the complete set of interfaces — an
/// inline shape's interface exists only as a service's payload.
#[test]
fn a_declaration_colliding_with_an_inline_shape_face_is_refused() {
    for suffix in ["Consumer", "Provider"] {
        let error = generate(&interaction_package(
            vec![empty_struct(&format!("ServiceVehAdasLogs{suffix}"))],
            Vec::new(),
            vec![service(
                "veh.adas.logs",
                v2::service::Shape::Inline(interface(
                    "",
                    "",
                    vec![interaction(
                        "ping",
                        1,
                        "",
                        v2::decl::Kind::CommandDef(v2::CommandDef {
                            params: Vec::new(),
                            contracts: Vec::new(),
                        }),
                    )],
                )),
            )],
        ))
        .expect_err("an inline-shape face collision is refused");
        assert!(
            error.message.contains(suffix) && error.message.contains("ServiceVehAdasLogs"),
            "got: {}",
            error.message
        );
    }
}

/// A declaration colliding with a struct generated for a tuple in an
/// interaction position is refused.
#[test]
fn a_declaration_colliding_with_a_generated_tuple_struct_is_refused() {
    let get_pair = interaction(
        "getPair",
        1,
        "",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params: Vec::new(),
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![tuple_field("a", "Speed"), tuple_field("b", "Speed")],
                    })),
                })),
            }),
            contracts: Vec::new(),
        }),
    );
    let error = generate(&interaction_package(
        vec![empty_struct("VehicleStatusGetPairResult")],
        vec![vehicle_status(vec![get_pair])],
        Vec::new(),
    ))
    .expect_err("a generated-tuple collision is refused");
    assert!(
        error.message.contains("VehicleStatusGetPairResult") && error.message.contains("tuple"),
        "got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------------
// What the check must NOT reject: Rust's namespaces genuinely separate these.
// ---------------------------------------------------------------------------

/// A *struct* named `VEHICLE_STATUS_TIMING` does not collide with the *const*
/// of that name: Rust resolves types and values separately. Rejecting it would
/// refuse a contract that compiles.
#[test]
fn a_struct_named_like_a_generated_const_is_admitted() {
    let source = generate(&interaction_package(
        vec![empty_struct("VEHICLE_STATUS_TIMING")],
        vec![vehicle_status(Vec::new())],
        Vec::new(),
    ))
    .expect("a type-namespace name does not collide with a const")
    .rust_source;
    assert!(
        source.contains("struct VEHICLE_STATUS_TIMING"),
        "got:\n{source}"
    );
    assert!(
        source.contains("const VEHICLE_STATUS_TIMING"),
        "got:\n{source}"
    );
}

/// A constant named like a generated face does not collide with it either —
/// the face is a trait, which lives in the type namespace.
#[test]
fn a_const_named_like_a_generated_face_is_admitted() {
    generate(&interaction_package(
        vec![int_const("VehicleStatusConsumer")],
        vec![vehicle_status(Vec::new())],
        Vec::new(),
    ))
    .expect("a value-namespace name does not collide with a trait");
}

/// An interface whose name differs from a declaration only by case is fine:
/// the face name is built through `camel_case`, so `vehicleStatus` generates
/// `VehicleStatusConsumer` and the lower-camel declaration is untouched.
#[test]
fn a_lower_camel_declaration_does_not_collide_with_a_face() {
    generate(&interaction_package(
        vec![empty_struct("vehicleStatusConsumer")],
        vec![interface("vehicleStatus", "", Vec::new())],
        Vec::new(),
    ))
    .expect("camel_case normalization keeps these apart");
}

// ---------------------------------------------------------------------------
// Nested tuple names.
// ---------------------------------------------------------------------------

/// A tuple field whose own type is a tuple, for the nesting cases.
fn nested_tuple_field(name: &str, fields: Vec<v2::TupleField>) -> v2::TupleField {
    v2::TupleField {
        name: name.to_string(),
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType { fields })),
        }),
    }
}

/// `query getPair(): (a: (x: (m: Speed, n: Speed), y: Speed), b: Speed)` — a
/// tuple inside a tuple inside a tuple, so there are two levels below the
/// outermost generated struct. One level would not distinguish the fix from
/// the bug: the first nested level is the one the old check missed, and the
/// second proves the discovery walk recurses rather than peeking once.
fn nested_tuple_interface() -> v2::Interface {
    interface(
        "VehicleStatus",
        "",
        vec![interaction(
            "getPair",
            1,
            "",
            v2::decl::Kind::QueryDef(v2::QueryDef {
                params: Vec::new(),
                return_type: Some(v2::ReturnType {
                    kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                            fields: vec![
                                nested_tuple_field(
                                    "a",
                                    vec![
                                        nested_tuple_field(
                                            "x",
                                            vec![
                                                tuple_field("m", "Speed"),
                                                tuple_field("n", "Speed"),
                                            ],
                                        ),
                                        tuple_field("y", "Speed"),
                                    ],
                                ),
                                tuple_field("b", "Speed"),
                            ],
                        })),
                    })),
                }),
                contracts: Vec::new(),
            }),
        )],
    )
}

/// Every level of a nested tuple generates a struct, and every level is
/// checked for collisions.
///
/// A tuple directly inside another tuple is not discovered while the
/// interaction is walked: it is found by emitting the OUTER tuple's fields,
/// which the caller does after this module returns. The check therefore runs
/// that walk itself first — without it, only the outermost name is known and
/// the nested ones reach rustc as `error[E0428]` with no ridl diagnostic at
/// all.
#[test]
fn a_declaration_colliding_with_a_nested_tuple_struct_is_refused() {
    for name in [
        // One level down: the field `a` of the return tuple.
        "VehicleStatusGetPairResultA",
        // Two levels down: the field `x` of that field's own tuple.
        "VehicleStatusGetPairResultAX",
    ] {
        let error = generate(&interaction_package(
            vec![empty_struct(name)],
            vec![nested_tuple_interface()],
            Vec::new(),
        ))
        .unwrap_err();
        assert!(
            error.message.contains(name) && error.message.contains("tuple"),
            "a nested tuple name must be refused, got: {}",
            error.message
        );
    }
}

/// The same package without the colliding declarations still generates every
/// level, so the check found those names rather than inventing them.
#[test]
fn nested_tuple_structs_are_all_generated() {
    let source = generate(&interaction_package(
        Vec::new(),
        vec![nested_tuple_interface()],
        Vec::new(),
    ))
    .expect("a nested tuple return generates")
    .rust_source;
    for name in [
        "struct VehicleStatusGetPairResult ",
        "struct VehicleStatusGetPairResultA ",
        "struct VehicleStatusGetPairResultAX",
    ] {
        assert!(
            source.contains(name.trim_end()),
            "expected {name}, got:\n{source}"
        );
    }
}

// ---------------------------------------------------------------------------
// Generated names colliding with each other.
// ---------------------------------------------------------------------------

/// Two interfaces whose names differ only in the first letter's case generate
/// one type name.
///
/// `camel_case` maps `vehicleStatus` and `VehicleStatus` to `VehicleStatus`,
/// so both faces and both consts are declared twice — four `error[E0428]`s and
/// not one ridl diagnostic. typl does not rule the pair out, so this backend
/// has to.
#[test]
fn two_interfaces_normalizing_to_one_type_name_are_refused() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![
            interface("vehicleStatus", "", Vec::new()),
            interface("VehicleStatus", "", Vec::new()),
        ],
        Vec::new(),
    ))
    .expect_err("two interfaces cannot claim one generated name");
    assert!(
        error.message.contains("claimed by both")
            && error.message.contains("interface vehicleStatus")
            && error.message.contains("interface VehicleStatus"),
        "the refusal names both claimants, got: {}",
        error.message
    );
}

/// A declared interface and an inline service shape can arrive at the same
/// generated name: `interface ServiceVehAdasLogs` and the interface generated
/// for `service veh.adas.logs` are both `ServiceVehAdasLogs`.
#[test]
fn an_interface_colliding_with_an_inline_shape_name_is_refused() {
    let error = generate(&interaction_package(
        Vec::new(),
        vec![interface("ServiceVehAdasLogs", "", Vec::new())],
        vec![service(
            "veh.adas.logs",
            v2::service::Shape::Inline(interface(
                "",
                "",
                vec![interaction(
                    "ping",
                    1,
                    "",
                    v2::decl::Kind::CommandDef(v2::CommandDef {
                        params: Vec::new(),
                        contracts: Vec::new(),
                    }),
                )],
            )),
        )],
    ))
    .expect_err("an interface and an inline shape cannot claim one name");
    assert!(
        error.message.contains("claimed by both")
            && error.message.contains("interface ServiceVehAdasLogs")
            && error
                .message
                .contains("inline shape of service veh.adas.logs"),
        "the refusal names both claimants, got: {}",
        error.message
    );
}

/// Two services with distinct addresses that mangle to one type name are
/// refused for the same reason.
#[test]
fn two_inline_shapes_normalizing_to_one_type_name_are_refused() {
    let shape = |address: &str| {
        service(
            address,
            v2::service::Shape::Inline(interface(
                "",
                "",
                vec![interaction(
                    "ping",
                    1,
                    "",
                    v2::decl::Kind::CommandDef(v2::CommandDef {
                        params: Vec::new(),
                        contracts: Vec::new(),
                    }),
                )],
            )),
        )
    };
    let error = generate(&interaction_package(
        Vec::new(),
        Vec::new(),
        // `veh.adas.logs` and `veh.adas.Logs` both mangle to
        // `ServiceVehAdasLogs`.
        vec![shape("veh.adas.logs"), shape("veh.adas.Logs")],
    ))
    .expect_err("two inline shapes cannot claim one name");
    assert!(
        error.message.contains("claimed by both"),
        "got: {}",
        error.message
    );
}

/// Interfaces that merely share a prefix are untouched — the check refuses a
/// genuine duplicate, not a resemblance.
#[test]
fn distinct_interface_names_are_admitted() {
    generate(&interaction_package(
        Vec::new(),
        vec![
            interface("VehicleStatus", "", Vec::new()),
            interface("VehicleStatusExtended", "", Vec::new()),
        ],
        Vec::new(),
    ))
    .expect("distinct generated names do not collide");
}

// ---------------------------------------------------------------------------
// `internal` visibility on the interaction layer (ADR-0008 decision 7).
// ---------------------------------------------------------------------------

/// An inline service shape as the CHECKER emits it: an anonymous interface
/// (`name == ""`) whose visibility is `VISIBILITY_UNSPECIFIED`, not public.
///
/// This helper exists because [`interface`] hardcodes `Visibility::Public`,
/// which no inline shape ever carries: `Checker::lower_service_inline` leaves
/// the field unset, because an anonymous shape is not addressable as a type
/// and has no visibility of its own — the enclosing `Service` carries the
/// authoritative one, and a service takes no `internal` modifier (ridl §14.5).
/// A fixture that says `Public` here would be testing a value the real
/// producer never emits, and the UNSPECIFIED default arm of [`vis_tokens`]
/// would go unexercised.
fn inline_shape(interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        visibility: v2::Visibility::Unspecified as i32,
        ..interface("", "", interactions)
    }
}

/// The package both visibility tests read. Three shapes side by side, so the
/// rule is exercised per declaration rather than per module:
///
/// 1. `internal interface Hidden` — every generated name package-private;
/// 2. public `interface Shown` — every generated name unchanged, the
///    regression direction;
/// 3. an **inline service shape** whose `Interface.visibility` is
///    `VISIBILITY_UNSPECIFIED` ([`inline_shape`]) — every generated name
///    public, which is what makes the "no special case for inline shapes"
///    reasoning true rather than merely asserted.
///
/// Each of the three carries a timed signal and a contract-bearing query, so
/// all four generated names of all three are present and non-empty. An empty
/// constant is exactly how the contracts const was overlooked in the first
/// place.
fn visibility_package() -> v2::Package {
    let hidden = v2::Interface {
        visibility: v2::Visibility::Internal as i32,
        ..interface(
            "Hidden",
            "",
            vec![
                interaction(
                    "rawTicks",
                    1,
                    "",
                    v2::decl::Kind::SignalDef(v2::SignalDef {
                        payload: "Counter".to_string(),
                        declared_init: None,
                        init: Some(init_value(true, Some("0"))),
                        timing: Some(timing(
                            v2::TimingMode::StrictPeriodic,
                            Some("10000"),
                            Some("10000"),
                        )),
                    }),
                ),
                interaction(
                    "getBounds",
                    2,
                    "",
                    v2::decl::Kind::QueryDef(v2::QueryDef {
                        params: Vec::new(),
                        return_type: Some(v2::ReturnType {
                            kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                                optional: false,
                                kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                                    fields: vec![
                                        tuple_field("min", "Counter"),
                                        tuple_field("max", "Counter"),
                                    ],
                                })),
                            })),
                        }),
                        contracts: vec![contract(
                            v2::ContractKind::Ensure,
                            "result.min <= result.max",
                            &[],
                            &[],
                            true,
                            "Hidden.getBounds.ensure[0]",
                        )],
                    }),
                ),
            ],
        )
    };
    // A signal, a contract-bearing query, and a tuple-returning query, so both
    // metadata constants of the interface built from them are non-empty AND the
    // shape induces a tuple struct of its own. The tuple return is what makes
    // the public direction of issue #167 observable: without it, `Shown` and
    // the inline shape generate no induced struct at all and an emitter that
    // made every one of them `pub(crate)` would go unnoticed.
    let pair = |owner: &str, ensure_id: &str| {
        vec![
            interaction(
                "cabinTemp",
                1,
                "",
                v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Counter".to_string(),
                    declared_init: None,
                    init: Some(init_value(true, Some("0"))),
                    timing: Some(timing(v2::TimingMode::Range, Some("50000"), Some("500000"))),
                }),
            ),
            interaction(
                "readSummary",
                2,
                "",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: Vec::new(),
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(named_type("Counter"))),
                    }),
                    contracts: vec![contract(
                        v2::ContractKind::Ensure,
                        &format!("result >= {owner}Floor"),
                        &[],
                        &[],
                        true,
                        ensure_id,
                    )],
                }),
            ),
            interaction(
                "getBounds",
                3,
                "",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: Vec::new(),
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                                fields: vec![
                                    tuple_field("min", "Counter"),
                                    tuple_field("max", "Counter"),
                                ],
                            })),
                        })),
                    }),
                    contracts: Vec::new(),
                }),
            ),
        ]
    };
    let shown = interface("Shown", "", pair("Shown", "Shown.readSummary.ensure[0]"));
    interaction_package(
        vec![public_decl(
            "Counter",
            primitive_type(
                v2::PrimitiveType::Integer,
                init_value(true, Some("0")),
                Some(v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32)),
            ),
        )],
        vec![hidden, shown],
        vec![
            service(
                "veh.cluster.hidden",
                v2::service::Shape::InterfaceRef("Hidden".to_string()),
            ),
            service(
                "veh.cluster.diag",
                v2::service::Shape::Inline(inline_shape(pair(
                    "Diag",
                    "veh.cluster.diag.readSummary.ensure[0]",
                ))),
            ),
        ],
    )
}

/// An **`internal` service's inline shape generates package-private items** —
/// the authoritative visibility is the enclosing `Service`'s, never the shape's
/// own `VISIBILITY_UNSPECIFIED` field (ridl §14.5,
/// `crates/ridlc/tests/corpus/veh-cluster/NOTES`).
///
/// This is the one assertion that separates `InterfaceShape::visibility()` from
/// the field it replaced. Every other visibility case agrees by accident:
/// [`vis_tokens`](crate::vis_tokens) maps both `UNSPECIFIED` and `PUBLIC` to
/// `pub`, so reading the inline shape's own field and reading the service's
/// produce identical output for a public service — which is every service the
/// checker can currently emit, since `internal service` is FORM-102 from
/// source.
///
/// Unreachable through the checker is not unreachable in principle, and the
/// gap is not hypothetical: the IR admits an internal service, `ridl diff`
/// already reasons about one (`classify_tests::service_inline` builds exactly
/// this package and `find_visibility` classifies internal-to-public on it as a
/// widening), and any producer that is not the checker — a hand-written
/// `.ir.json`, a registry, the rsdl lowering E3 adds — reaches it immediately.
/// Under the old read the whole generated API of such a service is published.
///
/// It also guards a tidy-up. `vis_tokens`'s `_ => pub` catch-all silently
/// equates the proto default with `PUBLIC`; making that match exhaustive, which
/// is the discipline ADR-0008 decision 21 argues for elsewhere, would break
/// every inline shape under the old read. This test is what turns that from a
/// silent breakage into a red one.
#[test]
fn an_internal_services_inline_shape_is_package_private() {
    let mut package = visibility_package();
    let diag = package
        .services
        .iter_mut()
        .find(|service| service.name == "veh.cluster.diag")
        .expect("the shared fixture declares the inline-shape service");
    assert!(
        matches!(diag.shape, Some(v2::service::Shape::Inline(_))),
        "this test is only meaningful on an INLINE shape",
    );
    diag.visibility = v2::Visibility::Internal as i32;

    let Generated { rust_source, .. } = generate(&package).expect("the package generates");

    // The struct the inline shape's tuple return induces is in this list on
    // purpose: it is the one name whose visibility can only be derived from the
    // owning `Service`, since a tuple carries no visibility field of its own AND
    // the shape's is UNSPECIFIED. It is where `InterfaceShape::visibility()` and
    // the field it replaced disagree twice over.
    for item in [
        "pub(crate) trait ServiceVehClusterDiagConsumer",
        "pub(crate) trait ServiceVehClusterDiagProvider",
        "pub(crate) const SERVICE_VEH_CLUSTER_DIAG_TIMING",
        "pub(crate) const SERVICE_VEH_CLUSTER_DIAG_CONTRACTS",
        "pub(crate) struct ServiceVehClusterDiagGetBoundsResult",
    ] {
        assert!(
            rust_source.contains(item),
            "an internal service's inline shape must emit `{item}`, got:\n{rust_source}"
        );
    }
    // `pub(crate) trait X` does not contain `pub trait X`, so these are
    // genuine negatives rather than restatements of the loop above.
    for leaked in [
        "pub trait ServiceVehClusterDiagConsumer",
        "pub trait ServiceVehClusterDiagProvider",
        "pub const SERVICE_VEH_CLUSTER_DIAG_TIMING",
        "pub const SERVICE_VEH_CLUSTER_DIAG_CONTRACTS",
        "pub struct ServiceVehClusterDiagGetBoundsResult",
    ] {
        assert!(
            !rust_source.contains(leaked),
            "an internal service's inline shape must not emit `{leaked}`, got:\n{rust_source}"
        );
    }

    // The regression direction, in the same module: the public interface and
    // the package-level service table are untouched.
    for item in ["pub trait ShownConsumer", "pub const SERVICES"] {
        assert!(
            rust_source.contains(item),
            "`{item}` is unaffected by a service's visibility, got:\n{rust_source}"
        );
    }
}

/// Every name an `internal interface` generates is `pub(crate)` — **five**
/// names, not four: the two faces, the two metadata constants, and the struct
/// a query's tuple return induces (issue #167). Every name a public interface
/// generates stays `pub`; every name an **inline service shape** generates
/// stays `pub`; and the genuinely package-level names — the interaction
/// vocabulary and the service table — stay `pub` regardless, because they are
/// emitted once per module and belong to no single declaration.
///
/// The three shapes sit in one package on purpose: `internal` is a property of
/// the declaration, so a module holding all three must generate all three
/// spellings.
#[test]
fn internal_interface_generates_package_private_items() {
    let Generated { rust_source, .. } =
        generate(&visibility_package()).expect("the package generates");

    // The five names `Hidden` generates, all package-private. The tuple struct
    // is the fifth: `getBounds` returns `(min : Counter, max : Counter)`, and
    // the struct that return induces belongs to the same `internal`
    // declaration as the four names above it.
    for item in [
        "pub(crate) trait HiddenConsumer",
        "pub(crate) trait HiddenProvider",
        "pub(crate) const HIDDEN_TIMING",
        "pub(crate) const HIDDEN_CONTRACTS",
        "pub(crate) struct HiddenGetBoundsResult",
    ] {
        assert!(
            rust_source.contains(item),
            "an internal interface must emit `{item}`, got:\n{rust_source}"
        );
    }
    // ... and none of them under a `pub` spelling. `pub(crate) trait X` does
    // not contain `pub trait X`, so these are genuine negatives.
    for leaked in [
        "pub trait HiddenConsumer",
        "pub trait HiddenProvider",
        "pub const HIDDEN_TIMING",
        "pub const HIDDEN_CONTRACTS",
        "pub struct HiddenGetBoundsResult",
    ] {
        assert!(
            !rust_source.contains(leaked),
            "an internal interface must not emit `{leaked}`, got:\n{rust_source}"
        );
    }

    // The regression direction: a public interface in the same module is
    // untouched.
    for item in [
        "pub trait ShownConsumer",
        "pub trait ShownProvider",
        "pub const SHOWN_TIMING",
        "pub const SHOWN_CONTRACTS",
    ] {
        assert!(
            rust_source.contains(item),
            "a public interface must still emit `{item}`, got:\n{rust_source}"
        );
    }

    // An inline service shape carries `VISIBILITY_UNSPECIFIED`, which is not a
    // missing value to be guessed at: an anonymous shape has no visibility of
    // its own and the enclosing service — which cannot be `internal` at all
    // (ridl §14.5) — carries the authoritative one. So all four of its names
    // are public. This is asserted rather than left implicit because it is the
    // link that makes "no special case for inline shapes" true.
    for item in [
        "pub trait ServiceVehClusterDiagConsumer",
        "pub trait ServiceVehClusterDiagProvider",
        "pub const SERVICE_VEH_CLUSTER_DIAG_TIMING",
        "pub const SERVICE_VEH_CLUSTER_DIAG_CONTRACTS",
        "pub struct ServiceVehClusterDiagGetBoundsResult",
    ] {
        assert!(
            rust_source.contains(item),
            "an inline service shape is public, so it must emit `{item}`, got:\n{rust_source}"
        );
    }

    // The names that are deliberately NOT affected, and the reason is the same
    // for all of them: they belong to no single declaration. The vocabulary is
    // emitted once per module and is shared by every interface in it; the
    // service table is the package's published deployment surface, and a
    // service takes no `internal` modifier (ridl §14.5). A tuple struct is not
    // in this list — it is named after, and induced by, exactly one
    // declaration, which is what issue #167 turned on.
    for item in [
        "pub enum Provenance",
        "pub trait SignalHandle<T>",
        "pub struct TimingConst",
        "pub struct ContractStub",
        "pub const SERVICES",
    ] {
        assert!(
            rust_source.contains(item),
            "`{item}` is package-level and stays public, got:\n{rust_source}"
        );
    }

    // The regression direction for the tuple struct itself: the same shape
    // under the PUBLIC interface still generates a `pub` struct. Without this,
    // an emitter that made every induced struct `pub(crate)` would pass the
    // assertions above.
    assert!(
        rust_source.contains("pub struct ShownGetBoundsResult"),
        "a public interface's tuple return must still induce a `pub` struct, \
         got:\n{rust_source}"
    );

    insta::assert_snapshot!(rust_source);
}

/// `pub(crate)` is not cosmetic: the generated module compiles, and the items
/// an `internal interface` produces are genuinely unreachable from another
/// crate while the public interface's are reachable.
///
/// A snapshot alone cannot show this — it records the spelling, not what the
/// spelling means to rustc. So the module is compiled as a real library crate
/// and dependent crates are compiled against it, one per generated name:
/// **all five** of `Hidden` must fail with E0603 (`private`), and all five of
/// `Shown` plus all five of the inline shape must build. Every name is probed
/// rather than a representative one, because a partial fix leaves exactly the
/// unprobed name public — which is what issue #167 was: four of the five names
/// were probed here and the fifth, the struct a tuple return induces, was not.
/// Both directions are asserted, because a test that only checked the failures
/// would also pass if the whole module failed to build.
#[test]
fn internal_interface_items_are_unreachable_from_another_crate() {
    let Generated { rust_source, .. } =
        generate(&visibility_package()).expect("the package generates");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let lib_path = dir.path().join("veh_cluster.rs");
    std::fs::write(&lib_path, &rust_source).expect("the generated source is written");

    let rlib = dir.path().join("libveh_cluster.rlib");
    let status = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "lib"])
        .args(["--crate-name", "veh_cluster"])
        // The generated module is a contract, not a consumer of itself:
        // `pub(crate)` items nothing in the crate uses are dead code by
        // construction, which is the point of the test rather than a defect.
        .args(["-A", "dead_code"])
        .arg("-o")
        .arg(&rlib)
        .arg(&lib_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the generated module must compile as a library crate, source:\n{rust_source}"
    );

    // Compiles a one-line crate that names `item` through `veh_cluster`, and
    // returns rustc's stderr together with whether it built.
    let probe = |name: &str, item: &str| -> (bool, String) {
        let path = dir.path().join(format!("{name}.rs"));
        std::fs::write(&path, item).expect("the probe is written");
        let output = std::process::Command::new("rustc")
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "lib",
                "--emit",
                "metadata",
            ])
            .arg("--extern")
            .arg(format!("veh_cluster={}", rlib.display()))
            .arg("-o")
            .arg(dir.path().join(format!("{name}.rmeta")))
            .arg(&path)
            .output()
            .expect("rustc runs");
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };

    // A trait is named in a bound, a const in an expression, and a struct in a
    // parameter type, so each name is used the only way its namespace admits.
    let names_trait = |item: &str| format!("pub fn probe<T: veh_cluster::{item}>(_: T) {{}}\n");
    let names_const =
        |item: &str| format!("pub fn probe() -> usize {{ veh_cluster::{item}.len() }}\n");
    let names_type = |item: &str| format!("pub fn probe(_: veh_cluster::{item}) {{}}\n");

    // Reachable: the public interface, and the inline service shape — whose
    // `Interface.visibility` is UNSPECIFIED, the value the checker actually
    // emits for an anonymous shape.
    for (probe_name, what, source) in [
        (
            "reaches_shown_consumer",
            "a public interface's consumer face",
            names_trait("ShownConsumer"),
        ),
        (
            "reaches_shown_provider",
            "a public interface's provider face",
            names_trait("ShownProvider"),
        ),
        (
            "reaches_shown_timing",
            "a public interface's timing const",
            names_const("SHOWN_TIMING"),
        ),
        (
            "reaches_shown_contracts",
            "a public interface's contract const",
            names_const("SHOWN_CONTRACTS"),
        ),
        (
            "reaches_inline_consumer",
            "an inline service shape's consumer face",
            names_trait("ServiceVehClusterDiagConsumer"),
        ),
        (
            "reaches_inline_provider",
            "an inline service shape's provider face",
            names_trait("ServiceVehClusterDiagProvider"),
        ),
        (
            "reaches_inline_timing",
            "an inline service shape's timing const",
            names_const("SERVICE_VEH_CLUSTER_DIAG_TIMING"),
        ),
        (
            "reaches_inline_contracts",
            "an inline service shape's contract const",
            names_const("SERVICE_VEH_CLUSTER_DIAG_CONTRACTS"),
        ),
        (
            "reaches_shown_tuple",
            "a public interface's induced tuple struct",
            names_type("ShownGetBoundsResult"),
        ),
        (
            "reaches_inline_tuple",
            "an inline service shape's induced tuple struct",
            names_type("ServiceVehClusterDiagGetBoundsResult"),
        ),
    ] {
        let (ok, err) = probe(probe_name, &source);
        assert!(
            ok,
            "{what} must stay reachable from another crate, rustc said:\n{err}"
        );
    }

    // Unreachable: every one of the five names the internal interface
    // generates, each refused specifically as a privacy error rather than as
    // any error at all.
    for (probe_name, what, source) in [
        (
            "hides_consumer",
            "consumer face",
            names_trait("HiddenConsumer"),
        ),
        (
            "hides_provider",
            "provider face",
            names_trait("HiddenProvider"),
        ),
        ("hides_timing", "timing const", names_const("HIDDEN_TIMING")),
        (
            "hides_contracts",
            "contract const",
            names_const("HIDDEN_CONTRACTS"),
        ),
        (
            "hides_tuple",
            "induced tuple struct",
            names_type("HiddenGetBoundsResult"),
        ),
    ] {
        let (ok, err) = probe(probe_name, &source);
        assert!(
            !ok,
            "an internal interface's {what} must not be reachable from another crate"
        );
        assert!(
            err.contains("E0603"),
            "the refusal for the {what} must be a privacy error (E0603), rustc said:\n{err}"
        );
    }
}
