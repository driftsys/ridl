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
        Ok(Some(generated)) => panic!("the model must be refused, got:\n{}", generated.source),
        Ok(None) => panic!("the model must be refused, got no file"),
        Err(GenerateError::Unrepresentable(message)) => message,
    }
}

/// The source of the codec module of `model`, which must generate one.
fn module(model: &v1::Model, base: &str) -> String {
    generate_codec(model, base)
        .expect("the codec module generates")
        .expect("the package has a payload type")
        .source
}

#[test]
fn the_cabin_codec_module() {
    insta::assert_snapshot!("cabin", module(&cabin_model(), "veh.cabin"));
}

#[test]
fn codec_barrel_imports_keep_domain_modules_distinct() {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-single");
    let output = ridlc::compile_workspace(&mut db, &entry).expect("load the single fixture");
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "the fixture must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let package = &output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "single")
        .expect("the fixture declares single")
        .ir;
    let source = module(&codegen::lower(package, &[&output.std_ir]), "single");
    assert!(
        source.contains("import * as fb from \"./flatc_out/single.js\";"),
        "the single-component barrel must be under flatc_out/:\n{source}"
    );
    assert!(
        source.contains("`-o` is the flatc_out\n// subdirectory of this file's directory."),
        "the comment must name the flatc_out directory:\n{source}"
    );
    assert!(
        source.contains("import type * as t from \"./single.js\";"),
        "the domain module must stay beside the codec module:\n{source}"
    );

    let source = module(&common_model(), "veh.common");
    assert!(
        source.contains("import * as fb from \"./veh/common.js\";"),
        "the two-component barrel path must stay unchanged:\n{source}"
    );
    assert!(
        source.contains("`-o` is the directory\n// of this file."),
        "the two-component comment must name the directory of the module:\n{source}"
    );
    assert!(
        source.contains("import * as t from \"./veh.common.js\";"),
        "the two-component domain path must stay unchanged:\n{source}"
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
    // The bounded roots by declaration index and by tuple index.
    let projection = model.flatbuffers.as_ref().expect("a projection");
    let bounded = |root: &v1::FbRoot| matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_)));
    let declaration_bounded = |index: u32| {
        projection
            .roots
            .iter()
            .any(|root| root.tuple.is_none() && root.declaration == index && bounded(root))
    };
    let tuple_bounded = |index: u32| {
        projection
            .roots
            .iter()
            .any(|root| root.tuple == Some(index) && bounded(root))
    };
    // The `codec<Name>` constant of a role, or the qualified name of a
    // payload type with no finite bound, computed from the model the way the
    // record states it.
    let constant = |payload: Option<&v1::Payload>, tuple: Option<&v1::TupleRef>| {
        if let Some(reference) = payload.and_then(|payload| payload.r#type.as_ref()) {
            if reference.foreign {
                let foreign = &model.foreign[reference.index as usize];
                let name = &foreign
                    .declaration
                    .as_ref()
                    .expect("declared")
                    .name
                    .as_ref()
                    .expect("named")
                    .declared;
                return if payload.expect("a payload").flatbuffers_max_size.is_some() {
                    Ok(format!(
                        "codec{}{name}",
                        ridl_ir::name::pascal_case(&foreign.package.replace('.', "_"))
                    ))
                } else {
                    Err(format!("{}.{name}", foreign.package))
                };
            }
            let name = &model.declarations[reference.index as usize]
                .name
                .as_ref()
                .expect("named")
                .declared;
            return if declaration_bounded(reference.index) {
                Ok(format!("codec{name}"))
            } else {
                Err(format!("{}.{name}", reference.package))
            };
        }
        let tuple = tuple.expect("a named payload or a tuple");
        let name = &model.tuples[tuple.index as usize]
            .name
            .as_ref()
            .expect("named")
            .rust;
        if tuple_bounded(tuple.index) {
            Ok(format!("codec{name}"))
        } else {
            Err(format!(
                "{}.{name}",
                model.scope.as_ref().expect("a scope").package
            ))
        }
    };
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
            use v1::interaction::Shape;
            let roles: Vec<(&str, Result<String, String>)> = match &interaction.shape {
                Some(Shape::Signal(signal)) => {
                    vec![("payload", constant(signal.payload.as_ref(), None))]
                }
                Some(Shape::Event(event)) => {
                    vec![("payload", constant(event.payload.as_ref(), None))]
                }
                Some(Shape::Fixed(fixed)) => {
                    vec![("payload", constant(fixed.named.as_ref(), None))]
                }
                Some(Shape::Command(command)) => vec![(
                    "request",
                    constant(command.request.as_ref(), command.request_tuple.as_ref()),
                )],
                Some(Shape::Query(query)) => {
                    let mut roles = vec![(
                        "request",
                        constant(query.request.as_ref(), query.request_tuple.as_ref()),
                    )];
                    let fallible = matches!(
                        query.reply.as_ref().and_then(|reply| reply.kind.as_ref()),
                        Some(v1::reply::Kind::Fallible(_))
                    );
                    // A fallible `T | E` reply has no reply role.
                    if query.reply_payload.is_some() || query.reply_tuple.is_some() || !fallible {
                        roles.push((
                            "reply",
                            constant(query.reply_payload.as_ref(), query.reply_tuple.as_ref()),
                        ));
                    }
                    roles
                }
                None => panic!("{label}: an interaction has a shape"),
            };
            let name = &interaction.name.as_ref().expect("named").declared;
            let entry = format!(
                "  {name}: {{ iface: {}, ord: {}, kind: \"{kind}\"",
                interface.number, slot.ordinal
            );
            let unbounded = roles
                .iter()
                .find_map(|(_, constant)| constant.as_ref().err());
            match unbounded {
                Some(qualified) => {
                    let line = format!(
                        "  // {name} has no entry: {qualified} has no finite FlatBuffers bound\n"
                    );
                    assert!(source.contains(&line), "{label}: no `{line}` in:\n{source}");
                    assert!(
                        !source.contains(&entry),
                        "{label}: an entry for `{name}` in:\n{source}"
                    );
                }
                None => {
                    let mut roles: Vec<String> = roles
                        .iter()
                        .map(|(role, constant)| {
                            format!("{role}: {}", constant.as_ref().expect("bounded"))
                        })
                        .collect();
                    if let Some(limit) = interaction.limit {
                        roles.push(format!("limit: {limit}"));
                    }
                    let line = format!("{entry}, {} }},\n", roles.join(", "));
                    assert!(source.contains(&line), "{label}: no `{line}` in:\n{source}");
                }
            }
        }
    }
    assert!(!source.contains("field_1"), "{label}: a positional name");
}

