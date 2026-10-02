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
        member: Some(v2::struct_member::Member::Field(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(ty),
            init: Some(v2::InitValue {
                derivable: true,
                value: None,
            }),
            ..Default::default()
        })),
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
                        },
                        v2::UnionArm {
                            name: "own".to_string(),
                            ordinal: 2,
                            type_ref: "Own".to_string(),
                            doc: String::new(),
                        },
                        // The scope's own enum, spelled with its package: the
                        // spelling a text test for a dot calls foreign while
                        // the declaring package is the scope's.
                        v2::UnionArm {
                            name: "qualified".to_string(),
                            ordinal: 3,
                            type_ref: "b.Own".to_string(),
                            doc: String::new(),
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
        "the request's model is the artifact byte for byte, indented once more"
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
