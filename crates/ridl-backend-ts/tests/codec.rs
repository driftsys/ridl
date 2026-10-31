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
        "    gears: object.gears.map((element) => Number(element) as t.GearPosition),",
        "  object.maybe = value.maybe === undefined ? [] : [...value.maybe];",
        "    maybe: object.maybe.map((element) => element as t.Speed),",
        "  object.tag = value.tag === undefined ? [] : Array.from(value.tag);",
        "    tag: Uint8Array.from(object.tag) as Uint8Array,",
        "  object.notes = value.notes === undefined ? [] : value.notes.map(([key, value]) => { const entry = new fb.ExtrasNotesEntryT(); entry.key = key; entry.value = value; return entry; });",
        "    notes: object.notes.map((entry) => [entry.key as ridl_std.Label, entry.value as ridl_std.Name] as const),",
        "  object.default_ = value.default;",
        "    new: object.new_ as t.Counter,",
        "  object.class_ = value.class;",
        "      return new fb.PickT(fb.PickUnion.speed, new fb.PickSpeedBoxT(value.value));",
        "      return { kind: \"speed\", value: (object.value as fb.PickSpeedBoxT).value as t.Speed };",
        "      return new fb.PickT(fb.PickUnion.gear, new fb.PickGearBoxT(BigInt(value.value)));",
        "      return { kind: \"gear\", value: Number((object.value as fb.PickGearBoxT).value) as t.GearPosition };",
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
    assert!(
        source.contains("import type * as ridl_std from"),
        "{source}"
    );
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

/// A member that carries a stream is left out of the interface table, as the
/// Rust backend leaves it out of the face: its request holds only the
/// non-stream parameters, so a plain call would carry an incomplete request.
/// A member of the same interface without a stream keeps its entry. Every
/// stream of the package is a parameter, so the members would otherwise have
/// a representable entry: without the skip, the table lists them.
#[test]
fn a_stream_member_has_no_entry_in_the_interface_table() {
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
            !source.contains(&format!("  {member}: {{")),
            "`{member}` has an entry in:\n{source}"
        );
        let note = format!("  // {member} has no entry: it carries a stream\n");
        assert!(
            source.contains(&note),
            "the skip of `{member}` has no note in:\n{source}"
        );
    }
}

/// A package whose members reply with a stream, and take one, is accepted:
/// the codec does not try to represent the payload of a stream member.
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
