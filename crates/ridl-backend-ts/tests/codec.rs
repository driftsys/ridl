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
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "{} must compile with no error, got: {:?}",
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
        .filter_map(|root| match root.tuple {
            Some(tuple) => Some(
                model.tuples[tuple as usize]
                    .name
                    .as_ref()
                    .expect("an induced tuple has a name")
                    .rust
                    .clone(),
            ),
            None => {
                let declaration = &model.declarations[root.declaration as usize];
                // An internal root has no codec: `<base>.ts` does not export it.
                (declaration.visibility() != v1::Visibility::Internal).then(|| {
                    declaration
                        .name
                        .as_ref()
                        .expect("a declaration has a name")
                        .declared
                        .clone()
                })
            }
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

/// The declaration of `model` named `name`.
fn declaration<'a>(model: &'a mut v1::Model, name: &str) -> &'a mut v1::Declaration {
    model
        .declarations
        .iter_mut()
        .find(|declaration| {
            declaration
                .name
                .as_ref()
                .is_some_and(|n| n.declared == name)
        })
        .unwrap_or_else(|| panic!("the model declares `{name}`"))
}

/// The type of the field at `slot` of the struct `name` in `model`.
fn field<'a>(model: &'a mut v1::Model, name: &str, slot: usize) -> &'a mut v1::Type {
    let Some(v1::declaration::Kind::Struct(shape)) = &mut declaration(model, name).kind else {
        panic!("`{name}` is a struct");
    };
    let Some(v1::slot::Occupant::Field(field)) = &mut shape.slots[slot].occupant else {
        panic!("`{name}`'s slot {slot} is a field");
    };
    field.r#type.as_mut().expect("a typed field")
}

/// The type of the first field of the struct `name` in `model`.
fn first_field<'a>(model: &'a mut v1::Model, name: &str) -> &'a mut v1::Type {
    field(model, name, 0)
}

/// A stream is not a typl-surface kind; a field of one is refused.
#[test]
fn a_stream_field_is_refused() {
    let mut model = cabin_model();
    first_field(&mut model, "Warning").kind =
        Some(v1::r#type::Kind::Stream(v1::StreamType { element: None }));
    assert_eq!(
        refusal(&model),
        "`veh.cabin.Warning` reaches a stream, which the codec module does not convert yet"
    );
}

/// The fixture's `common.typl` is the corpus's `veh.common` package, unchanged.
#[test]
fn the_codec_fixture_copies_the_corpus_common_typl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read(root.join("tests/fixtures/codec/common.typl")).expect("read");
    let corpus = std::fs::read(root.join("../ridlc/tests/corpus/veh-cluster/common/common.typl"))
        .expect("read");
    assert!(
        fixture == corpus,
        "the fixture's common.typl differs from the corpus's"
    );
}

/// The model of the fixture `tests/fixtures/codec`, package `veh.common`,
/// lowered with the workspace's `ridl.std` as the one other package, so the
/// foreign `ridl.std.Label`, `Name`, `Message` and `Timestamp` resolve.
fn common_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    // TYPL-406 (`missing-docs`), RIDL-406 (an info on `SensorReading.timestamp`)
    // and RIDL-150 (the payload ceiling `bounds` is above, which the codecs
    // do not enforce) are the diagnostics the fixture draws.
    assert!(
        output.diagnostics.iter().all(|diagnostic| matches!(
            diagnostic.code.as_str(),
            "TYPL-406" | "RIDL-406" | "RIDL-150"
        )),
        "the fixture must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "veh.common")
        .expect("the fixture declares veh.common")
        .ir
        .clone();
    codegen::lower(&package, &[&output.std_ir])
}

