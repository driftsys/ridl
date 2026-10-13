//! The model's own tests — the ones that need no front end.
//!
//! The corpus-driven tests (positional correspondence, determinism, the
//! nesting bound, the D-P4 floor) live in `crates/ridlc/tests/codegen_model.rs`:
//! each of them needs the compiler, and reaching it from here would mean a
//! dev-dependency on a crate that depends on this one. That file's header
//! records the same reason `crates/ridlc/tests/ir_canonical.rs` records.

use super::{from_binary, from_json, lower, to_binary, to_json_pretty, to_text_format, v1};
use crate::v2;

/// A package with one named scalar, one enum whose lowest member is not zero,
/// and one struct holding a field of each.
fn package() -> v2::Package {
    v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "SpeedKph".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(
                            v2::PrimitiveType::Integer as i32,
                        )),
                    }),
                    constraint: Some(v2::Constraint {
                        min: Some("0".to_string()),
                        max: Some("300".to_string()),
                        ..Default::default()
                    }),
                    init: Some(v2::InitValue {
                        derivable: true,
                        value: Some("0".to_string()),
                    }),
                    width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "GearState".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                    values: vec![value("REVERSE", -1), value("DRIVE", 2), value("NEUTRAL", 0)],
                    reserved: vec![v2::Reserved {
                        ordinal: 0,
                        name: None,
                        value: Some(7),
                    }],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Dashboard".to_string(),
                visibility: v2::Visibility::Internal as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![
                        member(1, "currentSpeed", named("SpeedKph")),
                        v2::StructMember {
                            member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
                                ordinal: 2,
                                name: Some("legacyChecksum".to_string()),
                                value: None,
                            })),
                        },
                        member(3, "gear", named("GearState")),
                    ],
                    fixed_layout: true,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

fn value(name: &str, value: i64) -> v2::EnumValue {
    v2::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
        links: Vec::new(),
        see: Vec::new(),
        since: Vec::new(),
    }
}

fn named(reference: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(reference.to_string())),
    }
}

fn member(ordinal: u32, name: &str, ty: v2::FieldType) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(Box::new(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(ty),
            init: Some(v2::InitValue {
                derivable: true,
                value: None,
            }),
            ..Default::default()
        }))),
    }
}

fn model() -> v1::Model {
    lower(&package(), &[])
}

