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

/// Every declared identifier carries the three pinned transforms and the one
/// composition the wire backends make of them (design note D-2).
#[test]
fn every_identifier_carries_its_four_spellings() {
    let model = model();
    let name = model.declarations[2].name.as_ref().expect("a name");
    assert_eq!(name.declared, "Dashboard");
    assert_eq!(name.snake, "dashboard");
    assert_eq!(name.camel, "Dashboard");
    assert_eq!(name.screaming, "DASHBOARD");

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
