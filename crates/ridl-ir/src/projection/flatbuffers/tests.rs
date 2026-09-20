//! The facts' own tests. That these facts agree with the `.fbs` schema
//! `ridl-backend-flatbuffers` emits is the drift test, which lives in that
//! crate because only there is there a schema to compare against.

use super::*;

fn field(name: &str, ordinal: u32, ty: v2::FieldType) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(ty),
            ..Default::default()
        })),
    }
}

fn retired(ordinal: u32) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
            ordinal,
            ..Default::default()
        })),
    }
}

fn primitive(primitive: v2::PrimitiveType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Primitive(primitive as i32)),
    }
}

fn named(reference: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(reference.to_string())),
    }
}

fn decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        kind: Some(kind),
        ..Default::default()
    }
}

fn package(name: &str, decls: Vec<v2::Decl>) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        decls,
        ..Default::default()
    }
}

/// A named string scalar bounded to `characters`.
fn string_type(characters: u64) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            len_max: Some(characters),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// A named bytes scalar bounded to `length` bytes.
fn bytes_type(length: u64) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::Bytes as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            len_max: Some(length),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn int_type(width: v2::IntWidth) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::Integer as i32,
            )),
        }),
        width: Some(v2::type_def::Width::IntWidth(width as i32)),
        ..Default::default()
    })
}

fn holder_of(field_type: v2::FieldType) -> v2::Decl {
    decl(
        "Holder",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field("value", 1, field_type)],
            ..Default::default()
        }),
    )
}

fn bound(packages: &[&v2::Package], name: &str) -> Option<u64> {
    let home = packages[0];
    let decl = home
        .decls
        .iter()
        .find(|decl| decl.name == name)
        .expect("the fixture declares it");
    max_size(
        Packages {
            package: home,
            others: &packages[1..],
        },
        decl,
    )
}

// --- the layouts ------------------------------------------------------

#[test]
fn a_struct_slot_is_its_ordinal_minus_one_and_a_tombstone_keeps_its_slot() {
    let def = v2::StructDef {
        members: vec![
            field("desired", 1, primitive(v2::PrimitiveType::Float)),
            field("trim", 3, primitive(v2::PrimitiveType::Integer)),
            retired(4),
        ],
        ..Default::default()
    };

    let layout = struct_table("Setpoint", &def).expect("layout");

    assert_eq!(
        layout.slots,
        vec![
            FieldSlot {
                id: 0,
                source: SlotSource::Field {
                    name: "desired".to_string(),
                    ordinal: 1
                }
            },
            FieldSlot {
                id: 2,
                source: SlotSource::Field {
                    name: "trim".to_string(),
                    ordinal: 3
                }
            },
            FieldSlot {
                id: 3,
                source: SlotSource::Retired { ordinal: 4 }
            },
        ]
    );
    // A gap in the ids is still covered by the vtable, which runs from 0 to
    // the highest id used.
    assert_eq!(layout.vtable_slots(), 4);
}

#[test]
fn ordinal_zero_is_refused_rather_than_underflowed() {
    let def = v2::StructDef {
        members: vec![field("bad", 0, primitive(v2::PrimitiveType::Boolean))],
        ..Default::default()
    };

    let error = struct_table("Broken", &def).expect_err("ordinal 0 is refused");

    assert!(error.message.contains("ordinal 0"), "{}", error.message);
}

#[test]
fn the_generated_tables_take_the_slots_adr_0019_fixes() {
    // A map entry is `key` then `value`; a union's wrapper leaves id 0 to the
    // implicit discriminant; a union arm's box takes id 0 itself.
    let ids = |layout: TableLayout| layout.slots.iter().map(|slot| slot.id).collect::<Vec<_>>();

    assert_eq!(ids(map_entry_table()), vec![0, 1]);
    assert_eq!(ids(union_wrapper_table()), vec![1]);
    assert_eq!(union_wrapper_table().vtable_slots(), 2);
    assert_eq!(ids(union_arm_box_table()), vec![0]);
}

#[test]
fn a_tuple_table_is_positional_from_zero() {
    let tuple = v2::TupleType {
        fields: vec![
            v2::TupleField {
                name: "min".to_string(),
                r#type: Some(primitive(v2::PrimitiveType::Float)),
            },
            v2::TupleField {
                name: "max".to_string(),
                r#type: Some(primitive(v2::PrimitiveType::Float)),
            },
        ],
    };

    let layout = tuple_table("TelemetryBounds", &tuple).expect("layout");

    assert_eq!(
        layout.slots,
        vec![
            FieldSlot {
                id: 0,
                source: SlotSource::TupleField { position: 1 }
            },
            FieldSlot {
                id: 1,
                source: SlotSource::TupleField { position: 2 }
            },
        ]
    );
}