/// Every declared identifier carries the pinned transforms and the
/// compositions the backends make of them (design note D-2).
#[test]
fn every_identifier_carries_its_five_spellings() {
    let model = model();
    let name = model.declarations[2].name.as_ref().expect("a name");
    assert_eq!(name.declared, "Dashboard");
    assert_eq!(name.snake, "dashboard");
    assert_eq!(name.camel, "Dashboard");
    assert_eq!(name.screaming, "DASHBOARD");
    assert_eq!(name.pascal, "Dashboard");

    let v1::declaration::Kind::Struct(def) = model.declarations[2].kind.as_ref().expect("a kind")
    else {
        panic!("the third declaration is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
        panic!("the first slot holds a field");
    };
    let name = field.name.as_ref().expect("a field name");
    assert_eq!(name.declared, "currentSpeed");
    assert_eq!(name.snake, "current_speed");
    assert_eq!(name.camel, "CurrentSpeed");
    assert_eq!(name.screaming, "CURRENT_SPEED");
    assert_eq!(name.pascal, "CurrentSpeed");

    let gear = model
        .declarations
        .iter()
        .find(|decl| {
            decl.name
                .as_ref()
                .is_some_and(|name| name.declared == "GearState")
        })
        .expect("GearState is lowered");
    let Some(v1::declaration::Kind::Enum(def)) = gear.kind.as_ref() else {
        panic!("GearState is an enum");
    };
    let reverse = def
        .values
        .iter()
        .filter_map(|value| value.name.as_ref())
        .find(|name| name.declared == "REVERSE")
        .expect("REVERSE is lowered");
    assert_eq!(reverse.pascal, "Reverse");
}

/// A package name carries the dotted form, its segments, the wire backends'
/// `type_name` — which `camel_case` does not compute — and the TypeScript
/// import alias.
#[test]
fn a_package_name_carries_the_four_forms_a_target_reads() {
    let name = model().name.expect("a package name");
    assert_eq!(name.dotted, "veh.common");
    assert_eq!(name.segments, ["veh", "common"]);
    assert_eq!(name.joined_camel, "VehCommon");
    assert_eq!(name.underscored, "veh_common");
}

/// The zero member and the init member (typl §5.8): the value 0 where it is
/// declared, whatever position it was declared in, and the lowest value
/// otherwise.
#[test]
fn an_enum_carries_its_zero_member_and_its_init_member_by_index() {
    let model = model();
    let v1::declaration::Kind::Enum(def) = model.declarations[1].kind.as_ref().expect("a kind")
    else {
        panic!("the second declaration is an enum");
    };
    assert_eq!(def.zero_member, Some(2), "NEUTRAL is the zero member");
    assert_eq!(
        def.init_member,
        Some(2),
        "the zero member is the init member when one is declared"
    );
    assert_eq!(
        def.retired.len(),
        1,
        "a retired enum value keeps its identity in the model"
    );
    assert_eq!(def.retired[0].value, Some(7));
}

/// The lenient request reader refuses an invalid interaction class, exactly
/// as the strict reader does, and so does the request writer.
#[test]
fn the_lenient_request_reader_rejects_an_invalid_class() {
    let model = v1::Model {
        interfaces: vec![v1::Interface {
            slots: vec![v1::InteractionSlot {
                ordinal: 1,
                occupant: Some(v1::interaction_slot::Occupant::Interaction(Box::new(
                    v1::Interaction {
                        kind: v1::Kind::Command as i32,
                        family: v1::Family::Intent as i32,
                        shape: Some(v1::interaction::Shape::Command(v1::CommandShape {
                            shape: v1::Shape::None as i32,
                            ..Default::default()
                        })),
                        ..Default::default()
                    },
                ))),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let request = v1::CodegenRequest {
        model: Some(model),
        ..Default::default()
    };
    let json = crate::v2::render_json(&request).expect("the raw writer renders it");
    let err = crate::codegen::contract::request_from_json(&json)
        .expect_err("the lenient reader must refuse it");
    assert!(
        err.to_string().contains("invalid interaction class"),
        "{err}"
    );
    let err =
        crate::codegen::contract::request_to_json(&request).expect_err("the writer must refuse it");
    assert!(
        matches!(err, crate::codegen::SerializeError::Class(_)),
        "{err}"
    );
}

/// The model copies the family of an interaction and the shape of an intent
/// command from the IR (ADR-0012 decision 6).
#[test]
fn the_model_carries_the_family_and_the_command_shape() {
    let mut package = package();
    package.interfaces = vec![v2::Interface {
        name: "Gestures".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![v2::Decl {
            name: "toggleLight".to_string(),
            visibility: v2::Visibility::Public as i32,
            ordinal: 1,
            kind: Some(v2::decl::Kind::CommandDef(v2::CommandDef {
                family: v2::Family::Intent as i32,
                shape: v2::Shape::Toggle as i32,
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    }];
    let model = lower(&package, &[]);
    let slot = &model.interfaces[0].slots[0];
    let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = slot.occupant.as_ref()
    else {
        panic!("a live slot");
    };
    assert_eq!(interaction.family, v1::Family::Intent as i32);
    let Some(v1::interaction::Shape::Command(command)) = interaction.shape.as_ref() else {
        panic!("a command, got {:?}", interaction.shape);
    };
    assert_eq!(command.shape, v1::Shape::Toggle as i32);
}

/// Without a zero member the init is the lowest declared value.
#[test]
fn an_enum_with_no_zero_member_inits_at_its_lowest_value() {
    let mut package = package();
    let Some(v2::decl::Kind::EnumDef(def)) = package.decls[1].kind.as_mut() else {
        panic!("the second declaration is an enum");
    };
    def.values.retain(|value| value.value != 0);
    let model = lower(&package, &[]);
    let v1::declaration::Kind::Enum(def) = model.declarations[1].kind.as_ref().expect("a kind")
    else {
        panic!("the second declaration is an enum");
    };
    assert_eq!(def.zero_member, None);
    assert_eq!(def.init_member, Some(0), "REVERSE is the lowest value");
}

/// A tombstone holds its ordinal slot in place, in the member order the proto
/// backend writes `reserved N;` in (design note D-6).
#[test]
fn a_tombstone_holds_its_slot_in_member_order() {
    let model = model();
    let v1::declaration::Kind::Struct(def) = model.declarations[2].kind.as_ref().expect("a kind")
    else {
        panic!("the third declaration is a struct");
    };
    let ordinals: Vec<u32> = def.slots.iter().map(|slot| slot.ordinal).collect();
    assert_eq!(ordinals, [1, 2, 3]);
    let Some(v1::slot::Occupant::Retired(retired)) = def.slots[1].occupant.as_ref() else {
        panic!("the second slot is a tombstone");
    };
    assert_eq!(
        retired.name.as_ref().expect("a retired name").declared,
        "legacyChecksum"
    );
    assert!(def.fixed_layout, "the IR's fixed-layout flag is carried");
}

/// A resolved reference names its package, its index and its kind, so a
/// plugin resolves nothing (design note D-3).
#[test]
fn a_reference_is_resolved_to_its_declaration() {
    let model = model();
    let v1::declaration::Kind::Struct(def) = model.declarations[2].kind.as_ref().expect("a kind")
    else {
        panic!("the third declaration is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
        panic!("the first slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert!(reference.resolved);
    assert!(!reference.foreign);
    assert_eq!(reference.package, "veh.common");
    assert_eq!(reference.index, 0);
    assert_eq!(reference.kind, v1::DeclKind::Scalar as i32);
}

/// The scope is part of the model: a plugin must be able to tell a package it
/// was not given from a name nothing declares (design note D-1).
#[test]
fn the_model_records_the_scope_it_was_lowered_over() {
    let other = v2::Package {
        name: "veh.other".to_string(),
        ..Default::default()
    };
    let model = lower(&package(), &[&other]);
    let scope = model.scope.expect("a scope");
    assert_eq!(scope.package, "veh.common");
    assert_eq!(scope.others, ["veh.other"]);
}

/// A reference no package in scope declares is carried as unresolved rather
/// than dropped: the lowering is total (design note D-1).
#[test]
fn an_unresolved_reference_is_carried_as_a_fact() {
    let mut package = package();
    let Some(v2::decl::Kind::StructDef(def)) = package.decls[2].kind.as_mut() else {
        panic!("the third declaration is a struct");
    };
    def.members
        .push(member(4, "cabin", named("veh.other.Temperature")));
    let model = lower(&package, &[]);
    let v1::declaration::Kind::Struct(def) = model.declarations[2].kind.as_ref().expect("a kind")
    else {
        panic!("the third declaration is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[3].occupant.as_ref() else {
        panic!("the fourth slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert!(!reference.resolved);
    assert!(reference.foreign, "the reference is dotted");
    assert_eq!(reference.reference, "veh.other.Temperature");

    let closure = model.declarations[2].closure.as_ref().expect("a closure");
    assert!(closure.reaches_foreign);
    assert!(closure.reaches_unresolved);
}

/// An unresolved reference has no declaring package, so its flag follows
/// its spelling: a bare name no package declares is not foreign.
#[test]
fn a_bare_unresolved_reference_is_not_foreign() {
    let mut package = package();
    let Some(v2::decl::Kind::StructDef(def)) = package.decls[2].kind.as_mut() else {
        panic!("the third declaration is a struct");
    };
    def.members.push(member(4, "cabin", named("Temperature")));
    let model = lower(&package, &[]);
    let v1::declaration::Kind::Struct(def) = model.declarations[2].kind.as_ref().expect("a kind")
    else {
        panic!("the third declaration is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[3].occupant.as_ref() else {
        panic!("the fourth slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert!(!reference.resolved);
    assert!(!reference.foreign, "the reference is bare");
    assert_eq!(reference.package, "");
    assert_eq!(reference.reference, "Temperature");
    assert_eq!(
        reference.index, 0,
        "an unresolved reference indexes nothing"
    );
    assert_eq!(reference.kind, v1::DeclKind::Unspecified as i32);

    let closure = model.declarations[2].closure.as_ref().expect("a closure");
    assert!(
        !closure.reaches_foreign,
        "a bare unresolved reference does not reach another package"
    );
    assert!(closure.reaches_unresolved);
}

/// Package `a`: a named scalar, an enum, and one declaration of each kind
/// that holds a reference — a union arm, a struct field, an enum set's
/// backing enum, a constant's type — each spelled bare, as `a` writes it.
fn package_a() -> v2::Package {
    v2::Package {
        name: "a".to_string(),
        decls: vec![
            v2::Decl {
                name: "Small".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(
                            v2::PrimitiveType::Integer as i32,
                        )),
                    }),
                    constraint: Some(v2::Constraint {
                        min: Some("0".to_string()),
                        max: Some("100".to_string()),
                        ..Default::default()
                    }),
                    init: Some(v2::InitValue {
                        derivable: true,
                        value: Some("0".to_string()),
                    }),
                    width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Gear".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                    values: vec![value("PARK", 0), value("DRIVE", 1)],
                    reserved: vec![],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Choice".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![v2::UnionArm {
                        name: "small".to_string(),
                        ordinal: 1,
                        type_ref: "Small".to_string(),
                        doc: String::new(),
                        links: Vec::new(),
                        see: Vec::new(),
                        since: Vec::new(),
                    }],
                    is_result: false,
                    reserved: vec![],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Pair".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "small", named("Small"))],
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Gears".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                    backing_enum: Some("Gear".to_string()),
                    bits: vec![value("PARK", 0), value("DRIVE", 1)],
                    width: v2::IntWidth::U8 as i32,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "LIMIT".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                    type_ref: Some("Small".to_string()),
                    value: "5".to_string(),
                    regex: None,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// The foreign declaration `name` of `package`, with its index in
/// `Model.foreign`.
fn foreign_declaration<'m>(
    model: &'m v1::Model,
    package: &str,
    name: &str,
) -> (u32, &'m v1::Declaration) {
    model
        .foreign
        .iter()
        .enumerate()
        .find_map(|(index, foreign)| {
            let declaration = foreign.declaration.as_ref()?;
            (foreign.package == package && declaration.name.as_ref()?.declared == name)
                .then_some((index as u32, declaration))
        })
        .unwrap_or_else(|| panic!("`{package}.{name}` is in `Model.foreign`"))
}

/// `reference` is foreign, names `a.<name>`, and its index resolves in
/// `Model.foreign` to that declaration.
fn assert_foreign_reference(model: &v1::Model, reference: &v1::TypeRef, name: &str) {
    assert!(
        reference.foreign,
        "`{}` is declared by `a`, not by the scope's package `b`",
        reference.reference
    );
    assert_eq!(reference.package, "a");
    let (index, _) = foreign_declaration(model, "a", name);
    assert_eq!(
        reference.index, index,
        "`{}` indexes `Model.foreign`",
        reference.reference
    );
}

/// A reference inside a foreign declaration is foreign too: `foreign` and
/// `index` follow one rule, the declaring package against `Scope.package`,
/// so a plugin that trusts the flag reads the table the index points into
/// (driftsys/ridl#586).
#[test]
fn a_reference_inside_a_foreign_declaration_is_foreign() {
    let a = package_a();
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![v2::Decl {
            name: "Uses".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    member(1, "choice", named("a.Choice")),
                    member(2, "pair", named("a.Pair")),
                    member(3, "gears", named("a.Gears")),
                    member(4, "limit", named("a.LIMIT")),
                ],
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = lower(&b, &[&a]);
    assert_eq!(model.scope.as_ref().expect("a scope").package, "b");

    // The references `b` writes itself are dotted and foreign.
    let v1::declaration::Kind::Struct(uses) = model.declarations[0].kind.as_ref().expect("a kind")
    else {
        panic!("`Uses` is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = uses.slots[0].occupant.as_ref() else {
        panic!("the first slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert!(reference.resolved);
    assert_foreign_reference(&model, reference, "Choice");

    // The union arm inside the foreign `Choice`, spelled bare in `a`.
    let (_, choice) = foreign_declaration(&model, "a", "Choice");
    let Some(v1::declaration::Kind::Union(union)) = choice.kind.as_ref() else {
        panic!("`Choice` is a union");
    };
    let arm = union.arms[0].r#type.as_ref().expect("an arm type");
    assert!(arm.resolved);
    assert_eq!(arm.reference, "Small", "the arm keeps `a`'s bare spelling");
    assert_foreign_reference(&model, arm, "Small");

    // The struct field inside the foreign `Pair`.
    let (_, pair) = foreign_declaration(&model, "a", "Pair");
    let Some(v1::declaration::Kind::Struct(def)) = pair.kind.as_ref() else {
        panic!("`Pair` is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
        panic!("the first slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert_foreign_reference(&model, reference, "Small");

    // The backing enum of the foreign enum set `Gears`.
    let (_, gears) = foreign_declaration(&model, "a", "Gears");
    let Some(v1::declaration::Kind::EnumSet(def)) = gears.kind.as_ref() else {
        panic!("`Gears` is an enum set");
    };
    let backing = def.backing_enum.as_ref().expect("a backing enum");
    assert_foreign_reference(&model, backing, "Gear");

    // The declared type of the foreign constant `LIMIT`.
    let (_, limit) = foreign_declaration(&model, "a", "LIMIT");
    let Some(v1::declaration::Kind::Constant(def)) = limit.kind.as_ref() else {
        panic!("`LIMIT` is a constant");
    };
    let Some(v1::constant::Typed::Named(reference)) = def.typed.as_ref() else {
        panic!("`LIMIT` is typed by a named scalar");
    };
    assert_foreign_reference(&model, reference, "Small");
}

/// The FlatBuffers projection states a reference by the same rule: the enum
/// a boxed arm wraps is foreign when its declaring package is not the
/// scope's, and its index then points into `Model.foreign`
/// (driftsys/ridl#586).
#[test]
fn a_projected_enum_reference_is_foreign_by_its_declaring_package() {
    let a = package_a();
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![
            v2::Decl {
                name: "Pick".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                    arms: vec![
                        v2::UnionArm {
                            name: "gear".to_string(),
                            ordinal: 1,
                            type_ref: "a.Gear".to_string(),
                            doc: String::new(),
                            links: Vec::new(),
                            see: Vec::new(),
                            since: Vec::new(),
                        },
                        v2::UnionArm {
                            name: "own".to_string(),
                            ordinal: 2,
                            type_ref: "Own".to_string(),
                            doc: String::new(),
                            links: Vec::new(),
                            see: Vec::new(),
                            since: Vec::new(),
                        },
                        // The scope's own enum, spelled with its package: the
                        // spelling a text test for a dot calls foreign while
                        // the declaring package is the scope's.
                        v2::UnionArm {
                            name: "qualified".to_string(),
                            ordinal: 3,
                            type_ref: "b.Own".to_string(),
                            doc: String::new(),
                            links: Vec::new(),
                            see: Vec::new(),
                            since: Vec::new(),
                        },
                    ],
                    is_result: false,
                    reserved: vec![],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Own".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                    values: vec![value("ONE", 0)],
                    reserved: vec![],
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let model = lower(&b, &[&a]);
    let projection = model.flatbuffers.as_ref().expect("a projection");
    let boxed_enum = |arm_box: u32| -> &v1::TypeRef {
        let table = projection
            .tables
            .iter()
            .find(|table| {
                matches!(
                    table.source,
                    Some(v1::fb_table::Source::ArmBox(v1::ArmBox { declaration: 0, arm })) if arm == arm_box
                )
            })
            .expect("the arm is boxed");
        let Some(v1::fb_wire::Kind::Enum(wire)) =
            table.slots[0].wire.as_ref().expect("a wire").kind.as_ref()
        else {
            panic!("the box wraps an enum");
        };
        wire.r#type.as_ref().expect("a reference")
    };

    let gear = boxed_enum(0);
    assert!(gear.resolved);
    assert_foreign_reference(&model, gear, "Gear");

    let own = boxed_enum(1);
    assert!(own.resolved);
    assert!(!own.foreign, "`Own` is declared by the scope's package");
    assert_eq!(own.package, "b");
    assert_eq!(own.index, 1, "`Own` indexes `Model.declarations`");

    let qualified = boxed_enum(2);
    assert!(qualified.resolved);
    assert!(
        !qualified.foreign,
        "`b.Own` is declared by the scope's package, whatever its spelling"
    );
    assert_eq!(qualified.package, "b");
    assert_eq!(qualified.index, 1, "`b.Own` indexes `Model.declarations`");

    // The lowering states the same arm by the same rule, and copies nothing
    // of the scope's own package into `Model.foreign`.
    let v1::declaration::Kind::Union(pick) = model.declarations[0].kind.as_ref().expect("a kind")
    else {
        panic!("`Pick` is a union");
    };
    let lowered = pick.arms[2].r#type.as_ref().expect("an arm type");
    assert!(lowered.resolved);
    assert!(
        !lowered.foreign,
        "the lowered `b.Own` is declared by the scope's package"
    );
    assert_eq!(lowered.package, "b");
    assert_eq!(
        lowered.index, 1,
        "the lowered `b.Own` indexes `Model.declarations`"
    );
    assert!(
        model.foreign.iter().all(|foreign| foreign.package != "b"),
        "no declaration of `b` is copied into `Model.foreign`"
    );
}

/// `Closure.reaches_foreign` follows the rule `TypeRef.foreign` follows: the
/// declaring package against `Scope.package`, not the spelling. The arm of
/// the foreign copy `a.Choice` is the bare `Small`, which `a` declares
/// (driftsys/ridl#594).
#[test]
fn the_closure_of_a_foreign_copy_reaches_foreign_through_a_bare_reference() {
    let a = package_a();
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![v2::Decl {
            name: "Uses".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![member(1, "choice", named("a.Choice"))],
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = lower(&b, &[&a]);
    let (_, choice) = foreign_declaration(&model, "a", "Choice");
    let closure = choice.closure.as_ref().expect("a closure");
    assert!(
        closure.reaches_foreign,
        "the bare `Small` inside `a.Choice` is declared by `a`, not by `b`"
    );
    assert!(!closure.reaches_unresolved);
}

/// A local declaration that names a declaration of another package reaches
/// foreign, and a reference to a foreign constant does too: the constant
/// resolves, so its declaring package decides. The two packages hold the
/// same number of declarations, so a comparison by declaration count would
/// not tell them apart (driftsys/ridl#594).
#[test]
fn a_local_declaration_reaches_foreign_through_a_resolved_reference() {
    let a = v2::Package {
        name: "a".to_string(),
        decls: vec![
            v2::Decl {
                name: "Gear".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                    values: vec![value("PARK", 0), value("DRIVE", 1)],
                    reserved: vec![],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "LIMIT".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                    type_ref: Some("integer".to_string()),
                    value: "5".to_string(),
                    regex: None,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![
            v2::Decl {
                name: "Probe".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "g", named("a.Gear"))],
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Capped".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "limit", named("a.LIMIT"))],
                    ..Default::default()
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(a.decls.len(), b.decls.len());
    let model = lower(&b, &[&a]);

    let probe = model.declarations[0].closure.as_ref().expect("a closure");
    assert!(
        probe.reaches_foreign,
        "`a.Gear` is declared by `a`, not by the scope's package `b`"
    );
    assert!(!probe.reaches_unresolved);

    let capped = model.declarations[1].closure.as_ref().expect("a closure");
    assert!(
        capped.reaches_foreign,
        "the constant `a.LIMIT` is declared by `a`, not by `b`"
    );
    assert!(
        capped.reaches_unresolved,
        "a reference to a constant is not a type position"
    );
}

/// An unresolved reference has no declaring package, so the closure falls
/// back to the spelling, as `TypeRef.foreign` does: `b.Missing` inside `b`
/// is dotted and reaches foreign (driftsys/ridl#594).
#[test]
fn an_unresolved_self_qualified_reference_reaches_foreign() {
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![v2::Decl {
            name: "Holder".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![member(1, "missing", named("b.Missing"))],
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = lower(&b, &[&package_a()]);
    let v1::declaration::Kind::Struct(def) = model.declarations[0].kind.as_ref().expect("a kind")
    else {
        panic!("`Holder` is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
        panic!("the first slot holds a field");
    };
    let Some(v1::r#type::Kind::Named(reference)) =
        field.r#type.as_ref().expect("a type").kind.as_ref()
    else {
        panic!("the field is typed by a named reference");
    };
    assert!(!reference.resolved);
    assert!(reference.foreign, "the unresolved reference is dotted");

    let closure = model.declarations[0].closure.as_ref().expect("a closure");
    assert!(
        closure.reaches_foreign,
        "the closure states `b.Missing` as `TypeRef.foreign` does"
    );
    assert!(closure.reaches_unresolved);
}

/// A bare unresolved reference inside a foreign copy does not reach foreign:
/// it has no declaring package and its text is not dotted, whatever package
/// it was written in (driftsys/ridl#594).
#[test]
fn a_bare_unresolved_reference_inside_a_foreign_copy_is_not_foreign() {
    let mut a = package_a();
    let Some(v2::decl::Kind::UnionDef(choice)) = a.decls[2].kind.as_mut() else {
        panic!("the third declaration of `a` is the union `Choice`");
    };
    choice.arms[0].type_ref = "Missing".to_string();
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![v2::Decl {
            name: "Uses".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![member(1, "choice", named("a.Choice"))],
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = lower(&b, &[&a]);
    let (_, choice) = foreign_declaration(&model, "a", "Choice");
    let closure = choice.closure.as_ref().expect("a closure");
    assert!(
        !closure.reaches_foreign,
        "the bare `Missing` names no declaration and is not dotted"
    );
    assert!(closure.reaches_unresolved);
}

/// A reference `b` spells with its own package is not foreign: its declaring
/// package is `Scope.package`. A reference that resolves to a constant
/// follows the same rule (driftsys/ridl#594).
#[test]
fn a_self_qualified_reference_does_not_reach_foreign() {
    let b = v2::Package {
        name: "b".to_string(),
        decls: vec![
            v2::Decl {
                name: "Own".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                    values: vec![value("ONE", 0)],
                    reserved: vec![],
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Holder".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "own", named("b.Own"))],
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "MAX".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                    type_ref: Some("integer".to_string()),
                    value: "5".to_string(),
                    regex: None,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Limits".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "max", named("b.MAX"))],
                    ..Default::default()
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let model = lower(&b, &[&package_a()]);

    let holder = model.declarations[1].closure.as_ref().expect("a closure");
    assert!(
        !holder.reaches_foreign,
        "`b.Own` is declared by the scope's package, whatever its spelling"
    );
    assert!(!holder.reaches_unresolved);

    let limits = model.declarations[3].closure.as_ref().expect("a closure");
    assert!(
        !limits.reaches_foreign,
        "the constant `b.MAX` is declared by the scope's package"
    );
    assert!(
        limits.reaches_unresolved,
        "a reference to a constant is not a type position"
    );
}

/// The three encodings the model carries, over one lowered package.
#[test]
fn the_model_round_trips_through_each_encoding() {
    let model = model();
    let json = to_json_pretty(&model).expect("the model serializes as JSON");
    assert_eq!(from_json(&json).expect("the JSON parses back"), model);
    assert_eq!(
        to_json_pretty(&from_json(&json).expect("the JSON parses back")).expect("and again"),
        json,
        "the canonical encoding is byte-identical on a second write"
    );
    let binary = to_binary(&model);
    assert_eq!(from_binary(&binary).expect("the binary decodes"), model);
    assert!(
        to_text_format(&model)
            .expect("the model renders as prototext")
            .contains("veh.common"),
        "prototext comes from the same descriptor pool the IR's does"
    );
}

/// The backend contract's messages (`plugin.proto`): the request round-trips
/// through the encoding the process host puts on the pipe, its model is the
/// `--emit codegen-model` artifact one indentation level deeper, and the two
/// version fields lead.
#[test]
fn a_request_round_trips_and_leads_with_its_two_version_fields() {
    let request = v1::CodegenRequest {
        schema: super::SCHEMA.to_string(),
        toolchain: "0.2.0".to_string(),
        model: Some(model()),
        options: vec![v1::BackendOption {
            key: "wire-encoding".to_string(),
            value: "flatbuffers".to_string(),
        }],
        artifact_base: "veh.common".to_string(),
        deployment: None,
        generated_marker: String::new(),
        header: String::new(),
    };
    let json = super::request_to_json(&request).expect("the request renders");
    assert_eq!(
        super::request_from_json(&json).expect("the JSON parses back"),
        request
    );
    let keys: Vec<&str> = json
        .lines()
        .filter_map(|line| line.strip_prefix("  \""))
        .filter_map(|line| line.split_once('"'))
        .map(|(key, _)| key)
        .collect();
    assert_eq!(
        keys,
        [
            "schema",
            "toolchain",
            "model",
            "options",
            "artifactBase",
            "generatedMarker",
            "header"
        ],
        "the top-level keys, in schema order, with the two version fields first"
    );
    assert_eq!(super::SCHEMA, "ridl.codegen.v1");

    // The artifact's first line is its opening brace, which the request
    // spells as `"model": {`; every line after it appears indented once more.
    let model_json = to_json_pretty(&model()).expect("the model renders");
    let nested: String = model_json
        .lines()
        .skip(1)
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        json.contains(&format!("  \"model\": {{\n{nested}")),
        "the request's model is the artifact's text, indented one level deeper"
    );
}

/// A response round-trips, a text file's content is a JSON string and not
/// base64, and the error test counts an unset severity as an error.
#[test]
fn a_response_round_trips_with_text_content_as_a_string() {
    let response = v1::CodegenResponse {
        files: vec![
            super::text_file(
                "veh.common.rs".to_string(),
                "pub struct Speed;\n".to_string(),
            ),
            v1::GeneratedFile {
                path: "veh.common.bin".to_string(),
                content: Some(v1::generated_file::Content::Binary(vec![0, 255])),
            },
        ],
        diagnostics: vec![v1::Diagnostic {
            severity: v1::DiagnosticSeverity::Warning as i32,
            message: "w".to_string(),
        }],
    };
    let json = super::response_to_json(&response).expect("the response renders");
    assert!(
        json.contains("\"text\": \"pub struct Speed;\\n\""),
        "{json}"
    );
    assert!(json.contains("\"binary\": \"AP8=\""), "{json}");
    assert_eq!(
        super::response_from_json(&json).expect("the JSON parses back"),
        response
    );
    assert!(!super::has_error(&response));

    for severity in [
        v1::DiagnosticSeverity::Error as i32,
        v1::DiagnosticSeverity::Unspecified as i32,
        // Outside the schema: a value a lenient reader can carry.
        77,
    ] {
        let failed = v1::CodegenResponse {
            files: Vec::new(),
            diagnostics: vec![v1::Diagnostic {
                severity,
                message: "e".to_string(),
            }],
        };
        assert!(super::has_error(&failed), "severity {severity} is an error");
    }
    assert!(!super::has_error(&v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![v1::Diagnostic {
            severity: v1::DiagnosticSeverity::Info as i32,
            message: "i".to_string(),
        }],
    }));
}

/// The path rule every host applies before it writes a file.
#[test]
fn a_generated_file_path_is_relative_and_plain() {
    for path in ["veh.common.rs", "com/acme/veh/Speed.kt", "a.b/c-d_e"] {
        assert_eq!(super::check_path(path), Ok(()), "{path}");
    }
    for path in [
        "",
        "/etc/passwd",
        "../escape.rs",
        "a/../b.rs",
        "./a.rs",
        "a//b.rs",
        "a/",
        "a\\b.rs",
        "C:file.rs",
        "a\0b",
    ] {
        assert!(super::check_path(path).is_err(), "{path:?} must be refused");
    }
}

/// The model backend writes the request's model back as
/// `<artifact_base>.codegen.json`, takes no option, and answers a request
/// with no model with a diagnostic rather than a panic.
#[test]
fn the_model_backend_writes_the_model_back() {
    use super::Backend as _;

    let request = v1::CodegenRequest {
        schema: super::SCHEMA.to_string(),
        toolchain: "0.2.0".to_string(),
        model: Some(model()),
        options: Vec::new(),
        artifact_base: "veh.common".to_string(),
        deployment: None,
        generated_marker: String::new(),
        header: String::new(),
    };
    let response = super::ModelBackend.generate(&request);
    assert_eq!(super::ModelBackend.language(), "model");
    assert!(response.diagnostics.is_empty());
    assert_eq!(response.files.len(), 1);
    assert_eq!(response.files[0].path, "veh.common.codegen.json");
    assert_eq!(
        response.files[0].content,
        Some(v1::generated_file::Content::Text(
            to_json_pretty(&model()).expect("the model renders")
        ))
    );

    let with_option = v1::CodegenRequest {
        options: vec![v1::BackendOption {
            key: "indent".to_string(),
            value: "2".to_string(),
        }],
        ..request.clone()
    };
    let response = super::ModelBackend.generate(&with_option);
    assert!(super::has_error(&response));
    assert!(response.files.is_empty());
    assert!(response.diagnostics[0].message.contains("`indent`"));

    let without_model = v1::CodegenRequest {
        model: None,
        ..request
    };
    let response = super::ModelBackend.generate(&without_model);
    assert!(super::has_error(&response));
    assert!(response.files.is_empty());
}

/// Every `Spellings` field keeps its field number. A field number is the wire
/// identity a plugin built against an earlier model decodes by, so
/// renumbering one (`pascal = 5` to `pascal = 9`, say) breaks every such
/// plugin while every other test of this workspace still passes: the encoder
/// and the decoder here are generated from the one renumbered schema.
#[test]
fn spellings_field_numbers_are_pinned() {
    let spellings = v2::codegen_model_descriptor()
        .parent_pool()
        .get_message_by_name("ridl.codegen.v1.Spellings")
        .expect("model.proto declares Spellings");
    let fields: Vec<(String, u32)> = spellings
        .fields()
        .map(|field| (field.name().to_string(), field.number()))
        .collect();
    let expected = [
        ("declared", 1),
        ("snake", 2),
        ("camel", 3),
        ("screaming", 4),
        ("pascal", 5),
    ]
    .map(|(name, number)| (name.to_string(), number));
    assert_eq!(fields, expected);
}

#[test]
fn utf8_bytes_init_is_constructible_in_the_model() {
    let mut package = package();
    let Some(v2::decl::Kind::TypeDef(td)) = package.decls[0].kind.as_mut() else {
        panic!("expected scalar");
    };
    td.backing = Some(v2::Backing {
        kind: Some(v2::backing::Kind::Primitive(
            v2::PrimitiveType::Bytes as i32,
        )),
    });
    td.constraint = Some(v2::Constraint {
        len_min: Some(2),
        len_max: Some(2),
        ..Default::default()
    });
    td.width = None;
    td.declared_init = Some("é".into());
    td.init = Some(v2::InitValue {
        derivable: true,
        value: Some("é".into()),
    });
    let model = lower(&package, &[]);
    let init = model.declarations[0].init.as_ref().unwrap();
    assert!(init.derivable);
    assert_eq!(init.value.as_deref(), Some("é"));
}

#[test]
fn explicit_field_init_does_not_require_the_scalar_types_own_init() {
    for optional in [false, true] {
        let mut package = package();
        let Some(v2::decl::Kind::TypeDef(td)) = package.decls[0].kind.as_mut() else {
            panic!("expected scalar");
        };
        td.backing = Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::Bytes as i32,
            )),
        });
        td.constraint = Some(v2::Constraint {
            len_min: Some(2),
            len_max: Some(2),
            ..Default::default()
        });
        td.width = None;
        td.init = Some(v2::InitValue {
            derivable: false,
            value: None,
        });
        let Some(v2::decl::Kind::StructDef(def)) = package.decls[2].kind.as_mut() else {
            panic!("expected struct");
        };
        let Some(v2::struct_member::Member::Field(field)) = def.members[0].member.as_mut() else {
            panic!("expected field");
        };
        field.r#type.as_mut().unwrap().optional = optional;
        field.declared_init = Some("é".into());
        field.init = Some(v2::InitValue {
            derivable: true,
            value: Some("é".into()),
        });
        let model = lower(&package, &[]);
        assert!(!model.declarations[0].init.as_ref().unwrap().derivable);
        assert!(model.declarations[2].init.as_ref().unwrap().derivable);
        let Some(v1::declaration::Kind::Struct(def)) = model.declarations[2].kind.as_ref() else {
            panic!("expected model struct");
        };
        let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
            panic!("expected model field");
        };
        assert!(field.init.as_ref().unwrap().derivable);
    }
}

#[test]
fn map_model_init_requires_at_most_one_generated_entry() {
    for (min, expected) in [(0, true), (1, true), (2, false)] {
        let mut package = package();
        let Some(v2::decl::Kind::StructDef(def)) = package.decls[2].kind.as_mut() else {
            panic!("expected struct");
        };
        let Some(v2::struct_member::Member::Field(field)) = def.members[0].member.as_mut() else {
            panic!("expected field");
        };
        field.r#type = Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                min,
                max: 3,
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                })),
                value: Some(Box::new(named("SpeedKph"))),
            }))),
        });
        let model = lower(&package, &[]);
        assert_eq!(
            model.declarations[2].init.as_ref().unwrap().derivable,
            expected
        );
        let Some(v1::declaration::Kind::Struct(def)) = model.declarations[2].kind.as_ref() else {
            panic!("expected model struct");
        };
        let Some(v1::slot::Occupant::Field(field)) = def.slots[0].occupant.as_ref() else {
            panic!("expected model field");
        };
        assert_eq!(field.init.as_ref().unwrap().derivable, expected);
    }
}

/// A package with a struct of two bounded scalars (`Pair`), a string type
/// def with no length bound (`Label`), a struct holding it (`Tagged`), and
/// one interface with a signal over each struct.
fn sized_package() -> v2::Package {
    let mut package = package();
    package.decls.push(v2::Decl {
        name: "Pair".to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                member(1, "left", named("SpeedKph")),
                member(2, "right", named("SpeedKph")),
            ],
            ..Default::default()
        })),
        ..Default::default()
    });
    package.decls.push(v2::Decl {
        name: "Label".to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            ..Default::default()
        })),
        ..Default::default()
    });
    package.decls.push(v2::Decl {
        name: "Tagged".to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![member(1, "label", named("Label"))],
            ..Default::default()
        })),
        ..Default::default()
    });
    let signal = |name: &str, payload: &str| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef {
            payload: payload.to_string(),
            ..Default::default()
        })),
        ..Default::default()
    };
    let interface = v2::Interface {
        name: "Gauges".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![signal("pair", "Pair"), signal("tagged", "Tagged")],
        number: 1,
        ..Default::default()
    };
    v2::Package {
        interfaces: vec![interface],
        ..package
    }
}

/// The payload of the signal at `position` in the one interface.
fn signal_payload(model: &v1::Model, position: usize) -> &v1::Payload {
    let v1::Model { interfaces, .. } = model;
    let slot = &interfaces[0].slots[position];
    match slot.occupant.as_ref() {
        Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
            match interaction.shape.as_ref() {
                Some(v1::interaction::Shape::Signal(signal)) => {
                    signal.payload.as_ref().expect("a payload")
                }
                other => panic!("a signal, got {other:?}"),
            }
        }
        _ => panic!("a live slot"),
    }
}

#[test]
fn a_payload_carries_both_size_states() {
    let model = lower(&sized_package(), &[]);
    let sizes = signal_payload(&model, 0)
        .sizes
        .as_ref()
        .expect("a payload carries its sizes");
    let flatbuffers = signal_payload(&model, 0)
        .flatbuffers_max_size
        .expect("a bounded struct");
    assert_eq!(
        sizes.flatbuffers.as_ref().and_then(|s| s.state.as_ref()),
        Some(&v1::size_state::State::Bounded(flatbuffers))
    );
    assert!(matches!(
        sizes.proto3.as_ref().and_then(|s| s.state.as_ref()),
        Some(v1::size_state::State::Bounded(size)) if *size > 0
    ));
}

#[test]
fn field_two_still_equals_the_flatbuffers_bounded_value() {
    let model = lower(&sized_package(), &[]);
    let payload = signal_payload(&model, 0);
    let state = payload
        .sizes
        .as_ref()
        .and_then(|sizes| sizes.flatbuffers.as_ref())
        .and_then(|state| state.state.as_ref());
    assert!(payload.flatbuffers_max_size.is_some());
    assert_eq!(
        state,
        payload
            .flatbuffers_max_size
            .map(v1::size_state::State::Bounded)
            .as_ref()
    );
}

#[test]
fn an_unbounded_payload_keeps_its_cause_in_the_state() {
    let model = lower(&sized_package(), &[]);
    let payload = signal_payload(&model, 1);
    assert_eq!(payload.flatbuffers_max_size, None);
    let reference = payload.r#type.as_ref().expect("a type");
    let root = model
        .flatbuffers
        .as_ref()
        .expect("the projection is lowered")
        .roots
        .iter()
        .find(|root| root.declaration == reference.index)
        .expect("a root for the payload");
    let Some(v1::fb_root::Bound::Unbounded(expected)) = root.bound.as_ref() else {
        panic!("the root is unbounded");
    };
    let state = payload
        .sizes
        .as_ref()
        .and_then(|sizes| sizes.flatbuffers.as_ref())
        .and_then(|state| state.state.as_ref());
    assert_eq!(
        state,
        Some(&v1::size_state::State::Unbounded(expected.clone()))
    );
}

/// `sized_package` plus one interface `Calls` holding, in order: a command
/// with two parameters, a command with the one parameter `Pair`, a query
/// with no parameter returning `Pair`, a query over `Pair` with the
/// fallible reply `Pair | Pair`, and a query over `Pair` returning `Tagged`,
/// which has no FlatBuffers bound.
fn call_package() -> v2::Package {
    let param = |name: &str| v2::Param {
        name: name.to_string(),
        r#type: Some(named("Pair")),
        ..Default::default()
    };
    let decl = |name: &str, kind: v2::decl::Kind| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(kind),
        ..Default::default()
    };
    let value = v2::ReturnType {
        kind: Some(v2::return_type::Kind::Value(named("Pair"))),
    };
    let fallible = v2::ReturnType {
        kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
            ok: "Pair".to_string(),
            err: "Pair".to_string(),
        })),
    };
    let package = sized_package();
    let v2::Package { interfaces, .. } = package.clone();
    let calls = v2::Interface {
        name: "Calls".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![
            decl(
                "two",
                v2::decl::Kind::CommandDef(v2::CommandDef {
                    params: vec![param("a"), param("b")],
                    ..Default::default()
                }),
            ),
            decl(
                "one",
                v2::decl::Kind::CommandDef(v2::CommandDef {
                    params: vec![param("a")],
                    ..Default::default()
                }),
            ),
            decl(
                "none",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    return_type: Some(value),
                    ..Default::default()
                }),
            ),
            decl(
                "risky",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: vec![param("a")],
                    return_type: Some(fallible),
                    ..Default::default()
                }),
            ),
            decl(
                "loose",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: vec![param("a")],
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(named("Tagged"))),
                    }),
                    ..Default::default()
                }),
            ),
        ],
        number: 2,
        ..Default::default()
    };
    v2::Package {
        interfaces: interfaces.into_iter().chain([calls]).collect(),
        ..package
    }
}

