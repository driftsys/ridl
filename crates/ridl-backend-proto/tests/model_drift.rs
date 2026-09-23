//! The fact-level drift test (the codegen model design note §8.1).
//!
//! No backend reads the lowered codegen model yet: this one derives the
//! proto3 projection from the raw IR, and `ridl_ir::codegen::lower` derives
//! the same facts again for the model a plugin will read. Two derivations of
//! one fact drift, and the pull request that introduces the divergence is
//! where it has to fail — so the emitted schema is compiled with `protox` and
//! read back from its `FileDescriptorSet`, and each fact the model also
//! states is asserted equal to the model's field, over the fixtures this
//! crate already compiles.
//!
//! What is compared: each message name against the declaration's own spelling
//! or the tuple's wire induced name; each field's name against
//! `Field.name.snake` and its number against the slot's ordinal; each
//! `reserved` range against a tombstone's slot; each enum value against
//! `<enum screaming>_<value screaming>` at its own number, with the
//! synthesized `_UNSPECIFIED` exactly where the model states no zero member;
//! each `oneof` arm against `Arm.name.snake` at `Arm.ordinal`; each identity
//! table against `<joined CamelCase name>Ordinal`; and each `import` against
//! a package the model lists as foreign.
//!
//! It is the form of drift test stage P2b can write. When this backend stops
//! deriving a fact and reads it from the model instead, that fact is a
//! function of the model by construction and leaves this file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

/// The proto3 facts of one compiled schema, as the target itself reads them
/// rather than as a text scan of what was written.
///
/// The compiled `FileDescriptorSet` is reduced to plain data here so that no
/// signature in this file has to name `prost_types`, which is not a
/// dependency of this crate: `protox` is, and it is the only thing that has
/// to know the descriptor's own types.
#[derive(Default)]
struct Schema {
    /// Message name to its fields, `(name, number)` in declaration order.
    messages: BTreeMap<String, Vec<(String, i32)>>,
    /// Message name to the fields of its `oneof`, in declaration order.
    oneof_fields: BTreeMap<String, Vec<(String, i32)>>,
    /// Message name to every field number it reserves, ascending.
    reserved: BTreeMap<String, Vec<u32>>,
    /// Enum name to its values, `(name, number)` in declaration order.
    enums: BTreeMap<String, Vec<(String, i32)>>,
    /// Every package this file imports.
    imports: BTreeSet<String>,
}

/// Compiles the emitted schema with `protox` and reduces the one file it
/// produced to the facts the model also states.
fn compile_schema(file_name: &str, source: &str, siblings: &[(&str, &str)]) -> Schema {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, text) in siblings {
        std::fs::write(dir.path().join(name), text).expect("write sibling schema");
    }
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    let set = protox::compile([&path], [dir.path()])
        .unwrap_or_else(|error| panic!("the emitted schema must compile: {error}\n\n{source}"));
    let file = set
        .file
        .into_iter()
        .find(|file| file.name.as_deref() == Some(file_name))
        .expect("the compiled set holds the file that was compiled");

    let mut schema = Schema::default();
    for message in &file.message_type {
        let name = message.name.clone().unwrap_or_default();
        schema.messages.insert(
            name.clone(),
            message
                .field
                .iter()
                .map(|field| {
                    (
                        field.name.clone().unwrap_or_default(),
                        field.number.unwrap_or_default(),
                    )
                })
                .collect(),
        );
        schema.oneof_fields.insert(
            name.clone(),
            message
                .field
                .iter()
                .filter(|field| field.oneof_index.is_some())
                .map(|field| {
                    (
                        field.name.clone().unwrap_or_default(),
                        field.number.unwrap_or_default(),
                    )
                })
                .collect(),
        );
        let mut reserved: Vec<u32> = message
            .reserved_range
            .iter()
            .flat_map(|range| {
                let start = range.start.unwrap_or_default();
                let end = range.end.unwrap_or_default();
                (start..end).map(|number| number as u32)
            })
            .collect();
        reserved.sort_unstable();
        schema.reserved.insert(name, reserved);
    }
    for enumeration in &file.enum_type {
        schema.enums.insert(
            enumeration.name.clone().unwrap_or_default(),
            enumeration
                .value
                .iter()
                .map(|value| {
                    (
                        value.name.clone().unwrap_or_default(),
                        value.number.unwrap_or_default(),
                    )
                })
                .collect(),
        );
    }
    schema.imports = file
        .dependency
        .iter()
        .map(|import| import.trim_end_matches(".proto").to_string())
        .collect();
    schema
}