#[test]
fn a_union_arms_discriminant_is_its_ordinal_and_a_tombstone_keeps_it_occupied() {
    // The shape driftsys/ridl#302 is about: the first arm is retired, so the
    // live arms carry ordinals 2 and 3 while the schema's implicit numbering
    // gives them 1 and 2.
    let def = v2::UnionDef {
        arms: vec![
            v2::UnionArm {
                name: "engage".to_string(),
                ordinal: 2,
                type_ref: "Setpoint".to_string(),
                ..Default::default()
            },
            v2::UnionArm {
                name: "disengage".to_string(),
                ordinal: 3,
                type_ref: "Percent".to_string(),
                ..Default::default()
            },
        ],
        reserved: vec![v2::Reserved {
            ordinal: 1,
            ..Default::default()
        }],
        ..Default::default()
    };

    let discriminants: Vec<u32> = def.arms.iter().map(union_arm_discriminant).collect();

    assert_eq!(discriminants, vec![2, 3]);
}

#[test]
fn an_enum_with_no_zero_member_needs_the_null_default() {
    let with_zero = v2::EnumDef {
        values: vec![v2::EnumValue {
            name: "OFF".to_string(),
            value: 0,
            ..Default::default()
        }],
        ..Default::default()
    };
    let without_zero = v2::EnumDef {
        values: vec![v2::EnumValue {
            name: "ARMED".to_string(),
            value: 1,
            ..Default::default()
        }],
        ..Default::default()
    };

    assert!(!enum_field_needs_null_default(&with_zero));
    assert!(enum_field_needs_null_default(&without_zero));
}

// --- the bound --------------------------------------------------------

#[test]
fn only_a_struct_or_a_union_mints_a_root_table() {
    assert!(mints_root_table(&decl(
        "Setpoint",
        v2::decl::Kind::StructDef(v2::StructDef::default())
    )));
    assert!(mints_root_table(&decl(
        "Command",
        v2::decl::Kind::UnionDef(v2::UnionDef::default())
    )));
    assert!(!mints_root_table(&decl(
        "Percent",
        int_type(v2::IntWidth::U8)
    )));
}

#[test]
fn a_scalar_struct_is_bounded_by_its_own_table() {
    let pkg = package(
        "veh.cruise",
        vec![decl(
            "Flags",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field("on", 1, primitive(v2::PrimitiveType::Boolean))],
                ..Default::default()
            }),
        )],
    );

    // root (4 + 7) + table: soffset 4, one inline byte, slack for the one
    // slot and for the soffset, and a vtable of 4 header bytes, 2 for the one
    // slot, and its own slack.
    assert_eq!(
        bound(&[&pkg], "Flags"),
        Some(ROOT + OFFSET + 1 + ALIGN_SLACK * 2 + (VTABLE_HEADER + VTABLE_SLOT + ALIGN_SLACK))
    );
}

#[test]
fn a_wider_member_costs_more_than_a_narrower_one() {
    let narrow = package(
        "veh.cruise",
        vec![
            decl("Small", int_type(v2::IntWidth::U8)),
            holder_of(named("Small")),
        ],
    );
    let wide = package(
        "veh.cruise",
        vec![
            decl("Small", int_type(v2::IntWidth::U64)),
            holder_of(named("Small")),
        ],
    );

    assert_eq!(
        bound(&[&wide], "Holder").unwrap() - bound(&[&narrow], "Holder").unwrap(),
        7
    );
}

#[test]
fn a_string_is_charged_four_bytes_a_character() {
    let eight = package(
        "veh.cruise",
        vec![decl("Label", string_type(8)), holder_of(named("Label"))],
    );
    let four = package(
        "veh.cruise",
        vec![decl("Label", string_type(4)), holder_of(named("Label"))],
    );

    // Four more characters, at the four bytes UTF-8 can spend on one.
    assert_eq!(
        bound(&[&eight], "Holder").unwrap() - bound(&[&four], "Holder").unwrap(),
        16
    );
}

#[test]
fn an_array_charges_its_maximum_element_count() {
    let holder = |max: u64| {
        package(
            "veh.cruise",
            vec![holder_of(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(primitive(v2::PrimitiveType::Boolean))),
                    min: 0,
                    max,
                }))),
            })],
        )
    };

    let eight = holder(8);
    let four = holder(4);
    assert_eq!(
        bound(&[&eight], "Holder").unwrap() - bound(&[&four], "Holder").unwrap(),
        4
    );
}