/// One public interaction declaration, for a test that extends
/// `call_package`'s `Calls` interface with a shape the fixture does not hold.
fn call_decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(kind),
        ..Default::default()
    }
}

/// `call_package`'s `Calls` interface, to push an interaction onto.
fn calls_interface(package: &mut v2::Package) -> &mut v2::Interface {
    package
        .interfaces
        .iter_mut()
        .find(|interface| interface.name == "Calls")
        .expect("the Calls interface")
}

fn call_shape(model: &v1::Model, position: usize) -> &v1::interaction::Shape {
    let v1::Model { interfaces, .. } = model;
    let interface = interfaces
        .iter()
        .find(|interface| declared_name(interface) == "Calls")
        .expect("the Calls interface");
    match interface.slots[position].occupant.as_ref() {
        Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
            interaction.shape.as_ref().expect("a shape")
        }
        _ => panic!("a live slot"),
    }
}

fn state(sizes: &Option<v1::PayloadSizes>, proto3: bool) -> &v1::size_state::State {
    let sizes = sizes.as_ref().expect("sizes are written");
    let chosen = if proto3 {
        &sizes.proto3
    } else {
        &sizes.flatbuffers
    };
    chosen
        .as_ref()
        .and_then(|s| s.state.as_ref())
        .expect("a state")
}

fn undefined() -> v1::size_state::State {
    v1::size_state::State::Absent(v1::SizeAbsent {
        cause: v1::AbsentCause::EncodingUndefined as i32,
        detail: None,
    })
}

#[test]
fn a_two_parameter_command_states_a_request_size() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("a command");
    };
    for proto3 in [true, false] {
        assert!(
            matches!(
                state(&command.request_sizes, proto3),
                v1::size_state::State::Bounded(_)
            ),
            "the induced argument struct is sized like any other message"
        );
    }
}

#[test]
fn a_one_parameter_command_request_sizes_equal_its_payload_sizes() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 1) else {
        panic!("a command");
    };
    let request = command.request.as_ref().expect("a single named parameter");
    assert_eq!(command.request_sizes, request.sizes);
    assert!(matches!(
        state(&command.request_sizes, true),
        v1::size_state::State::Bounded(size) if *size > 0
    ));
}

#[test]
fn a_zero_parameter_query_states_a_request_size() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 2) else {
        panic!("a query");
    };
    for proto3 in [true, false] {
        assert!(matches!(
            state(&query.request_sizes, proto3),
            v1::size_state::State::Bounded(_)
        ));
    }
    assert_eq!(
        query.reply_sizes,
        query.reply_payload.as_ref().and_then(|p| p.sizes.clone()),
        "a plain reply is sized like its payload"
    );
}

