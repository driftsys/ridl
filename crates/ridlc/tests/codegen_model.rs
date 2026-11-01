//! The lowered codegen model: its invariants, its encoding, its nesting
//! bound, and the floor it has to meet
//! (`docs/wip/2026-09-22-codegen-model-design.md` §10).
//!
//! **Why this file is in `ridlc` and not in `ridl-ir`.** The design note asks
//! for these tests in `crates/ridl-ir`, which is where the model lives. Every
//! test below needs the front end, and three of them need the corpus
//! fixtures, which live beside this file; reaching either from `ridl-ir`
//! would mean a dev-dependency on `ridlc`, which depends on `ridl-ir` — a
//! dependency cycle added for a test. `crates/ridlc/tests/ir_canonical.rs`
//! made the same move for the same reason, and the model's tests that need no
//! front end stay in `crates/ridl-ir/src/codegen/tests.rs`.

use std::path::{Path, PathBuf};

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

/// Every corpus entry, in directory order.
fn corpus_entries() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the corpus directory is readable")
        .map(|entry| entry.expect("a corpus directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "the corpus must not be empty");
    entries
}

/// Every package of every corpus entry, with the scope the CLI hands the
/// backends: every sibling of the build plus `ridl.std`.
fn corpus_packages() -> Vec<(String, Vec<v2::Package>)> {
    corpus_entries()
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .expect("a named entry")
                .to_string_lossy()
                .into_owned();
            let mut db = RidlDatabase::default();
            let output = ridlc::compile_workspace(&mut db, &entry).expect("a corpus entry loads");
            let mut packages: Vec<v2::Package> = output
                .checked
                .iter()
                .map(|checked| checked.ir.clone())
                .collect();
            packages.push(output.std_ir.clone());
            (name, packages)
        })
        .collect()
}

/// `lower` keeps every repeated field in the IR's own order and count, which
/// is what lets a backend read the model and the IR side by side while it is
/// ported (design note D-1).
#[test]
fn the_model_is_positionally_aligned_with_the_ir_it_was_lowered_from() {
    let mut checked = 0usize;
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let model = codegen::lower(package, &others);
            let label = format!("{entry}: package {}", package.name);

            assert_eq!(
                model.declarations.len(),
                package.decls.len(),
                "{label}: one model declaration per IR declaration"
            );
            for (index, decl) in package.decls.iter().enumerate() {
                let lowered = &model.declarations[index];
                assert_eq!(
                    lowered.name.as_ref().expect("a lowered name").declared,
                    decl.name,
                    "{label}: declarations[{index}] is decls[{index}]"
                );
                if let (
                    Some(v2::decl::Kind::StructDef(def)),
                    Some(v1::declaration::Kind::Struct(lowered)),
                ) = (decl.kind.as_ref(), lowered.kind.as_ref())
                {
                    assert_eq!(
                        lowered.slots.len(),
                        def.members.len(),
                        "{label}: one slot per struct member of `{}`",
                        decl.name
                    );
                }
            }

            let shapes: Vec<_> = package.shapes().collect();
            assert_eq!(
                model.interfaces.len(),
                shapes.len(),
                "{label}: one model interface per shape"
            );
            for (index, shape) in shapes.iter().enumerate() {
                let lowered = &model.interfaces[index];
                assert_eq!(
                    lowered.slots.len(),
                    shape.interface.interactions.len(),
                    "{label}: one slot per interaction of shape {index}"
                );
                for (slot, interaction) in shape.interface.interactions.iter().enumerate() {
                    assert_eq!(
                        lowered.slots[slot].ordinal, interaction.ordinal,
                        "{label}: slot {slot} of shape {index} holds its IR ordinal"
                    );
                }
            }

            assert_eq!(
                model.services.len(),
                package.services.len(),
                "{label}: one model service per IR service"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the corpus must offer packages to check");
}

/// Every named induced tuple carries a FlatBuffers table, over the whole
/// corpus.
///
/// The lowering induces a tuple and names it; the projection is handed those
/// tuples and queues a table for each one it did not already reach. This is
/// the test that keeps the two joined: a tuple the lowering names and the
/// projection does not cover is silent otherwise — the model keeps the
/// tuple, and only the join between them is lost. The corpus is the input
/// because it holds the names a hand-built fixture does not: a service's
/// dotted identity, an inline shape, a retired slot (driftsys/ridl#713).
#[test]
fn every_named_induced_tuple_in_the_corpus_has_a_flatbuffers_table() {
    let mut checked = 0usize;
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let model = codegen::lower(package, &others);
            for tuple in &model.tuples {
                let Some(name) = tuple.name.as_ref() else {
                    continue;
                };
                if name.rust.is_empty() {
                    continue;
                }
                assert!(
                    tuple.flatbuffers_table.is_some(),
                    "{entry}: package {}: the induced tuple `{}` has no FlatBuffers \
                     table",
                    package.name,
                    name.wire
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the corpus must offer induced tuples to check");
}

/// The model is a function of the IR and the scope: lowering twice gives the
/// same bytes, and those bytes read back into the same model.
#[test]
fn lowering_is_deterministic_and_the_canonical_encoding_round_trips() {
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let label = format!("{entry}: package {}", package.name);
            let first = codegen::lower(package, &others);
            let second = codegen::lower(package, &others);
            assert_eq!(first, second, "{label}: lowering twice gives one model");

            let json = codegen::to_json_pretty(&first)
                .unwrap_or_else(|err| panic!("{label} must serialize: {err}"));
            let again = codegen::to_json_pretty(&second)
                .unwrap_or_else(|err| panic!("{label} must serialize again: {err}"));
            assert_eq!(json, again, "{label}: two lowerings write one artifact");

            let decoded = codegen::from_json(&json)
                .unwrap_or_else(|err| panic!("{label} must parse back: {err}"));
            assert_eq!(first, decoded, "{label} must round-trip equal");

            let binary = codegen::to_binary(&first);
            assert_eq!(
                codegen::from_binary(&binary).expect("the binary decodes"),
                first,
                "{label} must round-trip through the derived binary encoding"
            );
        }
    }
}

/// The source depth the parser admits: `MAX_TYPE_DEPTH` is 128 and FORM-102
/// is drawn at 128, so 127 is the deepest type nesting that compiles.
const DEEPEST_ADMITTED: usize = 127;

/// The bound a reader of `ridl.codegen.v1` must provision, asserted rather
/// than described (design note §6.2).
///
/// Per shape: the message levels below the `Model` root that the lowered
/// deepest admissible source nests, and the JSON levels its canonical
/// artifact nests. The map shape is the deepest and is therefore the bound.
/// The tuple shape is a constant, because a tuple position is a `TupleRef`
/// and each tuple is its own five-level entry in `Model.tuples`.
const SHAPE_BOUNDS: &[(&str, usize, usize)] =
    &[("arrays", 259, 262), ("maps", 261, 264), ("tuples", 6, 9)];

/// One package whose single struct field nests `depth` levels of the given
/// shape — the three shapes `ir_canonical.rs` measures, so the IR's figures
/// and the model's are measured over one source.
fn shaped_source(shape: &str, depth: usize) -> String {
    let payload = match shape {
        "arrays" => {
            let mut payload = "integer".to_string();
            for _ in 0..depth {
                payload = format!("[{payload}; 1]");
            }
            payload
        }
        "maps" => {
            let mut payload = "integer".to_string();
            for _ in 0..depth {
                payload = format!("[string : {payload}; 0..1]");
            }
            payload
        }
        "tuples" => {
            let mut payload = "integer".to_string();
            for level in (0..depth).rev() {
                payload = format!("(f{level}: {payload})");
            }
            payload
        }
        other => panic!("unknown shape {other}"),
    };
    format!("package veh.deep\n\nstruct Deep {{\n  payload : {payload}\n}}\n")
}

/// The deepest package the front end admits lowers to a model that nests
/// exactly the levels the design note states.
#[test]
fn the_deepest_package_the_front_end_admits_pins_the_model_nesting_bound() {
    with_sized_stack(|| {
        for (shape, message_levels, json_levels) in SHAPE_BOUNDS {
            let source = shaped_source(shape, DEEPEST_ADMITTED);
            let compiled = ridlc::compile("deep.typl", &source);
            let errors: Vec<_> = compiled
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(|diagnostic| format!("{}: {}", diagnostic.code.0, diagnostic.message))
                .collect();
            assert!(
                errors.is_empty(),
                "{shape} at source depth {DEEPEST_ADMITTED} must compile clean, got: {errors:?}"
            );
            let model = codegen::lower(&compiled.package, &[]);
            let json = codegen::to_json_pretty(&model).expect("the model serializes");
            let (objects, brackets) = depths(&json);
            assert_eq!(
                objects - 1,
                *message_levels,
                "{shape}: the message levels below the model root are the bound the design \
                 note states"
            );
            assert_eq!(
                brackets, *json_levels,
                "{shape}: the JSON levels are the bound the design note states"
            );
            // The request reader filters a `serde_json::Value` against the
            // descriptor, then runs the generated deserializer; the deepest
            // model the front end admits must still read back inside a
            // request.
            let request = v1::CodegenRequest {
                model: Some(model),
                ..Default::default()
            };
            let text = codegen::request_to_json(&request).expect("the request serializes");
            assert_eq!(
                codegen::request_from_json(&text).expect("the deepest request reads"),
                request,
                "{shape}: the deepest request must round-trip"
            );
        }
    });
}

/// Every corpus package lowers to a model whose request reads back equal
/// through `request_from_json`, which ignores unknown keys by filtering a
/// `serde_json::Value` against the descriptor before the generated
/// deserializer reads it: that path gives the value the strict path gave.
#[test]
fn every_corpus_request_round_trips_through_the_request_reader() {
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let request = v1::CodegenRequest {
                schema: codegen::SCHEMA.to_string(),
                toolchain: "t".to_string(),
                model: Some(codegen::lower(package, &others)),
                artifact_base: package.name.clone(),
                ..Default::default()
            };
            let json = codegen::request_to_json(&request).expect("the request renders");
            assert_eq!(
                codegen::request_from_json(&json).expect("the request reads"),
                request,
                "{entry}: package {} must round-trip",
                package.name
            );
        }
    }
}