#[test]
fn a_union_is_bounded_by_its_largest_arm() {
    let pkg = package(
        "veh.cruise",
        vec![
            decl("Percent", int_type(v2::IntWidth::U8)),
            decl(
                "Setpoint",
                v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![
                        field("a", 1, primitive(v2::PrimitiveType::Float)),
                        field("b", 2, primitive(v2::PrimitiveType::Float)),
                    ],
                    ..Default::default()
                }),
            ),
            decl(
                "Command",
                v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![
                        v2::UnionArm {
                            name: "engage".to_string(),
                            ordinal: 1,
                            type_ref: "Setpoint".to_string(),
                            ..Default::default()
                        },
                        v2::UnionArm {
                            name: "disengage".to_string(),
                            ordinal: 2,
                            type_ref: "Percent".to_string(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
            ),
        ],
    );

    let union_bound = bound(&[&pkg], "Command").expect("bounded");
    let setpoint_bound = bound(&[&pkg], "Setpoint").expect("bounded");

    // The union's buffer is its wrapper table plus the larger arm's table.
    assert!(
        union_bound > setpoint_bound,
        "{union_bound} vs {setpoint_bound}"
    );
}

#[test]
fn an_arm_that_is_not_a_table_pays_for_its_box() {
    let pkg = package(
        "veh.cruise",
        vec![
            decl("Percent", int_type(v2::IntWidth::U8)),
            decl(
                "Command",
                v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![v2::UnionArm {
                        name: "disengage".to_string(),
                        ordinal: 1,
                        type_ref: "Percent".to_string(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            ),
        ],
    );

    // The wrapper alone: root, its soffset, the discriminant byte and the
    // union offset inline, slack for its two slots and its soffset, and a
    // two-slot vtable.
    let wrapper_only = ROOT
        + OFFSET
        + (1 + OFFSET)
        + ALIGN_SLACK * 3
        + (VTABLE_HEADER + 2 * VTABLE_SLOT + ALIGN_SLACK);
    // What the box adds: a whole table of its own, holding one inline byte.
    assert_eq!(
        bound(&[&pkg], "Command").expect("bounded") - wrapper_only,
        OFFSET + 1 + ALIGN_SLACK * 2 + (VTABLE_HEADER + VTABLE_SLOT + ALIGN_SLACK)
    );
}

#[test]
fn a_foreign_reference_is_followed_into_the_package_that_declares_it() {
    let parts = package(
        "proto.parts",
        vec![decl("Percent", int_type(v2::IntWidth::U8))],
    );
    let vehicle = package(
        "proto.vehicle",
        vec![holder_of(named("proto.parts.Percent"))],
    );

    assert!(bound(&[&vehicle, &parts], "Holder").is_some());
    // Without the other package the reference does not resolve, and an
    // underivable bound is `None` rather than a guess.
    assert_eq!(bound(&[&vehicle], "Holder"), None);
}

#[test]
fn a_bare_string_map_key_has_no_bound() {
    // Not reachable from typl source — §15.3 refuses a bare `string` at a
    // field position (TYPL-208) and a map key takes §4.4–§4.5's `[0..256]`
    // default (driftsys/ridl#459) — but IR handed in directly can carry it,
    // and a string with no length is the one shape there is nothing to
    // charge for.
    let pkg = package(
        "veh.cruise",
        vec![holder_of(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(primitive(v2::PrimitiveType::String))),
                value: Some(Box::new(primitive(v2::PrimitiveType::Boolean))),
                min: 0,
                max: 8,
            }))),
        })],
    );

    assert_eq!(bound(&[&pkg], "Holder"), None);
}

#[test]
fn a_bounded_map_key_is_bounded() {
    let pkg = package(
        "veh.cruise",
        vec![
            decl("Label", string_type(8)),
            holder_of(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(named("Label"))),
                    value: Some(Box::new(primitive(v2::PrimitiveType::Boolean))),
                    min: 0,
                    max: 8,
                }))),
            }),
        ],
    );

    assert!(bound(&[&pkg], "Holder").is_some());
}

#[test]
fn a_type_that_reaches_itself_has_no_bound() {
    // TYPL-206 rejects a recursive composite, so this is totality over IR
    // handed in directly rather than a case a typl author meets.
    let pkg = package(
        "veh.cruise",
        vec![
            decl(
                "Left",
                v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field("right", 1, named("Right"))],
                    ..Default::default()
                }),
            ),
            decl(
                "Right",
                v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field("left", 1, named("Left"))],
                    ..Default::default()
                }),
            ),
        ],
    );

    assert_eq!(bound(&[&pkg], "Left"), None);
}