/// Every message the model states is in the schema, with the field names,
/// numbers and reservations the model states.
fn assert_messages_agree(label: &str, model: &v1::Model, schema: &Schema) {
    let mut checked = 0usize;
    for declaration in &model.declarations {
        let name = &declaration.name.as_ref().expect("a name").declared;
        match declaration.kind.as_ref() {
            Some(v1::declaration::Kind::Struct(def)) => {
                let fields = schema
                    .messages
                    .get(name)
                    .unwrap_or_else(|| panic!("{label}: the schema declares no message `{name}`"));
                let expected: Vec<(String, i32)> = def
                    .slots
                    .iter()
                    .filter_map(|slot| match slot.occupant.as_ref()? {
                        v1::slot::Occupant::Field(field) => Some((
                            field.name.as_ref().expect("a field name").snake.clone(),
                            slot.ordinal as i32,
                        )),
                        v1::slot::Occupant::Retired(_) => None,
                    })
                    .collect();
                assert_eq!(
                    *fields, expected,
                    "{label}: message `{name}` emits {fields:?}, the model states {expected:?}"
                );
                let mut retired: Vec<u32> = def
                    .slots
                    .iter()
                    .filter_map(|slot| match slot.occupant.as_ref()? {
                        v1::slot::Occupant::Retired(_) => Some(slot.ordinal),
                        v1::slot::Occupant::Field(_) => None,
                    })
                    .collect();
                retired.sort_unstable();
                assert_eq!(
                    *schema.reserved.get(name).expect("a reservation list"),
                    retired,
                    "{label}: message `{name}`'s reservations are the model's tombstone slots"
                );
                checked += 1;
            }
            Some(v1::declaration::Kind::Union(def)) => {
                let arms = schema
                    .oneof_fields
                    .get(name)
                    .unwrap_or_else(|| panic!("{label}: the schema declares no message `{name}`"));
                let expected: Vec<(String, i32)> = def
                    .arms
                    .iter()
                    .map(|arm| {
                        (
                            arm.name.as_ref().expect("an arm name").snake.clone(),
                            arm.ordinal as i32,
                        )
                    })
                    .collect();
                assert_eq!(
                    *arms, expected,
                    "{label}: union `{name}` emits {arms:?}, the model states {expected:?}"
                );
                checked += 1;
            }
            Some(v1::declaration::Kind::Enum(def)) => {
                let values = schema
                    .enums
                    .get(name)
                    .unwrap_or_else(|| panic!("{label}: the schema declares no enum `{name}`"));
                let prefix = &declaration.name.as_ref().expect("a name").screaming;
                let unspecified = format!("{prefix}_UNSPECIFIED");
                let synthesized = values.iter().any(|(value, _)| *value == unspecified);
                assert_eq!(
                    synthesized,
                    def.zero_member.is_none(),
                    "{label}: enum `{name}` synthesizes `{unspecified}` exactly when the model \
                     states no zero member"
                );
                for value in &def.values {
                    let member = format!(
                        "{prefix}_{}",
                        value.name.as_ref().expect("a value name").screaming
                    );
                    let number = i32::try_from(value.value).expect("a proto3 enum value");
                    assert!(
                        values.contains(&(member.clone(), number)),
                        "{label}: enum `{name}` must emit `{member} = {number}`, got {values:?}"
                    );
                }
                checked += 1;
            }
            // A named scalar, an enum set and a constant become no
            // declaration of their own in proto3 (design §3.1, ADR-0013
            // decision 5).
            _ => {}
        }
    }
    assert!(
        checked > 0,
        "{label}: the fixture must declare something to compare"
    );

    // Every induced tuple is a message named for the path that reached it,
    // with positional fields numbered from 1.
    for tuple in &model.tuples {
        let name = &tuple.name.as_ref().expect("an induced name").wire;
        if name.is_empty() {
            // An interaction position: no backend generates a type for it
            // today, and the model leaves both names empty.
            continue;
        }
        let fields = schema
            .messages
            .get(name)
            .unwrap_or_else(|| panic!("{label}: the schema declares no tuple message `{name}`"));
        let expected: Vec<(String, i32)> = (1..=tuple.fields.len())
            .map(|position| (format!("field_{position}"), position as i32))
            .collect();
        assert_eq!(
            *fields, expected,
            "{label}: tuple message `{name}` emits {fields:?}, the model states {expected:?}"
        );
    }
}