/// The D-P4 floor, over the interaction-face fixture: everything an IPC
/// binding needs is in the model, and each fact is the one the toolchain
/// already computes elsewhere (design note §3.6).
#[test]
fn the_face_fixture_carries_the_whole_ipc_floor() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ridl-backend-rust/tests/fixtures/interaction_face.ridl"),
    )
    .expect("the face fixture is readable");
    let compiled = ridlc::compile("interaction_face.ridl", &source);
    let model = codegen::lower(&compiled.package, &[]);

    let interface = model
        .interfaces
        .first()
        .expect("the fixture declares one interface");
    assert_eq!(
        interface.number, 1,
        "the interface number comes from the lock"
    );
    assert!(
        interface.provisional,
        "the fixture has no lock file, so its number is provisional"
    );

    // The ordinals are one sequence over the interface body, tombstones
    // counted (ridl §11), and each live slot names its kind.
    let ordinals: Vec<u32> = interface.slots.iter().map(|slot| slot.ordinal).collect();
    assert_eq!(
        ordinals,
        (1..=ordinals.len() as u32).collect::<Vec<_>>(),
        "the ordinals are one contiguous sequence"
    );
    let kinds: Vec<v1::Kind> = interface
        .slots
        .iter()
        .filter_map(|slot| match slot.occupant.as_ref()? {
            v1::interaction_slot::Occupant::Interaction(interaction) => {
                Some(v1::Kind::try_from(interaction.kind).expect("a named kind"))
            }
            v1::interaction_slot::Occupant::Retired(_) => None,
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            v1::Kind::Signal,
            v1::Kind::Event,
            v1::Kind::Command,
            v1::Kind::Query
        ],
        "each live slot lowers to the kind the fixture declares, in order"
    );
    // Every payload names a resolved type and carries the FlatBuffers bound
    // the codec emits as `MAX_SIZE` for it.
    let mut payloads = 0usize;
    for slot in &interface.slots {
        let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = slot.occupant.as_ref()
        else {
            continue;
        };
        for payload in interaction_payloads(interaction) {
            let reference = payload.r#type.as_ref().expect("a payload names a type");
            assert!(
                reference.resolved,
                "payload `{}` resolves in the model",
                reference.reference
            );
            let declaration = &model.declarations[reference.index as usize];
            let bound = model
                .flatbuffers
                .as_ref()
                .expect("the projection is lowered")
                .roots
                .iter()
                .find(|root| root.tuple.is_none() && root.declaration == reference.index)
                .and_then(|root| match root.bound.as_ref() {
                    Some(v1::fb_root::Bound::MaxSize(size)) => Some(*size),
                    _ => None,
                });
            assert_eq!(
                payload.flatbuffers_max_size,
                bound,
                "the payload bound of `{}` is the root bound of `{}`",
                reference.reference,
                declaration.name.as_ref().expect("a name").declared
            );
            payloads += 1;
        }
    }
    assert_eq!(
        payloads, 5,
        "the signal, the event, the command and the query's two halves each carry one payload"
    );

    // The timing bounds, as the IR resolves them: a signal and an event are
    // always timed, a fixed never is.
    for slot in &interface.slots {
        let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = slot.occupant.as_ref()
        else {
            continue;
        };
        match v1::Kind::try_from(interaction.kind).expect("a named kind") {
            v1::Kind::Signal | v1::Kind::Event => assert!(
                interaction.timing.is_some(),
                "`{}` is timed",
                interaction.name.as_ref().expect("a name").declared
            ),
            v1::Kind::Fixed => assert!(
                interaction.timing.is_none(),
                "a fixed carries no timing (ADR-0015 decision 4)"
            ),
            _ => {}
        }
    }

    // Every clause the fixture declares is translated, not refused.
    let translated: Vec<&v1::Comparison> = interface
        .slots
        .iter()
        .filter_map(|slot| match slot.occupant.as_ref()? {
            v1::interaction_slot::Occupant::Interaction(interaction) => Some(&**interaction),
            v1::interaction_slot::Occupant::Retired(_) => None,
        })
        .flat_map(interaction_clauses)
        .filter_map(|clause| match clause.translation.as_ref()? {
            v1::clause::Translation::Comparison(comparison) => Some(comparison),
            v1::clause::Translation::Refused(_) => None,
        })
        .collect();
    assert_eq!(
        translated.len(),
        3,
        "the fixture's three clauses — one require on the command, a require and an ensure on \
         the query — are all in the form the translator accepts"
    );
    for comparison in translated {
        assert_ne!(
            v1::ScalarClass::try_from(comparison.literal_class).expect("a named class"),
            v1::ScalarClass::Unspecified,
            "a translated clause names the subject's scalar class"
        );
    }

    // The catalog hash (the frame specification §6.1) is the one ADR-0014
    // decision 15 defines, over the package's unit and the scope the model
    // was lowered over.
    let catalog = model.catalog.as_ref().expect("a catalog");
    let unit = ridl_ir::v2::unit_of(&compiled.package);
    assert_eq!(catalog.package, unit, "the catalog is named after the unit");
    assert_eq!(
        catalog.hash,
        ridl_ir::catalog_hash::catalog_hash(unit, &[&compiled.package]).to_vec(),
        "the hash is `ridl_ir::catalog_hash::catalog_hash` over the lowered package's unit"
    );
    assert!(
        catalog.hash.iter().any(|byte| *byte != 0),
        "the hash is not the all-zero placeholder"
    );
}

fn interaction_payloads(interaction: &v1::Interaction) -> Vec<&v1::Payload> {
    let mut payloads = Vec::new();
    match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => payloads.extend(signal.payload.as_ref()),
        Some(v1::interaction::Shape::Event(event)) => payloads.extend(event.payload.as_ref()),
        Some(v1::interaction::Shape::Command(command)) => {
            payloads.extend(command.request.as_ref());
        }
        Some(v1::interaction::Shape::Query(query)) => {
            payloads.extend(query.request.as_ref());
            payloads.extend(query.reply_payload.as_ref());
        }
        Some(v1::interaction::Shape::Fixed(fixed)) => payloads.extend(fixed.named.as_ref()),
        None => {}
    }
    payloads
}

fn interaction_clauses(interaction: &v1::Interaction) -> &[v1::Clause] {
    match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Command(command)) => &command.clauses,
        Some(v1::interaction::Shape::Query(query)) => &query.clauses,
        _ => &[],
    }
}

/// The two depths of a canonical artifact, measured from its bytes — the same
/// scan `ir_canonical.rs` makes, for the same reason: parsing the artifact to
/// learn its depth would recurse as deep as the artifact nests.
fn depths(json: &str) -> (usize, usize) {
    let (mut objects, mut brackets) = (0usize, 0usize);
    let (mut max_objects, mut max_brackets) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for byte in json.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => {
                objects += 1;
                brackets += 1;
                max_objects = max_objects.max(objects);
                max_brackets = max_brackets.max(brackets);
            }
            b'[' => {
                brackets += 1;
                max_brackets = max_brackets.max(brackets);
            }
            b'}' => {
                objects = objects.saturating_sub(1);
                brackets = brackets.saturating_sub(1);
            }
            b']' => brackets = brackets.saturating_sub(1),
            _ => {}
        }
    }
    (max_objects, max_brackets)
}

/// Runs `test` on a thread whose stack fits the recursion the deep fixtures
/// drive — the same helper, and the same reason, as `ir_canonical.rs`.
fn with_sized_stack(test: impl FnOnce() + Send + 'static) {
    let outcome = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .expect("spawn the large-stack test thread")
        .join();
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}

/// The lowered system of `entry` under `ridlc/tests` or `examples`, the names
/// of the deployments its source declares, and every package of its workspace
/// with `ridl.std`: the inputs `select_deployment` reads.
fn system_and_packages(entry: &Path) -> (Option<v2::System>, Vec<String>, Vec<v2::Package>) {
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, entry).expect("the entry loads");
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    (output.system, output.declared_deployments, packages)
}

fn corpus_entry(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(name)
}

/// `examples/cabin` declares one deployment, `Bench`, so a build without
/// `--deployment` carries it.
/// The request carries the compatible catalog hashes the caller gives, in that
/// order, byte for byte.
#[test]
fn the_request_carries_the_compatible_catalogs_in_order() {
    let (_, packages) = corpus_packages()
        .into_iter()
        .next()
        .expect("a corpus entry");
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let request = ridlc::codegen_request(
        "x",
        &packages[0],
        &refs[1..],
        &[[2; 32], [1; 32]],
        Vec::new(),
        None,
        None,
    );
    let catalog = request.model.unwrap().catalog.unwrap();
    assert_eq!(catalog.compatible, vec![vec![2u8; 32], vec![1u8; 32]]);
}

/// The compatible list is data about history: it never changes the catalog
/// hash the model carries, whatever the list holds.
#[test]
fn the_compatible_catalogs_never_change_the_catalog_hash() {
    let (_, packages) = corpus_packages()
        .into_iter()
        .next()
        .expect("a corpus entry");
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let empty = codegen::lower_with(&packages[0], &refs[1..], &[]);
    let listed = codegen::lower_with(&packages[0], &refs[1..], &[[7; 32], [9; 32]]);
    let hash = |model: &v1::Model| model.catalog.as_ref().unwrap().hash.clone();
    assert_eq!(hash(&empty), hash(&listed));
    assert_eq!(listed.catalog.unwrap().compatible.len(), 2);
}