#[test]
fn an_overflowing_count_has_no_bound() {
    let pkg = package(
        "veh.cruise",
        vec![holder_of(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(primitive(v2::PrimitiveType::Integer))),
                min: 0,
                max: u64::MAX,
            }))),
        })],
    );

    assert_eq!(bound(&[&pkg], "Holder"), None);
}

#[test]
fn a_table_charges_alignment_slack_for_every_slot() {
    // The reason it is per slot: a builder that writes a table's fields in
    // declaration order pre-aligns again at every widening, and nothing here
    // obliges it to write them in non-increasing alignment order. `u8, u64,
    // u8, u64` is the shape that breaks a bound charging slack once per
    // table.
    let alternating = package(
        "veh.cruise",
        vec![
            decl("Narrow", int_type(v2::IntWidth::U8)),
            decl("Wide", int_type(v2::IntWidth::U64)),
            decl(
                "Holder",
                v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![
                        field("a", 1, named("Narrow")),
                        field("b", 2, named("Wide")),
                        field("c", 3, named("Narrow")),
                        field("d", 4, named("Wide")),
                    ],
                    ..Default::default()
                }),
            ),
        ],
    );

    let charged = bound(&[&alternating], "Holder").expect("bounded");
    // What a declaration-order builder spends at worst: the root offset and
    // the buffer's own alignment, the soffset and its alignment, the 18
    // inline bytes, one pre-align of up to seven before each of the four
    // fields, and the vtable with the one byte a 2-aligned object can waste.
    let worst_case = OFFSET
        + ALIGN_SLACK
        + OFFSET
        + ALIGN_SLACK
        + 18
        + 4 * ALIGN_SLACK
        + (VTABLE_HEADER + 4 * VTABLE_SLOT + 1);
    assert!(
        charged >= worst_case,
        "the bound {charged} must cover the worst write order, which costs {worst_case}"
    );
}

#[test]
fn a_shared_type_is_walked_once() {
    // A diamond: every level names the one below it twice. TYPL-206 rejects a
    // cycle, not sharing, so this is legal typl — and with no memoization it
    // costs time exponential in the depth. Thirty levels is 2^30 walks, which
    // is the difference between this test finishing and `ridlc` hanging.
    let mut decls = vec![decl(
        "S30",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field("x", 1, primitive(v2::PrimitiveType::Boolean))],
            ..Default::default()
        }),
    )];
    for level in (0..30).rev() {
        decls.push(decl(
            &format!("S{level}"),
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field("a", 1, named(&format!("S{}", level + 1))),
                    field("b", 2, named(&format!("S{}", level + 1))),
                ],
                ..Default::default()
            }),
        ));
    }
    let pkg = package("veh.cruise", decls);

    // The bound passes what an offset can address long before the top, so the
    // answer is `None`. What this test is about is that an answer arrives.
    assert_eq!(bound(&[&pkg], "S0"), None);
    // And the shallow end of the same graph is finite, so the walk is not
    // simply refusing everything.
    assert!(bound(&[&pkg], "S28").is_some());
}

#[test]
fn a_bound_larger_than_an_offset_can_address_is_refused() {
    // Every offset in the format is 32 bits. A bound above that describes a
    // buffer FlatBuffers cannot address, and the codec would emit it as a
    // `usize` that does not fit on the wasm32 target this encoding is for.
    let pkg = package(
        "veh.cruise",
        vec![
            decl("Blob", bytes_type(256)),
            holder_of(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(named("Blob"))),
                    min: 0,
                    max: 20_000_000,
                }))),
            }),
        ],
    );

    assert_eq!(bound(&[&pkg], "Holder"), None);
}

#[test]
fn a_bound_just_inside_the_offset_range_is_kept() {
    let pkg = package(
        "veh.cruise",
        vec![
            decl("Blob", bytes_type(256)),
            holder_of(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(named("Blob"))),
                    min: 0,
                    max: 1_000,
                }))),
            }),
        ],
    );

    let charged = bound(&[&pkg], "Holder").expect("bounded");

    assert!(charged <= MAX_ENCODABLE, "{charged}");
}

#[test]
fn a_declaration_with_no_table_of_its_own_has_no_bound() {
    let pkg = package(
        "veh.cruise",
        vec![decl("Percent", int_type(v2::IntWidth::U8))],
    );

    assert_eq!(bound(&[&pkg], "Percent"), None);
}