#[test]
fn every_bounded_root_has_a_codec_and_every_live_interaction_an_entry() {
    let model = cabin_model();
    let source = module(&model, "veh.cabin");
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

/// The three members that carry the largest payloads of the fixture fit the
/// default ceiling of their kind (ADR-0030): a signal and an event carry at
/// most 3,840 bytes in FlatBuffers, the encoding the fixture enables. A
/// manifest error would skip the size pass, so one fails the test too.
#[test]
fn the_codec_fixture_fits_the_default_ceilings() {
    let copy = tempfile::tempdir().expect("a temp dir");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec");
    for entry in std::fs::read_dir(&fixture).expect("read the fixture") {
        let entry = entry.expect("read a fixture file");
        std::fs::copy(entry.path(), copy.path().join(entry.file_name())).expect("copy a file");
    }
    let manifest = copy.path().join("ridl.toml");
    let text = std::fs::read_to_string(&manifest).expect("read the manifest");
    std::fs::write(
        &manifest,
        format!(
            "{text}\n[limits.kind]\nsignal = 3840\nevent = 3840\ncommand = 16128\nquery = 16128\n"
        ),
    )
    .expect("write the manifest");
    let diagnostics = ridlc::run_check(copy.path(), ridl_core::Frozen::No)
        .expect("the fixture checks")
        .diagnostics;
    let sized: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code.as_str() == "RIDL-151" || diagnostic.code.as_str().starts_with("MANI-")
        })
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    assert!(sized.is_empty(), "{sized:#?}");
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
    // and RIDL-414 (an info on the cohesion of `Sensors`) are the diagnostics
    // the fixture draws.
    assert!(
        output.diagnostics.iter().all(|diagnostic| matches!(
            diagnostic.code.as_str(),
            "TYPL-406" | "RIDL-406" | "RIDL-414"
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
    let source = module(&model, "veh.common");
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
        source.contains("return { min: t.makeSpeedUnchecked(object.field1), max: t.makeSpeedUnchecked(object.field2) };"),
        "{source}"
    );
    // The array, the map and the optional field.
    assert!(
        source.contains("object.readings = [...value.readings];"),
        "{source}"
    );
    assert!(
        source
            .contains("readings: object.readings.map((element) => t.makeSpeedUnchecked(element)),"),
        "{source}"
    );
    assert!(source.contains("object.meta = value.meta.map(([key, value]) => { const entry = new fb.SensorBoundsMetaEntryT(); entry.key = checkWellFormed(key, \"veh.common.SensorBounds.meta.key\"); entry.value = checkWellFormed(value, \"veh.common.SensorBounds.meta.value\"); return entry; });"), "{source}");
    assert!(source.contains("meta: object.meta.map((entry) => [ridl_std.makeLabelUnchecked(entry.key as string), ridl_std.makeNameUnchecked(entry.value as string)] as const),"), "{source}");
    assert!(
        source.contains("object.override = value.override === undefined ? null : value.override;"),
        "{source}"
    );
    assert!(
        source.contains(
            "if (object.override !== null) result.override = t.makeSpeedUnchecked(object.override);"
        ),
        "{source}"
    );
    // The required enum with no zero member reads as absent.
    assert!(source.contains("mode: object.mode === null ? absent(\"veh.common.Setting\", \"mode\") : unpackMode(object.mode),"), "{source}");
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
        "bounds: { iface: 1, ord: 4, kind: \"event\", payload: codecBoundsSummary }",
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
    let source = module(&model, "veh.common");
    assert_complete("tests/fixtures/codec", &model, &source);
    let model = codegen::lower(&rust_fixture("interaction_face.ridl"), &[]);
    let source = module(&model, "face.demo");
    assert_complete("interaction_face.ridl", &model, &source);
}

/// A foreign named scalar needs no converter: the field is a type assertion
/// to the type `<base>.ts` imports from the foreign module.
#[test]
fn a_foreign_named_scalar_converts_by_assertion() {
    let source = module(&common_model(), "veh.common");
    assert!(
        source.contains("import * as ridl_std from \"./ridl.std.js\";"),
        "{source}"
    );
    assert!(
        source.contains("name: ridl_std.makeNameUnchecked(object.name as string),"),
        "{source}"
    );
    assert!(
        source.contains("timestamp: ridl_std.makeTimestampUnchecked(object.timestamp),"),
        "{source}"
    );
}

/// A foreign payload type that is a named scalar is encoded by the foreign
/// package's own codec module.
#[test]
fn a_foreign_scalar_payload_uses_the_foreign_codec_module() {
    let source = module(&common_model(), "veh.common");
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
    let source = module(&common_model(), "veh.common");
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

/// An interface named after an export of the module is refused, with both
/// names.
#[test]
fn an_interface_named_as_a_reserved_name_is_refused() {
    for reserved in ["RULES", "PayloadCodec", "fb", "ridl_std_codec"] {
        let mut model = common_model();
        let interface = model
            .interfaces
            .iter_mut()
            .find(|interface| {
                matches!(&interface.identity, Some(v1::interface::Identity::Declared(name)) if name.declared == "Sensors")
            })
            .expect("Sensors");
        interface.identity = Some(v1::interface::Identity::Declared(v1::Spellings {
            declared: reserved.to_string(),
            ..Default::default()
        }));
        assert_eq!(
            refusal(&model),
            format!(
                "`{reserved}` in package `veh.common` collides with `{reserved}`, which the codec module also declares"
            )
        );
    }
}

/// Sets the payload of `Sensors.<member>`, a signal or an event, to a
/// foreign named scalar `<package>.<name>` with a finite bound.
fn set_foreign_scalar_payload(model: &mut v1::Model, member: &str, package: &str, name: &str) {
    let index = model.foreign.len() as u32;
    model.foreign.push(v1::ForeignDeclaration {
        package: package.to_string(),
        declaration: Some(v1::Declaration {
            name: Some(v1::Spellings {
                declared: name.to_string(),
                ..Default::default()
            }),
            visibility: v1::Visibility::Public as i32,
            kind: Some(v1::declaration::Kind::Scalar(v1::Scalar::default())),
            ..Default::default()
        }),
    });
    let reference = v1::TypeRef {
        reference: format!("{package}.{name}"),
        resolved: true,
        package: package.to_string(),
        foreign: true,
        index,
        kind: v1::DeclKind::Unspecified as i32,
    };
    let interface = model.interfaces.iter_mut().find(|interface| {
        matches!(&interface.identity, Some(v1::interface::Identity::Declared(name)) if name.declared == "Sensors")
    }).expect("Sensors");
    let slot = interface
        .slots
        .iter_mut()
        .find(|slot| {
            matches!(&slot.occupant, Some(v1::interaction_slot::Occupant::Interaction(interaction))
                if interaction.name.as_ref().is_some_and(|name| name.declared == member))
        })
        .expect("the member");
    let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = &mut slot.occupant else {
        panic!("an interaction");
    };
    let payload = match &mut interaction.shape {
        Some(v1::interaction::Shape::Signal(signal)) => signal.payload.as_mut(),
        Some(v1::interaction::Shape::Event(event)) => event.payload.as_mut(),
        _ => panic!("a signal or an event"),
    }
    .expect("a payload");
    payload.r#type = Some(reference);
    payload.flatbuffers_max_size = Some(16);
}

/// Two foreign payload types whose `codec<Package><Name>` constants spell
/// the same are refused, with both names.
#[test]
fn two_foreign_payload_types_that_spell_one_constant_are_refused() {
    let mut model = common_model();
    set_foreign_scalar_payload(&mut model, "latest", "a.b", "C");
    set_foreign_scalar_payload(&mut model, "result", "a", "BC");
    assert_eq!(
        refusal(&model),
        "`a.BC` in package `veh.common` collides with `a.b.C`, which the codec module also declares"
    );
}

/// A local payload type whose `codec<Name>` constant spells a foreign one is
/// refused, with both names.
#[test]
fn a_local_payload_type_that_spells_a_foreign_constant_is_refused() {
    let mut model = common_model();
    // `Sensors.profile` names `ridl.std.Name`, whose constant is
    // `codecRidlStdName`.
    declaration(&mut model, "SensorReading")
        .name
        .as_mut()
        .expect("named")
        .declared = "RidlStdName".to_string();
    assert_eq!(
        refusal(&model),
        "`ridl.std.Name` in package `veh.common` collides with `RidlStdName`, which the codec module also declares"
    );
}

/// A package with no payload type gets no codec module: `ridl build` writes
/// no file for it.
#[test]
fn a_package_without_a_payload_type_has_no_codec_module() {
    let output = ridlc::compile(
        "only.ridl",
        "package only\n\n/// A limit.\nconst LIMIT : integer = 6\n",
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let model = codegen::lower(&output.package, &[]);
    assert!(
        generate_codec(&model, "only").expect("generates").is_none(),
        "a package with no payload type has no codec module"
    );
}

/// The conversions the record's table states, over the fixture's `Extras`
/// and `Pick`: a 64-bit integer is a `bigint`, bytes are read back as a
/// `Uint8Array`, a boxed arm is
/// unboxed, an array of structs or enums is converted element by element,
/// an optional vector packs as `[]` when absent and reads as present, and a
/// field named after a TypeScript keyword is read through `flatc`'s `_`.
#[test]
fn the_extras_and_pick_conversions() {
    let source = module(&common_model(), "veh.common");
    for line in [
        "function packExtrasWide(value: { big: bigint; sparse: readonly t.Speed[] }): fb.ExtrasWideT {",
        "  tuple.field2 = [...value.sparse];",
        "  object.blob = Array.from(value.blob);",
        "    blob: Uint8Array.from(object.blob) as Uint8Array,",
        "  object.readings = value.readings.map((element) => packSensorReading(element));",
        "    readings: object.readings.map((element) => unpackSensorReading(element)),",
        "  object.gears = value.gears.map((element) => BigInt(element));",
        "    gears: object.gears.map((element) => unpackGearPosition(element)),",
        "  object.maybe = value.maybe === undefined ? [] : [...value.maybe];",
        "    maybe: object.maybe.map((element) => t.makeSpeedUnchecked(element)),",
        "  object.tag = value.tag === undefined ? [] : Array.from(value.tag);",
        "    tag: Uint8Array.from(object.tag) as Uint8Array,",
        "  object.notes = value.notes === undefined ? [] : value.notes.map(([key, value]) => { const entry = new fb.ExtrasNotesEntryT(); entry.key = checkWellFormed(key, \"veh.common.Extras.notes.key\"); entry.value = checkWellFormed(value, \"veh.common.Extras.notes.value\"); return entry; });",
        "    notes: object.notes.map((entry) => [ridl_std.makeLabelUnchecked(entry.key as string), ridl_std.makeNameUnchecked(entry.value as string)] as const),",
        "  object.default_ = value.default;",
        "    new: t.makeCounterUnchecked(object.new_),",
        "  object.class_ = value.class;",
        "      return new fb.PickT(fb.PickUnion.speed, new fb.PickSpeedBoxT(value.value));",
        "      return { kind: \"speed\", value: t.makeSpeedUnchecked((object.value as fb.PickSpeedBoxT).value) };",
        "      return new fb.PickT(fb.PickUnion.gear, new fb.PickGearBoxT(BigInt(value.value)));",
        "      return { kind: \"gear\", value: unpackGearPosition((object.value as fb.PickGearBoxT).value) };",
    ] {
        assert!(source.contains(line), "no `{line}` in:\n{source}");
    }
    assert!(!source.contains("if (object.maybe !== null)"), "{source}");
}

/// A `codec<Name>` constant is declared only for a payload type an interface
/// entry names; a foreign one is always named by an entry.
#[test]
fn only_the_constants_an_entry_names_are_declared() {
    let source = module(&common_model(), "veh.common");
    for name in [
        "SpeedLimitPayload",
        "GearPosition",
        "WarningFlags",
        "SensorFault",
    ] {
        assert!(
            !source.contains(&format!("const codec{name}:")),
            "`codec{name}` in:\n{source}"
        );
        assert!(
            source.contains(&format!("export function encode{name}(")),
            "{source}"
        );
    }
    assert!(
        source.contains("const codecSetting: PayloadCodec<t.Setting>"),
        "{source}"
    );
}

/// An optional array element, map key or map value is refused, as the
/// generated Rust codec refuses it: FlatBuffers has no absent element.
#[test]
fn an_optional_element_key_or_value_is_refused() {
    let mut model = common_model();
    let Some(v1::r#type::Kind::Array(array)) = &mut field(&mut model, "SensorBounds", 1).kind
    else {
        panic!("`SensorBounds.readings` is an array");
    };
    array.element.as_mut().expect("an element").optional = true;
    assert_eq!(
        refusal(&model),
        "`veh.common.SensorBounds` reaches an array element that is optional, which FlatBuffers cannot represent: only a table field may be absent"
    );
    for (position, what) in [(0, "a map key"), (1, "a map value")] {
        let mut model = common_model();
        let Some(v1::r#type::Kind::Map(map)) = &mut field(&mut model, "SensorBounds", 3).kind
        else {
            panic!("`SensorBounds.meta` is a map");
        };
        let ty = if position == 0 {
            map.key.as_mut()
        } else {
            map.value.as_mut()
        };
        ty.expect("typed").optional = true;
        assert_eq!(
            refusal(&model),
            format!(
                "`veh.common.SensorBounds` reaches {what} that is optional, which FlatBuffers cannot represent: only a table field may be absent"
            )
        );
    }
}

/// The model of the fixture `tests/fixtures/codec-foreign-only`, package
/// `veh.names`, whose only payload root is `ridl.std.Name`.
fn names_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-foreign-only");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "the fixture must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "veh.names")
        .expect("the fixture declares veh.names")
        .ir
        .clone();
    codegen::lower(&package, &[&output.std_ir])
}

/// A package whose only payload root is a foreign scalar gets a module: its
/// interface table, over the foreign package's codec module, with no builder,
/// `<base>.ts` or barrel import, because the package has no table of its own.
/// `codec_imports` names the foreign package, under the same predicate.
#[test]
fn a_package_whose_only_payload_root_is_foreign_gets_a_module_without_a_barrel() {
    let model = names_model();
    let source = module(&model, "veh.names");
    for line in [
        "import type { PayloadCodec } from \"@driftsys/ridl-runtime\";",
        "import type * as ridl_std from \"./ridl.std.js\";",
        "import * as ridl_std_codec from \"./ridl.std.codec.js\";",
        "export const PAYLOAD_TYPES = [\n] as const;",
        "const codecRidlStdName: PayloadCodec<ridl_std.Name> = { type: \"ridl.std.Name\", encode: ridl_std_codec.encodeName, decode: ridl_std_codec.decodeName };",
        "  rename: { iface: 1, ord: 1, kind: \"command\", request: codecRidlStdName },",
    ] {
        assert!(source.contains(line), "no `{line}` in:\n{source}");
    }
    for absent in [
        "flatbuffers",
        "import type * as t",
        "import * as fb",
        "./veh/names.js",
    ] {
        assert!(!source.contains(absent), "`{absent}` in:\n{source}");
    }
    assert_eq!(
        ridl_backend_ts::codec_imports(&model).expect("the module generates"),
        BTreeSet::from(["ridl.std".to_string()])
    );
    // A package with no payload root at all imports nothing.
    let output = ridlc::compile(
        "only.ridl",
        "package only\n\n/// A limit.\nconst LIMIT : integer = 6\n",
    );
    let model = codegen::lower(&output.package, &[]);
    assert!(
        ridl_backend_ts::codec_imports(&model)
            .expect("generates")
            .is_empty()
    );
}

/// The interface of `model` named `name`.
fn interface<'a>(model: &'a mut v1::Model, name: &str) -> &'a mut v1::Interface {
    model
        .interfaces
        .iter_mut()
        .find(|interface| {
            matches!(&interface.identity, Some(v1::interface::Identity::Declared(spelled)) if spelled.declared == name)
        })
        .unwrap_or_else(|| panic!("the model declares the interface `{name}`"))
}

/// Renames the interface `Sensors` of `model` to `name`.
fn rename_sensors(model: &mut v1::Model, name: &str) {
    interface(model, "Sensors").identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: name.to_string(),
        ..Default::default()
    }));
}

