//! The fact-level drift test (the codegen model design note §8.1).
//!
//! No backend reads the lowered codegen model yet: this one derives the
//! FlatBuffers projection from the raw IR, and `ridl_ir::codegen::lower`
//! derives it again for the model a plugin will read. Two derivations of one
//! fact drift, and the pull request that introduces the divergence is where
//! it has to fail — so the emitted `.fbs` is read back, the facts the model
//! also states are extracted, and each is asserted equal to the model's
//! field, over the fixtures this crate already compiles.
//!
//! What is compared, per table the model lists: the table exists in the
//! schema under that name, its field ids are the model's `FbSlot.id` in the
//! model's order, each field's **name** is the one the model's `FbSlot.source`
//! spells, its `= null` markers are the model's `needs_null_default`, and each
//! field's type text is what the model's `FbWire` spells. Beyond the tables:
//! each union's member names, and each interface's identity table.
//!
//! It is the form of drift test stage P2b can write. When a backend stops
//! deriving a fact and reads it from the model instead, that fact is a
//! function of the model by construction and leaves this file.

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

/// One field line of an emitted table: its name, its type text, its id, and
/// whether it carries an explicit `= null` default.
#[derive(Debug, PartialEq, Eq)]
struct Field {
    name: String,
    type_text: String,
    id: u32,
    null: bool,
    deprecated: bool,
}

#[derive(Default)]
struct Schema {
    tables: BTreeMap<String, Vec<Field>>,
    unions: BTreeMap<String, Vec<String>>,
    enums: BTreeMap<String, Vec<(String, i64)>>,
}