#[test]
fn a_fallible_query_reply_is_undefined() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 3) else {
        panic!("a query");
    };
    assert_eq!(state(&query.reply_sizes, true), &undefined());
    assert_eq!(state(&query.reply_sizes, false), &undefined());
    assert_eq!(query.request_sizes, query.request.as_ref().unwrap().sizes);
}

#[test]
fn an_unbounded_query_reply_keeps_the_replys_own_attribution() {
    // The reply's unbounded state is attributed to the reply type, not to
    // the cause alone: the attribution carries the member that cannot be
    // bounded. A lowering that sized the reply without its named type would
    // write the bare cause with no member, and fails here.
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 4) else {
        panic!("a query");
    };
    let reference = query
        .reply_payload
        .as_ref()
        .expect("the reply is one named type")
        .r#type
        .as_ref()
        .expect("a type");
    let root = model
        .flatbuffers
        .as_ref()
        .expect("the projection is lowered")
        .roots
        .iter()
        .find(|root| root.declaration == reference.index)
        .expect("a root for the reply type");
    let Some(v1::fb_root::Bound::Unbounded(expected)) = root.bound.as_ref() else {
        panic!("the root is unbounded");
    };
    assert!(
        expected.member.is_some(),
        "the attribution names the member that cannot be bounded"
    );
    assert_eq!(
        state(&query.reply_sizes, false),
        &v1::size_state::State::Unbounded(expected.clone())
    );
}

#[test]
fn absent_cause_pins_each_tag_to_its_schema_value() {
    use crate::projection::size::AbsentCause;
    let pairs = [
        (
            AbsentCause::EncodingUndefined,
            v1::AbsentCause::EncodingUndefined,
        ),
        (AbsentCause::NoMessage, v1::AbsentCause::NoMessage),
        (AbsentCause::RefusedMember, v1::AbsentCause::RefusedMember),
        (AbsentCause::NoBound, v1::AbsentCause::NoBound),
        (AbsentCause::Overflow, v1::AbsentCause::Overflow),
        (AbsentCause::Unresolved, v1::AbsentCause::Unresolved),
    ];
    for (cause, expected) in pairs {
        assert_eq!(super::lower::absent_cause(cause), expected);
    }
    // Every tag of the schema, including the unspecified zero: a plugin
    // reads the number, so a renumber is a wire change, not a rename.
    assert_eq!(v1::AbsentCause::Unspecified as i32, 0);
    assert_eq!(v1::AbsentCause::EncodingUndefined as i32, 1);
    assert_eq!(v1::AbsentCause::NoMessage as i32, 2);
    assert_eq!(v1::AbsentCause::RefusedMember as i32, 3);
    assert_eq!(v1::AbsentCause::NoBound as i32, 4);
    assert_eq!(v1::AbsentCause::Overflow as i32, 5);
    assert_eq!(v1::AbsentCause::Unresolved as i32, 6);
}

/// The declared name of an `interface` declaration.
fn declared_name(interface: &v1::Interface) -> &str {
    match interface.identity.as_ref() {
        Some(v1::interface::Identity::Declared(name)) => &name.declared,
        _ => "",
    }
}

/// The package of `call_package` plus three interfaces over `Pair` (bounded
/// under both encodings) and `Tagged` (which has no bounded state under
/// either: absent with `REFUSED_MEMBER` under proto3, unbounded under
/// FlatBuffers):
///
/// - `Tally`: a signal, a query with a request and a reply, a tombstone, and
///   a command, in that order.
/// - `Mixed`: a signal over `Pair`, a signal over `Tagged`, a signal over
///   `Tagged` again.
/// - `Gaps`: only a tombstone.
fn budget_package() -> v2::Package {
    let package = call_package();
    let param = v2::Param {
        name: "a".to_string(),
        r#type: Some(named("Pair")),
        ..Default::default()
    };
    let decl = |name: &str, kind: v2::decl::Kind| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(kind),
        ..Default::default()
    };
    let signal = |name: &str, payload: &str| {
        decl(
            name,
            v2::decl::Kind::SignalDef(v2::SignalDef {
                payload: payload.to_string(),
                ..Default::default()
            }),
        )
    };
    let tombstone = |ordinal: u32| v2::Decl {
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: Some("gone".to_string()),
            value: None,
        })),
        ..Default::default()
    };
    let interface = |name: &str, number: u32, interactions: Vec<v2::Decl>| v2::Interface {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions,
        number,
        ..Default::default()
    };
    let tally = interface(
        "Tally",
        3,
        vec![
            signal("level", "Pair"),
            decl(
                "ask",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: vec![param.clone()],
                    return_type: Some(v2::ReturnType {
                        kind: Some(v2::return_type::Kind::Value(named("Pair"))),
                    }),
                    ..Default::default()
                }),
            ),
            tombstone(3),
            decl(
                "set",
                v2::decl::Kind::CommandDef(v2::CommandDef {
                    params: vec![param],
                    ..Default::default()
                }),
            ),
            decl(
                "table",
                v2::decl::Kind::FixedDef(v2::FixedDef {
                    payload: Some(named("Pair")),
                }),
            ),
        ],
    );
    let mixed = interface(
        "Mixed",
        4,
        vec![
            signal("first", "Pair"),
            signal("second", "Tagged"),
            signal("third", "Tagged"),
        ],
    );
    let gaps = interface("Gaps", 5, vec![tombstone(1)]);
    let v2::Package { interfaces, .. } = package.clone();
    v2::Package {
        interfaces: interfaces.into_iter().chain([tally, mixed, gaps]).collect(),
        ..package
    }
}

fn interface_named<'m>(model: &'m v1::Model, name: &str) -> &'m v1::Interface {
    let v1::Model { interfaces, .. } = model;
    interfaces
        .iter()
        .find(|interface| declared_name(interface) == name)
        .expect("the interface")
}

fn live_member<'m>(interface: &'m v1::Interface, name: &str) -> &'m v1::Interaction {
    interface
        .slots
        .iter()
        .find_map(|slot| match slot.occupant.as_ref() {
            Some(v1::interaction_slot::Occupant::Interaction(interaction))
                if interaction.name.as_ref().map(|n| n.declared.as_str()) == Some(name) =>
            {
                Some(&**interaction)
            }
            _ => None,
        })
        .expect("the live member")
}

/// The reservation state under one encoding.
fn reserved(reservation: &Option<v1::Reservation>, proto3: bool) -> &v1::reservation_state::State {
    let reservation = reservation.as_ref().expect("a reservation is written");
    let chosen = if proto3 {
        &reservation.proto3
    } else {
        &reservation.flatbuffers
    };
    chosen
        .as_ref()
        .and_then(|s| s.state.as_ref())
        .expect("a state")
}

fn bytes(size: u64) -> v1::reservation_state::State {
    v1::reservation_state::State::Bytes(size)
}

fn unsized_by(name: &str) -> v1::reservation_state::State {
    v1::reservation_state::State::Unsized(name.to_string())
}

/// The bounded size of the `Pair` payload of the signal `level`.
fn pair_size(model: &v1::Model, proto3: bool) -> u64 {
    let level = live_member(interface_named(model, "Tally"), "level");
    let Some(v1::interaction::Shape::Signal(signal)) = level.shape.as_ref() else {
        panic!("a signal");
    };
    match state(&signal.payload.as_ref().expect("a payload").sizes, proto3) {
        v1::size_state::State::Bounded(size) => u64::from(*size),
        other => panic!("a bounded payload, got {other:?}"),
    }
}

#[test]
fn a_query_reserves_request_plus_reply() {
    let model = lower(&budget_package(), &[]);
    let ask = live_member(interface_named(&model, "Tally"), "ask");
    for proto3 in [true, false] {
        let one = pair_size(&model, proto3);
        assert!(one > 0, "the fixture payload has a size");
        assert_eq!(reserved(&ask.reservation, proto3), &bytes(2 * one));
    }
}

#[test]
fn a_command_reserves_its_request_only() {
    let model = lower(&budget_package(), &[]);
    let set = live_member(interface_named(&model, "Tally"), "set");
    let level = live_member(interface_named(&model, "Tally"), "level");
    for proto3 in [true, false] {
        let one = pair_size(&model, proto3);
        assert_eq!(reserved(&set.reservation, proto3), &bytes(one));
        assert_eq!(reserved(&level.reservation, proto3), &bytes(one));
    }
}

#[test]
fn a_table_budget_sums_every_live_member() {
    let model = lower(&budget_package(), &[]);
    let tally = interface_named(&model, "Tally");
    for proto3 in [true, false] {
        // level (1) + ask (2) + set (1) + table (0, a fixed)
        let one = pair_size(&model, proto3);
        assert_eq!(reserved(&tally.table_budget, proto3), &bytes(4 * one));
    }
}

/// (driftsys/ridl#852) A `fixed` is provisioned, never in flight, so it
/// reserves nothing and adds nothing to the table budget — the same number
/// the runtime's `Sizes` row carries for it. Its payload has a size, so a
/// model that counted the payload would give a different number from the
/// runtime for any interface with a `fixed` member.
#[test]
fn a_fixed_reserves_nothing() {
    let model = lower(&budget_package(), &[]);
    let table = live_member(interface_named(&model, "Tally"), "table");
    for proto3 in [true, false] {
        assert!(pair_size(&model, proto3) > 0, "the payload has a size");
        assert_eq!(reserved(&table.reservation, proto3), &bytes(0));
    }
}

/// (driftsys/ridl#852) An unbounded `fixed` payload still reserves nothing,
/// rather than making the interface's budget unsized: the runtime's row is 0
/// whatever the payload is.
#[test]
fn an_unbounded_fixed_payload_reserves_nothing_and_keeps_the_budget_sized() {
    let model = lower(&unbounded_fixed_package(), &[]);
    let kinds = interface_named(&model, "Tally");
    let table = live_member(kinds, "table");
    for proto3 in [true, false] {
        assert_eq!(reserved(&table.reservation, proto3), &bytes(0));
        assert_eq!(
            reserved(&kinds.table_budget, proto3),
            &bytes(4 * pair_size(&model, proto3)),
            "the unbounded fixed does not make the budget unsized"
        );
    }
}

/// [`budget_package`] with the `fixed` member's payload the unbounded
/// `Tagged` type instead of `Pair`.
fn unbounded_fixed_package() -> v2::Package {
    let mut package = budget_package();
    for interface in &mut package.interfaces {
        if interface.name != "Tally" {
            continue;
        }
        for interaction in &mut interface.interactions {
            if interaction.name == "table" {
                interaction.kind = Some(v2::decl::Kind::FixedDef(v2::FixedDef {
                    payload: Some(named("Tagged")),
                }));
            }
        }
    }
    package
}

#[test]
fn one_unsized_member_makes_the_budget_unsized() {
    let model = lower(&budget_package(), &[]);
    let mixed = interface_named(&model, "Mixed");
    let second = live_member(mixed, "second");
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&second.reservation, proto3),
            &unsized_by("second.payload: Tagged")
        );
        assert_eq!(reserved(&mixed.table_budget, proto3), &unsized_by("second"));
    }
}

#[test]
fn a_tombstone_does_not_count() {
    let model = lower(&budget_package(), &[]);
    let tally = interface_named(&model, "Tally");
    assert_eq!(tally.slots.len(), 5, "the tombstone holds its slot");
    let live_sum: u64 = tally
        .slots
        .iter()
        .filter_map(|slot| match slot.occupant.as_ref() {
            Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
                match reserved(&interaction.reservation, true) {
                    v1::reservation_state::State::Bytes(size) => Some(*size),
                    v1::reservation_state::State::Unsized(_) => None,
                }
            }
            _ => None,
        })
        .sum();
    assert_eq!(reserved(&tally.table_budget, true), &bytes(live_sum));
    let gaps = interface_named(&model, "Gaps");
    assert_eq!(reserved(&gaps.table_budget, true), &bytes(0));
    assert_eq!(reserved(&gaps.table_budget, false), &bytes(0));
}

/// The `Bare` interface: a query with no parameter and no return type, a
/// fixed member with no payload, and a query whose one `Pair` parameter is
/// bounded and which declares no return type. The ridl surface admits none of
/// the three; the IR can carry all three.
fn bare_package() -> v2::Package {
    let package = sized_package();
    let decl = |name: &str, kind: v2::decl::Kind| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(kind),
        ..Default::default()
    };
    let bare = v2::Interface {
        name: "Bare".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![
            decl("blank", v2::decl::Kind::QueryDef(v2::QueryDef::default())),
            decl("tick", v2::decl::Kind::FixedDef(v2::FixedDef::default())),
            decl(
                "quiet",
                v2::decl::Kind::QueryDef(v2::QueryDef {
                    params: vec![v2::Param {
                        name: "a".to_string(),
                        r#type: Some(named("Pair")),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            ),
        ],
        number: 6,
        ..Default::default()
    };
    let v2::Package { interfaces, .. } = package.clone();
    v2::Package {
        interfaces: interfaces.into_iter().chain([bare]).collect(),
        ..package
    }
}

#[test]
fn a_query_with_no_return_type_states_a_reply_size() {
    let model = lower(&bare_package(), &[]);
    let blank = live_member(interface_named(&model, "Bare"), "blank");
    let Some(v1::interaction::Shape::Query(query)) = blank.shape.as_ref() else {
        panic!("a query");
    };
    // The reply is an induced empty struct, and so is the request.
    for proto3 in [true, false] {
        assert!(matches!(
            state(&query.reply_sizes, proto3),
            v1::size_state::State::Bounded(_)
        ));
        assert!(matches!(
            reserved(&blank.reservation, proto3),
            v1::reservation_state::State::Bytes(_)
        ));
    }
}

#[test]
fn a_fixed_member_with_no_payload_reserves_nothing() {
    let model = lower(&bare_package(), &[]);
    let tick = live_member(interface_named(&model, "Bare"), "tick");
    for proto3 in [true, false] {
        assert_eq!(reserved(&tick.reservation, proto3), &bytes(0));
    }
}

/// The `Odd` interface: one member whose kind this lowering does not know,
/// which the IR can carry and the ridl surface cannot express.
fn odd_package() -> v2::Package {
    let package = sized_package();
    let odd = v2::Interface {
        name: "Odd".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![v2::Decl {
            name: "later".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: None,
            ..Default::default()
        }],
        number: 7,
        ..Default::default()
    };
    let v2::Package { interfaces, .. } = package.clone();
    v2::Package {
        interfaces: interfaces.into_iter().chain([odd]).collect(),
        ..package
    }
}

#[test]
fn a_member_of_an_unknown_kind_is_unsized_and_not_zero() {
    // A member kind a newer toolchain adds must never be summed as zero: a
    // reservation that counted it as zero would be smaller than the truth
    // and a plugin would reserve too little.
    let model = lower(&odd_package(), &[]);
    let odd = interface_named(&model, "Odd");
    let later = live_member(odd, "later");
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&later.reservation, proto3),
            &unsized_by("later: no known payload shape")
        );
        assert_eq!(reserved(&odd.table_budget, proto3), &unsized_by("later"));
    }
}

#[test]
fn each_encoding_lands_in_its_own_column() {
    // `Tagged` holds a `string` with no length bound. Under proto3 that is a
    // member no leaf bounds, so the state is absent with `REFUSED_MEMBER`;
    // under FlatBuffers the same declaration is unbounded. The two states
    // differ in kind, so a lowering that filled one column with the other
    // encoding's sizer fails here.
    let model = lower(&sized_package(), &[]);
    let tagged = signal_payload(&model, 1);
    assert_eq!(
        state(&tagged.sizes, true),
        &v1::size_state::State::Absent(v1::SizeAbsent {
            cause: v1::AbsentCause::RefusedMember as i32,
            detail: None,
        })
    );
    assert!(matches!(
        state(&tagged.sizes, false),
        v1::size_state::State::Unbounded(_)
    ));
}

/// The `Kinds` interface: five queries whose reply types are, in order, a
/// primitive, an inline scalar, a map, an array of a named type, and a
/// payload position that declares no type at all. Each takes one bounded
/// parameter, so the request is sized and the reply is what the reservation
/// reports. No codec defines a size for any of the reply types, so each
/// reservation is unsized and states the reply's kind.
fn kinds_package() -> v2::Package {
    let package = sized_package();
    let primitive = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::Boolean as i32,
        )),
    };
    let inline = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            ..Default::default()
        }))),
    };
    let map = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
            key: Some(Box::new(named("Label"))),
            value: Some(Box::new(named("Pair"))),
            min: 0,
            max: 2,
        }))),
    };
    let array = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(named("Pair"))),
            min: 0,
            max: 2,
        }))),
    };
    let replying = |name: &str, reply: v2::FieldType| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::QueryDef(v2::QueryDef {
            params: vec![v2::Param {
                name: "a".to_string(),
                r#type: Some(named("Pair")),
                ..Default::default()
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(reply)),
            }),
            ..Default::default()
        })),
        ..Default::default()
    };
    let kinds = v2::Interface {
        name: "Kinds".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![
            replying("flag", primitive),
            replying("count", inline),
            replying("table", map),
            replying("list", array),
            replying(
                "void",
                v2::FieldType {
                    optional: false,
                    kind: None,
                },
            ),
        ],
        number: 8,
        ..Default::default()
    };
    let v2::Package { interfaces, .. } = package.clone();
    v2::Package {
        interfaces: interfaces.into_iter().chain([kinds]).collect(),
        ..package
    }
}