#[test]
fn a_request_for_cabin_carries_its_one_deployment_without_a_flag() {
    let cabin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cabin");
    let (system, declared, packages) = system_and_packages(&cabin);
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let deployment = ridlc::select_deployment(system.as_ref(), &declared, None, &refs)
        .expect("no name is not an error")
        .expect("the one deployment is selected");
    assert_eq!(deployment.name, "Bench");
    assert_eq!(deployment.channels.len(), 5);

    let request = ridlc::codegen_request(
        "veh.cabin",
        &packages[0],
        &refs[1..],
        &[],
        Vec::new(),
        Some(deployment),
        None,
    );
    assert_eq!(
        request
            .deployment
            .as_ref()
            .map(|deployment| deployment.name.as_str()),
        Some("Bench")
    );
}

/// The request `codegen_request` builds for `name` with no deployment, next
/// to the request written out field by field, and the production serializer's
/// bytes for each. The deployment section is absent from the bytes.
fn assert_request_without_deployment(entry: &str, name: &str) {
    let (system, declared, packages) = system_and_packages(&corpus_entry(entry));
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let selected =
        ridlc::select_deployment(system.as_ref(), &declared, None, &refs).expect("not an error");
    assert!(selected.is_none(), "{entry}: no deployment without a name");

    let request = ridlc::codegen_request(
        name,
        &packages[0],
        &refs[1..],
        &[],
        Vec::new(),
        selected,
        None,
    );
    let expected = v1::CodegenRequest {
        schema: codegen::SCHEMA.to_string(),
        toolchain: env!("CARGO_PKG_VERSION").to_string(),
        model: Some(codegen::lower(&packages[0], &refs[1..])),
        options: Vec::new(),
        artifact_base: name.to_string(),
        deployment: None,
        generated_marker: codegen::generated_marker(Some(&packages[0].name)),
        header: String::new(),
    };
    assert_eq!(request, expected, "{entry}");
    let json = codegen::request_to_json(&request).expect("the request serializes");
    assert_eq!(
        json,
        codegen::request_to_json(&expected).expect("the literal serializes"),
        "{entry}"
    );
    assert!(
        !json.contains("\"deployment\""),
        "{entry}: no deployment key in the request:\n{json}"
    );
}

/// A workspace with no system builds the request it built before the
/// deployment section existed.
#[test]
fn a_workspace_without_a_system_builds_the_request_of_today() {
    assert_request_without_deployment("veh-common", "veh.common");
}

/// Appendix A declares `Production` and `Bench`; with no name the request
/// carries neither.
#[test]
fn two_deployments_and_no_flag_select_none() {
    let (system, declared, packages) = system_and_packages(&corpus_entry("rsdl-appendix-a"));
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let selected =
        ridlc::select_deployment(system.as_ref(), &declared, None, &refs).expect("not an error");
    assert!(selected.is_none());
    assert_request_without_deployment("rsdl-appendix-a", "veh.topology");
}

#[test]
fn an_unknown_name_lists_the_known_deployments() {
    let (system, declared, packages) = system_and_packages(&corpus_entry("rsdl-appendix-a"));
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let err = ridlc::select_deployment(system.as_ref(), &declared, Some("Nope"), &refs)
        .expect_err("no deployment is named Nope");
    assert_eq!(
        err,
        ridlc::UnknownDeployment {
            requested: "Nope".to_string(),
            known: vec!["Bench".to_string(), "Production".to_string()],
            has_system: true,
        }
    );
    let named = ridlc::select_deployment(system.as_ref(), &declared, Some("Production"), &refs)
        .expect("Production is declared")
        .expect("and selected");
    assert_eq!(named.name, "Production");

    // The known names are sorted, whatever order the source declares them in.
    let reversed = ["Zulu".to_string(), "Alpha".to_string()];
    let err = ridlc::select_deployment(system.as_ref(), &reversed, Some("Nope"), &refs)
        .expect_err("no deployment is named Nope");
    assert_eq!(err.known, ["Alpha", "Zulu"]);
    assert!(
        err.to_string()
            .ends_with("the system declares: Alpha, Zulu"),
        "{err}"
    );
}

/// The whole deployment section `examples/cabin` carries, end to end: the
/// rsdl source, the lowering, and the emitter together, against a golden
/// document.
///
/// The section's own tests in `crates/ridl-ir` build a `System` by hand, so
/// none of them covers the path from an rsdl file to the bytes a plugin
/// reads. This one does: the two interfaces with their numbers, the three
/// placed instances with what each offers and maps, the five channels in
/// their emitted order, and each channel's consumer links with their
/// crossings, encodings and sizing. A signal channel's omitted sizing fields
/// are part of the document, so a zero written in place of an absent value
/// shows here too.
///
/// The catalog hash is content-addressed, so the golden would change with
/// every edit to `examples/cabin/cabin.ridl`. Its length is checked and the
/// bytes are then replaced, which is the one substitution this document
/// makes.
#[test]
fn the_cabin_deployment_section_is_rendered_end_to_end() {
    let cabin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cabin");
    let (system, declared, packages) = system_and_packages(&cabin);
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let mut deployment = ridlc::select_deployment(system.as_ref(), &declared, None, &refs)
        .expect("no name is not an error")
        .expect("the one deployment is selected");
    for region in &mut deployment.regions {
        assert_eq!(
            region.hash.len(),
            32,
            "the driver embeds the 32-byte catalog hash"
        );
        region.hash = vec![1, 2, 3];
    }
    let request = v1::CodegenRequest {
        deployment: Some(deployment),
        ..Default::default()
    };
    let json = codegen::request_to_json(&request).expect("the request renders");
    assert_eq!(
        json,
        r#"{
  "schema": "",
  "toolchain": "",
  "options": [],
  "artifactBase": "",
  "deployment": {
    "system": "veh.cabin.Vehicle",
    "name": "Bench",
    "regions": [
      {
        "catalog": "veh.cabin",
        "hash": "AQID",
        "interfaces": [
          {
            "name": "Cabin",
            "number": 1,
            "inline": false,
            "provisional": true,
            "service": "veh.cabin.control"
          },
          {
            "name": "Horn",
            "number": 2,
            "inline": false,
            "provisional": true,
            "service": "veh.cabin.control"
          }
        ]
      }
    ],
    "instances": [
      {
        "component": "veh.cabin.Climate",
        "instance": "Unit",
        "machine": "Hpc",
        "external": false,
        "offers": [
          {
            "catalog": "veh.cabin",
            "number": 1,
            "name": "Cabin",
            "inline": false
          },
          {
            "catalog": "veh.cabin",
            "number": 2,
            "name": "Horn",
            "inline": false
          }
        ],
        "maps": []
      },
      {
        "component": "veh.cabin.Panel",
        "instance": "Unit",
        "machine": "Hpc",
        "external": false,
        "offers": [],
        "maps": [
          "veh.cabin"
        ]
      },
      {
        "component": "veh.cabin.Telemetry",
        "instance": "Unit",
        "machine": "Gateway",
        "external": false,
        "offers": [],
        "maps": [
          "veh.cabin"
        ]
      }
    ],
    "channels": [
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 1,
        "interface": "Cabin",
        "inline": false,
        "memberOrdinal": 1,
        "member": "temperature",
        "kind": "KIND_SIGNAL",
        "producer": {
          "component": "veh.cabin.Climate",
          "instance": "Unit",
          "machine": "Hpc"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "Hpc",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          },
          {
            "component": "veh.cabin.Telemetry",
            "instance": "Unit",
            "machine": "Gateway",
            "crossing": "CROSSING_DIFFERENT_MACHINE",
            "encoding": "ENCODING_PROTO3",
            "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          }
        ]
      },
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 1,
        "interface": "Cabin",
        "inline": false,
        "memberOrdinal": 2,
        "member": "warning",
        "kind": "KIND_EVENT",
        "producer": {
          "component": "veh.cabin.Climate",
          "instance": "Unit",
          "machine": "Hpc"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "Hpc",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "depth": {
              "value": 10,
              "source": "VALUE_SOURCE_DERIVED"
            },
            "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          },
          {
            "component": "veh.cabin.Telemetry",
            "instance": "Unit",
            "machine": "Gateway",
            "crossing": "CROSSING_DIFFERENT_MACHINE",
            "encoding": "ENCODING_PROTO3",
            "depth": {
              "value": 10,
              "source": "VALUE_SOURCE_DERIVED"
            },
            "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          }
        ],
        "depth": {
          "value": 10,
          "source": "VALUE_SOURCE_DERIVED"
        }
      },
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 1,
        "interface": "Cabin",
        "inline": false,
        "memberOrdinal": 3,
        "member": "setLevel",
        "kind": "KIND_COMMAND",
        "producer": {
          "component": "veh.cabin.Climate",
          "instance": "Unit",
          "machine": "Hpc"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "Hpc",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "slots": 16,
            "slotsSource": "VALUE_SOURCE_DEFAULT",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          },
          {
            "component": "veh.cabin.Telemetry",
            "instance": "Unit",
            "machine": "Gateway",
            "crossing": "CROSSING_DIFFERENT_MACHINE",
            "encoding": "ENCODING_PROTO3",
            "slots": 16,
            "slotsSource": "VALUE_SOURCE_DEFAULT",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          }
        ]
      },
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 1,
        "interface": "Cabin",
        "inline": false,
        "memberOrdinal": 4,
        "member": "average",
        "kind": "KIND_QUERY",
        "producer": {
          "component": "veh.cabin.Climate",
          "instance": "Unit",
          "machine": "Hpc"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "Hpc",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "slots": 16,
            "slotsSource": "VALUE_SOURCE_DEFAULT",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          },
          {
            "component": "veh.cabin.Telemetry",
            "instance": "Unit",
            "machine": "Gateway",
            "crossing": "CROSSING_DIFFERENT_MACHINE",
            "encoding": "ENCODING_PROTO3",
            "slots": 16,
            "slotsSource": "VALUE_SOURCE_DEFAULT",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          }
        ]
      },
      {
        "catalog": "veh.cabin",
        "interfaceNumber": 2,
        "interface": "Horn",
        "inline": false,
        "memberOrdinal": 1,
        "member": "active",
        "kind": "KIND_SIGNAL",
        "producer": {
          "component": "veh.cabin.Climate",
          "instance": "Unit",
          "machine": "Hpc"
        },
        "consumers": [
          {
            "component": "veh.cabin.Panel",
            "instance": "Unit",
            "machine": "Hpc",
            "crossing": "CROSSING_SAME_MACHINE",
            "encoding": "ENCODING_FLATBUFFERS",
            "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
            "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
          }
        ]
      }
    ],
    "bindings": [
      {
        "name": "websocket",
        "version": "2",
        "frameHeaderMaxBytes": 20,
        "envelopeBytes": 16
      }
    ]
  },
  "generatedMarker": "",
  "header": ""
}"#
    );
}

