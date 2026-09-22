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
        }
    });
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
                .find(|root| root.declaration == reference.index)
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

    // The catalog's placeholder hash (the frame specification §6.1).
    let catalog = model.catalog.as_ref().expect("a catalog");
    assert_eq!(catalog.hash.len(), 32, "the hash is 32 bytes");
    assert!(
        catalog.hash.iter().all(|byte| *byte == 0),
        "the hash is the placeholder until E16.2 (driftsys/ridl#378)"
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