/// Every reserved name of the file, taken by an interface, is refused.
#[test]
fn every_reserved_name_is_refused() {
    for reserved in [
        "flatbuffers",
        "PayloadCodec",
        "t",
        "fb",
        "CATALOG_NAME",
        "CATALOG_HASH",
        "PAYLOAD_TYPES",
        "RULES",
    ] {
        let mut model = common_model();
        rename_sensors(&mut model, reserved);
        assert_eq!(
            refusal(&model),
            format!(
                "`{reserved}` in package `veh.common` collides with `{reserved}`, which the codec module also declares"
            )
        );
    }
}

/// Every helper name of `signalChecks`, taken by an interface of a package
/// with a signal, is refused.
#[test]
fn every_signal_helper_name_is_refused() {
    for reserved in [
        "signalChecks",
        "signalCheck",
        "Codec",
        "SignalCheckEntry",
        "CodecError",
    ] {
        let mut model = common_model();
        rename_sensors(&mut model, reserved);
        assert_eq!(
            refusal(&model),
            format!(
                "`{reserved}` in package `veh.common` collides with `runtime signal helper {reserved}`, which the codec module also declares"
            )
        );
    }
}

/// The alias of a foreign package reached through a field only (`ridl_std`
/// for `DriverProfile.name`) is a name of the file, so an interface of that
/// name is refused.
#[test]
fn an_interface_named_as_a_foreign_type_alias_is_refused() {
    let mut model = common_model();
    // Without `profile`, no payload type is foreign: `ridl.std` is reached
    // through fields alone.
    interface(&mut model, "Sensors").slots.retain(|slot| {
        !matches!(&slot.occupant, Some(v1::interaction_slot::Occupant::Interaction(interaction))
            if interaction.name.as_ref().is_some_and(|name| name.declared == "profile"))
    });
    let source = module(&model, "veh.common");
    assert!(!source.contains("ridl_std_codec"), "{source}");
    assert!(source.contains("import * as ridl_std from"), "{source}");
    rename_sensors(&mut model, "ridl_std");
    assert_eq!(
        refusal(&model),
        "`ridl_std` in package `veh.common` collides with `ridl_std`, which the codec module also declares"
    );
}

/// A struct named after an induced tuple of a field (`ExtrasWide`, the
/// tuple of `Extras.wide`) would give two `pack<Name>` converters, and is
/// refused.
#[test]
fn a_struct_named_as_a_reached_tuple_is_refused() {
    let mut model = common_model();
    declaration(&mut model, "SensorFault")
        .name
        .as_mut()
        .expect("named")
        .declared = "ExtrasWide".to_string();
    assert_eq!(
        refusal(&model),
        "`ExtrasWide` in package `veh.common` collides with `ExtrasWide`, which the codec module also declares"
    );
}

/// A `codec<Name>` constant no entry names is not declared, so it is not
/// registered: a local payload type `RidlStdName` that no entry names
/// generates beside the foreign `codecRidlStdName`.
#[test]
fn an_unused_constant_does_not_collide() {
    let mut model = common_model();
    declaration(&mut model, "SpeedLimitPayload")
        .name
        .as_mut()
        .expect("named")
        .declared = "RidlStdName".to_string();
    let source = module(&model, "veh.common");
    assert!(
        source.contains("export function encodeRidlStdName("),
        "{source}"
    );
    assert!(
        source.contains("const codecRidlStdName: PayloadCodec<ridl_std.Name>"),
        "{source}"
    );
    assert_eq!(source.matches("const codecRidlStdName").count(), 1);
}

/// The mutable live interaction of `model.interfaces[0]` at `slot`.
fn interaction_at(model: &mut v1::Model, slot: usize) -> &mut v1::Interaction {
    match model.interfaces[0].slots[slot].occupant.as_mut() {
        Some(v1::interaction_slot::Occupant::Interaction(interaction)) => interaction,
        _ => panic!("slot {slot} of the first interface holds a live interaction"),
    }
}

/// A member entry carries `limit` when the model records a limit for the
/// member, and no `limit` otherwise: the consumer reads each member's limit
/// from the table, so a limit the catalog records must reach it.
#[test]
fn a_member_entry_carries_the_recorded_limit() {
    let mut model = cabin_model();
    interaction_at(&mut model, 0).limit = Some(64);
    let source = module(&model, "veh.cabin");
    assert!(
        source.contains(
            "  temperature: { iface: 1, ord: 1, kind: \"signal\", payload: codecTemperature, limit: 64 },\n"
        ),
        "the entry of `temperature` carries `limit: 64`:\n{source}"
    );
    assert!(
        source
            .contains("  warning: { iface: 1, ord: 2, kind: \"event\", payload: codecWarning },\n"),
        "the entry of `warning`, which has no limit, carries none:\n{source}"
    );
}

/// The package `veh.hvac` of the Rust backend's `streams.ridl` fixture, without
/// its three members that reply with a stream: every stream it holds is a
/// parameter.
const STREAM_PARAMETERS: &str = "\
package veh.hvac

/// A temperature, in degrees Celsius.
type Temperature : integer [-40..85]

/// A fan level.
type FanLevel : integer [0..10]

/// A filter state.
struct FilterState {
  level: FanLevel
}

/// A climate report.
struct ClimateReport {
  temperature: Temperature
}

/// The cabin climate.
interface Cabin {
  /// The cabin temperature.
  signal cabinTemp : Temperature @1s

  /// A stream parameter and no other.
  command replayTrace(samples: <FilterState>) @[..50ms]

  /// A stream of bytes beside a named parameter.
  command putBlob(level: FanLevel, data: <bytes>) @[..50ms]

  /// A stream parameter and a named reply.
  query lookup(samples: <FilterState>): ClimateReport @[..50ms]
}
";