/// The package every podbuf layout test lowers: a struct whose head has pad
/// bytes, a union with a struct arm and a scalar arm, and a struct with an
/// optional field, an inline struct, an inline union, a string, a float and a
/// map.
const PODBUF_SOURCE: &str = "package demo.pod\n\n\
    type Name : string [0..8]\n\
    type Code : integer [0..65535]\n\n\
    struct Inner {\n  a : integer [0..255]\n  b : integer [0..4294967295]\n}\n\n\
    union Shape {\n  inner : Inner\n  code : Code\n}\n\n\
    struct Sample {\n  flag : boolean\n  big : float [0.0..1.0 step 0.5]\n  \
    name : Name\n  maybe : Code?\n  inner : Inner\n  shape : Shape\n  \
    table : [string [1..4] : Inner; 0..4]\n}\n\n\
    struct Flagged {\n  readings : [Code; 1..8]\n  \
    o1 : integer [0..255]?\n  o2 : integer [0..255]?\n  o3 : integer [0..255]?\n  \
    o4 : integer [0..255]?\n  o5 : integer [0..255]?\n  o6 : integer [0..255]?\n  \
    o7 : integer [0..255]?\n  o8 : integer [0..255]?\n  o9 : integer [0..255]?\n}\n\n\
    struct Twin1 {\n  x : integer [0..255]\n}\n\n\
    struct Twin2 {\n  x : integer [0..255]\n}\n\n\
    struct Both {\n  a : Twin1\n  b : Twin2\n}\n";

fn podbuf_model() -> v1::Model {
    let compiled = ridlc::compile("pod.typl", PODBUF_SOURCE);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{}: {}", diagnostic.code.0, diagnostic.message))
        .collect();
    assert!(
        errors.is_empty(),
        "the source must compile clean: {errors:?}"
    );
    codegen::lower(&compiled.package, &[])
}

/// The index in `Model.declarations` of the declaration called `name`.
fn declaration_index(model: &v1::Model, name: &str) -> u32 {
    model
        .declarations
        .iter()
        .position(|declaration| {
            declaration
                .name
                .as_ref()
                .is_some_and(|n| n.declared == name)
        })
        .unwrap_or_else(|| panic!("no declaration {name}")) as u32
}

/// The head of the declaration called `name`, and its index in `heads`.
fn declaration_head<'m>(model: &'m v1::Model, name: &str) -> (u32, &'m v1::PodbufHead) {
    let declaration = declaration_index(model, name);
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    projection
        .heads
        .iter()
        .enumerate()
        .find(|(_, head)| head.source == Some(v1::podbuf_head::Source::Declaration(declaration)))
        .map(|(index, head)| (index as u32, head))
        .unwrap_or_else(|| panic!("no head for {name}"))
}

fn field<'h>(head: &'h v1::PodbufHead, name: &str) -> &'h v1::PodbufField {
    head.fields
        .iter()
        .find(|field| {
            matches!(
                field.source.as_ref().and_then(|s| s.kind.as_ref()),
                Some(v1::podbuf_source::Kind::Field(f)) if f.name == name
            )
        })
        .unwrap_or_else(|| panic!("no field {name}"))
}

fn form_kind(field: &v1::PodbufField) -> &v1::podbuf_form::Kind {
    field
        .form
        .as_ref()
        .and_then(|form| form.kind.as_ref())
        .expect("a form")
}

#[test]
fn the_model_carries_the_podbuf_head_of_every_struct() {
    let model = podbuf_model();
    let (_, inner) = declaration_head(&model, "Inner");
    assert_eq!((inner.size, inner.align), (8, 8));
    assert_eq!(
        inner
            .fields
            .iter()
            .map(|f| (f.offset, f.size, f.align))
            .collect::<Vec<_>>(),
        vec![(0, 1, 1), (4, 4, 4)]
    );
    assert_eq!(
        inner
            .pads
            .iter()
            .map(|p| (p.offset, p.size))
            .collect::<Vec<_>>(),
        vec![(1, 3)]
    );
    assert_eq!(
        form_kind(field(inner, "a")),
        &v1::podbuf_form::Kind::Int(v1::PodbufInt {
            width: v1::IntWidth::U8 as i32,
            signed: false,
        })
    );

    let (_, sample) = declaration_head(&model, "Sample");
    assert_eq!(sample.size, 48);
    let offsets: Vec<_> = sample
        .fields
        .iter()
        .map(|f| (f.offset, f.size, f.align))
        .collect();
    assert_eq!(
        offsets,
        vec![
            (0, 1, 1),
            (1, 1, 1),
            (2, 2, 2),
            (4, 4, 4),
            (8, 8, 4),
            (16, 8, 8),
            (24, 16, 8),
            (40, 8, 4)
        ]
    );
    assert_eq!(
        form_kind(field(sample, "name")),
        &v1::podbuf_form::Kind::Descriptor(Box::new(v1::PodbufDescriptor {
            kind: Some(v1::podbuf_descriptor::Kind::Text(v1::PodbufBound {
                min: 0,
                max: 8
            })),
        }))
    );
}

#[test]
fn an_optional_field_names_its_flag_bit_and_the_flags_field_has_a_source() {
    let model = podbuf_model();
    let (_, sample) = declaration_head(&model, "Sample");
    assert_eq!(
        field(sample, "maybe").optional,
        Some(v1::PodbufFlagBit { flags: 0, bit: 0 })
    );
    assert!(sample.fields.iter().any(|f| f.offset == 1
        && f.source.as_ref().and_then(|s| s.kind.as_ref())
            == Some(&v1::podbuf_source::Kind::Flags(0))));
}

#[test]
fn an_inline_field_names_the_head_of_its_type() {
    let model = podbuf_model();
    let (inner_at, _) = declaration_head(&model, "Inner");
    let (shape_at, _) = declaration_head(&model, "Shape");
    let (_, sample) = declaration_head(&model, "Sample");
    assert_eq!(
        form_kind(field(sample, "inner")),
        &v1::podbuf_form::Kind::InlineHead(inner_at)
    );
    assert_eq!(
        form_kind(field(sample, "shape")),
        &v1::podbuf_form::Kind::InlineHead(shape_at)
    );
}

#[test]
fn a_union_head_carries_its_tag_and_one_head_per_arm() {
    let model = podbuf_model();
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    let shape_decl = declaration_index(&model, "Shape");
    let (_, shape) = declaration_head(&model, "Shape");
    assert_eq!(shape.size, 16);
    let kinds: Vec<_> = shape
        .fields
        .iter()
        .map(|f| {
            (
                f.offset,
                f.size,
                f.source.as_ref().and_then(|s| s.kind.clone()),
            )
        })
        .collect();
    assert_eq!(kinds[0], (0, 1, Some(v1::podbuf_source::Kind::Tag(true))));
    assert_eq!(kinds[1].0, 4);
    assert_eq!(kinds[1].1, 8);
    assert_eq!(kinds[1].2, Some(v1::podbuf_source::Kind::Arm(true)));
    let Some(v1::podbuf_form::Kind::Descriptor(descriptor)) =
        shape.fields[1].form.as_ref().and_then(|f| f.kind.clone())
    else {
        panic!("the active arm is a descriptor");
    };
    let Some(v1::podbuf_descriptor::Kind::Arms(arms)) = descriptor.kind else {
        panic!("a descriptor to the arms");
    };
    assert_eq!(arms.arms.len(), 2);
    for (position, arm) in arms.arms.iter().enumerate() {
        assert_eq!(arm.ordinal, position as u32 + 1);
        let head = &projection.heads[arm.head as usize];
        assert_eq!(
            head.source,
            Some(v1::podbuf_head::Source::Arm(v1::ArmHead {
                declaration: shape_decl,
                foreign: None,
                arm: arm.ordinal,
            }))
        );
    }
    // The scalar arm is a one-field head whose value is at offset 0.
    let scalar = &projection.heads[arms.arms[1].head as usize];
    assert_eq!(scalar.size, 8);
    assert_eq!(scalar.fields.len(), 1);
    assert_eq!(scalar.fields[0].offset, 0);
    assert_eq!(
        scalar.fields[0]
            .source
            .as_ref()
            .and_then(|s| s.kind.clone()),
        Some(v1::podbuf_source::Kind::Arm(true))
    );
}