/// The `InducedName.rust` of the induced tuple of `Sensors.<member>` with the
/// path segment `segment` (`Params` or `Reply`).
fn induced_name(model: &v1::Model, member: &str, segment: &str) -> String {
    let interface = model
        .interfaces
        .iter()
        .position(|interface| {
            matches!(&interface.identity, Some(v1::interface::Identity::Declared(name)) if name.declared == "Sensors")
        })
        .expect("Sensors");
    let slot = model.interfaces[interface]
        .slots
        .iter()
        .position(|slot| {
            matches!(&slot.occupant, Some(v1::interaction_slot::Occupant::Interaction(interaction))
                if interaction.name.as_ref().is_some_and(|name| name.declared == member))
        })
        .expect("the member");
    model
        .tuples
        .iter()
        .find(|tuple| {
            matches!(&tuple.origin, Some(v1::induced_tuple::Origin::Interaction(path))
                if path.interface as usize == interface && path.slot as usize == slot && path.segments == [segment])
        })
        .and_then(|tuple| tuple.name.as_ref())
        .map(|name| name.rust.clone())
        .expect("the induced tuple")
}

#[test]
fn the_common_codec_module() {
    let model = common_model();
    let source = generate_codec(&model, "veh.common")
        .expect("the fixture's codec module generates")
        .source;
    insta::assert_snapshot!("common", source);
    // The union packs by the arm's declared name and the `flatc` member.
    assert!(
        source.contains("switch (value.kind) {\n    case \"ok\":"),
        "{source}"
    );
    assert!(
        source.contains(
            "return new fb.SensorResultT(fb.SensorResultUnion.err, packSensorFault(value.value));"
        ),
        "{source}"
    );
    assert!(source.contains("case fb.SensorResultUnion.ok:\n      return { kind: \"ok\", value: unpackSensorReading(object.value as fb.SensorReadingT) };"), "{source}");
    // The `kind` is the declared arm name; the `flatc` member is its snake case.
    assert!(
        source.contains(
            "    case \"errFault\":\n      return new fb.OutcomeT(fb.OutcomeUnion.err_fault, \
             packSensorFault(value.value));"
        ),
        "{source}"
    );
    assert!(
        source.contains("case fb.OutcomeUnion.ok_reading:\n      return { kind: \"okReading\", "),
        "{source}"
    );
    // The tuple is positional on the `flatc` side and named on the plain side.
    assert!(
        source.contains("tuple.field1 = value.min;\n  tuple.field2 = value.max;"),
        "{source}"
    );
    assert!(
        source.contains("return { min: object.field1 as t.Speed, max: object.field2 as t.Speed };"),
        "{source}"
    );
    // The array, the map and the optional field.
    assert!(
        source.contains("object.readings = [...value.readings];"),
        "{source}"
    );
    assert!(
        source.contains("readings: object.readings.map((element) => element as t.Speed),"),
        "{source}"
    );
    assert!(source.contains("object.meta = value.meta.map(([key, value]) => { const entry = new fb.SensorBoundsMetaEntryT(); entry.key = key; entry.value = value; return entry; });"), "{source}");
    assert!(source.contains("meta: object.meta.map((entry) => [entry.key as ridl_std.Label, entry.value as ridl_std.Name] as const),"), "{source}");
    assert!(
        source.contains("object.override = value.override === undefined ? null : value.override;"),
        "{source}"
    );
    assert!(
        source.contains(
            "if (object.override !== null) result.override = object.override as t.Speed;"
        ),
        "{source}"
    );
    // The required enum with no zero member reads as absent.
    assert!(source.contains("mode: object.mode === null ? absent(\"veh.common.Setting\", \"mode\") : Number(object.mode) as t.Mode,"), "{source}");
    // The induced tuples of the interactions are exported interfaces.
    let set_limit = induced_name(&model, "setLimit", "Params");
    assert!(
        source.contains(&format!(
            "export interface {set_limit} {{\n  limit: t.Speed;\n  actual: t.Speed;\n}}"
        )),
        "{source}"
    );
    let span = induced_name(&model, "span", "Reply");
    assert!(
        source.contains(&format!(
            "export function encode{span}(value: {span}): Uint8Array"
        )),
        "{source}"
    );
    // The member table skips the retired ordinal.
    for line in [
        "latest: { iface: 1, ord: 1, kind: \"signal\", payload: codecSensorReading }",
        "result: { iface: 1, ord: 3, kind: \"event\", payload: codecSensorResult }",
        "bounds: { iface: 1, ord: 4, kind: \"event\", payload: codecSensorBounds }",
        &format!("setLimit: {{ iface: 1, ord: 5, kind: \"command\", request: codec{set_limit} }}"),
        "profile: { iface: 1, ord: 6, kind: \"query\", request: codecRidlStdName, reply: codecDriverProfile }",
        &format!(
            "span: {{ iface: 1, ord: 7, kind: \"query\", request: codec{}, reply: codec{span} }}",
            induced_name(&model, "span", "Params")
        ),
        "setting: { iface: 1, ord: 8, kind: \"signal\", payload: codecSetting }",
    ] {
        assert!(source.contains(line), "no `{line}` in:\n{source}");
    }
    assert!(!source.contains("ord: 2,"), "{source}");
}