/// Every parameter-stream member gets a table entry and a typed upload function.
/// A member of the same interface without a stream keeps its ordinary entry.
#[test]
fn parameter_stream_members_have_entries_in_the_interface_table() {
    let output = ridlc::compile("stream-parameters.ridl", STREAM_PARAMETERS);
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "the package must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let model = codegen::lower(&output.package, &[]);
    let source = module(&model, "veh.hvac");
    assert!(
        source.contains("  cabinTemp: { iface: "),
        "`cabinTemp` has no entry in:\n{source}"
    );
    for member in ["replayTrace", "putBlob", "lookup"] {
        assert!(
            source.contains(&format!("  {member}: {{")),
            "`{member}` has no upload entry in:\n{source}"
        );
        assert!(
            source.contains(&format!("export function Cabin_{member}(")),
            "missing upload function {member}"
        );
    }
}

/// A package with reply streams and parameter streams is accepted: reply-only
/// queries and uploads get typed functions.
#[test]
fn a_package_with_stream_replies_is_accepted() {
    let model = codegen::lower(&rust_fixture("streams.ridl"), &[]);
    match generate_codec(&model, "veh.hvac") {
        Ok(Some(_)) => {}
        Ok(None) => panic!("streams.ridl must have a codec module, got no file"),
        Err(GenerateError::Unrepresentable(message)) => {
            panic!("streams.ridl must be accepted, got: {message}")
        }
    }
}

/// The model of the fixture `tests/fixtures/codec-open`, package `open.demo`.
fn open_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-open");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    // TYPL-406 (`missing-docs`) and TYPL-413 (an open enum with no value) are
    // the diagnostics the fixture draws.
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| matches!(diagnostic.code.as_str(), "TYPL-406" | "TYPL-413")),
        "the fixture must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "open.demo")
        .expect("the fixture declares open.demo")
        .ir
        .clone();
    codegen::lower(&package, &[&output.std_ir])
}

#[test]
fn the_open_codec_module() {
    let source = module(&open_model(), "open.demo");
    // An open enum is converted by a pair of functions that keep every value
    // and refuse a value outside the safe-integer range.
    assert!(
        source.contains("function unpackDtcStatus(raw: bigint | null): t.DtcStatusValue {"),
        "an open enum must unpack to its value type, got:\n{source}"
    );
    assert!(
        source.contains("function packDtcStatus(value: t.DtcStatusValue): bigint {"),
        "an open enum must pack from its value type, got:\n{source}"
    );
    assert!(
        source.contains("status: unpackDtcStatus(object.field1)")
            || source.contains("status: unpackDtcStatus(object.status)"),
        "a field of an open enum must use the unpack function, got:\n{source}"
    );
    // A required field of an open enum with no zero member is never absent.
    assert!(
        !source.contains("absent(\"open.demo.Report\", \"mode\")"),
        "an open enum field must not throw when absent, got:\n{source}"
    );
    // A required open enum with no zero member in the position of an induced
    // tuple is never absent either: the tuple converter unpacks it as a field.
    assert!(
        source.contains("mode: unpackMode(object.field2)"),
        "a tuple position of an open enum must use the unpack function, got:\n{source}"
    );
    assert!(
        !source.contains("absent(\"open.demo.ControlSelectParams\""),
        "an open enum tuple position must not throw when absent, got:\n{source}"
    );
    insta::assert_snapshot!("open", source);
}

/// A field of an open enum set of another package is checked against the
/// width of the set on encode, as a field of a local open set is; a strict
/// foreign set converts by assertion.
#[test]
fn a_foreign_open_enum_set_is_checked_against_its_width() {
    for (open, checked) in [(true, true), (false, false)] {
        let mut model = open_model();
        let reference = foreign_thing(
            &mut model,
            v1::declaration::Kind::EnumSet(v1::EnumSet {
                open,
                width: v1::IntWidth::U8 as i32,
                ..Default::default()
            }),
        );
        field(&mut model, "Report", 2).kind = Some(v1::r#type::Kind::Named(reference));
        let source = module(&model, "open.demo");
        assert_eq!(
            source.contains("object.flags = pack_veh_other_Thing(value.flags);"),
            checked,
            "{source}"
        );
        assert_eq!(
            source.contains("function pack_veh_other_Thing(value: veh_other.Thing): number {"),
            checked,
            "{source}"
        );
    }
}

/// The model of the fixture `tests/fixtures/codec-strict`, package
/// `strict.demo`.
fn strict_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-strict");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() == "TYPL-406"),
        "the fixture must compile with no error, got: {:?}",
        output.diagnostics,
    );
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "strict.demo")
        .expect("the fixture declares strict.demo")
        .ir
        .clone();
    codegen::lower(&package, &[&output.std_ir])
}

/// A strict enum and a strict enum set unpack through a function that throws
/// a `RuleError` with the `variant` rule, in every position a type can stand.
#[test]
fn the_strict_codec_module() {
    let source = module(&strict_model(), "strict.demo");
    assert!(
        source.contains("function unpackDtcStatus(raw: number | bigint): t.DtcStatus {"),
        "a strict enum must unpack through a checking function, got:\n{source}"
    );
    assert!(
        source.contains("throw new RuleError(\"variant\""),
        "a strict value must be refused with the variant rule, got:\n{source}"
    );
    assert!(
        source.contains("function unpackFlags(value: number): t.Flags {"),
        "a narrow strict set must check its declared bits, got:\n{source}"
    );
    assert!(
        source.contains("function unpackWideFlags(value: bigint): t.WideFlags {"),
        "a wide strict set must check its declared bits, got:\n{source}"
    );
    assert!(
        !source.contains("Number(object.") && !source.contains("Number(element)"),
        "no strict enum may be read by an unchecked cast, got:\n{source}"
    );
    assert!(
        source.contains("unpackDtcStatus(fb.DtcStatusBox"),
        "a strict enum payload must unpack through the checking function, got:\n{source}"
    );
    insta::assert_snapshot!("strict", source);
}