#[test]
fn a_map_field_names_its_entry_head() {
    let model = podbuf_model();
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    let (inner_at, _) = declaration_head(&model, "Inner");
    let (_, sample) = declaration_head(&model, "Sample");
    let v1::podbuf_form::Kind::Descriptor(descriptor) = form_kind(field(sample, "table")) else {
        panic!("a descriptor");
    };
    let Some(v1::podbuf_descriptor::Kind::Map(map)) = descriptor.kind.as_ref() else {
        panic!("a map");
    };
    assert_eq!((map.elem_size, map.min, map.max), (16, 0, 4));
    let entry = &projection.heads[map.entry_head as usize];
    assert_eq!(
        entry.source,
        Some(v1::podbuf_head::Source::MapEntry(v1::MapEntry {
            path: vec!["Sample".to_string(), "table".to_string()],
        }))
    );
    assert_eq!(entry.size, 16);
    let value = &entry.fields[1];
    assert_eq!(value.offset, 8);
    assert_eq!(
        value.source.as_ref().and_then(|s| s.kind.clone()),
        Some(v1::podbuf_source::Kind::Value(true))
    );
    assert_eq!(
        value.form.as_ref().and_then(|f| f.kind.clone()),
        Some(v1::podbuf_form::Kind::InlineHead(inner_at))
    );
}