/// Each interface's identity table, and each import.
fn assert_identity_and_imports_agree(label: &str, model: &v1::Model, schema: &Schema) {
    for interface in &model.interfaces {
        let name = match interface.identity.as_ref().expect("an identity") {
            v1::interface::Identity::Declared(declared) => declared.declared.clone(),
            v1::interface::Identity::Service(service) => service.joined_camel.clone(),
        };
        let table = format!("{name}Ordinal");
        let prefix = ridl_ir::name::snake_case(&table).to_uppercase();
        let values = schema
            .enums
            .get(&table)
            .unwrap_or_else(|| panic!("{label}: the schema declares no identity table `{table}`"));
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_ref()
            else {
                continue;
            };
            // proto3 scopes an enum's values as siblings of the enum itself,
            // so every member carries its own table's SCREAMING_SNAKE name as
            // a prefix — a composition of the pinned transform the printer
            // makes, which is why the model states the bare spelling and this
            // test composes it (design note §7).
            let member = format!(
                "{prefix}_{}",
                interaction
                    .name
                    .as_ref()
                    .expect("an interaction name")
                    .screaming
            );
            assert!(
                values.contains(&(member.clone(), slot.ordinal as i32)),
                "{label}: `{table}` must emit `{member} = {}`, got {values:?}",
                slot.ordinal
            );
        }
    }

    let foreign: BTreeSet<String> = model
        .foreign
        .iter()
        .map(|foreign| foreign.package.clone())
        .collect();
    assert!(
        schema.imports.is_subset(&foreign),
        "{label}: the schema imports {:?}, the model lists the foreign packages {foreign:?}",
        schema.imports
    );
}

fn compile_fixture(relative_to_fixtures: &str) -> v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
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
    let generated = ridl_backend_proto::generate(&package).expect("generate");
    let schema = compile_schema("veh.cruise.proto", &generated.proto_source, &[]);
    let model = codegen::lower(&package, &[]);
    assert_messages_agree("cruise.ridl", &model, &schema);
    assert_identity_and_imports_agree("cruise.ridl", &model, &schema);
}

/// The same comparison across a package boundary: the `import` a foreign
/// reference draws, against the package the model lists as foreign.
#[test]
fn a_cross_package_schema_states_the_facts_the_model_states() {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cross-package");
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

    let parts_out = ridl_backend_proto::generate(&parts).expect("parts");
    let vehicle_out = ridl_backend_proto::generate_with(&vehicle, &[&parts]).expect("vehicle");
    let schema = compile_schema(
        "proto.vehicle.proto",
        &vehicle_out.proto_source,
        &[("proto.parts.proto", &parts_out.proto_source)],
    );
    let model = codegen::lower(&vehicle, &[&parts]);
    assert_messages_agree("proto.vehicle", &model, &schema);
    assert_identity_and_imports_agree("proto.vehicle", &model, &schema);
    assert!(
        !model.foreign.is_empty(),
        "the referencing package reaches a declaration of its sibling"
    );
}