#[test]
fn every_bounded_root_of_the_fixture_has_a_codec_and_every_live_interaction_an_entry() {
    let model = common_model();
    let source = generate_codec(&model, "veh.common")
        .expect("generates")
        .source;
    assert_complete("tests/fixtures/codec", &model, &source);
    let model = codegen::lower(&rust_fixture("interaction_face.ridl"), &[]);
    let source = generate_codec(&model, "face.demo")
        .expect("generates")
        .source;
    assert_complete("interaction_face.ridl", &model, &source);
}

/// A foreign named scalar needs no converter: the field is a type assertion
/// to the type `<base>.ts` imports from the foreign module.
#[test]
fn a_foreign_named_scalar_converts_by_assertion() {
    let source = generate_codec(&common_model(), "veh.common")
        .expect("generates")
        .source;
    assert!(
        source.contains("import type * as ridl_std from \"./ridl.std.js\";"),
        "{source}"
    );
    assert!(
        source.contains("name: object.name as ridl_std.Name,"),
        "{source}"
    );
    assert!(
        source.contains("timestamp: object.timestamp as ridl_std.Timestamp,"),
        "{source}"
    );
}

/// A foreign payload type that is a named scalar is encoded by the foreign
/// package's own codec module.
#[test]
fn a_foreign_scalar_payload_uses_the_foreign_codec_module() {
    let source = generate_codec(&common_model(), "veh.common")
        .expect("generates")
        .source;
    assert!(
        source.contains("import * as ridl_std_codec from \"./ridl.std.codec.js\";"),
        "{source}"
    );
    assert!(source.contains("const codecRidlStdName: PayloadCodec<ridl_std.Name> = { type: \"ridl.std.Name\", encode: ridl_std_codec.encodeName, decode: ridl_std_codec.decodeName };"), "{source}");
}

/// Adds a foreign declaration `veh.other.Thing` of `kind` to `model` and
/// returns a resolved reference to it.
fn foreign_thing(model: &mut v1::Model, kind: v1::declaration::Kind) -> v1::TypeRef {
    let index = model.foreign.len() as u32;
    model.foreign.push(v1::ForeignDeclaration {
        package: "veh.other".to_string(),
        declaration: Some(v1::Declaration {
            name: Some(v1::Spellings {
                declared: "Thing".to_string(),
                snake: "thing".to_string(),
                camel: "Thing".to_string(),
                screaming: "THING".to_string(),
                pascal: "Thing".to_string(),
            }),
            visibility: v1::Visibility::Public as i32,
            kind: Some(kind),
            ..Default::default()
        }),
    });
    v1::TypeRef {
        reference: "veh.other.Thing".to_string(),
        resolved: true,
        package: "veh.other".to_string(),
        foreign: true,
        index,
        kind: v1::DeclKind::Unspecified as i32,
    }
}