/// A field of a strict enum set of another package is checked against the
/// declared bits through a function of the module.
#[test]
fn a_foreign_strict_enum_set_is_checked_against_its_bits() {
    let mut model = strict_model();
    let reference = foreign_thing(
        &mut model,
        v1::declaration::Kind::EnumSet(v1::EnumSet {
            open: false,
            width: v1::IntWidth::U8 as i32,
            declared_mask: 0b101,
            ..Default::default()
        }),
    );
    field(&mut model, "Report", 2).kind = Some(v1::r#type::Kind::Named(reference));
    let source = module(&model, "strict.demo");
    assert!(
        source.contains("function unpack_veh_other_Thing(value: number): veh_other.Thing {"),
        "{source}"
    );
    assert!(source.contains("(value & ~5) !== 0"), "{source}");
    assert!(
        source.contains("flags: unpack_veh_other_Thing(object.flags)"),
        "{source}"
    );
}

/// A foreign enum set `name` of `package`, with a one-byte width.
fn foreign_set(
    model: &mut v1::Model,
    package: &str,
    name: &str,
    open: bool,
    mask: i64,
) -> v1::TypeRef {
    let index = model.foreign.len() as u32;
    model.foreign.push(v1::ForeignDeclaration {
        package: package.to_string(),
        declaration: Some(v1::Declaration {
            name: Some(v1::Spellings {
                declared: name.to_string(),
                ..Default::default()
            }),
            visibility: v1::Visibility::Public as i32,
            kind: Some(v1::declaration::Kind::EnumSet(v1::EnumSet {
                open,
                width: v1::IntWidth::U8 as i32,
                declared_mask: mask,
                ..Default::default()
            })),
            ..Default::default()
        }),
    });
    v1::TypeRef {
        reference: format!("{package}.{name}"),
        resolved: true,
        package: package.to_string(),
        foreign: true,
        index,
        kind: v1::DeclKind::Unspecified as i32,
    }
}

/// The model of a package `only` with one struct, `Holder`, of two fields, and
/// no enum or enum set of its own.
fn holder_model() -> v1::Model {
    let output = ridlc::compile(
        "only.ridl",
        "package only\n\n/// A struct.\nstruct Holder {\n  /// A value.\n  a : integer [0..9]\n  /// A value.\n  b : integer [0..9]\n}\n",
    );
    codegen::lower(&output.package, &[])
}

/// A module whose only enum reach is a strict enum set of another package
/// still exports `RuleError`, because the checking function throws it.
#[test]
fn a_foreign_strict_set_alone_exports_the_rule_error() {
    let mut model = holder_model();
    let reference = foreign_set(&mut model, "veh.other", "Thing", false, 1);
    field(&mut model, "Holder", 0).kind = Some(v1::r#type::Kind::Named(reference));
    let source = module(&model, "only");
    assert!(
        source.contains("export class RuleError extends RangeError {"),
        "{source}"
    );
    assert!(
        source.contains("function unpack_veh_other_Thing(value: number): veh_other.Thing {"),
        "{source}"
    );
}

/// Two sets of other packages whose alias and name spell one function name
/// are refused, for a strict set and for an open set.
#[test]
fn two_foreign_sets_that_spell_one_function_are_refused() {
    for open in [false, true] {
        let mut model = holder_model();
        let first = foreign_set(&mut model, "veh.body", "door_flags", open, 1);
        let second = foreign_set(&mut model, "veh.body.door", "flags", open, 1);
        field(&mut model, "Holder", 0).kind = Some(v1::r#type::Kind::Named(first));
        field(&mut model, "Holder", 1).kind = Some(v1::r#type::Kind::Named(second));
        assert_eq!(
            refusal(&model),
            "`veh.body.door_flags` in package `only` collides with `veh.body.door.flags`, which the codec module also declares"
        );
    }
}

/// `RuleError` is a name of the file whenever a strict enum is reached, so an
/// interface of that name is refused.
#[test]
fn an_interface_named_rule_error_beside_a_strict_enum_is_refused() {
    let mut model = strict_model();
    interface(&mut model, "Control").identity =
        Some(v1::interface::Identity::Declared(v1::Spellings {
            declared: "RuleError".to_string(),
            ..Default::default()
        }));
    assert_eq!(
        refusal(&model),
        "`RuleError` in package `strict.demo` collides with `RuleError`, which the codec module also declares"
    );
}

/// The unpack function of a strict enum is a name of the file, so an
/// interface of that name is refused.
#[test]
fn an_interface_named_as_the_unpack_of_a_strict_enum_is_refused() {
    let mut model = strict_model();
    interface(&mut model, "Control").identity =
        Some(v1::interface::Identity::Declared(v1::Spellings {
            declared: "unpackDtcStatus".to_string(),
            ..Default::default()
        }));
    assert_eq!(
        refusal(&model),
        "`unpackDtcStatus` in package `strict.demo` collides with `DtcStatus`, which the codec module also declares"
    );
}

/// An enum with no zero member is held as `T | null` by `flatc`, so its
/// unpack reports a value the buffer leaves out as absent; an enum with a zero
/// member never receives `null`.
#[test]
fn a_strict_enum_with_no_zero_member_reports_an_absent_value() {
    let source = module(&strict_model(), "strict.demo");
    assert!(
        source.contains("function unpackMode(raw: number | bigint | null): t.Mode {"),
        "{source}"
    );
    assert!(
        source.contains("raw === null ? absent(\"strict.demo.Mode\", \"value\") : Number(raw)"),
        "{source}"
    );
    assert!(
        source.contains("function unpackDtcStatus(raw: number | bigint): t.DtcStatus {"),
        "{source}"
    );
}

/// A module whose only enum reach is an open enum set of another package
/// still exports `RuleError`, because the width check throws it.
#[test]
fn a_foreign_open_set_alone_exports_the_rule_error() {
    let mut model = holder_model();
    let reference = foreign_set(&mut model, "veh.other", "Thing", true, 1);
    field(&mut model, "Holder", 0).kind = Some(v1::r#type::Kind::Named(reference));
    let source = module(&model, "only");
    assert!(
        source.contains("export class RuleError extends RangeError {"),
        "{source}"
    );
    assert!(
        source.contains("function pack_veh_other_Thing(value: veh_other.Thing): number {"),
        "{source}"
    );
}

/// Two foreign sets of one name in two packages each get a function with
/// the mask of their own declaration.
#[test]
fn foreign_sets_of_one_name_get_a_function_and_a_mask_each() {
    let mut model = holder_model();
    let first = foreign_set(&mut model, "veh.a", "Flags", false, 0b1);
    let second = foreign_set(&mut model, "veh.b", "Flags", false, 0b110);
    field(&mut model, "Holder", 0).kind = Some(v1::r#type::Kind::Named(first));
    field(&mut model, "Holder", 1).kind = Some(v1::r#type::Kind::Named(second));
    let source = module(&model, "only");
    assert!(
        source.contains(
            "function unpack_veh_a_Flags(value: number): veh_a.Flags {\n  if ((value & ~1) !== 0) {"
        ),
        "{source}"
    );
    assert!(
        source.contains(
            "function unpack_veh_b_Flags(value: number): veh_b.Flags {\n  if ((value & ~6) !== 0) {"
        ),
        "{source}"
    );
}

/// The model of a package with an interface named `RuleError` and no enum.
fn rule_error_model(declarations: &str, extra: &str) -> v1::Model {
    let text = format!(
        "package only\n\n{declarations}/// A struct.\nstruct Holder {{\n  /// A value.\n  a : integer [0..9]\n{extra}}}\n\n/// An interface.\ninterface RuleError {{\n  /// Sends.\n  command send(holder: Holder) @[..50ms]\n}}\n"
    );
    let output = ridlc::compile("only.ridl", &text);
    codegen::lower(&output.package, &[])
}

/// `RuleError` is reserved only when the module reaches an enum or an enum
/// set: without one, the module has no such class and an interface may take
/// the name.
#[test]
fn rule_error_is_not_reserved_without_an_enum() {
    let source = module(&rule_error_model("", ""), "only");
    assert!(source.contains("export const RuleError = {"), "{source}");
    assert!(!source.contains("class RuleError"), "{source}");
}

/// A no-zero strict enum reached only as a payload root, or only as a map
/// value, still gets the `absent` function its unpack calls.
#[test]
fn a_strict_enum_reached_alone_emits_the_absent_function() {
    let root = ridlc::compile(
        "only.ridl",
        "package only\n\n/// An enum.\nenum Mode {\n  LOW = 1\n  HIGH = 2\n}\n",
    );
    let root = module(&codegen::lower(&root.package, &[]), "only");
    assert!(root.contains("absent(\"only.Mode\", \"value\")"), "{root}");
    assert!(
        root.contains("function absent(type: string, field: string): never {"),
        "{root}"
    );

    let map = ridlc::compile(
        "only.ridl",
        "package only\n\n/// An enum.\ninternal enum Mode {\n  LOW = 1\n  HIGH = 2\n}\n\n/// A key.\ntype Key : string [0..8]\n\n/// A struct.\nstruct Holder {\n  /// A map.\n  table : [Key : Mode; 0..3]\n}\n",
    );
    let map = module(&codegen::lower(&map.package, &[]), "only");
    assert!(map.contains("unpackMode(entry.value)"), "{map}");
    assert!(
        map.contains("function absent(type: string, field: string): never {"),
        "{map}"
    );
}

/// `RuleError` is reserved when the module reaches an open enum, so an
/// interface of that name is refused there too.
#[test]
fn an_interface_named_rule_error_beside_an_open_enum_is_refused() {
    let model = rule_error_model(
        "/// An open enum.\nopen enum Kind {\n  /// A.\n  A = 1\n}\n\n",
        "  /// A kind.\n  kind : Kind\n",
    );
    assert_eq!(
        refusal(&model),
        "`RuleError` in package `only` collides with `RuleError`, which the codec module also declares"
    );
}

/// Two open sets of other packages that share a name but not a width each get
/// a `pack_<alias>_<Name>` with the width check of their own declaration.
#[test]
fn foreign_open_sets_of_one_name_check_their_own_width() {
    let mut model = holder_model();
    let first = foreign_set(&mut model, "veh.a", "Flags", true, 1);
    let second = foreign_set(&mut model, "veh.b", "Flags", true, 1);
    for (index, width) in [(0, v1::IntWidth::U8), (1, v1::IntWidth::U16)] {
        let Some(v1::declaration::Kind::EnumSet(shape)) = model.foreign[index]
            .declaration
            .as_mut()
            .and_then(|declaration| declaration.kind.as_mut())
        else {
            panic!("the foreign declaration is an enum set");
        };
        shape.width = width as i32;
    }
    field(&mut model, "Holder", 0).kind = Some(v1::r#type::Kind::Named(first));
    field(&mut model, "Holder", 1).kind = Some(v1::r#type::Kind::Named(second));
    let source = module(&model, "only");
    assert!(
        source.contains("function pack_veh_a_Flags(value: veh_a.Flags): number {\n  if (!Number.isInteger(value) || value < 0 || value > 255) {"),
        "{source}"
    );
    assert!(
        source.contains("function pack_veh_b_Flags(value: veh_b.Flags): number {\n  if (!Number.isInteger(value) || value < 0 || value > 65535) {"),
        "{source}"
    );
}

/// The stream fixture is compiled through the manifest so limits reach the model.
fn streams_model() -> v1::Model {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-streams");
    let output = ridlc::compile_workspace(&mut db, &entry).expect("load stream fixture");
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let package = &output
        .checked
        .iter()
        .find(|p| p.ir.name == "streams.demo")
        .unwrap()
        .ir;
    codegen::lower(package, &[&output.std_ir])
}

fn stream_member_mut<'a>(model: &'a mut v1::Model, member: &str) -> &'a mut v1::Interaction {
    model
        .interfaces
        .iter_mut()
        .flat_map(|i| &mut i.slots)
        .find_map(|slot| match &mut slot.occupant {
            Some(v1::interaction_slot::Occupant::Interaction(i))
                if i.name.as_ref().unwrap().declared == member =>
            {
                Some(i)
            }
            _ => None,
        })
        .expect("stream member")
}

#[test]
fn stream_reply_exports_match_model() {
    let model = streams_model();
    let source = module(&model, "streams.demo");
    // This independent list includes every live reply-only stream and no upload.
    let expected = [
        ("Feed_scalars", "t.Count"),
        ("Feed_structs", "t.Chunk"),
        ("Feed_unions", "t.Choice"),
        ("Feed_statuses", "t.StatusValue"),
        ("Feed_data", "t.Data"),
        ("Feed_labels", "t.Text"),
        ("Feed_readBytes", "Uint8Array"),
        ("Feed_text", "string"),
        ("Feed_zero", "Uint8Array"),
        ("Feed_fetch", "Uint8Array"),
        ("Other_fetch", "Uint8Array"),
        ("session_fetch", "Uint8Array"),
        ("opts_fetch", "Uint8Array"),
        ("entry_fetch", "Uint8Array"),
        ("args_fetch", "Uint8Array"),
        ("raw_fetch", "Uint8Array"),
        ("element_fetch", "t.Chunk"),
        ("requestType_fetch", "Uint8Array"),
        ("elementType_fetch", "t.Chunk"),
        ("streamEntry_fetch", "Uint8Array"),
        ("streamEntry1_fetch", "Uint8Array"),
    ];
    let actual: BTreeSet<_> = exported_functions(&source, "")
        .into_iter()
        .filter(|name| {
            !name.starts_with("encode")
                && !name.starts_with("decode")
                && !name.starts_with("Feed_upload")
                && !name.starts_with("Feed_duplex")
        })
        .collect();
    assert_eq!(
        actual,
        expected.iter().map(|(name, _)| name.to_string()).collect()
    );
    for (name, ty) in expected {
        let signature = source
            .lines()
            .find(|line| line.starts_with(&format!("export function {name}(")))
            .unwrap();
        assert!(
            signature.ends_with(&format!("): ReadableStream<{ty}> {{")),
            "{signature}"
        );
    }
    for interface in &model.interfaces {
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(i)) = &slot.occupant else {
                continue;
            };
            if i.streams.len() != 1 || i.streams[0].no != 0 {
                continue;
            }
            let name = &i.name.as_ref().unwrap().declared;
            let v1::interface::Identity::Declared(interface_name) =
                interface.identity.as_ref().unwrap()
            else {
                continue;
            };
            let table_start = source
                .find(&format!("export const {} = {{", interface_name.declared))
                .unwrap();
            let entry = source[table_start..]
                .lines()
                .find(|line| line.starts_with(&format!("  {name}: {{")))
                .unwrap();
            assert!(
                entry.contains(&format!(
                    "iface: {}, ord: {}, kind: \"query\"",
                    interface.number, slot.ordinal
                )),
                "{entry}"
            );
            assert!(entry.contains("streams: [{ no: 0, form:"), "{entry}");
        }
    }
    assert!(!source.contains("ridl-transport-ws"));
    insta::assert_snapshot!("streams", source);
}

#[test]
fn stream_tables_keep_limits_and_roles() {
    let source = module(&streams_model(), "streams.demo");
    let entry = source
        .lines()
        .find(|line| line.starts_with("  readBytes: {"))
        .expect("stream entry");
    assert!(entry.contains("limit: 512"), "{entry}");
    assert!(
        entry.contains("request: codecCount") && entry.contains("form: \"bytes\""),
        "{entry}"
    );
    assert!(!entry.contains("reply:"), "{entry}");
    let entry = source
        .lines()
        .find(|line| line.starts_with("  structs: {"))
        .unwrap();
    assert!(
        entry.contains("element: codecChunk") && !entry.contains("reply:"),
        "{entry}"
    );
    let entry = source
        .lines()
        .find(|line| line.starts_with("  latest: {"))
        .unwrap();
    assert!(
        entry.contains("reply: codecCount") && !entry.contains("streams:"),
        "{entry}"
    );
    let output = ridlc::compile(
        "raw.ridl",
        "package raw.only\ninterface Raw { query fetch(): <bytes> @[..50ms] }",
    );
    let raw = module(&codegen::lower(&output.package, &[]), "raw.only");
    assert!(raw.contains("export function Raw_fetch("), "{raw}");
    assert!(
        !raw.contains("* as t from"),
        "induced requests use no domain namespace: {raw}"
    );
    // The induced empty request has a flatc table, so this barrel does exist.
    assert!(raw.contains("import * as fb from \"./raw/only.js\""));
}

#[test]
fn stream_names_and_unsupported_shapes_are_explicit() {
    let mut model = streams_model();
    let v1::interaction::Shape::Query(q) = stream_member_mut(&mut model, "unions")
        .shape
        .as_mut()
        .unwrap()
    else {
        panic!()
    };
    q.params[2].r#type.as_mut().unwrap().optional = true;
    let tuple = q.request_tuple.as_ref().unwrap().index as usize;
    model.tuples[tuple].fields[2]
        .r#type
        .as_mut()
        .unwrap()
        .optional = true;
    let source = module(&model, "streams.demo");
    for (member, reason) in [
        ("uploadFallible", "inline fallible replies have no codec"),
        ("multiple", "multiple parameter streams are not supported"),
    ] {
        assert!(
            source.contains(&format!("// {member} has no entry: {reason}")),
            "{source}"
        );
        assert!(!source.contains(&format!("export function Feed_{member}(")));
        assert!(!source.contains(&format!("  {member}: {{")));
    }
    assert!(
        source.contains(
            "session2: t.Count, session1: t.Count, opts1: t.Count | undefined, opts: StreamOptions"
        ),
        "{source}"
    );
    // Two concatenations can collide even when their source paths differ.
    model.interfaces[0].identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: "A_B".into(),
        ..Default::default()
    }));
    stream_member_mut(&mut model, "scalars")
        .name
        .as_mut()
        .unwrap()
        .declared = "c".into();
    model.interfaces[1].identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: "A".into(),
        ..Default::default()
    }));
    let last = model.interfaces[1].slots[0].occupant.as_mut().unwrap();
    let v1::interaction_slot::Occupant::Interaction(i) = last else {
        panic!()
    };
    i.name.as_mut().unwrap().declared = "B_c".into();
    let error = refusal(&model);
    assert!(
        error.contains("A_B.c") && error.contains("A.B_c"),
        "{error}"
    );
    let mut model = streams_model();
    model.interfaces[1].identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: "decodeStream".into(),
        ..Default::default()
    }));
    let error = refusal(&model);
    assert!(
        error.contains("decodeStream") && error.contains("runtime"),
        "{error}"
    );
}