#[test]
fn an_unsized_reply_states_the_kind_of_its_type() {
    // Each kind is named distinctly, so a lowering that spelled two kinds
    // the same way fails here.
    let model = lower(&kinds_package(), &[]);
    let kinds = interface_named(&model, "Kinds");
    let expected = [
        ("flag", "flag.reply: primitive"),
        ("count", "count.reply: inline scalar"),
        ("table", "table.reply: map"),
        ("list", "list.reply: array of Pair"),
        // A field position with no kind at all gets its own word, not the
        // `map` of a position that does carry a kind.
        ("void", "void.reply: no type"),
    ];
    for (member, text) in expected {
        for proto3 in [true, false] {
            assert_eq!(
                reserved(&live_member(kinds, member).reservation, proto3),
                &unsized_by(text),
                "{member}"
            );
        }
    }
}

#[test]
fn a_multi_parameter_request_reserves_a_number() {
    let model = lower(&call_package(), &[]);
    let two = live_member(interface_named(&model, "Calls"), "two");
    for proto3 in [true, false] {
        assert!(matches!(
            reserved(&two.reservation, proto3),
            v1::reservation_state::State::Bytes(_)
        ));
    }
}

#[test]
fn a_reservation_sums_in_u64_above_the_u32_range() {
    // Two payloads whose bounds are each the largest a `u32` carries sum
    // past `u32::MAX`. The sum is a `u64`, so it is the exact total; a sum
    // narrowed or clamped to `u32::MAX` fails here.
    let bounded = |size: u32| {
        Some(v1::PayloadSizes {
            proto3: Some(v1::SizeState {
                state: Some(v1::size_state::State::Bounded(size)),
            }),
            flatbuffers: Some(v1::SizeState {
                state: Some(v1::size_state::State::Bounded(size)),
            }),
        })
    };
    let payloads = [
        ("request", "Huge".to_string(), bounded(u32::MAX)),
        ("reply", "Huge".to_string(), bounded(u32::MAX)),
    ];
    let reservation = super::lower::reserve("ask", Some(&payloads));
    let expected = u64::from(u32::MAX) * 2;
    assert!(expected > u64::from(u32::MAX));
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&Some(reservation.clone()), proto3),
            &bytes(expected)
        );
    }
}

/// A request with the fields every request carries and no deployment.
fn minimal_request() -> v1::CodegenRequest {
    v1::CodegenRequest {
        schema: super::SCHEMA.to_string(),
        toolchain: "t".to_string(),
        model: Some(v1::Model::default()),
        options: vec![],
        artifact_base: "a".to_string(),
        deployment: None,
        generated_marker: String::new(),
        header: String::new(),
    }
}

/// A request with no deployment is the request that existed before the
/// deployment section: its JSON has no `deployment` key.
#[test]
fn a_request_without_a_deployment_serializes_as_before() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    assert!(!json.contains("deployment"), "{json}");
}

#[test]
fn a_request_with_a_deployment_round_trips() {
    let deployment = v1::Deployment {
        system: "veh.cabin.Vehicle".to_string(),
        name: "Bench".to_string(),
        ..Default::default()
    };
    let request = v1::CodegenRequest {
        deployment: Some(deployment),
        generated_marker: String::new(),
        header: String::new(),
        ..minimal_request()
    };
    let json = super::request_to_json(&request).expect("the request renders");
    assert_eq!(
        super::request_from_json(&json).expect("the JSON parses back"),
        request
    );
}

/// A section with one of every message the deployment schema declares, filled
/// with values that are not the field's default, so that every JSON key is
/// written.
fn populated_deployment() -> v1::Deployment {
    v1::Deployment {
        system: "veh.cabin.Cabin".to_string(),
        name: "Bench".to_string(),
        regions: vec![v1::Region {
            catalog: "veh.cabin".to_string(),
            hash: vec![1, 2, 3],
            interfaces: vec![v1::RegionInterface {
                name: "Climate".to_string(),
                number: 1,
                inline: true,
                provisional: true,
                service: "veh.cabin.climate".to_string(),
            }],
        }],
        instances: vec![v1::Instance {
            component: "veh.cabin.Provider".to_string(),
            instance: "primary".to_string(),
            machine: "head".to_string(),
            external: true,
            offers: vec![v1::InterfaceKey {
                catalog: "veh.cabin".to_string(),
                number: 1,
                name: "Climate".to_string(),
                inline: true,
            }],
            maps: vec!["veh.cabin".to_string()],
        }],
        channels: vec![v1::Channel {
            catalog: "veh.cabin".to_string(),
            interface_number: 1,
            interface: "Climate".to_string(),
            inline: true,
            member_ordinal: 2,
            member: "TempChanged".to_string(),
            kind: v1::Kind::Event as i32,
            producer: Some(v1::Endpoint {
                component: "veh.cabin.Provider".to_string(),
                instance: "primary".to_string(),
                machine: "head".to_string(),
            }),
            consumers: vec![v1::Consumer {
                component: "veh.cabin.Panel".to_string(),
                instance: "Unit".to_string(),
                machine: "head".to_string(),
                crossing: v1::Crossing::SameMachine as i32,
                encoding: v1::Encoding::Flatbuffers as i32,
                depth: Some(v1::Depth {
                    value: Some(10),
                    source: v1::ValueSource::Derived as i32,
                }),
                slots: Some(16),
                slots_source: v1::ValueSource::Default as i32,
                budget: Some(4096),
                budget_source: v1::ValueSource::Declared as i32,
            }],
            depth: Some(v1::Depth {
                value: Some(10),
                source: v1::ValueSource::Derived as i32,
            }),
        }],
        bindings: vec![v1::Binding {
            name: "websocket".to_string(),
            version: "1".to_string(),
            frame_header_max_bytes: Some(14),
            envelope_bytes: Some(8),
        }],
    }
}

/// The JSON key of every field of the deployment section, pinned against a
/// golden document. A round trip through the same serde impls cannot show a
/// renamed key, because it writes and reads the same name.
#[test]
fn a_populated_deployment_section_renders_the_keys_a_plugin_reads() {
    let request = v1::CodegenRequest {
        deployment: Some(populated_deployment()),
        generated_marker: String::new(),
        header: String::new(),
        ..minimal_request()
    };
    let json = super::request_to_json(&request).expect("the request renders");
    assert_eq!(
        json,
        r#"{
  "schema": "ridl.codegen.v1",
  "toolchain": "t",
  "model": {
    "declarations": [],
    "tuples": [],
    "interfaces": [],
    "services": [],
    "foreign": [],
    "tupleCollisions": []
  },
  "options": [],
  "artifactBase": "a",
  "deployment": {
    "system": "veh.cabin.Cabin",
    "name": "Bench",
    "regions": [
      {
        "catalog": "veh.cabin",
        "hash": "AQID",
        "interfaces": [
          {
            "name": "Climate",
            "number": 1,
            "inline": true,
            "provisional": true,
            "service": "veh.cabin.climate"
          }
        ]
      }
    ],
    "instances": [
      {
        "component": "veh.cabin.Provider",
        "instance": "primary",
        "machine": "head",
        "external": true,
        "offers": [
          {
            "catalog": "veh.cabin",
            "number": 1,
            "name": "Climate",
            "inline": true
          }
        ],
        "maps": [
          "veh.cabin"
        ]
      }
    ],
    "channels": [
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 1,
        "interface": "Climate",
        "inline": true,
        "memberOrdinal": 2,
        "member": "TempChanged",
        "kind": "KIND_EVENT",
        "producer": {
          "component": "veh.cabin.Provider",
          "instance": "primary",
          "machine": "head"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "head",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "depth": {
              "value": 10,
              "source": "VALUE_SOURCE_DERIVED"
            },
            "slots": 16,
            "slotsSource": "VALUE_SOURCE_DEFAULT",
            "budget": "4096",
            "budgetSource": "VALUE_SOURCE_DECLARED"
          }
        ],
        "depth": {
          "value": 10,
          "source": "VALUE_SOURCE_DERIVED"
        }
      }
    ],
    "bindings": [
      {
        "name": "websocket",
        "version": "1",
        "frameHeaderMaxBytes": 14,
        "envelopeBytes": 8
      }
    ]
  },
  "generatedMarker": "",
  "header": ""
}"#
    );
}