/// A foreign struct, union or enum needs a converter of its own package,
/// which the module does not write yet: a field, an arm, an element and a
/// payload type of one are refused.
#[test]
fn a_foreign_struct_union_or_enum_is_refused() {
    use v1::declaration::Kind;
    let kinds = [
        ("struct", Kind::Struct(v1::Struct::default())),
        ("union", Kind::Union(v1::Union::default())),
        ("enum", Kind::Enum(v1::Enum::default())),
    ];
    for (word, kind) in kinds {
        let kind = || kind.clone();
        // A field.
        let mut model = common_model();
        let reference = foreign_thing(&mut model, kind());
        first_field(&mut model, "DriverProfile").kind = Some(v1::r#type::Kind::Named(reference));
        assert_eq!(
            refusal(&model),
            format!(
                "`veh.common.DriverProfile` reaches `veh.other.Thing`, a {word} of another package, which the codec module does not convert yet"
            )
        );
        // An arm.
        let mut model = common_model();
        let reference = foreign_thing(&mut model, kind());
        let Some(Kind::Union(union)) = &mut declaration(&mut model, "SensorResult").kind else {
            panic!("a union");
        };
        union.arms[1].r#type = Some(reference);
        assert_eq!(
            refusal(&model),
            format!(
                "`veh.common.SensorResult` reaches `veh.other.Thing`, a {word} of another package, which the codec module does not convert yet"
            )
        );
        // An element.
        let mut model = common_model();
        let reference = foreign_thing(&mut model, kind());
        let Some(v1::r#type::Kind::Array(array)) = &mut field(&mut model, "SensorBounds", 1).kind
        else {
            panic!("`SensorBounds.readings` is an array");
        };
        array.element.as_mut().expect("an element").kind = Some(v1::r#type::Kind::Named(reference));
        assert_eq!(
            refusal(&model),
            format!(
                "`veh.common.SensorBounds` reaches `veh.other.Thing`, a {word} of another package, which the codec module does not convert yet"
            )
        );
    }
}

/// A payload type of another package that is a struct is refused too.
#[test]
fn a_foreign_struct_payload_type_is_refused() {
    let mut model = common_model();
    let reference = foreign_thing(
        &mut model,
        v1::declaration::Kind::Struct(v1::Struct::default()),
    );
    let interface = model.interfaces.iter_mut().find(|interface| {
        matches!(&interface.identity, Some(v1::interface::Identity::Declared(name)) if name.declared == "Sensors")
    }).expect("Sensors");
    let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
        &mut interface.slots[0].occupant
    else {
        panic!("latest");
    };
    let Some(v1::interaction::Shape::Signal(signal)) = &mut interaction.shape else {
        panic!("a signal");
    };
    signal.payload.as_mut().expect("a payload").r#type = Some(reference);
    assert_eq!(
        refusal(&model),
        "`Sensors.latest` has the payload type `veh.other.Thing`, a struct of another package, which the codec module does not convert yet"
    );
}

/// An internal payload root has no `encode`/`decode`: `<base>.ts` does not
/// export its type. It stays in `PAYLOAD_TYPES`, which equals the wasm list.
#[test]
fn an_internal_payload_root_is_skipped() {
    let source = generate_codec(&common_model(), "veh.common")
        .expect("generates")
        .source;
    assert!(
        source.contains("  \"veh.common.RawWheelFrame\",\n"),
        "{source}"
    );
    for item in [
        "encodeRawWheelFrame",
        "decodeRawWheelFrame",
        "packRawWheelFrame",
        "codecRawWheelFrame",
    ] {
        assert!(!source.contains(item), "`{item}` in:\n{source}");
    }
}

/// An interaction whose payload type is internal is refused: its entry would
/// name a codec the module cannot write.
#[test]
fn an_internal_interaction_payload_is_refused() {
    let mut model = common_model();
    declaration(&mut model, "SensorReading").visibility = v1::Visibility::Internal as i32;
    assert_eq!(
        refusal(&model),
        "`Sensors.latest` has the payload type `veh.common.SensorReading`, which is internal; `<base>.ts` does not export it, so the codec module cannot name it"
    );
}