#[test]
fn generated_stream_docs_render_model_contract() {
    let mut model = streams_model();
    let i = stream_member_mut(&mut model, "structs");
    // Inject canonical model data to isolate rendering from parser behavior.
    i.doc = "Reads Count.".into();
    i.links = vec![v1::DocLink {
        offset: 6,
        len: 5,
        target: "streams.demo.Count".into(),
        ..Default::default()
    }];
    i.rules = vec![v1::Rule {
        text: "response bound <= 50000 us".into(),
        ..Default::default()
    }];
    i.since = vec!["1.2.0".into()];
    i.deprecated = Some("Use scalars.".into());
    let v1::interaction::Shape::Query(q) = i.shape.as_mut().unwrap() else {
        panic!()
    };
    q.params[0].doc = "The first Count.".into();
    q.params[0].links = vec![v1::DocLink {
        offset: 10,
        len: 5,
        target: "streams.demo.Count".into(),
        ..Default::default()
    }];
    let source = module(&model, "streams.demo");
    let signature = source
        .find("export function Feed_structs(")
        .expect("callable");
    let start = source[..signature].rfind("/**").unwrap();
    let doc = &source[start..signature];
    for expected in [
        "Reads {@link t.Count}.",
        "@remarks",
        "Rules:",
        "response bound <= 50000 us",
        "@since 1.2.0",
        "@deprecated Use scalars.",
        "@param seed - The first {@link t.Count}.",
    ] {
        assert!(doc.contains(expected), "missing {expected}: {doc}");
    }
    assert!(
        !doc.contains("github.com") && !doc.contains("rejects") && !doc.contains("range 0"),
        "{doc}"
    );
    let sig = source.find("export function Feed_unions(").unwrap();
    let start = source[..sig].rfind("/**").unwrap();
    assert!(source[start..sig].contains("@param session2 - The session number."));
    assert!(source[start..sig].contains("@param opts1 - A selection."));
    // A codec with only an induced request has no imported domain namespace.
    let mut model = streams_model();
    let i = stream_member_mut(&mut model, "text");
    i.doc = "Reads Count.".into();
    i.links = vec![v1::DocLink {
        offset: 6,
        len: 5,
        target: "streams.demo.Count".into(),
        ..Default::default()
    }];
    let v1::interaction::Shape::Query(q) = i.shape.as_ref().unwrap() else {
        panic!()
    };
    let tuple = q.request_tuple.as_ref().unwrap().index;
    model.interfaces.truncate(1);
    model.interfaces[0].slots.retain(|slot| matches!(&slot.occupant,
        Some(v1::interaction_slot::Occupant::Interaction(i)) if i.name.as_ref().unwrap().declared == "text"));
    model
        .flatbuffers
        .as_mut()
        .unwrap()
        .roots
        .retain(|r| r.tuple == Some(tuple));
    let source = module(&model, "streams.demo");
    assert!(!source.contains("* as t from"));
    assert!(source.contains("Reads `streams.demo.Count`."), "{source}");
}