/// The field numbers and names of every message the deployment schema
/// declares, read off the compiled descriptor.
///
/// Neither the JSON a plugin reads nor a round trip through this crate's own
/// serde impls can show a renumbered field: JSON is keyed by name, and the
/// round trip writes and reads the same number. This reads the schema
/// (`proto/ridl/codegen/v1/deployment.proto`), as the `Spellings` test above
/// reads `model.proto`.
#[test]
fn the_deployment_sections_field_numbers_are_the_schemas() {
    let pool = v2::codegen_model_descriptor().parent_pool().clone();
    let fields = |message: &str| -> Vec<(String, u32)> {
        pool.get_message_by_name(message)
            .unwrap_or_else(|| panic!("the compiled schema declares {message}"))
            .fields()
            .map(|field| (field.name().to_string(), field.number()))
            .collect()
    };
    let expected = |rows: &[(&str, u32)]| -> Vec<(String, u32)> {
        rows.iter()
            .map(|(name, number)| ((*name).to_string(), *number))
            .collect()
    };

    assert_eq!(
        fields("ridl.codegen.v1.CodegenRequest"),
        expected(&[
            ("schema", 1),
            ("toolchain", 2),
            ("model", 3),
            ("options", 4),
            ("artifact_base", 5),
            ("deployment", 6),
            ("generated_marker", 7),
            ("header", 8),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Deployment"),
        expected(&[
            ("system", 1),
            ("name", 2),
            ("regions", 3),
            ("instances", 4),
            ("channels", 5),
            ("bindings", 6),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Region"),
        expected(&[("catalog", 1), ("hash", 2), ("interfaces", 3)])
    );
    assert_eq!(
        fields("ridl.codegen.v1.RegionInterface"),
        expected(&[
            ("name", 1),
            ("number", 2),
            ("inline", 3),
            ("provisional", 4),
            ("service", 5),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.InterfaceKey"),
        expected(&[("catalog", 1), ("number", 2), ("name", 3), ("inline", 4)])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Instance"),
        expected(&[
            ("component", 1),
            ("instance", 2),
            ("machine", 3),
            ("external", 4),
            ("offers", 5),
            ("maps", 6),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Endpoint"),
        expected(&[("component", 1), ("instance", 2), ("machine", 3)])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Depth"),
        expected(&[("value", 1), ("source", 2)])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Consumer"),
        expected(&[
            ("component", 1),
            ("instance", 2),
            ("machine", 3),
            ("crossing", 4),
            ("encoding", 5),
            ("depth", 6),
            ("slots", 7),
            ("slots_source", 8),
            ("budget", 9),
            ("budget_source", 10),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Channel"),
        expected(&[
            ("catalog", 1),
            ("interface_number", 2),
            ("interface", 3),
            ("inline", 4),
            ("member_ordinal", 5),
            ("member", 6),
            ("kind", 7),
            ("producer", 8),
            ("consumers", 9),
            ("depth", 10),
        ])
    );
    assert_eq!(
        fields("ridl.codegen.v1.Binding"),
        expected(&[
            ("name", 1),
            ("version", 2),
            ("frame_header_max_bytes", 3),
            ("envelope_bytes", 4),
        ])
    );
}

/// Every message the deployment schema declares, read off the compiled
/// descriptor.
///
/// The gate above reads a hand-written list of message names, so a message
/// added to `proto/ridl/codegen/v1/deployment.proto` and left out of that
/// list would be checked by nothing. This is the list the gate must cover.
#[test]
fn the_deployment_schema_declares_the_messages_the_field_gate_reads() {
    let pool = v2::codegen_model_descriptor().parent_pool().clone();
    let mut declared: Vec<String> = pool
        .all_messages()
        .filter(|message| message.parent_file().name() == "ridl/codegen/v1/deployment.proto")
        .map(|message| message.full_name().to_string())
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        [
            "ridl.codegen.v1.Binding",
            "ridl.codegen.v1.Channel",
            "ridl.codegen.v1.Consumer",
            "ridl.codegen.v1.Deployment",
            "ridl.codegen.v1.Depth",
            "ridl.codegen.v1.Endpoint",
            "ridl.codegen.v1.Instance",
            "ridl.codegen.v1.InterfaceKey",
            "ridl.codegen.v1.Region",
            "ridl.codegen.v1.RegionInterface",
        ]
    );
}

/// The value number of every enum the deployment schema declares.
///
/// A plugin reads the number, so a renumber is a wire change and not a
/// rename, and neither the JSON — keyed by the value's name — nor a round
/// trip through this crate's own impls can show one. `Crossing` is restated
/// with the IR's values (`proto/ridl/codegen/v1/deployment.proto`, the
/// schema header), so each of its values is checked against the IR's too.
#[test]
fn the_deployment_sections_enum_values_are_the_schemas() {
    assert_eq!(v1::Crossing::Unspecified as i32, 0);
    assert_eq!(v1::Crossing::SameMachine as i32, 1);
    assert_eq!(v1::Crossing::DifferentMachine as i32, 2);
    assert_eq!(v1::Crossing::OffBoard as i32, 3);
    let pairs = [
        (v1::Crossing::Unspecified, v2::Crossing::Unspecified),
        (v1::Crossing::SameMachine, v2::Crossing::SameMachine),
        (
            v1::Crossing::DifferentMachine,
            v2::Crossing::DifferentMachine,
        ),
        (v1::Crossing::OffBoard, v2::Crossing::OffBoard),
    ];
    for (section, ir) in pairs {
        assert_eq!(section as i32, ir as i32);
    }

    assert_eq!(v1::Encoding::Unspecified as i32, 0);
    assert_eq!(v1::Encoding::Proto3 as i32, 1);
    assert_eq!(v1::Encoding::Flatbuffers as i32, 2);
    assert_eq!(v1::Encoding::ReprC as i32, 3);

    assert_eq!(v1::ValueSource::Unspecified as i32, 0);
    assert_eq!(v1::ValueSource::Derived as i32, 1);
    assert_eq!(v1::ValueSource::Declared as i32, 2);
    assert_eq!(v1::ValueSource::Default as i32, 3);
    assert_eq!(v1::ValueSource::Underivable as i32, 4);
}

#[test]
fn comment_preamble_puts_the_marker_first() {
    let text = super::comment_preamble(
        "@generated by ridl from package a.b. Do not edit.",
        "Copyright X\n\nSPDX-License-Identifier: MIT",
        "//",
    );
    assert_eq!(
        text,
        "// @generated by ridl from package a.b. Do not edit.\n// Copyright X\n//\n// SPDX-License-Identifier: MIT\n\n"
    );
}

#[test]
fn comment_preamble_uses_the_hash_token() {
    let marker = "@generated by ridl from package a.b. Do not edit.";
    assert_eq!(
        super::comment_preamble(marker, "", "#"),
        "# @generated by ridl from package a.b. Do not edit.\n\n"
    );
}

#[test]
fn comment_preamble_of_nothing_is_empty() {
    assert_eq!(super::comment_preamble("", "", "//"), "");
}

#[test]
fn normalise_header_removes_carriage_returns_and_trailing_space() {
    assert_eq!(
        super::normalise_header("A  \r\n\r\nB\t\r\n\r\n").as_deref(),
        Some("A\n\nB")
    );
}

#[test]
fn normalise_header_treats_a_lone_carriage_return_as_a_line_break() {
    assert_eq!(super::normalise_header("A\rB").as_deref(), Some("A\nB"));
    assert_eq!(
        super::normalise_header("A\r\rB\r\nC").as_deref(),
        Some("A\n\nB\nC")
    );
}

#[test]
fn normalise_header_keeps_an_interior_whitespace_only_line_as_empty() {
    assert_eq!(
        super::normalise_header("A\n   \nB").as_deref(),
        Some("A\n\nB")
    );
}

#[test]
fn header_control_character_finds_a_control_character_other_than_tab_and_line_breaks() {
    assert_eq!(super::header_control_character("A\tB\r\nC\rD\n"), None);
    assert_eq!(super::header_control_character("A\u{0}B"), Some('\u{0}'));
    assert_eq!(
        super::header_control_character("A\u{1b}[0m"),
        Some('\u{1b}')
    );
    assert_eq!(super::header_control_character("A\u{7f}"), Some('\u{7f}'));
    for c in ['\u{2028}', '\u{2029}', '\u{85}', '\x0b', '\x0c'] {
        assert_eq!(
            super::header_control_character(&format!("A{c}B")),
            Some(c),
            "{c:?}"
        );
    }
    assert_eq!(
        super::header_control_character("A\u{2028}B\u{0}"),
        Some('\u{2028}')
    );
    assert_eq!(
        super::header_control_character("A\u{0}B\u{2028}"),
        Some('\u{0}')
    );
}

#[test]
fn comment_preamble_with_an_empty_marker_starts_with_a_bare_comment_token() {
    assert_eq!(super::comment_preamble("", "H", "//"), "//\n// H\n\n");
}

#[test]
fn normalise_header_removes_leading_blank_lines() {
    assert_eq!(super::normalise_header("\n \nA\n").as_deref(), Some("A"));
}

#[test]
fn normalise_header_of_blank_text_is_none() {
    for text in ["", "\n\n", "  \r\n"] {
        assert_eq!(super::normalise_header(text), None, "{text:?}");
    }
}

#[test]
fn generated_marker_names_the_package() {
    assert_eq!(
        super::generated_marker(Some("veh.cabin")),
        "@generated by ridl from package veh.cabin. Do not edit."
    );
    assert_eq!(
        super::generated_marker(None),
        "@generated by ridl. Do not edit."
    );
    assert!(super::generated_marker(None).starts_with(super::GENERATED_MARKER_PREFIX));
    assert!(super::generated_marker(Some("a")).starts_with(super::GENERATED_MARKER_PREFIX));
}

#[test]
fn the_marker_and_the_header_survive_a_json_round_trip() {
    let request = v1::CodegenRequest {
        generated_marker: "@generated by ridl from package a. Do not edit.".to_string(),
        header: "Copyright X\n\nMIT".to_string(),
        ..minimal_request()
    };
    let json = super::request_to_json(&request).expect("the request renders");
    assert!(json.contains("\"generatedMarker\""), "{json}");
    assert!(json.contains("\"header\""), "{json}");
    assert_eq!(super::request_from_json(&json).expect("it reads"), request);

    // The dialect writes every non-`optional` field, so an empty header is an
    // empty string in the JSON, and reads back as empty.
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    assert!(json.contains("\"header\": \"\""), "{json}");
    let back = super::request_from_json(&json).expect("it reads");
    assert!(back.header.is_empty() && back.generated_marker.is_empty());
}

/// Parses `json` as a JSON object, adds `key: 1` to the object at `path`
/// (object keys from the root) and renders it again.
fn with_unknown_key(json: &str, path: &[&str]) -> String {
    let mut value: serde_json::Value = serde_json::from_str(json).expect("the JSON parses");
    let mut at = &mut value;
    for key in path {
        at = at
            .get_mut(*key)
            .unwrap_or_else(|| panic!("the request has a `{key}` object"));
    }
    at.as_object_mut()
        .expect("the path ends at an object")
        .insert("futureField".to_string(), serde_json::json!(1));
    serde_json::to_string_pretty(&value).expect("the JSON renders")
}

/// A request holding a model, a deployment and a header, so every nesting
/// level the tests add a key to exists.
fn full_request() -> v1::CodegenRequest {
    v1::CodegenRequest {
        schema: super::SCHEMA.to_string(),
        toolchain: "0.2.0".to_string(),
        model: Some(model()),
        options: vec![v1::BackendOption {
            key: "k".to_string(),
            value: "v".to_string(),
        }],
        artifact_base: "veh.common".to_string(),
        deployment: Some(populated_deployment()),
        generated_marker: "@generated by ridl. Do not edit.".to_string(),
        header: "Copyright X".to_string(),
    }
}

#[test]
fn request_from_json_ignores_an_unknown_top_level_key() {
    let request = full_request();
    let json = super::request_to_json(&request).expect("the request renders");
    let extended = with_unknown_key(&json, &[]);
    assert!(extended.contains("futureField"));
    assert_eq!(
        super::request_from_json(&extended).expect("the unknown key is ignored"),
        request
    );
}

#[test]
fn request_from_json_reads_a_key_written_in_its_proto_name() {
    let request = full_request();
    let json = super::request_to_json(&request).expect("the request renders");
    let renamed = json
        .replace("\"artifactBase\"", "\"artifact_base\"")
        .replace("\"generatedMarker\"", "\"generated_marker\"");
    assert!(renamed.contains("\"artifact_base\""));
    assert!(renamed.contains("\"generated_marker\""));
    assert_eq!(
        super::request_from_json(&renamed).expect("the proto-name keys are read"),
        request
    );
}

#[test]
fn request_from_json_reads_null_for_a_message_field_as_absent() {
    let request = full_request();
    let json = super::request_to_json(&request).expect("the request renders");
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("the JSON parses");
    value["model"] = serde_json::Value::Null;
    let nulled = serde_json::to_string(&value).expect("the JSON renders");
    let read = super::request_from_json(&nulled).expect("null reads as absent");
    assert_eq!(read.model, None);
    assert_eq!(read.artifact_base, request.artifact_base);
}

#[test]
fn request_from_json_ignores_an_unknown_key_inside_the_model_and_its_messages() {
    let request = full_request();
    let json = super::request_to_json(&request).expect("the request renders");
    for path in [
        &["model"][..],
        &["deployment"][..],
        &["deployment", "regions", "0"][..],
    ] {
        // `regions` is an array: reach its first element through a custom
        // walk when the path holds an index.
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("the JSON parses");
        let mut at = &mut value;
        for key in path {
            at = match key.parse::<usize>() {
                Ok(index) => &mut at[index],
                Err(_) => &mut at[*key],
            };
        }
        at.as_object_mut()
            .unwrap_or_else(|| panic!("{path:?} is an object"))
            .insert("futureField".to_string(), serde_json::json!({"a": [1]}));
        let extended = serde_json::to_string(&value).expect("the JSON renders");
        assert_eq!(
            super::request_from_json(&extended)
                .unwrap_or_else(|err| panic!("{path:?}: the unknown key is ignored: {err}")),
            request,
            "{path:?}"
        );
    }
    // A nested message of the model: the first declaration.
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("the JSON parses");
    value["model"]["declarations"][0]
        .as_object_mut()
        .expect("a declaration is an object")
        .insert("futureField".to_string(), serde_json::json!(true));
    let extended = serde_json::to_string(&value).expect("the JSON renders");
    assert_eq!(
        super::request_from_json(&extended).expect("the nested unknown key is ignored"),
        request
    );
}

#[test]
fn request_from_json_still_rejects_a_wrong_type() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    let wrong = json.replace("\"artifactBase\": \"a\"", "\"artifactBase\": 5");
    assert_ne!(wrong, json, "the replacement applies");
    super::request_from_json(&wrong).expect_err("a number is not a string");
    super::request_from_json("{").expect_err("malformed JSON");
    super::request_from_json("{\"artifactBase\": \"a\"} x").expect_err("trailing text");
}

#[test]
fn request_from_json_still_applies_the_nesting_cap() {
    let past = format!("{{\"futureField\": {}}}", "[".repeat(1_001));
    let err = super::request_from_json(&past).expect_err("past the ceiling");
    assert!(err.to_string().contains("nests deeper"), "{err}");
}

#[test]
fn every_fixture_request_round_trips_through_the_request_reader() {
    for request in [minimal_request(), full_request()] {
        let json = super::request_to_json(&request).expect("the request renders");
        assert_eq!(
            super::request_from_json(&json).expect("the request reads"),
            request
        );
    }
}

#[test]
fn response_from_json_still_rejects_an_unknown_key() {
    let json = super::response_to_json(&v1::CodegenResponse::default()).expect("it renders");
    let extended = with_unknown_key(&json, &[]);
    super::response_from_json(&extended).expect_err("the response reader stays strict");
}

#[test]
fn request_from_json_rejects_an_unknown_enum_name() {
    let json = super::request_to_json(&full_request()).expect("the request renders");
    let known = [
        "\"CROSSING_SAME_MACHINE\"",
        "\"CROSSING_DIFFERENT_MACHINE\"",
        "\"CROSSING_OFF_BOARD\"",
        "\"ENCODING_PROTO3\"",
        "\"ENCODING_FLATBUFFERS\"",
        "\"VALUE_SOURCE_DERIVED\"",
        "\"VALUE_SOURCE_DECLARED\"",
        "\"VALUE_SOURCE_DEFAULT\"",
    ]
    .into_iter()
    .find(|name| json.contains(name))
    .expect("the full request carries an enum value in the deployment");
    let extended = json.replacen(known, "\"A_VALUE_FROM_A_LATER_RIDL\"", 1);
    super::request_from_json(&extended).expect_err("an unknown enum name is an error");

    // An enum field of the model.
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("the JSON parses");
    let declaration = value["model"]["declarations"][0]
        .as_object_mut()
        .expect("a declaration is an object");
    declaration.insert(
        "visibility".to_string(),
        serde_json::json!("VISIBILITY_FUTURE"),
    );
    let extended = serde_json::to_string(&value).expect("the JSON renders");
    super::request_from_json(&extended).expect_err("an unknown enum name in the model is an error");
}

#[test]
fn request_from_json_ignores_an_unknown_key_inside_a_repeated_message_element() {
    let request = full_request();
    let json = super::request_to_json(&request).expect("the request renders");
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("the JSON parses");
    for path in [["model", "declarations"], ["deployment", "regions"]] {
        let elements = value[path[0]][path[1]]
            .as_array_mut()
            .unwrap_or_else(|| panic!("{path:?} is an array"));
        assert!(!elements.is_empty(), "{path:?} holds an element");
        for element in elements {
            element
                .as_object_mut()
                .expect("an element is an object")
                .insert("futureField".to_string(), serde_json::json!({"a": [1]}));
        }
    }
    let extended = serde_json::to_string(&value).expect("the JSON renders");
    assert_eq!(
        super::request_from_json(&extended).expect("the unknown keys are ignored"),
        request
    );
}

#[test]
fn request_from_json_rejects_a_field_written_in_both_its_names() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    let both = json.replacen("{", "{\"artifact_base\": \"b\",", 1);
    assert!(both.contains("\"artifactBase\"") && both.contains("\"artifact_base\""));
    let err = super::request_from_json(&both).expect_err("one field in two names is a duplicate");
    assert!(err.to_string().contains("duplicate field"), "{err}");
}

#[test]
fn request_from_json_rejects_null_for_a_repeated_field_as_the_strict_reader_does() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    let nulled = json.replace("\"options\": []", "\"options\": null");
    assert_ne!(nulled, json, "the replacement applies");
    let strict = serde_json::from_str::<v1::CodegenRequest>(&nulled);
    assert!(strict.is_err(), "the strict reader rejects null here");
    super::request_from_json(&nulled).expect_err("the request reader rejects it too");
}

#[test]
fn request_from_json_reports_a_second_phase_error_without_a_position() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    let wrong = json.replace("\"artifactBase\": \"a\"", "\"artifactBase\": 5");
    let text = super::request_from_json(&wrong)
        .expect_err("a number is not a string")
        .to_string();
    assert!(!text.contains("line") && !text.contains("column"), "{text}");
}

#[test]
fn request_from_json_rejects_a_field_written_twice() {
    let json = super::request_to_json(&minimal_request()).expect("the request renders");
    let twice = json.replacen("{", "{\"artifactBase\": \"b\",", 1);
    super::request_from_json(&twice).expect_err("a duplicate field is an error");
}

/// `Catalog.compatible` is field 4 on the wire: the binary encoding of one
/// one-byte entry is the tag of field 4 with the length-delimited wire type
/// (`4 << 3 | 2`), the length, then the byte. The JSON and text encodings
/// use the field's name, so only this test reads its number.
#[test]
fn the_compatible_list_is_field_four_on_the_wire() {
    let catalog = v1::Catalog {
        compatible: vec![vec![0xab]],
        ..Default::default()
    };
    assert_eq!(
        prost::Message::encode_to_vec(&catalog),
        vec![0x22, 0x01, 0xab]
    );
}

// ---------------------------------------------------------------------
// Doc links, `see`, `since` and rules (ADR-0028)
// ---------------------------------------------------------------------

/// The field number of every doc field on every doc carrier, of `DocLink`, and
/// of `Rule`. A plugin reads the number, so a renumber is a wire change that
/// neither the JSON nor a round trip through this crate's own impls shows.
#[test]
fn doc_carrier_field_numbers() {
    let pool = v2::codegen_model_descriptor().parent_pool().clone();
    let number = |message: &str, field: &str| -> u32 {
        pool.get_message_by_name(message)
            .unwrap_or_else(|| panic!("the compiled schema declares {message}"))
            .get_field_by_name(field)
            .unwrap_or_else(|| panic!("{message} declares {field}"))
            .number()
    };
    let carriers: [(&str, u32, u32, u32, u32, u32); 8] = [
        ("Declaration", 4, 20, 21, 22, 23),
        ("Field", 5, 8, 9, 10, 11),
        ("EnumValue", 3, 4, 5, 6, 7),
        ("Arm", 4, 7, 8, 9, 10),
        ("Interface", 4, 20, 21, 22, 23),
        ("Interaction", 4, 20, 21, 22, 23),
        ("Service", 3, 8, 9, 10, 11),
        ("Param", 3, 4, 5, 6, 7),
    ];
    for (message, doc, links, see, since, rules) in carriers {
        let message = format!("ridl.codegen.v1.{message}");
        let actual = [
            number(&message, "doc"),
            number(&message, "links"),
            number(&message, "see"),
            number(&message, "since"),
            number(&message, "rules"),
        ];
        assert_eq!(actual, [doc, links, see, since, rules], "{message}");
    }

    let rows: &[(&str, &[(&str, u32)])] = &[
        (
            "DocLink",
            &[("text", 1), ("offset", 2), ("len", 3), ("target", 4)],
        ),
        (
            "Rule",
            &[
                ("text", 1),
                ("range", 10),
                ("step", 11),
                ("unit", 12),
                ("length", 13),
                ("pattern", 14),
                ("collection_bound", 15),
                ("timing", 16),
                ("response_bound", 17),
                ("errors", 18),
                ("contract", 19),
            ],
        ),
        ("Rule.Range", &[("min", 1), ("max", 2)]),
        ("Rule.Length", &[("min", 1), ("max", 2)]),
        ("Rule.CollectionBound", &[("min", 1), ("max", 2)]),
        ("Rule.Timing", &[("mode", 1), ("min_us", 2), ("max_us", 3)]),
        ("Rule.ResponseBound", &[("min_us", 1), ("max_us", 2)]),
        ("Rule.Errors", &[("names", 1)]),
        ("Rule.Contract", &[("kind", 1), ("source", 2)]),
    ];
    for (message, fields) in rows {
        let message = format!("ridl.codegen.v1.{message}");
        let declared = pool
            .get_message_by_name(&message)
            .unwrap_or_else(|| panic!("the compiled schema declares {message}"))
            .fields()
            .count();
        assert_eq!(
            declared,
            fields.len(),
            "{message} declares exactly these fields"
        );
        for (field, expected) in *fields {
            assert_eq!(number(&message, field), *expected, "{message}.{field}");
        }
    }
}

/// The doc text, one link, one `see` entry and one `since` version of one
/// item. Each value is distinct to `tag`, so a value copied from the wrong
/// item is caught.
struct DocParts {
    doc: String,
    links: Vec<v2::DocLink>,
    see: Vec<v2::DocLink>,
    since: Vec<String>,
}

fn doc_parts(tag: &str) -> DocParts {
    DocParts {
        doc: format!("See [{tag}] for details."),
        links: vec![v2::DocLink {
            text: tag.to_string(),
            offset: 4,
            len: tag.len() as u32 + 2,
            target: format!("veh.docs.{tag}"),
        }],
        see: vec![v2::DocLink {
            text: format!("{tag}Ref"),
            offset: 0,
            len: 0,
            target: format!("veh.docs.{tag}Ref"),
        }],
        since: vec![format!("{tag}-1.0")],
    }
}

fn named_decl(name: &str, kind: v2::decl::Kind, tag: &str) -> v2::Decl {
    let DocParts {
        doc,
        links,
        see,
        since,
    } = doc_parts(tag);
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc,
        links,
        see,
        since,
        kind: Some(kind),
        ..Default::default()
    }
}

fn doc_field(ordinal: u32, name: &str, ty: v2::FieldType) -> v2::StructMember {
    let DocParts {
        doc,
        links,
        see,
        since,
    } = doc_parts(name);
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(Box::new(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(ty),
            doc,
            links,
            see,
            since,
            ..Default::default()
        }))),
    }
}

/// One item of each doc carrier, each with a link, a `see` entry and a
/// `since` version, and one item of every rule kind: a ranged unit type with
/// a step, a string with a length and a pattern, an array field with a
/// collection bound, a signal with timing, and a fallible query with a
/// response bound, a `require` and an `ensure`.
fn docs_package() -> v2::Package {
    let speed = v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit("km/h".to_string())),
        }),
        constraint: Some(v2::Constraint {
            min: Some("0.0".to_string()),
            max: Some("250.0".to_string()),
            step: Some("0.5".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let vin = v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            len_min: Some(1),
            len_max: Some(17),
            pattern: Some("^[A-Z0-9]+$".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let gear = v2::EnumDef {
        values: vec![{
            let DocParts {
                doc,
                links,
                see,
                since,
            } = doc_parts("PARK");
            v2::EnumValue {
                name: "PARK".to_string(),
                value: 0,
                doc,
                links,
                see,
                since,
            }
        }],
        reserved: Vec::new(),
    };
    let choice = v2::UnionDef {
        arms: vec![{
            let DocParts {
                doc,
                links,
                see,
                since,
            } = doc_parts("fast");
            v2::UnionArm {
                name: "fast".to_string(),
                ordinal: 1,
                type_ref: "Speed".to_string(),
                doc,
                links,
                see,
                since,
            }
        }],
        ..Default::default()
    };
    let car = v2::StructDef {
        members: vec![
            doc_field(1, "speed", named("Speed")),
            doc_field(2, "vin", named("Vin")),
            doc_field(
                3,
                "stops",
                v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                        element: Some(Box::new(named("Speed"))),
                        min: 1,
                        max: 8,
                    }))),
                },
            ),
        ],
        fixed_layout: false,
    };
    let signal = v2::SignalDef {
        payload: "Speed".to_string(),
        timing: Some(v2::Timing {
            mode: v2::TimingMode::Range as i32,
            min_us: Some("1000".to_string()),
            max_us: Some("2000".to_string()),
            default_applied: false,
        }),
        ..Default::default()
    };
    let query = v2::QueryDef {
        params: vec![{
            let DocParts {
                doc,
                links,
                see,
                since,
            } = doc_parts("vin");
            v2::Param {
                name: "vin".to_string(),
                r#type: Some(named("Vin")),
                doc,
                links,
                see,
                since,
            }
        }],
        return_type: Some(v2::ReturnType {
            kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                ok: "Speed".to_string(),
                err: "DriveError".to_string(),
            })),
        }),
        contracts: vec![
            v2::Contract {
                kind: v2::ContractKind::Require as i32,
                source: "vin != \"\"".to_string(),
                ..Default::default()
            },
            v2::Contract {
                kind: v2::ContractKind::Ensure as i32,
                source: "result >= 0.0".to_string(),
                ..Default::default()
            },
        ],
        timing: Some(v2::Timing {
            mode: v2::TimingMode::Range as i32,
            min_us: Some("100".to_string()),
            max_us: Some("5000".to_string()),
            default_applied: false,
        }),
        family: v2::Family::Dispatch as i32,
    };
    let drive = {
        let DocParts {
            doc,
            links,
            see,
            since,
        } = doc_parts("Drive");
        v2::Interface {
            name: "Drive".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc,
            links,
            see,
            since,
            interactions: vec![
                named_decl("speed", v2::decl::Kind::SignalDef(signal), "speedSignal"),
                named_decl("lookup", v2::decl::Kind::QueryDef(query), "lookup"),
            ],
            number: 1,
            ..Default::default()
        }
    };
    let garage = {
        let DocParts {
            doc,
            links,
            see,
            since,
        } = doc_parts("Garage");
        v2::Service {
            name: "veh.docs.Garage".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc,
            links,
            see,
            since,
            shapes: vec![v2::ServiceShape {
                kind: Some(v2::service_shape::Kind::InterfaceRef("Drive".to_string())),
            }],
            ..Default::default()
        }
    };
    v2::Package {
        name: "veh.docs".to_string(),
        decls: vec![
            named_decl("Speed", v2::decl::Kind::TypeDef(speed), "Speed"),
            named_decl("Vin", v2::decl::Kind::TypeDef(vin), "Vin"),
            named_decl("Gear", v2::decl::Kind::EnumDef(gear), "Gear"),
            named_decl("Choice", v2::decl::Kind::UnionDef(choice), "Choice"),
            named_decl("Car", v2::decl::Kind::StructDef(car), "Car"),
            v2::Decl {
                is_error: true,
                ..named_decl(
                    "DriveError",
                    v2::decl::Kind::StructDef(v2::StructDef::default()),
                    "DriveError",
                )
            },
        ],
        interfaces: vec![drive],
        services: vec![garage],
        ..Default::default()
    }
}

