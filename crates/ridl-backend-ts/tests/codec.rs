//! The codec module `<base>.codec.ts` that `generate_codec` writes from the
//! codegen model: the snapshot of cabin's module, the completeness of its
//! functions and member tables against the model, and its refusals.

use std::collections::BTreeSet;
use std::path::Path;

use ridl_backend_ts::{GenerateError, generate_codec};
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

/// The model of `examples/cabin`'s `veh.cabin` package, lowered as the
/// workspace compile hands it to a backend.
fn cabin_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cabin");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    // TYPL-406 (`missing-docs`) is the one diagnostic the example may draw.
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() == "TYPL-406"),
        "examples/cabin must compile with no diagnostic, got: {:?}",
        output.diagnostics,
    );
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "veh.cabin")
        .expect("examples/cabin declares veh.cabin")
        .ir
        .clone();
    codegen::lower(&package, &[])
}

/// The package of a fixture of the Rust backend's tests, compiled as one file.
fn rust_fixture(file_name: &str) -> v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-rust/tests/fixtures")
        .join(file_name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let output = ridlc::compile(&path.display().to_string(), &text);
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() == "TYPL-406"),
        "{} must compile with no diagnostic, got: {:?}",
        path.display(),
        output.diagnostics,
    );
    output.package
}

fn refusal(model: &v1::Model) -> String {
    match generate_codec(model, "base") {
        Ok(generated) => panic!("the model must be refused, got:\n{}", generated.source),
        Err(GenerateError::Unrepresentable(message)) => message,
    }
}

#[test]
fn the_cabin_codec_module() {
    insta::assert_snapshot!(
        "cabin",
        generate_codec(&cabin_model(), "veh.cabin")
            .expect("cabin's codec module generates")
            .source
    );
}

/// The names `<prefix><Name>(` of the functions the source exports.
fn exported_functions(source: &str, prefix: &str) -> BTreeSet<String> {
    let start = format!("export function {prefix}");
    source
        .lines()
        .filter_map(|line| line.strip_prefix(&start))
        .map(|rest| rest.split('(').next().unwrap_or_default().to_string())
        .collect()
}

fn assert_complete(label: &str, model: &v1::Model, source: &str) {
    let roots: BTreeSet<String> = model
        .flatbuffers
        .as_ref()
        .expect("the model carries a FlatBuffers projection")
        .roots
        .iter()
        .filter(|root| matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_))))
        .map(|root| match root.tuple {
            Some(tuple) => model.tuples[tuple as usize]
                .name
                .as_ref()
                .expect("an induced tuple has a name")
                .rust
                .clone(),
            None => model.declarations[root.declaration as usize]
                .name
                .as_ref()
                .expect("a declaration has a name")
                .declared
                .clone(),
        })
        .collect();
    assert!(!roots.is_empty(), "{label}: the model has bounded roots");
    assert_eq!(
        exported_functions(source, "encode"),
        roots,
        "{label}: encode"
    );
    assert_eq!(
        exported_functions(source, "decode"),
        roots,
        "{label}: decode"
    );
    for interface in &model.interfaces {
        let Some(v1::interface::Identity::Declared(_)) = interface.identity else {
            continue;
        };
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = &slot.occupant
            else {
                continue;
            };
            let kind = match interaction.kind() {
                v1::Kind::Signal => "signal",
                v1::Kind::Event => "event",
                v1::Kind::Command => "command",
                v1::Kind::Query => "query",
                v1::Kind::Fixed => "fixed",
                v1::Kind::Unspecified => panic!("{label}: an interaction has a kind"),
            };
            let name = &interaction.name.as_ref().expect("named").declared;
            let line = format!(
                "{name}: {{ iface: {}, ord: {}, kind: \"{kind}\"",
                interface.number, slot.ordinal
            );
            assert!(source.contains(&line), "{label}: no `{line}` in:\n{source}");
        }
    }
    assert!(!source.contains("field_1"), "{label}: a positional name");
}

#[test]
fn every_bounded_root_has_a_codec_and_every_live_interaction_an_entry() {
    let model = cabin_model();
    let source = generate_codec(&model, "veh.cabin")
        .expect("cabin's codec module generates")
        .source;
    assert_complete("examples/cabin", &model, &source);
}

#[test]
fn a_model_without_a_catalog_hash_is_refused() {
    let mut model = cabin_model();
    model.catalog.as_mut().expect("a catalog").hash.clear();
    assert_eq!(
        refusal(&model),
        "malformed codegen model: `Catalog.hash` is 0 bytes, not 32"
    );
}

#[test]
fn a_tuple_collision_is_refused() {
    let mut model = cabin_model();
    model.tuple_collisions.push(v1::TupleCollision {
        name: "Pair".to_string(),
        first_fields: vec!["a".to_string()],
        second_fields: vec!["b".to_string()],
    });
    assert_eq!(
        refusal(&model),
        "the induced tuple `Pair` has two shapes, (a) and (b); the codec module cannot name both"
    );
}

#[test]
fn an_induced_tuple_named_as_a_declaration_is_refused() {
    let mut model = codegen::lower(&rust_fixture("induced_params.ridl"), &[]);
    let interfaces = model.interfaces.clone();
    let is_both = |path: &v1::InteractionPath| {
        let slot = &interfaces[path.interface as usize].slots[path.slot as usize];
        matches!(
            &slot.occupant,
            Some(v1::interaction_slot::Occupant::Interaction(interaction))
                if interaction.name.as_ref().is_some_and(|name| name.declared == "both")
        )
    };
    let tuple = model
        .tuples
        .iter_mut()
        .find(|tuple| {
            matches!(
                &tuple.origin,
                Some(v1::induced_tuple::Origin::Interaction(path)) if is_both(path)
            )
        })
        .expect("`Pump.both` induces a tuple");
    tuple.name.as_mut().expect("named").rust = "Level".to_string();
    assert_eq!(
        refusal(&model),
        "`Level` is both a declaration of the package and the induced tuple of `Pump.both`"
    );
}