#[test]
fn stream_codec_limits_and_malformed_metadata_are_explicit() {
    let mut model = streams_model();
    let root = model
        .declarations
        .iter()
        .position(|d| d.name.as_ref().unwrap().declared == "Count")
        .unwrap();
    model
        .flatbuffers
        .as_mut()
        .unwrap()
        .roots
        .iter_mut()
        .filter(|r| r.tuple.is_none() && r.declaration as usize == root)
        .for_each(|r| r.bound = None);
    let source = module(&model, "streams.demo");
    assert!(
        source.contains(
            "// scalars has no entry: streams.demo.Count has no finite FlatBuffers bound"
        ),
        "{source}"
    );
    assert!(!source.contains("export function Feed_scalars("));
    // Omitting the only stream must not leave unused request codec constants.
    let scalar_slot = model.interfaces[0].slots.iter().find(|slot| matches!(&slot.occupant,
        Some(v1::interaction_slot::Occupant::Interaction(i)) if i.name.as_ref().unwrap().declared == "scalars")).unwrap().clone();
    model.interfaces.truncate(1);
    model.interfaces[0].slots = vec![scalar_slot];
    let omitted = module(&model, "streams.demo");
    assert!(
        !omitted.contains("const codecFeedScalarsParams"),
        "{omitted}"
    );
    let mut model = streams_model();
    stream_member_mut(&mut model, "scalars").streams[0].form = v1::StreamForm::Unspecified as i32;
    assert!(refusal(&model).contains("malformed codegen model"));
    let mut model = streams_model();
    stream_member_mut(&mut model, "scalars").streams[0].element = None;
    assert!(refusal(&model).contains("malformed codegen model"));
    let mut model = streams_model();
    stream_member_mut(&mut model, "scalars").streams[0].no = 2;
    assert!(refusal(&model).contains("malformed codegen model"));
    let mut model = streams_model();
    let stream = stream_member_mut(&mut model, "scalars").streams[0].clone();
    stream_member_mut(&mut model, "scalars")
        .streams
        .push(stream);
    assert!(refusal(&model).contains("malformed codegen model"));
    for kind in [
        v1::declaration::Kind::Struct(v1::Struct::default()),
        v1::declaration::Kind::Union(v1::Union::default()),
        v1::declaration::Kind::Enum(v1::Enum::default()),
    ] {
        let mut model = streams_model();
        let reference = foreign_thing(&mut model, kind);
        stream_member_mut(&mut model, "scalars").streams[0].element = Some(reference);
        let error = refusal(&model);
        assert!(
            error.contains("veh.other.Thing") && error.contains("another package"),
            "{error}"
        );
    }
    for kind in [
        v1::declaration::Kind::Scalar(v1::Scalar::default()),
        v1::declaration::Kind::EnumSet(v1::EnumSet {
            width: v1::IntWidth::U8 as i32,
            ..Default::default()
        }),
    ] {
        let mut model = streams_model();
        let reference = foreign_thing(&mut model, kind);
        stream_member_mut(&mut model, "scalars").streams[0].element = Some(reference);
        let source = module(&model, "streams.demo");
        assert!(
            source.contains("element: codecVehOtherThing")
                && source.contains("import * as veh_other_codec"),
            "{source}"
        );
    }
    let mut model = streams_model();
    model.declarations[root].visibility = v1::Visibility::Internal as i32;
    let error = refusal(&model);
    assert!(
        error.contains("internal") && error.contains("Count"),
        "{error}"
    );
}

// Mutation: use the bare interface table in a function, skip module-name
// reservation for the capture, or allow a parameter to shadow that capture.
#[test]
fn stream_entry_bindings_avoid_interfaces_and_parameters() {
    let source = module(&streams_model(), "streams.demo");
    let names = [
        "session",
        "opts",
        "entry",
        "args",
        "raw",
        "element",
        "requestType",
        "elementType",
        "streamEntry",
        "streamEntry1",
    ];
    for binding in source.lines().filter_map(|line| {
        line.strip_prefix("const ")?
            .split_once(" = ")
            .map(|(name, _)| name)
    }) {
        assert!(
            !names.contains(&binding),
            "module binding shadows table {binding}"
        );
    }
    let mut captures = BTreeSet::new();
    for name in names {
        let capture = source
            .lines()
            .find_map(|line| {
                line.strip_prefix("const ")?
                    .strip_suffix(&format!(" = {name}.fetch;"))
            })
            .unwrap_or_else(|| panic!("{name}.fetch needs a module capture"));
        assert!(!names.contains(&capture), "capture shadows table {capture}");
        assert!(captures.insert(capture), "duplicate capture {capture}");
        let signature = source
            .lines()
            .find(|line| line.starts_with(&format!("export function {name}_fetch(")))
            .unwrap();
        assert!(!signature.contains(&format!(" {capture}:")), "{signature}");
        let body = source
            .split(&format!("export function {name}_fetch("))
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        assert!(
            body.contains(&format!("const entry = {capture};")),
            "{body}"
        );
    }
}

// Mutation: scan rendered TSDoc for link syntax instead of qualifying model links.
#[test]
fn stream_docs_preserve_literal_link_text() {
    let source = r#"package literal.docs
/// A sequence number.
type Count : integer [0..100]
/// An upload interface.
interface Feed {
  /// Accepts the literal prefix `{@link Count`.
  /// Keeps `{@link Count}` literal and escapes */ safely.
  /// Also links [Count].
  command upload(
    /// Accepts `{@link Count}` and [Count].
    seed: Count,
    data: <bytes>
  ) @[..50ms]
}
"#;
    let unmatched = r#"package literal.docs
/// A sequence number.
type Count : integer [0..100]
/// An upload interface.
interface Feed {
  /// Accepts the literal prefix `{@link Count`.
  command upload(data: <Count>) @[..50ms]
}
"#;
    for input in [unmatched, source] {
        let output = ridlc::compile("literal-docs.ridl", input);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|d| d.severity != ridl_core::diag::Severity::Error),
            "{:?}",
            output.diagnostics
        );
        let model = codegen::lower(&output.package, &[]);
        let generated = std::panic::catch_unwind(|| module(&model, "literal.docs"));
        assert!(
            generated.is_ok(),
            "valid literal documentation must not panic"
        );
        let source = generated.unwrap();
        let signature = source
            .find("export function Feed_upload(")
            .expect("upload function");
        let start = source[..signature].rfind("/**").unwrap();
        let doc = &source[start..signature];
        let expected: &[&str] = if input == unmatched {
            &["Accepts the literal prefix `{@link Count`."]
        } else {
            &[
                "Accepts the literal prefix `{@link Count`.",
                "Keeps `{@link Count}` literal and escapes *\\/ safely.",
                "Also links {@link t.Count}.",
                "@param seed - Accepts `{@link Count}` and {@link t.Count}.",
            ]
        };
        for expected in expected {
            assert!(doc.contains(expected), "missing {expected}: {doc}");
        }
        assert_eq!(
            doc.matches("*/").count(),
            1,
            "only the renderer closes the comment"
        );
    }
}

// Mutation: omit domain qualification for generated enumset bit objects.
#[test]
fn stream_doc_links_qualify_every_local_target_spelling() {
    let mut model = streams_model();
    // A domain declaration may have the same name as the codec namespace alias.
    let mut alias = model
        .declarations
        .iter()
        .find(|d| d.name.as_ref().unwrap().declared == "Count")
        .unwrap()
        .clone();
    alias.name.as_mut().unwrap().declared = "t".into();
    model.declarations.push(alias);
    let targets = [
        "Count",
        "Chunk",
        "Chunk.value",
        "Status",
        "Status.ACTIVE",
        "Flags",
        "Flags.READ",
    ];
    let expected = [
        "t.Count",
        "t.Chunk",
        "t.Chunk.value",
        "t.Status",
        "t.Status.ACTIVE",
        "t.Flags",
        "t.FlagsBits.READ",
    ];
    for (target, expected) in targets.into_iter().zip(expected) {
        let i = stream_member_mut(&mut model, "structs");
        i.doc = "Target.".into();
        i.links = vec![v1::DocLink {
            offset: 0,
            len: 6,
            target: format!("streams.demo.{target}"),
            ..Default::default()
        }];
        i.see = vec![v1::DocLink {
            target: format!("streams.demo.{target}"),
            ..Default::default()
        }];
        let source = module(&model, "streams.demo");
        let signature = source.find("export function Feed_structs(").unwrap();
        let start = source[..signature].rfind("/**").unwrap();
        let doc = &source[start..signature];
        assert!(
            doc.contains(&format!("{{@link {expected}}}.")),
            "{target}: {doc}"
        );
        assert!(
            doc.contains(&format!("@see {{@link {expected}}}")),
            "{target}: {doc}"
        );
    }
}

#[test]
fn generated_upload_encodes_only_plain_arguments() {
    let source = module(&streams_model(), "streams.demo");
    for (name, ty, result) in [
        ("upload", "Uint8Array", "accepted: Promise<void>"),
        ("uploadNamed", "t.Chunk", "accepted: Promise<void>"),
        ("uploadData", "t.Data", "accepted: Promise<void>"),
        ("uploadText", "string", "accepted: Promise<void>"),
        ("uploadChoice", "t.Chunk", "reply: Promise<t.UploadResult>"),
        (
            "uploadTuple",
            "Uint8Array",
            "reply: Promise<FeedUploadTupleReply>",
        ),
    ] {
        let signature = source
            .lines()
            .find(|line| line.starts_with(&format!("export function Feed_{name}(")));
        assert!(signature.is_some(), "missing generated upload {name}");
        let signature = signature.unwrap();
        assert!(
            signature.contains(&format!("upload: WritableStream<{ty}>; {result}")),
            "{signature}"
        );
        assert!(!signature.contains("values:"), "{signature}");
    }
    let signature = source
        .lines()
        .find(|line| line.starts_with("export function Feed_uploadNamed("))
        .unwrap();
    assert!(
        signature.contains("seed: t.Count, tail: t.Count, opts: StreamOptions"),
        "{signature}"
    );
    let start = source.find("export function Feed_uploadNamed(").unwrap();
    let doc = &source[source[..start].rfind("/**").unwrap()..start];
    for text in [
        "Receives numbered elements for {@link t.Count}.",
        "@see {@link t.Count}",
        "@param seed - The first {@link t.Count}.",
        "@param tail - The last {@link t.Count}.",
        "response within 50000 us",
    ] {
        assert!(doc.contains(text), "{doc}");
    }
    assert!(!source.contains("export function Feed_uploadFallible("));
    assert!(
        source.contains("// uploadFallible has no entry: inline fallible replies have no codec"),
        "{source}"
    );
}