#[test]
fn every_head_index_of_the_corpus_points_inside_the_list() {
    let mut checked = 0usize;
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let model = codegen::lower(package, &others);
            let Some(projection) = model.podbuf.as_ref() else {
                continue;
            };
            let count = projection.heads.len() as u32;
            for head in &projection.heads {
                let mut forms: Vec<&v1::PodbufForm> =
                    head.fields.iter().filter_map(|f| f.form.as_ref()).collect();
                while let Some(form) = forms.pop() {
                    match form.kind.as_ref() {
                        Some(v1::podbuf_form::Kind::InlineHead(at)) => {
                            assert!(*at < count, "{entry}: an inline head is outside the list");
                            checked += 1;
                        }
                        Some(v1::podbuf_form::Kind::InlineArray(array)) => {
                            forms.extend(array.element.as_deref());
                        }
                        Some(v1::podbuf_form::Kind::Descriptor(descriptor)) => {
                            match descriptor.kind.as_ref() {
                                Some(v1::podbuf_descriptor::Kind::Array(array)) => {
                                    forms.extend(array.element.as_deref());
                                }
                                Some(v1::podbuf_descriptor::Kind::Map(map)) => {
                                    assert!(map.entry_head < count);
                                    checked += 1;
                                }
                                Some(v1::podbuf_descriptor::Kind::Arms(arms)) => {
                                    for arm in &arms.arms {
                                        assert!(arm.head < count);
                                        checked += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert!(
        checked > 0,
        "the corpus must offer head references to check"
    );
}

#[test]
fn every_corpus_package_that_lays_out_carries_its_podbuf_projection_through_json() {
    let mut with_heads = 0usize;
    for (entry, packages) in corpus_packages() {
        for package in &packages {
            let others: Vec<&v2::Package> = packages.iter().collect();
            let model = codegen::lower(package, &others);
            let json = codegen::to_json_pretty(&model).expect("the model serializes");
            let decoded = codegen::from_json(&json).expect("the model reads back");
            assert_eq!(decoded.podbuf, model.podbuf, "{entry}: {}", package.name);
            let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
            if let Some(projection) = &model.podbuf
                && !projection.heads.is_empty()
            {
                with_heads += 1;
                assert!(
                    value["podbuf"]["heads"]
                        .as_array()
                        .is_some_and(|h| !h.is_empty())
                );
            }
        }
    }
    assert!(with_heads > 0, "the corpus must offer a package with heads");
}

#[test]
fn the_veh_cluster_fault_event_has_the_head_the_law_gives() {
    let packages = corpus_packages()
        .into_iter()
        .find(|(entry, _)| entry == "veh-cluster")
        .expect("the veh-cluster corpus")
        .1;
    let package = packages
        .iter()
        .find(|package| package.name == "veh.cluster")
        .expect("the veh.cluster package");
    let others: Vec<&v2::Package> = packages.iter().collect();
    let model = codegen::lower(package, &others);
    let (_, event) = declaration_head(&model, "FaultEvent");
    assert_eq!(event.size, 24);
    assert_eq!(
        event.fields.iter().map(|f| f.offset).collect::<Vec<_>>(),
        vec![0, 4, 16]
    );
    assert_eq!(
        event
            .pads
            .iter()
            .map(|p| (p.offset, p.size))
            .collect::<Vec<_>>(),
        vec![(2, 2), (12, 4)]
    );
    let json = codegen::to_json_pretty(&model).expect("the model serializes");
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    assert!(
        value["podbuf"]["heads"]
            .as_array()
            .expect("heads")
            .iter()
            .any(|head| head["size"] == 24),
        "the canonical JSON carries the 24-byte head"
    );
}

#[test]
fn a_bounded_array_names_its_stride_and_its_bounds() {
    let model = podbuf_model();
    let (_, flagged) = declaration_head(&model, "Flagged");
    let v1::podbuf_form::Kind::Descriptor(descriptor) = form_kind(field(flagged, "readings"))
    else {
        panic!("a descriptor");
    };
    let Some(v1::podbuf_descriptor::Kind::Array(array)) = descriptor.kind.as_ref() else {
        panic!("an array");
    };
    assert_eq!((array.elem_size, array.min, array.max), (2, 1, 8));
    assert_eq!(
        array.element.as_ref().and_then(|e| e.kind.clone()),
        Some(v1::podbuf_form::Kind::Int(v1::PodbufInt {
            width: v1::IntWidth::U16 as i32,
            signed: false,
        }))
    );
}

#[test]
fn optional_fields_take_bits_in_order_and_a_ninth_takes_a_second_flags_field() {
    let model = podbuf_model();
    let (_, flagged) = declaration_head(&model, "Flagged");
    let bits: Vec<_> = (1..=9)
        .map(|n| {
            let bit = field(flagged, &format!("o{n}")).optional.expect("optional");
            (bit.flags, bit.bit)
        })
        .collect();
    assert_eq!(
        bits,
        vec![
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (0, 5),
            (0, 6),
            (0, 7),
            (1, 0)
        ]
    );
    let flags: Vec<_> = flagged
        .fields
        .iter()
        .filter_map(|f| match f.source.as_ref().and_then(|s| s.kind.as_ref()) {
            Some(v1::podbuf_source::Kind::Flags(index)) => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(flags, vec![0, 1]);
}

/// Two declarations with the same layout are two heads, and a field names the
/// head of the type it was written with.
#[test]
fn two_declarations_with_one_layout_keep_their_own_heads() {
    let model = podbuf_model();
    let (one, _) = declaration_head(&model, "Twin1");
    let (two, _) = declaration_head(&model, "Twin2");
    assert_ne!(one, two);
    let (_, both) = declaration_head(&model, "Both");
    assert_eq!(
        form_kind(field(both, "a")),
        &v1::podbuf_form::Kind::InlineHead(one)
    );
    assert_eq!(
        form_kind(field(both, "b")),
        &v1::podbuf_form::Kind::InlineHead(two)
    );
}

/// The model of package `p.b`, which imports from `p.a`, in a two-member
/// workspace.
fn two_package_model(a: &str, b: &str) -> v1::Model {
    workspace_model(&[("a", "p.a", a), ("b", "p.b", b)], "p.b")
}

/// The codegen model of the package named `local`, lowered with every member
/// of a workspace of `members` (directory, package name, source) in scope.
fn workspace_model(members: &[(&str, &str, &str)], local: &str) -> v1::Model {
    let root = tempfile::tempdir().expect("a temp dir");
    let write = |path: &str, text: &str| {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let list = members
        .iter()
        .map(|(dir, _, _)| format!("\"{dir}\""))
        .collect::<Vec<_>>()
        .join(", ");
    write("ridl.toml", &format!("[workspace]\nmembers = [{list}]\n"));
    for (dir, name, text) in members {
        write(
            &format!("{dir}/ridl.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        );
        // A member that declares an interface is a ridl source.
        let extension = if text.contains("\ninterface ") {
            "ridl"
        } else {
            "typl"
        };
        write(&format!("{dir}/{dir}.{extension}"), text);
    }
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, root.path()).expect("the workspace loads");
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    let others: Vec<&v2::Package> = packages.iter().collect();
    let local = packages
        .iter()
        .find(|p| p.name == local)
        .unwrap_or_else(|| panic!("no package {local}"));
    codegen::lower(local, &others)
}

/// A union whose arm is a union: the arm head is a copy of the inner union's
/// head, whose arms belong to the inner union, so a tuple under them is found
/// under the inner arm's type, in the declaring package and in an importing
/// one. The arm heads of the inner union name the inner union.
#[test]
fn a_union_arm_that_is_a_union_keeps_its_tuples() {
    const A: &str = "package p.a\n\n        struct S {\n  f : (x: integer [0..255], y: integer [0..255])\n}\n\n        struct T {\n  g : integer [0..255]\n}\n\n        union Inner {\n  s : S\n  t : T\n}\n\n        union Outer {\n  i : Inner\n  s : S\n}\n";
    const B: &str = "package p.b\n\nimport p.a.Outer\n\nstruct Local {\n  o : Outer\n}\n";
    let arm_heads = |model: &v1::Model, at: u32| -> Vec<(u32, u32)> {
        let projection = model.podbuf.as_ref().expect("a podbuf projection");
        projection.heads[at as usize]
            .fields
            .iter()
            .find_map(|field| match form_kind(field) {
                v1::podbuf_form::Kind::Descriptor(descriptor) => match descriptor.kind.as_ref() {
                    Some(v1::podbuf_descriptor::Kind::Arms(arms)) => Some(
                        arms.arms
                            .iter()
                            .map(|arm| (arm.ordinal, arm.head))
                            .collect(),
                    ),
                    _ => None,
                },
                _ => None,
            })
            .expect("the arms")
    };

    let declaring = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.a");
    let projection = declaring.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (outer_at, _) = declaration_head(&declaring, "Outer");
    let inner_index = declaration_index(&declaring, "Inner");
    let (_, inner_arm) = arm_heads(&declaring, outer_at)[0];
    let inner_arms = arm_heads(&declaring, inner_arm);
    let (_, s_arm) = inner_arms[0];
    assert_eq!(
        projection.heads[s_arm as usize].source,
        Some(v1::podbuf_head::Source::Arm(v1::ArmHead {
            declaration: inner_index,
            arm: 1,
            foreign: None,
        }))
    );
    let v1::podbuf_form::Kind::InlineHead(f_at) =
        form_kind(field(&projection.heads[s_arm as usize], "f"))
    else {
        panic!("an inline head");
    };
    assert!(matches!(
        projection.heads[*f_at as usize].source,
        Some(v1::podbuf_head::Source::Tuple(_))
    ));

    let importing = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.b");
    let projection = importing.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, local) = declaration_head(&importing, "Local");
    let v1::podbuf_form::Kind::InlineHead(outer_at) = form_kind(field(local, "o")) else {
        panic!("an inline head");
    };
    let (_, inner_arm) = arm_heads(&importing, *outer_at)[0];
    let (_, s_arm) = arm_heads(&importing, inner_arm)[0];
    let Some(v1::podbuf_head::Source::Arm(arm)) = &projection.heads[s_arm as usize].source else {
        panic!("an arm head");
    };
    let inner_foreign = arm.foreign.expect("a foreign union");
    assert_eq!(
        importing.foreign[inner_foreign as usize]
            .declaration
            .as_ref()
            .and_then(|d| d.name.as_ref())
            .map(|n| n.declared.as_str()),
        Some("Inner")
    );
    let v1::podbuf_form::Kind::InlineHead(f_at) =
        form_kind(field(&projection.heads[s_arm as usize], "f"))
    else {
        panic!("an inline head");
    };
    assert_eq!(
        projection.heads[*f_at as usize].source,
        Some(v1::podbuf_head::Source::NestedTuple(v1::NestedTuple {
            path: ["p.a.Outer", "1", "1", "f"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            package: "p.a".to_string(),
            wire: "SF".to_string(),
        }))
    );
}

/// Two tuples of one package can induce one wire name with two shapes (a
/// struct field `sizeField2` next to position 2 of the tuple at `size`). The
/// declaring header refuses the pair through its C name claim; an importing
/// package refuses the head that reaches them, naming both, so that the two
/// never share one head.
#[test]
fn two_tuples_of_one_package_with_one_wire_name_are_refused_when_imported() {
    let model = two_package_model(
        "package p.a\n\nstruct Hub {\n           size       : (w: integer [0..255], inner: (p: integer [0..255], q: integer [0..255]))\n           sizeField2 : (a: integer [0..255], b: integer [0..255], c: integer [0..255])\n}\n",
        "package p.b\n\nimport p.a.Hub\n\nstruct Holder {\n  hub : Hub\n}\n",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert_eq!(projection.refused.len(), 1, "{:?}", projection.refused);
    let refused = &projection.refused[0];
    assert_eq!(
        refused.source,
        Some(v1::podbuf_refused::Source::Declaration(declaration_index(
            &model, "Holder"
        )))
    );
    for text in [
        "HubSizeField2",
        "p.a.Hub.size.2",
        "p.a.Hub.sizeField2",
        "rename",
    ] {
        assert!(
            refused.cause.contains(text),
            "{text} not in {}",
            refused.cause
        );
    }
}

/// Two distinct tuples of one package under one wire name are refused in an
/// importing package whatever their layouts, equal ones included: the
/// declaring header refuses the pair by its C name claim, so the importing
/// header must not define one head for both.
#[test]
fn two_tuples_of_one_package_with_one_wire_name_and_one_layout_are_refused_when_imported() {
    let model = two_package_model(
        "package p.a\n\nstruct Hub {\n  \
         sizeField2 : (a: integer [0..255], b: integer [0..255])\n  \
         size       : (x: integer [0..255], inner: (a: integer [0..255], b: integer [0..255]))\n}\n",
        "package p.b\n\nimport p.a.Hub\n\nstruct Holder {\n  hub : Hub\n}\n",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert_eq!(projection.refused.len(), 1, "{:?}", projection.refused);
    let refused = &projection.refused[0];
    assert_eq!(
        refused.source,
        Some(v1::podbuf_refused::Source::Declaration(declaration_index(
            &model, "Holder"
        )))
    );
    for text in [
        "HubSizeField2",
        "p.a.Hub.sizeField2",
        "p.a.Hub.size.2",
        "rename",
    ] {
        assert!(
            refused.cause.contains(text),
            "{text} not in {}",
            refused.cause
        );
    }
}

/// Two positions whose induced Rust names are one (`A.bC` and `AB.c`, both
/// `ABC`) share one entry: both resolve to that entry's head in the declaring
/// package, and to one nested head in an importing package.
#[test]
fn two_positions_with_one_induced_name_share_one_head() {
    const A: &str = "package p.a\n\n\
        struct A {\n  bC : (x: integer [0..255], y: integer [0..255])\n}\n\n\
        struct AB {\n  c : (x: integer [0..255], y: integer [0..255])\n}\n";
    const B: &str = "package p.b\n\nimport p.a.A\nimport p.a.AB\n\n\
        struct Local {\n  a : A\n  ab : AB\n}\n";
    let inline_at = |field: &v1::PodbufField| match form_kind(field) {
        v1::podbuf_form::Kind::InlineHead(at) => *at,
        other => panic!("an inline head, got {other:?}"),
    };

    let declaring = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.a");
    let projection = declaring.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, a) = declaration_head(&declaring, "A");
    let (_, ab) = declaration_head(&declaring, "AB");
    let at = inline_at(field(a, "bC"));
    assert_eq!(at, inline_at(field(ab, "c")));
    assert!(matches!(
        projection.heads[at as usize].source,
        Some(v1::podbuf_head::Source::Tuple(_))
    ));

    let importing = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.b");
    let projection = importing.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, local) = declaration_head(&importing, "Local");
    let a = &projection.heads[inline_at(field(local, "a")) as usize];
    let ab = &projection.heads[inline_at(field(local, "ab")) as usize];
    let at = inline_at(field(a, "bC"));
    assert_eq!(at, inline_at(field(ab, "c")));
    let Some(v1::podbuf_head::Source::NestedTuple(nested)) = &projection.heads[at as usize].source
    else {
        panic!("a nested tuple");
    };
    assert_eq!(
        (nested.package.as_str(), nested.wire.as_str()),
        ("p.a", "ABC")
    );
}

/// The ruled pair inside one declaration (`S.aInner` and `S.a.inner`, both
/// `SAInner`): the declaring package gives them one entry under the wire name
/// `SAInner`, and the importing lowering reuses the claim the same way, so the
/// nested head of `S.a.inner` carries that wire name, not `SAField1`. The C
/// headers accept the declaration, with distinct accessors for the two
/// positions (`two_positions_of_one_declaration_that_share_one_entry_have_one_text`
/// in the podbuf backend's tests).
#[test]
fn two_positions_of_one_declaration_with_one_induced_name_share_one_wire_name() {
    const A: &str = "package p.a\n\n\
        struct S {\n  aInner : (p: integer [0..255], q: integer [0..255])\n  \
        a : (inner: (p: integer [0..255], q: integer [0..255]))\n}\n";
    const B: &str = "package p.b\n\nimport p.a.S\n\nstruct Local {\n  s : S\n}\n";
    let inline_at = |field: &v1::PodbufField| match form_kind(field) {
        v1::podbuf_form::Kind::InlineHead(at) => *at,
        other => panic!("an inline head, got {other:?}"),
    };
    let declaring = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.a");
    let wires: Vec<String> = declaring
        .tuples
        .iter()
        .filter_map(|tuple| tuple.name.as_ref().map(|name| name.wire.clone()))
        .collect();
    assert_eq!(wires, vec!["SAInner".to_string(), "SA".to_string()]);

    let importing = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.b");
    let projection = importing.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, local) = declaration_head(&importing, "Local");
    let s = &projection.heads[inline_at(field(local, "s")) as usize];
    let a_inner = inline_at(field(s, "aInner"));
    let a = &projection.heads[inline_at(field(s, "a")) as usize];
    let inner = a
        .fields
        .iter()
        .find(|field| {
            matches!(
                field.source.as_ref().and_then(|s| s.kind.as_ref()),
                Some(v1::podbuf_source::Kind::Position(1))
            )
        })
        .map(inline_at)
        .expect("position 1");
    assert_eq!(a_inner, inner, "one entry, one head");
    let Some(v1::podbuf_head::Source::NestedTuple(nested)) =
        &projection.heads[inner as usize].source
    else {
        panic!("a nested tuple");
    };
    assert_eq!(
        (nested.package.as_str(), nested.wire.as_str()),
        ("p.a", "SAInner")
    );
}

/// A collision inside a declaration of another package is recorded by the
/// package that declares it, not by a package that imports it: `A.bC` and
/// `AB.c` both induce `ABC` with two shapes, so `p.a` carries one
/// `TupleCollision` and `p.b`, which reaches both through `Local`, carries
/// none. The two positions keep two entries in `p.b`, so its podbuf walk
/// meets both under one wire name and refuses `Local`, naming both paths.
#[test]
fn a_collision_inside_a_foreign_declaration_is_not_recorded_by_the_importer() {
    const A: &str = "package p.a\n\n\
        struct A {\n  bC : (x: integer [0..255], y: integer [0..255])\n}\n\n\
        struct AB {\n  c : (x: integer [0..255], y: integer [0..255], z: integer [0..255])\n}\n";
    const B: &str = "package p.b\n\nimport p.a.A\nimport p.a.AB\n\n\
        struct Local {\n  a : A\n  ab : AB\n}\n";
    let declaring = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.a");
    assert_eq!(
        declaring
            .tuple_collisions
            .iter()
            .map(|collision| collision.name.as_str())
            .collect::<Vec<_>>(),
        vec!["ABC"]
    );
    let importing = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.b");
    assert!(
        importing.tuple_collisions.is_empty(),
        "{:?}",
        importing.tuple_collisions
    );
    let projection = importing.podbuf.as_ref().expect("a podbuf projection");
    assert_eq!(projection.refused.len(), 1, "{:?}", projection.refused);
    let refused = &projection.refused[0];
    assert_eq!(
        refused.source,
        Some(v1::podbuf_refused::Source::Declaration(declaration_index(
            &importing, "Local"
        )))
    );
    for text in ["ABC", "p.a.A.bC", "p.a.AB.c"] {
        assert!(
            refused.cause.contains(text),
            "{text} not in {}",
            refused.cause
        );
    }
}

/// A tuple nested inside the induced reply tuple of a query is reached under
/// that tuple's own root: the interaction position has no walk path, and the
/// induced tuple re-roots its fields, so the reply, its nested `inner` and the
/// request each have a head and none is refused.
#[test]
fn a_tuple_inside_an_interaction_tuple_has_its_own_head() {
    const A: &str = "package p.a\n\ntype Pt : integer [0..255]\n\n\
        interface Probe {\n  query get(a: Pt, b: Pt): (x: Pt, inner: (p: Pt, q: Pt)) @[..50ms]\n}\n";
    let model = workspace_model(&[("a", "p.a", A)], "p.a");
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    assert_eq!(projection.heads.len(), 3, "{:?}", projection.heads);
    let reply = projection
        .heads
        .iter()
        .find(|head| {
            head.fields.iter().any(|field| {
                matches!(
                    field.source.as_ref().and_then(|s| s.kind.as_ref()),
                    Some(v1::podbuf_source::Kind::Position(2))
                ) && matches!(form_kind(field), v1::podbuf_form::Kind::InlineHead(_))
            })
        })
        .expect("the reply head lays out `inner` inline");
    let inner = reply
        .fields
        .iter()
        .find_map(|field| match form_kind(field) {
            v1::podbuf_form::Kind::InlineHead(at) => Some(*at),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        projection.heads[inner as usize].source,
        Some(v1::podbuf_head::Source::Tuple(_))
    ));
}

/// A colliding position does not take the claim of the entry that holds it:
/// `A.bC` claims `ABC`, `AB.c` collides with it, and `A.b.c`, reached after
/// both, has the shape of `A.bC` and reuses its claim, so one collision is
/// recorded, not two.
#[test]
fn a_colliding_position_leaves_the_first_claim_in_place() {
    const A: &str = "package p.a\n\n\
        struct A {\n  bC : (x: integer [0..255], y: integer [0..255])\n  \
        b : (c: (x: integer [0..255], y: integer [0..255]))\n}\n\n\
        struct AB {\n  c : (x: integer [0..255], y: integer [0..255], z: integer [0..255])\n}\n";
    let model = workspace_model(&[("a", "p.a", A)], "p.a");
    assert_eq!(
        model
            .tuple_collisions
            .iter()
            .map(|collision| (
                collision.first_fields.clone(),
                collision.second_fields.clone()
            ))
            .collect::<Vec<_>>(),
        vec![(
            vec!["x".to_string(), "y".to_string()],
            vec!["x".to_string(), "y".to_string(), "z".to_string()]
        )]
    );
}

/// A tuple as the key and as the value of a map is found under the entry
/// head's `key` and `value` positions, in the declaring package and in an
/// importing one.
#[test]
fn a_tuple_as_a_map_key_or_value_is_found_by_its_identity() {
    const A: &str = "package p.a\n\n\
        struct Table {\n  byPair : [(k1: integer [0..255], k2: integer [0..255]) : \
        (v1: integer [0..255], v2: integer [0..255]); 0..4]\n}\n";
    const B: &str = "package p.b\n\nimport p.a.Table\n\nstruct Local {\n  table : Table\n}\n";
    let entry_of = |projection: &v1::PodbufProjection, table: &v1::PodbufHead| -> u32 {
        match form_kind(field(table, "byPair")) {
            v1::podbuf_form::Kind::Descriptor(descriptor) => match descriptor.kind.as_ref() {
                Some(v1::podbuf_descriptor::Kind::Map(map)) => {
                    assert!(matches!(
                        projection.heads[map.entry_head as usize].source,
                        Some(v1::podbuf_head::Source::MapEntry(_))
                    ));
                    map.entry_head
                }
                other => panic!("a map, got {other:?}"),
            },
            other => panic!("a descriptor, got {other:?}"),
        }
    };
    let position = |head: &v1::PodbufHead, key: bool| -> u32 {
        let field = head
            .fields
            .iter()
            .find(
                |field| match field.source.as_ref().and_then(|s| s.kind.as_ref()) {
                    Some(v1::podbuf_source::Kind::Key(_)) => key,
                    Some(v1::podbuf_source::Kind::Value(_)) => !key,
                    _ => false,
                },
            )
            .expect("the position");
        match form_kind(field) {
            v1::podbuf_form::Kind::InlineHead(at) => *at,
            other => panic!("an inline head, got {other:?}"),
        }
    };

    let declaring = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.a");
    let projection = declaring.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, table) = declaration_head(&declaring, "Table");
    let entry = &projection.heads[entry_of(projection, table) as usize];
    let tuple_wire = |at: u32| -> String {
        match projection.heads[at as usize].source {
            Some(v1::podbuf_head::Source::Tuple(index)) => declaring.tuples[index as usize]
                .name
                .as_ref()
                .map(|name| name.wire.clone())
                .unwrap_or_default(),
            ref other => panic!("a tuple root, got {other:?}"),
        }
    };
    assert_eq!(tuple_wire(position(entry, true)), "TableByPairKey");
    assert_eq!(tuple_wire(position(entry, false)), "TableByPairValue");

    let importing = workspace_model(&[("a", "p.a", A), ("b", "p.b", B)], "p.b");
    let projection = importing.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, local) = declaration_head(&importing, "Local");
    let v1::podbuf_form::Kind::InlineHead(table_at) = form_kind(field(local, "table")) else {
        panic!("an inline head");
    };
    let entry =
        &projection.heads[entry_of(projection, &projection.heads[*table_at as usize]) as usize];
    let nested = |at: u32| match &projection.heads[at as usize].source {
        Some(v1::podbuf_head::Source::NestedTuple(nested)) => {
            (nested.package.clone(), nested.wire.clone())
        }
        other => panic!("a nested tuple, got {other:?}"),
    };
    assert_eq!(
        nested(position(entry, true)),
        ("p.a".to_string(), "TableByPairKey".to_string())
    );
    assert_eq!(
        nested(position(entry, false)),
        ("p.a".to_string(), "TableByPairValue".to_string())
    );
}

/// The nested key is the home package and the wire name: two packages whose
/// tuples induce one wire name have two heads.
#[test]
fn tuples_of_two_packages_with_one_wire_name_have_two_heads() {
    let model = workspace_model(
        &[
            (
                "a",
                "p.a",
                "package p.a\n\nstruct Pair {\n  a : (x: integer [0..255], y: integer [0..255])\n}\n",
            ),
            (
                "c",
                "p.c",
                "package p.c\n\nstruct Pair {\n  a : (x: integer [0..255], y: integer [0..255])\n}\n\nstruct Boxed {\n  pair : Pair\n}\n",
            ),
            (
                "b",
                "p.b",
                "package p.b\n\nimport p.a.Pair\nimport p.c.Boxed\n\nstruct Local {\n  pair : Pair\n  boxed : Boxed\n}\n",
            ),
        ],
        "p.b",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let inline_at = |field: &v1::PodbufField| match form_kind(field) {
        v1::podbuf_form::Kind::InlineHead(at) => *at,
        other => panic!("an inline head, got {other:?}"),
    };
    let nested = |at: u32| match &projection.heads[at as usize].source {
        Some(v1::podbuf_head::Source::NestedTuple(nested)) => {
            (nested.package.clone(), nested.wire.clone())
        }
        other => panic!("a nested tuple, got {other:?}"),
    };
    let (_, local) = declaration_head(&model, "Local");
    let a_pair = &projection.heads[inline_at(field(local, "pair")) as usize];
    let boxed = &projection.heads[inline_at(field(local, "boxed")) as usize];
    let c_pair = &projection.heads[inline_at(field(boxed, "pair")) as usize];
    let a_at = inline_at(field(a_pair, "a"));
    let c_at = inline_at(field(c_pair, "a"));
    assert_ne!(a_at, c_at, "one head for the tuples of two packages");
    assert_eq!(nested(a_at), ("p.a".to_string(), "PairA".to_string()));
    assert_eq!(nested(c_at), ("p.c".to_string(), "PairA".to_string()));
}

/// A tuple is found by its own identity, not by its shape: two same-shape
/// tuples of one package have two heads under two wire names; a tuple inside
/// a struct of another package is one `NestedTuple` head, keyed by that
/// package and the wire name it gives the tuple, wherever the walk reaches it
/// (at a field, at a tuple position, as an array element, or inside an arm of
/// a union of that package); and a same-shape tuple of this package keeps its
/// own head.
#[test]
fn an_inline_tuple_is_found_by_its_identity() {
    let model = two_package_model(
        "package p.a\n\n\
         struct Pair {\n  a : (x: integer [0..255], y: integer [0..255])\n  \
         b : (x: integer [0..255], y: integer [0..255])\n}\n\n\
         struct Hub {\n  size : (w: integer [0..255], inner: (p: integer [0..255], q: integer [0..255]))\n  \
         items : [(l: integer [0..255], r: integer [0..255]); 0..4]\n}\n\n\
         union Part {\n  pair : Pair\n  hub : Hub\n}\n",
        "package p.b\n\nimport p.a.Pair\nimport p.a.Part\n\n\
         struct Local {\n  own : (x: integer [0..255], y: integer [0..255])\n  pair : Pair\n  part : Part\n}\n",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let nested = |at: u32| match &projection.heads[at as usize].source {
        Some(v1::podbuf_head::Source::NestedTuple(nested)) => {
            (nested.package.as_str(), nested.wire.as_str())
        }
        other => panic!("a nested tuple, got {other:?}"),
    };
    let inline_at = |field: &v1::PodbufField| match form_kind(field) {
        v1::podbuf_form::Kind::InlineHead(at) => *at,
        other => panic!("an inline head, got {other:?}"),
    };
    let (_, local) = declaration_head(&model, "Local");

    // The local tuple has a root head of its own, which no foreign head shares.
    let own_at = inline_at(field(local, "own"));
    assert!(matches!(
        projection.heads[own_at as usize].source,
        Some(v1::podbuf_head::Source::Tuple(_))
    ));

    // The two members of `Pair` have two heads, under two wire names.
    let pair = &projection.heads[inline_at(field(local, "pair")) as usize];
    let a_at = inline_at(field(pair, "a"));
    let b_at = inline_at(field(pair, "b"));
    assert_ne!(a_at, b_at);
    assert_ne!(own_at, a_at);
    assert_eq!(nested(a_at), ("p.a", "PairA"));
    assert_eq!(nested(b_at), ("p.a", "PairB"));

    // Inside an arm of the foreign union, the same tuples name the same heads,
    // and the tuples of `Hub` are keyed by the names `p.a` gives them.
    let part = &projection.heads[inline_at(field(local, "part")) as usize];
    let arms = part
        .fields
        .iter()
        .find_map(|field| match form_kind(field) {
            v1::podbuf_form::Kind::Descriptor(descriptor) => match descriptor.kind.as_ref() {
                Some(v1::podbuf_descriptor::Kind::Arms(arms)) => Some(arms),
                _ => None,
            },
            _ => None,
        })
        .expect("the arms");
    let arm_pair = &projection.heads[arms.arms[0].head as usize];
    assert_eq!(inline_at(field(arm_pair, "a")), a_at);
    assert_eq!(inline_at(field(arm_pair, "b")), b_at);
    let arm_hub = &projection.heads[arms.arms[1].head as usize];
    let size_at = inline_at(field(arm_hub, "size"));
    assert_eq!(nested(size_at), ("p.a", "HubSize"));
    let inner = projection.heads[size_at as usize]
        .fields
        .iter()
        .find(|field| {
            matches!(
                field.source.as_ref().and_then(|s| s.kind.as_ref()),
                Some(v1::podbuf_source::Kind::Position(2))
            )
        })
        .expect("position 2");
    assert_eq!(nested(inline_at(inner)), ("p.a", "HubSizeField2"));
    let element = match form_kind(field(arm_hub, "items")) {
        v1::podbuf_form::Kind::Descriptor(descriptor) => match descriptor.kind.as_ref() {
            Some(v1::podbuf_descriptor::Kind::Array(array)) => {
                array.element.as_deref().expect("an element")
            }
            other => panic!("an array, got {other:?}"),
        },
        other => panic!("a descriptor, got {other:?}"),
    };
    let Some(v1::podbuf_form::Kind::InlineHead(element_at)) = element.kind.as_ref() else {
        panic!("an inline element, got {element:?}");
    };
    assert_eq!(nested(*element_at), ("p.a", "HubItemsElement"));
}

/// A struct of another package, written inline, has a head of its own that
/// points into `Model.foreign`.
#[test]
fn a_foreign_struct_written_inline_gets_a_foreign_head() {
    let model = two_package_model(
        "package p.a\n\nstruct Remote {\n  x : integer [0..255]\n  y : integer [0..65535]\n}\n",
        "package p.b\n\nimport p.a.Remote\n\nstruct Local {\n  r : Remote\n}\n",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, head) = declaration_head(&model, "Local");
    let v1::podbuf_form::Kind::InlineHead(at) = form_kind(field(head, "r")) else {
        panic!("an inline head");
    };
    let remote = &projection.heads[*at as usize];
    let Some(v1::podbuf_head::Source::Foreign(index)) = remote.source else {
        panic!("a foreign source, got {:?}", remote.source);
    };
    let foreign = &model.foreign[index as usize];
    assert_eq!(foreign.package, "p.a");
    assert_eq!(
        foreign
            .declaration
            .as_ref()
            .and_then(|d| d.name.as_ref())
            .map(|n| n.declared.as_str()),
        Some("Remote")
    );
    assert_eq!(remote.size, 8);
    assert_eq!(
        remote
            .fields
            .iter()
            .map(|f| (f.offset, f.size))
            .collect::<Vec<_>>(),
        vec![(0, 1), (2, 2)]
    );
}

/// A union of another package, written inline, has its arms indexed through
/// `Model.foreign`, and a tuple inside a struct of another package has a head
/// of its own.
#[test]
fn a_foreign_union_and_a_foreign_tuple_keep_their_heads() {
    let model = two_package_model(
        "package p.a\n\ntype Code : integer [0..65535]\n\nstruct Leaf {\n  x : integer [0..255]\n}\n\n\
         union Pick {\n  leaf : Leaf\n  code : Code\n}\n\n\
         struct Holder {\n  t : (a: integer [0..255], b: integer [0..65535])\n  inner : Leaf\n}\n",
        "package p.b\n\nimport p.a.Pick\nimport p.a.Holder\n\nstruct Local {\n  p : Pick\n  h : Holder\n}\n",
    );
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let (_, local) = declaration_head(&model, "Local");
    let foreign_name = |source: &Option<v1::podbuf_head::Source>| match source {
        Some(v1::podbuf_head::Source::Foreign(index)) => model.foreign[*index as usize]
            .declaration
            .as_ref()
            .and_then(|d| d.name.as_ref())
            .map(|n| n.declared.clone()),
        _ => None,
    };
    let v1::podbuf_form::Kind::InlineHead(pick_at) = form_kind(field(local, "p")) else {
        panic!("an inline head");
    };
    let pick = &projection.heads[*pick_at as usize];
    assert_eq!(foreign_name(&pick.source).as_deref(), Some("Pick"));
    let v1::podbuf_form::Kind::Descriptor(descriptor) = pick.fields[1]
        .form
        .as_ref()
        .and_then(|f| f.kind.as_ref())
        .expect("a form")
    else {
        panic!("the active arm is a descriptor");
    };
    let Some(v1::podbuf_descriptor::Kind::Arms(arms)) = descriptor.kind.as_ref() else {
        panic!("arms");
    };
    let pick_foreign = match &pick.source {
        Some(v1::podbuf_head::Source::Foreign(index)) => *index,
        _ => unreachable!(),
    };
    for arm in &arms.arms {
        let head = &projection.heads[arm.head as usize];
        assert_eq!(
            head.source,
            Some(v1::podbuf_head::Source::Arm(v1::ArmHead {
                declaration: 0,
                arm: arm.ordinal,
                foreign: Some(pick_foreign),
            }))
        );
    }

    let v1::podbuf_form::Kind::InlineHead(holder_at) = form_kind(field(local, "h")) else {
        panic!("an inline head");
    };
    let holder = &projection.heads[*holder_at as usize];
    assert_eq!(foreign_name(&holder.source).as_deref(), Some("Holder"));
    let v1::podbuf_form::Kind::InlineHead(tuple_at) = form_kind(field(holder, "t")) else {
        panic!("an inline head");
    };
    let tuple = &projection.heads[*tuple_at as usize];
    assert_eq!(
        tuple.source,
        Some(v1::podbuf_head::Source::NestedTuple(v1::NestedTuple {
            path: vec!["p.a.Holder".to_string(), "t".to_string()],
            package: "p.a".to_string(),
            wire: "HolderT".to_string(),
        }))
    );
    assert_eq!(tuple.size, 8);
    assert_eq!(tuple.fields.len(), 2);
    // The foreign nested tuple carries the typl name of each member next to
    // its position: a header that defines the head again has a name to read.
    let members: Vec<(Option<&v1::podbuf_source::Kind>, &str)> = tuple
        .fields
        .iter()
        .map(|field| {
            let source = field.source.as_ref().expect("a source");
            (source.kind.as_ref(), source.position_name.as_str())
        })
        .collect();
    assert_eq!(
        members,
        vec![
            (Some(&v1::podbuf_source::Kind::Position(1)), "a"),
            (Some(&v1::podbuf_source::Kind::Position(2)), "b"),
        ]
    );
}
