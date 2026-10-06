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
        ["schema", "toolchain", "model", "options", "artifactBase"],
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
fn a_two_parameter_command_has_an_undefined_request_size() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("a command");
    };
    assert_eq!(state(&command.request_sizes, true), &undefined());
    assert_eq!(state(&command.request_sizes, false), &undefined());
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
fn a_zero_parameter_query_request_is_undefined() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 2) else {
        panic!("a query");
    };
    assert_eq!(state(&query.request_sizes, true), &undefined());
    assert_eq!(state(&query.request_sizes, false), &undefined());
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
fn a_query_with_no_return_type_spells_its_reply_as_a_unit() {
    // The request is bounded, so the reservation walks past it and names the
    // reply: that is the only place the reply's text is visible. A reply text
    // spelled any other way fails here.
    let model = lower(&bare_package(), &[]);
    let quiet = live_member(interface_named(&model, "Bare"), "quiet");
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&quiet.reservation, proto3),
            &unsized_by("quiet.reply: ()")
        );
    }
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
        // level (1) + ask (2) + set (1)
        let one = pair_size(&model, proto3);
        assert_eq!(reserved(&tally.table_budget, proto3), &bytes(4 * one));
    }
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
    assert_eq!(tally.slots.len(), 4, "the tombstone holds its slot");
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
fn a_query_with_no_return_type_has_undefined_reply_sizes() {
    let model = lower(&bare_package(), &[]);
    let blank = live_member(interface_named(&model, "Bare"), "blank");
    let Some(v1::interaction::Shape::Query(query)) = blank.shape.as_ref() else {
        panic!("a query");
    };
    let expected = v1::PayloadSizes {
        proto3: Some(v1::SizeState {
            state: Some(v1::size_state::State::Absent(v1::SizeAbsent {
                cause: 1,
                detail: None,
            })),
        }),
        flatbuffers: Some(v1::SizeState {
            state: Some(v1::size_state::State::Absent(v1::SizeAbsent {
                cause: 1,
                detail: None,
            })),
        }),
    };
    assert_eq!(query.reply_sizes, Some(expected));
    // Neither payload is bounded: the first one, the request, is named.
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&blank.reservation, proto3),
            &unsized_by("blank.request: ()")
        );
    }
}

#[test]
fn a_fixed_member_with_no_payload_is_unsized_and_says_so() {
    let model = lower(&bare_package(), &[]);
    let tick = live_member(interface_named(&model, "Bare"), "tick");
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&tick.reservation, proto3),
            &unsized_by("tick.payload: no payload")
        );
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

/// The `Kinds` interface: five fixed members whose payloads are, in order, a
/// primitive, an inline scalar, a map, an array of a named type, and a
/// payload position that declares no type at all. No codec defines a size for
/// any of them, so each reservation is unsized and states the payload's kind.
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
    let fixed = |name: &str, payload: v2::FieldType| v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(v2::decl::Kind::FixedDef(v2::FixedDef {
            payload: Some(payload),
        })),
        ..Default::default()
    };
    let kinds = v2::Interface {
        name: "Kinds".to_string(),
        visibility: v2::Visibility::Public as i32,
        interactions: vec![
            fixed("flag", primitive),
            fixed("count", inline),
            fixed("table", map),
            fixed("list", array),
            fixed(
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
fn an_unsized_fixed_member_states_the_kind_of_its_payload() {
    // Each kind is named distinctly, so a lowering that spelled two kinds
    // the same way fails here.
    let model = lower(&kinds_package(), &[]);
    let kinds = interface_named(&model, "Kinds");
    let expected = [
        ("flag", "flag.payload: primitive"),
        ("count", "count.payload: inline scalar"),
        ("table", "table.payload: map"),
        ("list", "list.payload: array of Pair"),
        // A field position with no kind at all: its own word, not `map` and
        // not `no payload`, which is the member that carries no payload.
        ("void", "void.payload: no type"),
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
fn a_multi_parameter_request_names_every_parameter() {
    // The request of a call with several parameters has no encoding, and the
    // reservation spells the whole parameter list: a join that dropped a
    // name, or kept only the first, fails here.
    let model = lower(&call_package(), &[]);
    let two = live_member(interface_named(&model, "Calls"), "two");
    for proto3 in [true, false] {
        assert_eq!(
            reserved(&two.reservation, proto3),
            &unsized_by("two.request: (a, b)")
        );
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
  }
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