#[test]
fn upload_helper_and_function_names_are_checked() {
    for helper in [
        "encodeUpload",
        "byteUpload",
        "textUpload",
        "decodeStreamReply",
    ] {
        let mut model = streams_model();
        model.interfaces[1].identity = Some(v1::interface::Identity::Declared(v1::Spellings {
            declared: helper.into(),
            ..Default::default()
        }));
        let error = refusal(&model);
        assert!(
            error.contains(helper) && error.contains("helper"),
            "{error}"
        );
    }
    let mut model = streams_model();
    model.interfaces[1].identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: "Feed_uploadNamed".into(),
        ..Default::default()
    }));
    let error = refusal(&model);
    assert!(
        error.contains("Feed.uploadNamed") && error.contains("Feed_uploadNamed"),
        "{error}"
    );
}

#[test]
fn generated_duplex_functions() {
    let source = module(&streams_model(), "streams.demo");
    for (member, upload, reply) in [
        ("duplex", "Uint8Array", "t.Chunk"),
        ("duplexNamed", "t.Chunk", "t.Chunk"),
        ("duplexText", "string", "string"),
        ("duplexBytes", "t.Chunk", "Uint8Array"),
    ] {
        let start = source.find(&format!("export function Feed_{member}("));
        assert!(start.is_some(), "missing duplex function {member}");
        let body = source[start.unwrap()..].split("\n}\n").next().unwrap();
        assert!(
            body.contains(&format!(
                "upload: WritableStream<{upload}>; stream: ReadableStream<{reply}>"
            )),
            "{body}"
        );
        assert!(
            body.contains("session.transport.queryDuplex(entry.iface, entry.ord, args, opts)"),
            "{body}"
        );
        assert!(!body.lines().next().unwrap().contains("values:"), "{body}");
        if member == "duplexNamed" {
            assert!(body.contains("{ seed: seed, tail: tail }"), "{body}");
            assert!(body.contains("encodeUpload(raw.upload"), "{body}");
            assert!(body.contains("decodeStream((bytes) => replyElement.decode(bytes), (bytes) => session.codec.verify(replyElementType, bytes))"), "{body}");
        }
    }
    assert!(!source.contains("duplex streams are not generated yet"));
    let signature = source.find("export function Feed_duplexNamed(").unwrap();
    let doc_start = source[..signature].rfind("/**").unwrap();
    let doc = &source[doc_start..signature];
    assert!(
        doc.contains("Exchanges numbered elements for {@link t.Count}."),
        "{doc}"
    );
    assert!(
        doc.contains("@param seed - The first {@link t.Count}."),
        "{doc}"
    );

    // A duplex-only module must discover its reply helper without a reply-only call.
    let mut model = streams_model();
    model.interfaces.truncate(1);
    model.interfaces[0].slots.retain(|slot| {
        matches!(&slot.occupant,
        Some(v1::interaction_slot::Occupant::Interaction(i))
            if i.name.as_ref().is_some_and(|n| n.declared == "duplexNamed"))
    });
    let source = module(&model, "streams.demo");
    assert!(
        source.contains("StreamError, decodeStream, encodeUpload }"),
        "{source}"
    );
    let v1::interaction::Shape::Query(q) = stream_member_mut(&mut model, "duplexNamed")
        .shape
        .as_mut()
        .unwrap()
    else {
        panic!()
    };
    q.params[0].name.as_mut().unwrap().declared = "replyElement".into();
    q.params[2].name.as_mut().unwrap().declared = "replyElementType".into();
    let source = module(&model, "streams.demo");
    let signature = source
        .lines()
        .find(|l| l.starts_with("export function Feed_duplexNamed("))
        .unwrap();
    assert!(
        signature.contains("replyElement1: t.Count, replyElementType1: t.Count"),
        "{signature}"
    );
    let mut collision = model.interfaces[0].clone();
    collision.slots.clear();
    collision.identity = Some(v1::interface::Identity::Declared(v1::Spellings {
        declared: "decodeStream".into(),
        ..Default::default()
    }));
    model.interfaces.push(collision);
    let error = refusal(&model);
    assert!(
        error.contains("decodeStream") && error.contains("runtime"),
        "{error}"
    );
    model.interfaces.last_mut().unwrap().identity =
        Some(v1::interface::Identity::Declared(v1::Spellings {
            declared: "Feed_duplexNamed".into(),
            ..Default::default()
        }));
    let error = refusal(&model);
    assert!(
        error.contains("Feed_duplexNamed") && error.contains("Feed.duplexNamed"),
        "{error}"
    );
}

// Mutation: omit interface-table metadata reservations from check_names.
#[test]
fn streamed_iface_member_is_refused_as_table_metadata_collision() {
    let mut model = streams_model();
    stream_member_mut(&mut model, "readBytes")
        .name
        .as_mut()
        .unwrap()
        .declared = "iface".into();
    let message = refusal(&model);
    assert!(message.contains("Feed.iface"), "{message}");
    assert!(
        message.contains("interface-table metadata key `iface`"),
        "{message}"
    );
}

// Mutation: reserve metadata names only for streamed members.
#[test]
fn plain_iface_member_is_refused_as_table_metadata_collision() {
    let output = ridlc::compile(
        "metadata.ridl",
        r#"package metadata.demo
type Count: integer [0..10]
interface Feed {
  query iface(): Count @[..50ms]
}
"#,
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let message = refusal(&codegen::lower(&output.package, &[]));
    assert!(message.contains("Feed.iface"), "{message}");
    assert!(
        message.contains("interface-table metadata key `iface`"),
        "{message}"
    );
}

// Mutation: skip name registration for unsupported stream members.
#[test]
fn unsupported_streamed_iface_member_is_refused_as_table_metadata_collision() {
    let output = ridlc::compile(
        "unsupported-metadata.ridl",
        r#"package metadata.demo
interface Feed {
  command iface(a: <bytes>, b: <bytes>) @[..50ms]
}
"#,
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let message = refusal(&codegen::lower(&output.package, &[]));
    assert!(message.contains("Feed.iface"), "{message}");
    assert!(
        message.contains("interface-table metadata key `iface`"),
        "{message}"
    );
}

// Mutations: link every constant, or exclude every constant from stream docs.
#[test]
fn stream_docs_do_not_link_to_omitted_bytes_constants() {
    let output = ridlc::compile(
        "constant-docs.ridl",
        r#"package constant.docs
type Block: bytes [0..10]
const DEFAULT: Block = "00"
const RAW: bytes = "01"
const LIMIT: integer = 10
interface Feed {
  /// Reads [DEFAULT], [RAW] and [LIMIT].
  query read(): <Block> @[..50ms]
}
"#,
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let domain = ridl_backend_ts::generate(&output.package).unwrap().source;
    assert!(!domain.contains("export const DEFAULT"), "{domain}");
    assert!(!domain.contains("export const RAW"), "{domain}");
    assert!(domain.contains("export const LIMIT"), "{domain}");
    let source = module(&codegen::lower(&output.package, &[]), "constant.docs");
    let signature = source.find("export function Feed_read(").unwrap();
    let start = source[..signature].rfind("/**").unwrap();
    let doc = &source[start..signature];
    assert!(
        doc.contains("Reads `constant.docs.DEFAULT`, `constant.docs.RAW` and {@link t.LIMIT}."),
        "{doc}"
    );
    assert!(!doc.contains("{@link t.RAW}"), "{doc}");
    assert!(!doc.contains("{@link t.DEFAULT}"), "{doc}");
}

/// The package and the model of the workspace at `relative`, compiled as the
/// CLI does.
fn workspace(relative: &str, package_name: &str) -> (v2::Package, v1::Model) {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    let package = output
        .checked
        .iter()
        .find(|checked| checked.ir.name == package_name)
        .unwrap_or_else(|| panic!("{relative} declares {package_name}"))
        .ir
        .clone();
    let model = codegen::lower(&package, &[]);
    (package, model)
}

/// The decoder calls `make<Name>Unchecked` for a named scalar exactly when the
/// domain module emits that function, and keeps the type assertion otherwise.
#[test]
fn the_decoder_calls_the_unchecked_constructor_where_the_domain_module_has_one() {
    for (relative, package_name, must_have_codec) in [
        ("../../examples/cabin", "veh.cabin", true),
        ("tests/fixtures/ctor", "ctor", false),
    ] {
        let (package, model) = workspace(relative, package_name);
        let domain = ridl_backend_ts::generate(&package)
            .expect("the domain module generates")
            .source;
        let Some(codec) = generate_codec(&model, "base").expect("the codec generates") else {
            assert!(!must_have_codec, "{package_name} has a codec module");
            continue;
        };
        let mut scalars = 0;
        for decl in &package.decls {
            let Some(v2::decl::Kind::TypeDef(_)) = &decl.kind else {
                continue;
            };
            let name = &decl.name;
            // A scalar is a payload root when the codec module decodes it.
            if !codec.source.contains(&format!("function decode{name}(")) {
                continue;
            }
            scalars += 1;
            let unchecked = domain.contains(&format!("function make{name}Unchecked("));
            assert_eq!(
                codec.source.contains(&format!("make{name}Unchecked(")),
                unchecked,
                "{name}: the decoder and the domain module disagree"
            );
            assert_eq!(
                codec.source.contains(&format!(" as t.{name};")),
                !unchecked,
                "{name}: the decoder keeps an assertion for a type with no unchecked form"
            );
        }
        assert!(
            scalars > 0,
            "{package_name} has no scalar payload to compare"
        );
    }
}