/// A link as a tuple of its four fields, for a field-by-field comparison.
fn ir_link(link: &v2::DocLink) -> (String, u32, u32, String) {
    (
        link.text.clone(),
        link.offset,
        link.len,
        link.target.clone(),
    )
}

fn model_link(link: &v1::DocLink) -> (String, u32, u32, String) {
    (
        link.text.clone(),
        link.offset,
        link.len,
        link.target.clone(),
    )
}

/// Asserts that a carrier's `links`, `see` and `since` equal the IR item's,
/// field by field, and that each is not empty, so the comparison is not
/// between two empty lists.
fn assert_doc_copied(
    what: &str,
    links: (&[v1::DocLink], &[v2::DocLink]),
    see: (&[v1::DocLink], &[v2::DocLink]),
    since: (&[String], &[String]),
) {
    assert!(!links.1.is_empty() && !see.1.is_empty() && !since.1.is_empty());
    assert_eq!(
        links.0.iter().map(model_link).collect::<Vec<_>>(),
        links.1.iter().map(ir_link).collect::<Vec<_>>(),
        "{what}: links"
    );
    assert_eq!(
        see.0.iter().map(model_link).collect::<Vec<_>>(),
        see.1.iter().map(ir_link).collect::<Vec<_>>(),
        "{what}: see"
    );
    assert_eq!(since.0, since.1, "{what}: since");
}

fn ir_decl<'p>(package: &'p v2::Package, name: &str) -> &'p v2::Decl {
    package
        .decls
        .iter()
        .find(|decl| decl.name == name)
        .expect("the IR declaration")
}

fn model_decl<'m>(model: &'m v1::Model, name: &str) -> &'m v1::Declaration {
    model
        .declarations
        .iter()
        .find(|decl| decl.name.as_ref().map(|n| n.declared.as_str()) == Some(name))
        .expect("the model declaration")
}

/// The fixture's one interface, through the shape walk.
fn ir_drive(package: &v2::Package) -> &v2::Interface {
    package
        .shapes()
        .next()
        .expect("the Drive interface")
        .interface
}