/// Reads the emitted schema back into the facts the model also states.
fn read_back(source: &str) -> Schema {
    let mut schema = Schema::default();
    let mut table: Option<(String, Vec<Field>)> = None;
    let mut enumeration: Option<(String, Vec<(String, i64)>)> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("namespace ")
            || trimmed.starts_with("include ")
        {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("union ") {
            let (name, body) = rest.split_once('{').expect("a union line carries a body");
            let body = body.trim_end_matches('}').trim();
            let members = if body.is_empty() {
                Vec::new()
            } else {
                body.split(',')
                    .map(|member| {
                        let (member, _) = member
                            .split_once(':')
                            .expect("every arm is emitted in the alias form");
                        member.trim().to_string()
                    })
                    .collect()
            };
            schema.unions.insert(name.trim().to_string(), members);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("enum ") {
            let name = rest
                .split_once(':')
                .map(|(name, _)| name)
                .unwrap_or(rest)
                .trim()
                .to_string();
            enumeration = Some((name, Vec::new()));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("table ") {
            table = Some((rest.trim_end_matches('{').trim().to_string(), Vec::new()));
            continue;
        }
        if trimmed == "}" {
            if let Some((name, fields)) = table.take() {
                schema.tables.insert(name, fields);
            }
            if let Some((name, values)) = enumeration.take() {
                schema.enums.insert(name, values);
            }
            continue;
        }
        if let Some((_, values)) = enumeration.as_mut() {
            let entry = trimmed.trim_end_matches(',');
            let (name, value) = entry
                .split_once(" = ")
                .expect("an enum value carries a number");
            values.push((
                name.trim().to_string(),
                value.trim().parse().expect("an integer enum value"),
            ));
            continue;
        }
        let Some((_, fields)) = table.as_mut() else {
            panic!("`{trimmed}` is not a line this reader knows how to read");
        };
        let entry = trimmed.trim_end_matches(';');
        let (name, rest) = entry.split_once(':').expect("a field line carries a type");
        let (type_and_default, attributes) = rest
            .rsplit_once('(')
            .expect("a field line carries an id clause");
        let attributes = attributes.trim_end_matches(')');
        let id = attributes
            .split(',')
            .find_map(|part| part.trim().strip_prefix("id: "))
            .expect("a field line names its id")
            .trim()
            .parse()
            .expect("an integer id");
        let null = type_and_default.contains("= null");
        let type_text = type_and_default
            .split("= null")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        fields.push(Field {
            name: name.trim().to_string(),
            type_text,
            id,
            null,
            deprecated: attributes.contains("deprecated"),
        });
    }
    schema
}

/// The type text one wire kind spells in a `.fbs` schema.
fn wire_text(model: &v1::Model, wire: Option<&v1::FbWire>) -> Option<String> {
    let tables = &model.flatbuffers.as_ref()?.tables;
    Some(match wire?.kind.as_ref()? {
        v1::fb_wire::Kind::Scalar(scalar) => {
            match v1::FbScalarType::try_from(scalar.r#type).ok()? {
                v1::FbScalarType::Bool => "bool",
                v1::FbScalarType::Byte => "byte",
                v1::FbScalarType::Ubyte => "ubyte",
                v1::FbScalarType::Short => "short",
                v1::FbScalarType::Ushort => "ushort",
                v1::FbScalarType::Int => "int",
                v1::FbScalarType::Uint => "uint",
                v1::FbScalarType::Long => "long",
                v1::FbScalarType::Ulong => "ulong",
                v1::FbScalarType::Float => "float",
                v1::FbScalarType::Double => "double",
                v1::FbScalarType::Unspecified => return None,
            }
            .to_string()
        }
        // A FlatBuffers enum field is spelled with the enum's own name, local
        // or qualified — which is exactly the canonical reference the model
        // carries.
        v1::fb_wire::Kind::Enum(enumeration) => enumeration.r#type.as_ref()?.reference.clone(),
        v1::fb_wire::Kind::Text(_) => "string".to_string(),
        v1::fb_wire::Kind::Bytes(_) => "[ubyte]".to_string(),
        v1::fb_wire::Kind::Table(index) => tables.get(*index as usize)?.name.clone(),
        v1::fb_wire::Kind::UnionWrapper(index) => tables.get(*index as usize)?.name.clone(),
        v1::fb_wire::Kind::Vector(vector) => {
            format!("[{}]", wire_text(model, vector.element.as_deref())?)
        }
        v1::fb_wire::Kind::Map(map) => {
            format!("[{}]", tables.get(map.entry_table as usize)?.name)
        }
    })
}

/// Every table the model lists is in the schema, with the ids, the `= null`
/// markers and the type texts the model states.
/// The field name a slot spells, from the model alone.
///
/// The emitter's rules, in `crates/ridl-backend-flatbuffers/src/lib.rs`: a
/// live field takes its own `snake_case` spelling, a retired ordinal takes
/// `reserved_<ordinal>`, a tuple position takes `field_<position>`, a map
/// entry's two slots are `key` and `value`, and a wrapped union value is
/// `value`. Every one of them is a name two emitters must agree on, which is
/// what makes it a model fact rather than a printer's business.
fn expected_field_name(
    model: &v1::Model,
    table: &v1::FbTable,
    slot: &v1::FbSlot,
) -> Option<String> {
    match slot.source.as_ref()? {
        v1::fb_slot::Source::Field(index) => {
            let declaration = match table.source.as_ref()? {
                v1::fb_table::Source::StructDeclaration(decl) => {
                    model.declarations.get(*decl as usize)?
                }
                _ => return None,
            };
            let v1::declaration::Kind::Struct(structure) = declaration.kind.as_ref()? else {
                return None;
            };
            let slot = structure.slots.get(*index as usize)?;
            let v1::slot::Occupant::Field(field) = slot.occupant.as_ref()? else {
                return None;
            };
            Some(field.name.as_ref()?.snake.clone())
        }
        v1::fb_slot::Source::RetiredOrdinal(ordinal) => Some(format!("reserved_{ordinal}")),
        v1::fb_slot::Source::TuplePosition(position) => Some(format!("field_{position}")),
        v1::fb_slot::Source::Key(_) => Some("key".to_string()),
        v1::fb_slot::Source::Value(_) | v1::fb_slot::Source::Wrapped(_) => {
            Some("value".to_string())
        }
    }
}

fn assert_tables_agree(label: &str, model: &v1::Model, schema: &Schema) {
    let projection = model.flatbuffers.as_ref().expect("a lowered projection");
    assert!(
        !projection.tables.is_empty(),
        "{label}: the projection lists no table at all"
    );
    for table in &projection.tables {
        let emitted = schema
            .tables
            .get(&table.name)
            .unwrap_or_else(|| panic!("{label}: the schema declares no table `{}`", table.name));
        let ids: Vec<u32> = emitted.iter().map(|field| field.id).collect();
        let expected: Vec<u32> = table.slots.iter().map(|slot| slot.id).collect();
        assert_eq!(
            ids, expected,
            "{label}: table `{}` emits ids {ids:?}, the model states {expected:?}",
            table.name
        );
        for (slot, field) in table.slots.iter().zip(emitted.iter()) {
            if let Some(name) = expected_field_name(model, table, slot) {
                assert_eq!(
                    field.name, name,
                    "{label}: table `{}` emits the field `{}` where the model spells `{name}`",
                    table.name, field.name
                );
            }
            assert_eq!(
                field.null, slot.needs_null_default,
                "{label}: `{}.{}` emits `= null` = {}, the model states {}",
                table.name, field.name, field.null, slot.needs_null_default
            );
            // A deprecated placeholder holds a retired ordinal's slot; its
            // declared type is inert, so the model states it and the schema
            // spells `ubyte` for it either way.
            let Some(text) = wire_text(model, slot.wire.as_ref()) else {
                // A union wrapper's value slot and a foreign reference carry
                // no wire kind: the arm set is on `Union.arms`, and a foreign
                // table is declared by the package that owns it.
                continue;
            };
            assert_eq!(
                field.type_text, text,
                "{label}: `{}.{}` emits `{}`, the model states `{text}`",
                table.name, field.name, field.type_text
            );
            assert!(
                !field.deprecated
                    || matches!(slot.source, Some(v1::fb_slot::Source::RetiredOrdinal(_))),
                "{label}: `{}.{}` is deprecated but the model does not hold a retired ordinal there",
                table.name,
                field.name
            );
        }
    }
}

/// Each union's member names are its arms' snake_case spelling, in
/// declaration order, and the discriminant each takes is its ordinal.
fn assert_unions_agree(label: &str, model: &v1::Model, schema: &Schema) {
    for declaration in &model.declarations {
        let Some(v1::declaration::Kind::Union(def)) = declaration.kind.as_ref() else {
            continue;
        };
        let name = &declaration.name.as_ref().expect("a name").declared;
        let emitted = schema
            .unions
            .get(&format!("{name}Union"))
            .unwrap_or_else(|| panic!("{label}: the schema declares no union for `{name}`"));
        let expected: Vec<String> = def
            .arms
            .iter()
            .map(|arm| arm.name.as_ref().expect("an arm name").snake.clone())
            .collect();
        assert_eq!(
            *emitted, expected,
            "{label}: union `{name}` emits members {emitted:?}, the model states {expected:?}"
        );
        for arm in &def.arms {
            assert_eq!(
                arm.discriminant,
                arm.ordinal,
                "{label}: the discriminant of `{name}.{}` is its ordinal",
                arm.name.as_ref().expect("an arm name").declared
            );
        }
    }
}

/// Each interface's identity table is `<joined CamelCase name>Ordinal`,
/// holding one SCREAMING_SNAKE member per live interaction at its ordinal.
fn assert_identity_tables_agree(label: &str, model: &v1::Model, schema: &Schema) {
    for interface in &model.interfaces {
        let name = match interface.identity.as_ref().expect("an identity") {
            v1::interface::Identity::Declared(declared) => declared.declared.clone(),
            v1::interface::Identity::Service(service) => service.joined_camel.clone(),
        };
        let emitted = schema
            .enums
            .get(&format!("{name}Ordinal"))
            .unwrap_or_else(|| {
                panic!("{label}: the schema declares no identity table for `{name}`")
            });
        let expected: Vec<(String, i64)> = interface
            .slots
            .iter()
            .filter_map(|slot| match slot.occupant.as_ref()? {
                v1::interaction_slot::Occupant::Interaction(interaction) => Some((
                    interaction
                        .name
                        .as_ref()
                        .expect("an interaction name")
                        .screaming
                        .clone(),
                    i64::from(slot.ordinal),
                )),
                v1::interaction_slot::Occupant::Retired(_) => None,
            })
            .collect();
        assert_eq!(
            *emitted, expected,
            "{label}: the identity table of `{name}` emits {emitted:?}, the model states \
             {expected:?}"
        );
    }
}

fn compile_fixture(relative_to_fixtures: &str) -> v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures")
        .join(relative_to_fixtures);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let output = ridlc::compile(&path.display().to_string(), &text);
    assert!(
        output.diagnostics.is_empty(),
        "{} must compile with no diagnostic, got: {:?}",
        path.display(),
        output.diagnostics,
    );
    output.package
}

#[test]
fn the_emitted_schema_states_the_facts_the_model_states() {
    let package = compile_fixture("cruise.ridl");
    let generated = ridl_backend_flatbuffers::generate(&package).expect("generate");
    support::compile_with_planus("veh.cruise.fbs", &generated.fbs_source);
    let schema = read_back(&generated.fbs_source);
    let model = codegen::lower(&package, &[]);
    assert_tables_agree("cruise.ridl", &model, &schema);
    assert_unions_agree("cruise.ridl", &model, &schema);
    assert_identity_tables_agree("cruise.ridl", &model, &schema);
}

/// The same comparison across a package boundary, over the two-package
/// fixture this crate's corpus test already compiles. A foreign enum is the
/// one reference whose schema spelling the model has to carry qualified, and
/// a foreign struct is the one whose table this package's schema does not
/// declare at all.
#[test]
fn a_cross_package_schema_states_the_facts_the_model_states() {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures/cross-package");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output.diagnostics.is_empty(),
        "the cross-package fixture must compile with no diagnostic, got: {:?}",
        output.diagnostics,
    );
    let package = |name: &str| {
        output
            .checked
            .iter()
            .find(|checked| checked.ir.name == name)
            .unwrap_or_else(|| panic!("the fixture declares a {name} member"))
            .ir
            .clone()
    };
    let parts = package("proto.parts");
    let vehicle = package("proto.vehicle");

    let parts_out = ridl_backend_flatbuffers::generate(&parts).expect("parts");
    let vehicle_out =
        ridl_backend_flatbuffers::generate_with(&vehicle, &[&parts]).expect("vehicle");
    support::compile_with_planus_and_siblings(
        "proto.vehicle.fbs",
        &vehicle_out.fbs_source,
        &[("proto.parts.fbs", &parts_out.fbs_source)],
    );

    let model = codegen::lower(&vehicle, &[&parts]);
    let schema = read_back(&vehicle_out.fbs_source);
    assert_tables_agree("proto.vehicle", &model, &schema);
    assert_unions_agree("proto.vehicle", &model, &schema);
    assert_identity_tables_agree("proto.vehicle", &model, &schema);
    assert!(
        !model.foreign.is_empty(),
        "the referencing package reaches a declaration of its sibling"
    );
}