fn ir_fields(decl: &v2::Decl) -> Vec<&v2::Field> {
    match decl.kind.as_ref() {
        Some(v2::decl::Kind::StructDef(def)) => def
            .members
            .iter()
            .filter_map(|member| match member.member.as_ref() {
                Some(v2::struct_member::Member::Field(field)) => Some(&**field),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn model_fields(decl: &v1::Declaration) -> Vec<&v1::Field> {
    match decl.kind.as_ref() {
        Some(v1::declaration::Kind::Struct(def)) => def
            .slots
            .iter()
            .filter_map(|slot| match slot.occupant.as_ref() {
                Some(v1::slot::Occupant::Field(field)) => Some(&**field),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ir_params(decl: &v2::Decl) -> &[v2::Param] {
    match decl.kind.as_ref() {
        Some(v2::decl::Kind::CommandDef(command)) => &command.params,
        Some(v2::decl::Kind::QueryDef(query)) => &query.params,
        _ => &[],
    }
}

fn model_params(interaction: &v1::Interaction) -> &[v1::Param] {
    match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Command(command)) => &command.params,
        Some(v1::interaction::Shape::Query(query)) => &query.params,
        _ => &[],
    }
}

#[test]
fn lowering_copies_links_see_and_since() {
    let package = docs_package();
    let model = lower(&package, &[]);

    let ir = ir_decl(&package, "Car");
    let lowered = model_decl(&model, "Car");
    assert_doc_copied(
        "declaration Car",
        (&lowered.links, &ir.links),
        (&lowered.see, &ir.see),
        (&lowered.since, &ir.since),
    );

    let ir_field = ir_fields(ir)[0];
    let field = model_fields(lowered)[0];
    assert_doc_copied(
        "field speed",
        (&field.links, &ir_field.links),
        (&field.see, &ir_field.see),
        (&field.since, &ir_field.since),
    );

    let Some(v2::decl::Kind::EnumDef(ir_enum)) = ir_decl(&package, "Gear").kind.as_ref() else {
        panic!("Gear is an enum");
    };
    let Some(v1::declaration::Kind::Enum(lowered_enum)) = model_decl(&model, "Gear").kind.as_ref()
    else {
        panic!("Gear is lowered as an enum");
    };
    let (ir_value, value) = (&ir_enum.values[0], &lowered_enum.values[0]);
    assert_doc_copied(
        "enum value PARK",
        (&value.links, &ir_value.links),
        (&value.see, &ir_value.see),
        (&value.since, &ir_value.since),
    );

    let Some(v2::decl::Kind::UnionDef(ir_union)) = ir_decl(&package, "Choice").kind.as_ref() else {
        panic!("Choice is a union");
    };
    let Some(v1::declaration::Kind::Union(lowered_union)) =
        model_decl(&model, "Choice").kind.as_ref()
    else {
        panic!("Choice is lowered as a union");
    };
    let (ir_arm, arm) = (&ir_union.arms[0], &lowered_union.arms[0]);
    assert_doc_copied(
        "arm fast",
        (&arm.links, &ir_arm.links),
        (&arm.see, &ir_arm.see),
        (&arm.since, &ir_arm.since),
    );

    let ir_interface = ir_drive(&package);
    let interface = interface_named(&model, "Drive");
    assert_doc_copied(
        "interface Drive",
        (&interface.links, &ir_interface.links),
        (&interface.see, &ir_interface.see),
        (&interface.since, &ir_interface.since),
    );

    let ir_lookup = &ir_interface.interactions[1];
    let lookup = live_member(interface, "lookup");
    assert_doc_copied(
        "interaction lookup",
        (&lookup.links, &ir_lookup.links),
        (&lookup.see, &ir_lookup.see),
        (&lookup.since, &ir_lookup.since),
    );

    let (ir_param, param) = (&ir_params(ir_lookup)[0], &model_params(lookup)[0]);
    assert!(!ir_param.doc.is_empty());
    assert_eq!(param.doc, ir_param.doc, "parameter vin: doc");
    assert_doc_copied(
        "parameter vin",
        (&param.links, &ir_param.links),
        (&param.see, &ir_param.see),
        (&param.since, &ir_param.since),
    );

    let (ir_service, service) = (&package.services[0], &model.services[0]);
    assert_doc_copied(
        "service Garage",
        (&service.links, &ir_service.links),
        (&service.see, &ir_service.see),
        (&service.since, &ir_service.since),
    );
}

/// Whether the model rule's `oneof` member is the one that matches the
/// `rules::Rule` variant, with the same values.
fn kind_matches(rule: &crate::rules::Rule, kind: Option<&v1::rule::Kind>) -> bool {
    use crate::rules::Rule;
    use v1::rule::Kind;
    match (rule, kind) {
        (Rule::Range { min, max }, Some(Kind::Range(range))) => {
            range.min == *min && range.max == *max
        }
        (Rule::Step(step), Some(Kind::Step(other))) => step == other,
        (Rule::Unit(unit), Some(Kind::Unit(other))) => unit == other,
        (Rule::Length { min, max }, Some(Kind::Length(length))) => {
            length.min == *min && length.max == *max
        }
        (Rule::Pattern(pattern), Some(Kind::Pattern(other))) => pattern == other,
        (Rule::CollectionBound { min, max }, Some(Kind::CollectionBound(bound))) => {
            bound.min == *min && bound.max == *max
        }
        (
            Rule::Timing {
                mode,
                min_us,
                max_us,
            },
            Some(Kind::Timing(timing)),
        ) => timing.mode == *mode as i32 && timing.min_us == *min_us && timing.max_us == *max_us,
        (Rule::ResponseBound { min_us, max_us }, Some(Kind::ResponseBound(bound))) => {
            bound.min_us == *min_us && bound.max_us == *max_us
        }
        (Rule::Errors(names), Some(Kind::Errors(errors))) => errors.names == *names,
        (Rule::Contract { kind, source }, Some(Kind::Contract(contract))) => {
            contract.kind == *kind as i32 && contract.source == *source
        }
        _ => false,
    }
}

/// The variant name of a `rules::Rule`, to check that the fixture reaches
/// every kind.
fn variant(rule: &crate::rules::Rule) -> &'static str {
    use crate::rules::Rule;
    match rule {
        Rule::Range { .. } => "range",
        Rule::Step(_) => "step",
        Rule::Unit(_) => "unit",
        Rule::Length { .. } => "length",
        Rule::Pattern(_) => "pattern",
        Rule::CollectionBound { .. } => "collection_bound",
        Rule::Timing { .. } => "timing",
        Rule::ResponseBound { .. } => "response_bound",
        Rule::Errors(_) => "errors",
        Rule::Contract { .. } => "contract",
    }
}

/// Asserts that `lowered` is `expected`, in order: each `text` is
/// `rules::render` of the matching rule, and each `oneof` member matches the
/// rule's variant. Returns the variants seen.
fn assert_rules(
    what: &str,
    lowered: &[v1::Rule],
    expected: &[crate::rules::Rule],
) -> Vec<&'static str> {
    assert_eq!(
        lowered
            .iter()
            .map(|rule| rule.text.clone())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(crate::rules::render)
            .collect::<Vec<_>>(),
        "{what}: rule text"
    );
    for (rule, model_rule) in expected.iter().zip(lowered) {
        assert!(
            kind_matches(rule, model_rule.kind.as_ref()),
            "{what}: {rule:?} lowered as {:?}",
            model_rule.kind
        );
    }
    expected.iter().map(variant).collect()
}

#[test]
fn model_rules_text_matches_render() {
    use crate::rules::{ItemRef, rules};

    let package = docs_package();
    let model = lower(&package, &[]);
    // The lowered package followed by the others, as `lower` passes them.
    let deps = [&package];
    let mut seen: Vec<&'static str> = Vec::new();

    assert_eq!(model.declarations.len(), package.decls.len());
    for (decl, lowered) in package.decls.iter().zip(&model.declarations) {
        let expected = rules(&package, &deps, ItemRef::Decl(decl));
        seen.extend(assert_rules(&decl.name, &lowered.rules, &expected));
        let (fields, model_fields) = (ir_fields(decl), model_fields(lowered));
        assert_eq!(fields.len(), model_fields.len());
        for (field, lowered) in fields.into_iter().zip(model_fields) {
            let expected = rules(&package, &deps, ItemRef::Field(field));
            seen.extend(assert_rules(&field.name, &lowered.rules, &expected));
        }
    }

    let interface = ir_drive(&package);
    let lowered_interface = interface_named(&model, "Drive");
    for decl in &interface.interactions {
        let lowered = live_member(lowered_interface, &decl.name);
        let expected = rules(&package, &deps, ItemRef::Decl(decl));
        seen.extend(assert_rules(&decl.name, &lowered.rules, &expected));
        let (params, model_params) = (ir_params(decl), model_params(lowered));
        assert_eq!(params.len(), model_params.len());
        for (param, lowered) in params.iter().zip(model_params) {
            let expected = rules(&package, &deps, ItemRef::Param(param));
            seen.extend(assert_rules(&param.name, &lowered.rules, &expected));
        }
    }

    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        [
            "collection_bound",
            "contract",
            "errors",
            "length",
            "pattern",
            "range",
            "response_bound",
            "step",
            "timing",
            "unit",
        ],
        "the fixture reaches every rule kind"
    );
}

#[test]
fn rules_are_empty_where_no_item_ref_exists() {
    let package = docs_package();
    let model = lower(&package, &[]);

    let Some(v1::declaration::Kind::Enum(gear)) = model_decl(&model, "Gear").kind.as_ref() else {
        panic!("Gear is lowered as an enum");
    };
    assert!(gear.values[0].rules.is_empty(), "enum value");
    // The arm's type is the ranged `Speed`: an arm that took its type's rules
    // would fail here.
    let Some(v1::declaration::Kind::Union(choice)) = model_decl(&model, "Choice").kind.as_ref()
    else {
        panic!("Choice is lowered as a union");
    };
    assert!(choice.arms[0].rules.is_empty(), "union arm");
    assert!(
        interface_named(&model, "Drive").rules.is_empty(),
        "interface"
    );
    assert!(model.services[0].rules.is_empty(), "service");
}

#[test]
fn foreign_declaration_carries_rules() {
    let units = v2::Package {
        name: "veh.units".to_string(),
        decls: vec![
            v2::Decl {
                name: "Kph".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(
                            v2::PrimitiveType::Integer as i32,
                        )),
                    }),
                    constraint: Some(v2::Constraint {
                        min: Some("0".to_string()),
                        max: Some("300".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                // A bare reference: it resolves only in `veh.units`, the package
                // that declares `Leg`, and not in the lowered package.
                name: "Leg".to_string(),
                visibility: v2::Visibility::Public as i32,
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![member(1, "speed", named("Kph"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let app = v2::Package {
        name: "veh.app".to_string(),
        decls: vec![v2::Decl {
            name: "Trip".to_string(),
            visibility: v2::Visibility::Public as i32,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    member(1, "speed", named("veh.units.Kph")),
                    member(2, "leg", named("veh.units.Leg")),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = lower(&app, &[&units]);
    let texts = |rules: &[v1::Rule]| -> Vec<String> {
        rules.iter().map(|rule| rule.text.clone()).collect()
    };

    let field = model_fields(model_decl(&model, "Trip"))[0];
    assert_eq!(texts(&field.rules), ["range 0 ..= 300"]);

    let foreign = |name: &str| -> &v1::Declaration {
        model
            .foreign
            .iter()
            .filter(|foreign| foreign.package == "veh.units")
            .filter_map(|foreign| foreign.declaration.as_ref())
            .find(|decl| decl.name.as_ref().map(|n| n.declared.as_str()) == Some(name))
            .unwrap_or_else(|| panic!("the foreign declaration {name}"))
    };
    assert_eq!(texts(&foreign("Kph").rules), ["range 0 ..= 300"]);

    // The field of a foreign struct resolves its bare type name in the
    // package that declares the struct, not in the lowered package.
    let leg_field = model_fields(foreign("Leg"))[0];
    assert_eq!(texts(&leg_field.rules), ["range 0 ..= 300"]);
}

/// A tuple reached from a parameter is named from the interface and the
/// interaction, so it is emitted rather than skipped (ADR-0016).
#[test]
fn a_tuple_in_a_parameter_is_named_from_its_interaction() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "withSpan",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![v2::Param {
                name: "span".to_string(),
                r#type: Some(v2::FieldType {
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![v2::TupleField {
                            name: "lo".to_string(),
                            r#type: Some(named("Pair")),
                        }],
                    })),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let names: Vec<&str> = model
        .tuples
        .iter()
        .filter_map(|tuple| tuple.name.as_ref())
        .map(|name| name.rust.as_str())
        .collect();
    assert!(
        names.contains(&"CallsWithSpanParamSpan"),
        "the induced name is spelled from the interface and the interaction: {names:?}"
    );
}

/// A call with two parameters induces one argument struct, with one field
/// per parameter in declaration order (driftsys/ridl#713).
#[test]
fn a_two_parameter_command_induces_an_argument_struct() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("the `two` command");
    };
    let reference = command
        .request_tuple
        .as_ref()
        .expect("a two-parameter call induces an argument struct");
    assert!(command.request.is_none(), "and carries no named request");
    let induced = &model.tuples[reference.index as usize];
    assert_eq!(
        induced.name.as_ref().expect("a name").rust,
        "CallsTwoParams"
    );
    let fields: Vec<&str> = induced
        .fields
        .iter()
        .map(|field| field.name.as_ref().expect("a name").declared.as_str())
        .collect();
    assert_eq!(fields, vec!["a", "b"]);
}

/// A call with exactly one named parameter keeps the payload it has today
/// and induces nothing, so no existing face renames a type.
#[test]
fn a_one_parameter_command_induces_nothing() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 1) else {
        panic!("the `one` command");
    };
    assert!(command.request.is_some());
    assert!(command.request_tuple.is_none());
}

/// A call with no parameter induces an argument struct with no field: the
/// shape that skipped `readTicks` and `fuelLevel` before this change.
#[test]
fn a_zero_parameter_command_induces_an_empty_argument_struct() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "ping",
        v2::decl::Kind::CommandDef(v2::CommandDef::default()),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsPingParams")
        })
        .expect("the induced argument struct");
    assert!(induced.fields.is_empty());
}

/// A parameter's doc reaches the field of the induced argument struct, so
/// the generated struct documents its fields (ADR-0028).
#[test]
fn a_parameter_doc_reaches_the_induced_field() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "documented",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![
                v2::Param {
                    name: "first".to_string(),
                    r#type: Some(named("Pair")),
                    doc: "The first one.".to_string(),
                    ..Default::default()
                },
                v2::Param {
                    name: "second".to_string(),
                    r#type: Some(named("Pair")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsDocumentedParams")
        })
        .expect("the induced argument struct");
    assert_eq!(induced.fields[0].doc, "The first one.");
    assert_eq!(induced.fields[1].doc, "");
}

/// The doc reaches the field of the argument struct only. A tuple written
/// inside a parameter is a type of its own, and its fields carry no doc, as
/// the IR has none for them.
#[test]
fn a_tuple_inside_a_documented_parameter_keeps_undocumented_fields() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "withSpan",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![
                v2::Param {
                    name: "span".to_string(),
                    r#type: Some(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                            fields: vec![v2::TupleField {
                                name: "lo".to_string(),
                                r#type: Some(named("Pair")),
                            }],
                        })),
                        ..Default::default()
                    }),
                    doc: "The span.".to_string(),
                    ..Default::default()
                },
                v2::Param {
                    name: "other".to_string(),
                    r#type: Some(named("Pair")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let by_name = |wanted: &str| {
        model
            .tuples
            .iter()
            .find(|tuple| tuple.name.as_ref().is_some_and(|name| name.rust == wanted))
            .unwrap_or_else(|| panic!("the induced tuple {wanted}"))
    };
    assert_eq!(by_name("CallsWithSpanParams").fields[0].doc, "The span.");
    assert_eq!(by_name("CallsWithSpanParamSpan").fields[0].doc, "");
}

/// A query's request follows the same rule as a command's: the `none` query
/// of the fixture declares no parameter, so it induces an empty struct.
#[test]
fn a_query_request_follows_the_command_rule() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 2) else {
        panic!("the `none` query");
    };
    let reference = query
        .request_tuple
        .as_ref()
        .expect("a zero-parameter query induces an argument struct");
    let induced = &model.tuples[reference.index as usize];
    assert_eq!(
        induced.name.as_ref().expect("a name").rust,
        "CallsNoneParams"
    );
    assert!(induced.fields.is_empty());
}

/// A tuple reply induces a reply struct, one field per tuple field.
#[test]
fn a_tuple_reply_induces_a_reply_struct() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "bounds",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![
                            v2::TupleField {
                                name: "min".to_string(),
                                r#type: Some(named("Pair")),
                            },
                            v2::TupleField {
                                name: "max".to_string(),
                                r#type: Some(named("Pair")),
                            },
                        ],
                    })),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsBoundsReply")
        })
        .expect("the induced reply struct");
    assert_eq!(
        model
            .tuples
            .iter()
            .filter(|tuple| tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsBoundsReply"))
            .count(),
        1,
        "the tuple written in the return type and the induced reply struct \
         reach one name, so the claim is reused"
    );
    let fields: Vec<&str> = induced
        .fields
        .iter()
        .map(|field| field.name.as_ref().expect("a name").declared.as_str())
        .collect();
    assert_eq!(fields, vec!["min", "max"]);
}

/// A fallible reply is a sum, not a product, so it induces nothing: the
/// `risky` query of the fixture returns `Pair | Pair`.
#[test]
fn a_fallible_reply_induces_nothing() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 3) else {
        panic!("the `risky` query");
    };
    assert!(query.reply_tuple.is_none());
}

/// Every named induced tuple has a FlatBuffers table, so the codec can emit
/// its encode, verify and decode functions.
#[test]
fn every_named_induced_tuple_has_a_flatbuffers_table() {
    let model = lower(&call_package(), &[]);
    for tuple in &model.tuples {
        let Some(name) = tuple.name.as_ref() else {
            continue;
        };
        if name.rust.is_empty() {
            continue;
        }
        assert!(
            tuple.flatbuffers_table.is_some(),
            "`{}` has no FlatBuffers table",
            name.rust
        );
    }
}

/// An induced argument struct crosses the wire on its own, so it is a
/// FlatBuffers root: the Rust codec emits its `Payload` impl from the root,
/// and every descriptor's reservation row names that impl's `MAX_SIZE`.
#[test]
fn an_induced_argument_struct_is_a_flatbuffers_root() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("the `two` command");
    };
    let index = command
        .request_tuple
        .as_ref()
        .expect("a request tuple")
        .index;
    let projection = model.flatbuffers.as_ref().expect("a projection");
    assert!(
        projection
            .roots
            .iter()
            .any(|root| root.tuple == Some(index)),
        "the induced argument struct is a root"
    );
}

/// An induced struct holding an unbounded field has no finite bound, so its
/// root takes the unbounded arm with the attribution of the field that is
/// open, the way a declaration's root does.
#[test]
fn an_unbounded_induced_argument_struct_is_rooted_unbounded() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "label",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![
                v2::Param {
                    name: "pair".to_string(),
                    r#type: Some(named("Pair")),
                    ..Default::default()
                },
                v2::Param {
                    name: "tagged".to_string(),
                    r#type: Some(named("Tagged")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let index = model
        .tuples
        .iter()
        .position(|tuple| {
            tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsLabelParams")
        })
        .expect("the induced argument struct") as u32;
    let root = model
        .flatbuffers
        .as_ref()
        .expect("a projection")
        .roots
        .iter()
        .find(|root| root.tuple == Some(index))
        .expect("the induced argument struct is a root");
    let Some(v1::fb_root::Bound::Unbounded(unbounded)) = root.bound.as_ref() else {
        panic!("a struct holding an unbounded field has no finite bound");
    };
    assert_eq!(unbounded.cause, v1::FbUnboundedCause::Member as i32);
    assert_eq!(unbounded.member.as_deref(), Some("tagged"));
}
