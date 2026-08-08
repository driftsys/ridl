# proto3 Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit valid proto3 schemas from IR identity — the typl surface and the
interaction identity table — as roadmap story E9.8.

**Architecture:** A new crate `ridl-backend-proto` walks a `v2::Package` and
writes proto3 text directly into a `String`, the way `c_header.rs` writes the
extern-C header. Validity is established by compiling every emitted schema with
`protox` inside the test suite. Numbering is the identity mapping already fixed
by typl Appendix D and the schema-projection note: a struct field takes its typl
§7.4 ordinal, an interaction takes its ridl §11 ordinal.

**Tech Stack:** Rust 1.95.0, `ridl-ir` (IR v2 types and the pinned name
transform), `protox` (pure-Rust protobuf compiler, test-only here), `insta`
(golden snapshots), `proptest` (the stability property).

**Design of record:**
[`2026-08-08-proto3-projection-design.md`](2026-08-08-proto3-projection-design.md),
which ratifies against
[`2026-08-03-schema-projection-design.md`](../wip/2026-08-03-schema-projection-design.md)
("the note").

## Global Constraints

- **Never push to `main`.** All work lands on `e98-proto3-projection` through a
  PR. Squash merge.
- **The toolchain is pinned.** `rust-toolchain.toml` fixes rustc 1.95.0; run
  `just toolchain-check` if anything looks version-related (ADR-0009).
- **`just verify` before any PR** — `lint-commits` then the full `build` gate:
  toolchain-check, gate-parity, fmt-check, book-check, link-check, compile,
  test, lint, wasm-check, check.
- **Clippy runs with `-D warnings`.** A warning fails the gate.
- **`wasm-check` must pass**, so `ridl-backend-proto` builds under
  `--no-default-features` for `wasm32-unknown-unknown`. Keep `protox` a
  dev-dependency; it must never become a normal dependency.
- **Conventional Commits**, linted by git-std against `.git-std.toml`. A new
  crate adds its own scope there — the list is explicit, not path-derived.
- **Prose is plain and literal** in comments, commit messages and docs: no
  idioms, no figures of speech. Technical terms and acronyms stay as they are.
- **No wildcard arms over `Emit`.** `ridlc`'s `Emit` matches are deliberately
  exhaustive so the compiler names every site a new variant must reach.
- **Every emitted identifier goes through `ridl_ir::name::snake_case`** where
  the target namespace is snake_case (ADR-0016 decisions 1 and 2). Never write a
  second transform.

---

### Task 1: Crate skeleton, `Emit::Proto`, and the protox harness

Creates the crate, wires the emit end to end, and establishes the validity check
that every later task reuses. The deliverable is a package that emits a file
header and nothing else — which is already valid proto3, and proves the harness
works before there is any mapping to get wrong.

**Files:**

- Create: `crates/ridl-backend-proto/Cargo.toml`
- Create: `crates/ridl-backend-proto/src/lib.rs`
- Create: `crates/ridl-backend-proto/src/tests.rs`
- Modify: `crates/ridlc/src/lib.rs` — the `Emit` enum near line 270, its
  `ir_dump_suffix` match near line 352, and the emit dispatch near line 721
- Modify: `crates/ridlc/Cargo.toml` — add the dependency
- Modify: `Cargo.toml` (workspace members)
- Modify: `.git-std.toml` — add the `ridl-backend-proto` scope

**Interfaces:**

- Consumes: `ridl_ir::v2::Package`.
- Produces:
  `ridl_backend_proto::generate(&v2::Package) -> Result<Generated,
  GenerateError>`
  where `Generated { pub proto_source: String }` and
  `GenerateError { pub message: String }`. Every later task adds to `generate`.
  Also `ridl_backend_proto::tests::compile_with_protox(&str)`, the shared
  validity assertion.

- [ ] **Step 1: Write the failing test**

In `crates/ridl-backend-proto/src/tests.rs`:

```rust
use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as proto3 with protox, panicking with the compiler's own
/// message on failure. This is the story's acceptance check: every test that
/// emits a schema runs it through here.
pub(crate) fn compile_with_protox(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if let Err(error) = protox::compile([file_name], [dir.path()]) {
        panic!("emitted schema is not valid proto3:\n{error}\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn an_empty_package_emits_a_valid_file_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(
        generated.proto_source,
        "syntax = \"proto3\";\n\npackage veh.common;\n"
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL — the crate does
not exist yet, so the build fails.

- [ ] **Step 3: Create the manifest**

`crates/ridl-backend-proto/Cargo.toml`:

```toml
[package]
name = "ridl-backend-proto"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
ridl-ir = { path = "../ridl-ir" }

# protox is TEST-ONLY and must stay here. It is the validity check of
# ADR-0016's totality property, not part of emission: making it a normal
# dependency would put a protobuf compiler in every build and break
# `just wasm-check`.
[dev-dependencies]
insta.workspace = true
protox.workspace = true
tempfile = "3.15.0"
```

Add `protox` to the workspace `[workspace.dependencies]` in the root
`Cargo.toml` if it is not already there (it is currently a build-dependency of
`ridl-ir`), and add `crates/ridl-backend-proto` to `[workspace] members`.

- [ ] **Step 4: Write the minimal implementation**

`crates/ridl-backend-proto/src/lib.rs`:

```rust
//! IR v2 package to a proto3 schema (roadmap story E9.8, ADR-0013 decision 2).
//!
//! A **wire backend** in the sense ADR-0013 decision 1 gives the term: the
//! target describes bytes in transit, so the emit ceiling is two tiers — the
//! typl surface, and the interaction identity table — and nothing above them.
//! No `service` block, no call face, no value store.
//!
//! Text is written directly rather than through a `FileDescriptorProto`,
//! matching `c_header.rs`. The constraint information typl carries and proto3
//! cannot represent — units, ranges, steps — is emitted as comments, which a
//! descriptor would have addressed by index path through `SourceCodeInfo`.

use ridl_ir::v2;

#[cfg(test)]
mod tests;

/// The generated proto3 schema for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub proto_source: String,
}

/// A failure to generate a schema from a package.
///
/// Carried as a value so codegen stays total: no stage in the pipeline panics
/// (ADR-0004 section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the proto3 schema for `package`.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    let mut out = String::new();
    out.push_str("syntax = \"proto3\";\n\n");
    out.push_str(&format!("package {};\n", package.name));
    Ok(Generated { proto_source: out })
}
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS.

- [ ] **Step 6: Wire `Emit::Proto` into `ridlc`**

In `crates/ridlc/src/lib.rs`, add the variant to `Emit` (near line 270):

```rust
/// The proto3 schema, written to `<base>.proto`.
///
/// A wire backend: the typl surface plus the interaction identity table,
/// and nothing above them (ADR-0013 decision 2).
Proto,
```

Add it to the `ir_dump_suffix` match (near line 352) — it is a code emit, not an
IR dump, so it joins the `None` arm:

```rust
Emit::Rust | Emit::CHeader | Emit::TypeScript | Emit::Proto => None,
```

Add the dispatch arm beside the others (near line 721):

```rust
Emit::Proto => {
    match ridl_backend_proto::generate(ir) {
        Ok(generated) => {
            artifacts.push((format!("{base}.proto"), generated.proto_source));
        }
        Err(error) => diagnostics.push(error.message),
    }
}
```

Match the surrounding arms' exact error-collection style — read the `Emit::Rust`
and `Emit::TypeScript` arms and follow them rather than the sketch above, which
shows placement, not the local convention.

Add `ridl-backend-proto = { path = "../ridl-backend-proto" }` to
`crates/ridlc/Cargo.toml`.

- [ ] **Step 7: Add the git-std scope**

In `.git-std.toml`, add `ridl-backend-proto` to the enumerated scope list,
keeping the existing ordering convention of that list.

- [ ] **Step 8: Verify the whole workspace builds and the new emit runs**

Run: `cargo test --workspace --locked` Expected: PASS.

Run:
`cargo run -p ridl -- build crates/ridl/tests/baseline-corpus/cluster.ridl --emit proto`
Expected: writes `cluster.proto` containing the header. Inspect it by eye once.

- [ ] **Step 9: Commit**

```bash
git add crates/ridl-backend-proto crates/ridlc/src/lib.rs crates/ridlc/Cargo.toml Cargo.toml Cargo.lock .git-std.toml
git commit -m "feat(ridl-backend-proto): add the crate and wire the proto emit

The first wire backend in ADR-0013 decision 1's sense. This commit
establishes the crate, the Emit::Proto value and the protox validity
check; the mapping arrives in the commits that follow.

protox is a dev-dependency and must stay one: it is the test-time
validity check, not part of emission, and making it a normal dependency
would put a protobuf compiler in every build and break wasm-check."
```

---

### Task 2: Tier 2 — the interaction identity table

Emits one enum per interface shape. This is the tier that gives the baseline
corpus real output, since that corpus declares interfaces and named scalars but
no composites.

**Files:**

- Modify: `crates/ridl-backend-proto/src/lib.rs`
- Modify: `crates/ridl-backend-proto/src/tests.rs`

**Interfaces:**

- Consumes: `generate` from Task 1; `v2::Package::shapes()`, which yields
  `InterfaceShape { name, interface, service }` and already covers named
  interfaces and a service's inline shape uniformly — for an inline shape `name`
  is the dotted service address, because `Interface.name` is `""` there.
- Produces: `fn type_name(dotted: &str) -> String` and
  `fn screaming_snake_case(name: &str) -> String`, both used by Task 4.

- [ ] **Step 1: Write the failing tests**

Append to `crates/ridl-backend-proto/src/tests.rs`:

```rust
#[test]
fn an_interface_emits_its_ordinal_table() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "VehicleStatus".to_string(),
            interactions: vec![
                signal_decl("currentSpeed", 1),
                signal_decl("doorOpened", 2),
                reserved_decl(3),
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains(
        "enum VehicleStatusOrdinal {\n  \
         VEHICLE_STATUS_ORDINAL_UNSPECIFIED = 0;\n  \
         VEHICLE_STATUS_ORDINAL_CURRENT_SPEED = 1;\n  \
         VEHICLE_STATUS_ORDINAL_DOOR_OPENED = 2;\n  \
         reserved 3;\n  \
         VEHICLE_STATUS_ORDINAL_TYRE_PRESSURE = 4;\n}"
    ), "got:\n{}", generated.proto_source);

    compile_with_protox("veh.cluster.proto", &generated.proto_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
    // Interface.name is "" for an inline shape (ridl §14.5), so the enum takes
    // the service's dotted address instead.
    let package = v2::Package {
        name: "corpus.baseline".to_string(),
        services: vec![v2::Service {
            name: "corpus.baseline.hvac".to_string(),
            shapes: vec![v2::ServiceShape {
                id: 1,
                kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                    name: String::new(),
                    interactions: vec![signal_decl("cabinTemp", 1)],
                    ..Default::default()
                })),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains("enum CorpusBaselineHvacOrdinal {"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains(
        "CORPUS_BASELINE_HVAC_ORDINAL_CABIN_TEMP = 1;"
    ), "got:\n{}", generated.proto_source);

    compile_with_protox("corpus.baseline.proto", &generated.proto_source);
}

#[test]
fn an_ordinal_in_the_protobuf_reserved_span_is_refused() {
    // Field numbers 19,000 to 19,999 belong to protobuf itself (note §4.2).
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 19_000)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("19000"), "got: {}", error.message);
    assert!(error.message.contains("reserved by protobuf"), "got: {}", error.message);
}

#[test]
fn an_ordinal_above_the_proto_ceiling_is_refused() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 536_870_912)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("536870911"), "got: {}", error.message);
}

/// A signal interaction at `ordinal`. The kind is immaterial to tier 2: the
/// table is interface-wide and kind-blind (ridl §11, ADR-0013 decision 3).
fn signal_decl(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        ordinal,
        kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef::default())),
        ..Default::default()
    }
}

fn reserved_decl(ordinal: u32) -> v2::Decl {
    v2::Decl {
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            ..Default::default()
        })),
        ..Default::default()
    }
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL — no enum is
emitted, so the `contains` assertions fail and the two refusal tests fail on
`expect_err`.

- [ ] **Step 3: Implement the identity table**

In `crates/ridl-backend-proto/src/lib.rs`:

```rust
/// proto reserves field numbers 19,000 through 19,999 for its own use.
const PROTO_RESERVED: std::ops::RangeInclusive<u32> = 19_000..=19_999;
/// The largest field number proto admits.
const PROTO_MAX_FIELD_NUMBER: u32 = 536_870_911;

/// A dotted address becomes one CamelCase proto identifier:
/// `corpus.baseline.hvac` gives `CorpusBaselineHvac`. A named interface has no
/// dots, so its name passes through unchanged.
fn type_name(dotted: &str) -> String {
    dotted
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// SCREAMING_SNAKE of a ridl name, built on the pinned transform so there is
/// exactly one case algorithm in the toolchain (ADR-0016 decision 2).
fn screaming_snake_case(name: &str) -> String {
    ridl_ir::name::snake_case(name).to_uppercase()
}

/// Rejects an ordinal proto cannot carry, making ADR-0016 decision 6's
/// totality property true as stated rather than true by luck. Neither case is
/// reachable in practice: it would take one interface accumulating nineteen
/// thousand interactions and tombstones (note §4.2).
fn check_field_number(owner: &str, name: &str, number: u32) -> Result<(), GenerateError> {
    if PROTO_RESERVED.contains(&number) {
        return Err(GenerateError {
            message: format!(
                "{owner}.{name} takes field number {number}, which is reserved by protobuf \
                 itself (19000 to 19999). Renumber the declaration."
            ),
        });
    }
    if number > PROTO_MAX_FIELD_NUMBER {
        return Err(GenerateError {
            message: format!(
                "{owner}.{name} takes field number {number}, above proto's largest field \
                 number {PROTO_MAX_FIELD_NUMBER}."
            ),
        });
    }
    Ok(())
}

/// Tier 2 (ADR-0013 decision 3): one enum per interface shape, interface-wide
/// and kind-blind, matching ridl §11's single ordinal sequence. Retired
/// ordinals are held against reuse with `reserved`, and an `UNSPECIFIED = 0`
/// member leads because ridl ordinals are 1-based.
fn emit_identity_tables(out: &mut String, package: &v2::Package) -> Result<(), GenerateError> {
    for shape in package.shapes() {
        let enum_name = format!("{}Ordinal", type_name(shape.name));
        let prefix = screaming_snake_case(&enum_name);

        out.push_str(&format!("\nenum {enum_name} {{\n"));
        out.push_str(&format!("  {prefix}_UNSPECIFIED = 0;\n"));

        for decl in &shape.interface.interactions {
            match &decl.kind {
                Some(v2::decl::Kind::ReservedSlot(reserved)) => {
                    out.push_str(&format!("  reserved {};\n", reserved.ordinal));
                }
                _ => {
                    check_field_number(shape.name, &decl.name, decl.ordinal)?;
                    out.push_str(&format!(
                        "  {prefix}_{} = {};\n",
                        screaming_snake_case(&decl.name),
                        decl.ordinal
                    ));
                }
            }
        }
        out.push_str("}\n");
    }
    Ok(())
}
```

Call it from `generate`, after the header:

```rust
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    let mut out = String::new();
    out.push_str("syntax = \"proto3\";\n\n");
    out.push_str(&format!("package {};\n", package.name));
    emit_identity_tables(&mut out, package)?;
    Ok(Generated { proto_source: out })
}
```

Note the `an_empty_package_emits_a_valid_file_header` test from Task 1 still
passes: a package with no shapes appends nothing.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS, all five tests.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "feat(ridl-backend-proto): emit the interaction identity table

Tier 2 of ADR-0013 decision 2: one enum per interface shape, keyed by
the ridl section 11 ordinal. The table is interface-wide and kind-blind,
matching that section's single sequence across all five interaction
kinds, and a retired ordinal emits proto reserved so the slot is held
against reuse.

An inline service shape carries Interface.name == \"\", so its enum is
named from the service's dotted address instead. Package::shapes()
already walks named and inline shapes uniformly.

An ordinal proto cannot carry - inside 19000 to 19999, or above
536870911 - fails with a diagnostic rather than emitting a schema protoc
rejects. Neither is reachable in practice; the check makes ADR-0016
decision 6's totality property true as stated."
```

---

### Task 3: Tier 1 — named scalars, struct messages, and the RIDL-149 extension

The one task that changes the language surface. ADR-0016 decision 4 binds the
transform and the check to **the commit that starts projecting struct fields**,
so the `ridl-sem` check and the struct emission land together. They are not
separable into two commits without violating that decision.

**Files:**

- Modify: `crates/ridl-backend-proto/src/lib.rs`
- Modify: `crates/ridl-backend-proto/src/tests.rs`
- Modify: `crates/ridl-sem/src/check.rs` — `lower_struct` at line 1740
- Modify: `crates/ridl-sem/src/tests.rs` (or the crate's test module, matching
  where the existing RIDL-149 tests near `check.rs:4349` live)

**Interfaces:**

- Consumes: `type_name` and `check_field_number` from Task 2;
  `ridl_ir::name::snake_case`;
  `Checker::colliding_projected_name(&mut self,
  name, first, projected, range, first_range)`
  at `check.rs:3293`, the shared RIDL-149 helper already used by the
  interface-member and parameter checks.
- Produces: `fn proto_scalar(td: &v2::TypeDef) -> &'static str` and
  `fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String`, used by
  Tasks 4 and 5.

- [ ] **Step 1: Write the failing `ridl-sem` test**

Follow the file and module conventions of the existing RIDL-149 tests near
`check.rs:4349`. The new test:

```rust
#[test]
fn two_struct_fields_that_collide_after_the_transform_are_refused() {
    // ADR-0016 decision 4: struct fields join the transform and RIDL-149 in
    // the commit that starts projecting them, which is E9.8's proto backend.
    let diagnostics = check_source(
        "package p\n\
         struct Reading {\n\
           vinNumber : integer [0..1]\n\
           vin_number : integer [0..1]\n\
         }\n",
    );
    let ridl_149: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::RIDL_149)
        .collect();
    assert_eq!(ridl_149.len(), 1, "got: {diagnostics:#?}");
    assert!(ridl_149[0].message.contains("vin_number"));
}

#[test]
fn struct_fields_that_do_not_collide_are_accepted() {
    let diagnostics = check_source(
        "package p\n\
         struct Reading {\n\
           vinNumber : integer [0..1]\n\
           engineTemp : integer [0..1]\n\
         }\n",
    );
    assert!(
        !diagnostics.iter().any(|d| d.code == DiagCode::RIDL_149),
        "got: {diagnostics:#?}"
    );
}
```

Read the neighbouring tests first and match their helper names exactly —
`check_source` above is a stand-in for whatever the existing RIDL-149 tests
call.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p ridl-sem --locked ridl_149` Expected: FAIL —
`ridl_149.len()` is 0, because struct fields are outside the check today.

- [ ] **Step 3: Extend the check in `lower_struct`**

In `crates/ridl-sem/src/check.rs`, inside `lower_struct` (line 1740), add a
projection map beside the existing `reserved_names` pre-pass and populate it in
the `ast::StructMember::Field` arm, before `self.lower_field`:

```rust
// RIDL-149: two field names that collide after the pinned name
// transform (ADR-0016 decisions 3 and 4). Struct fields joined this
// check when E9.8 began projecting them onto proto3, whose field
// namespace is snake_case. Keyed on the projection, holding the first
// field's source name and span for the label.
let mut projected: HashMap<String, (String, TextRange)> = HashMap::new();
```

and in the field arm:

```rust
if let Some(name) = member_name(field.name()) {
    let range = member_name_range(field.name(), field.syntax());
    let key = ridl_ir::name::snake_case(&name);
    match projected.get(&key) {
        Some((first, first_range)) => {
            let (first, first_range) = (first.clone(), *first_range);
            self.colliding_projected_name(
                &name, &first, &key, range, first_range,
            );
        }
        None => {
            projected.insert(key, (name.clone(), range));
        }
    }
}
```

Place it after the existing TYPL-210 check so a field that both re-declares a
reserved name and collides reports both, matching how the interface-member check
orders its diagnostics. Read the interface-member site near `check.rs:2868` and
mirror its structure.

- [ ] **Step 4: Run the `ridl-sem` tests**

Run: `cargo test -p ridl-sem --locked` Expected: PASS.

- [ ] **Step 5: Measure the churn before going further**

ADR-0016's consequences record that E9.7's equivalent change was free, measured
rather than assumed. Do the same:

Run: `cargo test --workspace --locked` Expected: PASS with no snapshot changes.
If any test fails, a name in the corpus, the book or the tests collides under
the transform — record which, and report it before continuing, because it means
the change is not free.

- [ ] **Step 6: Write the failing proto tests for scalars and structs**

Append to `crates/ridl-backend-proto/src/tests.rs`:

```rust
#[test]
fn a_struct_emits_a_message_numbered_by_typl_ordinals() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "SensorReading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("currentSpeed", 1, float64_type()),
                    field_member("sensorId", 2, int64_type()),
                    reserved_member(3),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains(
        "message SensorReading {\n  \
         double current_speed = 1;\n  \
         int64 sensor_id = 2;\n  \
         reserved 3;\n}"
    ), "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_named_scalar_inlines_and_leaves_its_constraint_in_a_comment() {
    // type Speed : km/h [0.0..250.0 step 0.5]
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Speed".to_string(),
                kind: Some(v2::decl::Kind::TypeDef(speed_type_def())),
                ..Default::default()
            },
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("value", 1, named_type("Speed"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    // The named type does not become a declaration of its own: it inlines.
    assert!(!generated.proto_source.contains("message Speed"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("// Speed"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("double value = 1;"),
        "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_optional_field_takes_the_proto3_optional_keyword() {
    // ADR-0013 decision 7: proto3 represents absence structurally, so it does.
    let mut ty = float64_type();
    ty.optional = true;
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Reading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member("value", 1, ty)],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("optional double value = 1;"),
        "got:\n{}", generated.proto_source);
    compile_with_protox("veh.common.proto", &generated.proto_source);
}
```

Write the `field_member`, `reserved_member`, `float64_type`, `int64_type`,
`named_type` and `speed_type_def` helpers alongside, constructing the IR types
directly. Read `crates/ridl-backend-rust/src/tests.rs` for the shape the other
backends' fixtures take and follow it.

- [ ] **Step 7: Run and confirm they fail**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL — no message is
emitted.

- [ ] **Step 8: Implement scalar and struct emission**

```rust
/// The proto3 scalar for a resolved typl width (typl Appendix D). proto3 has
/// no `uint8`/`uint16` — varint keeps small values small — so both widen to
/// `uint32`. A range containing negatives takes `sint32`/`sint64`, because
/// plain `int32` varint costs 10 bytes for every negative value (ADR-0013
/// decision 4). A quantized float keeps its native form: the scaled-integer
/// encoding of typl §4.3 belongs to CAN/DBC and to SOME/IP per deployment, and
/// a wire backend must not apply it unasked.
fn proto_scalar(td: &v2::TypeDef) -> &'static str {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => match v2::IntWidth::try_from(*width) {
            Ok(v2::IntWidth::Uint8 | v2::IntWidth::Uint16 | v2::IntWidth::Uint32) => "uint32",
            Ok(v2::IntWidth::Uint64) => "uint64",
            Ok(v2::IntWidth::Int32) => "sint32",
            Ok(v2::IntWidth::Int64) => "sint64",
            _ => "int64",
        },
        Some(v2::type_def::Width::FloatWidth(width)) => {
            match v2::FloatWidth::try_from(*width) {
                Ok(v2::FloatWidth::Float32) => "float",
                _ => "double",
            }
        }
        // No width table: boolean, string and bytes backings.
        None => match v2::PrimitiveType::try_from(td.backing.unwrap_or_default().primitive) {
            Ok(v2::PrimitiveType::Boolean) => "bool",
            Ok(v2::PrimitiveType::Bytes) => "bytes",
            _ => "string",
        },
    }
}
```

Read `v2::TypeDef`, `v2::Backing` and the `IntWidth`/`FloatWidth` enums before
writing this — the sketch shows the mapping, and the exact accessor names must
come from the generated types. `crates/ridl-backend-rust/src/lib.rs` already
reads the same fields for the language layer and is the reference for how they
are matched.

Then the constraint comment and the message emitter:

```rust
/// The constraint information proto3 has no construct for, as a comment
/// (design §3.2). This is the only home for it: the alternative — a published
/// options extension over `google.protobuf.FieldOptions` — was rejected for
/// v0.1 because it serves a consumer that does not exist, and the IR is
/// already the machine-readable contract.
fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String {
    let mut parts = vec![declared.to_string()];
    if let Some(constraint) = &td.constraint {
        parts.push(render_constraint(constraint));
    }
    format!("// {}", parts.join(" — "))
}
```

`declared` is the ridl type name as declared (`Speed`); it is a separate
parameter from the `type_name` helper of Task 2, which converts a dotted address
into a proto identifier. Do not conflate them.

Implement `render_constraint` over `v2::Constraint`, producing the source form
(`km/h [0.0..250.0 step 0.5]`). Read `v2::Constraint` for its exact fields; the
strings are already canonical text, per ADR-0007 decision 9.

Emit messages in `generate`, walking `package.decls` before the identity tables
so types precede the tables that reference nothing — order is free in proto3,
but reading order matters to a human. A field's name goes through
`ridl_ir::name::snake_case`; its number is `field.ordinal`, checked with
`check_field_number`; a `?` field takes the `optional` keyword.

- [ ] **Step 9: Run and confirm they pass**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS.

- [ ] **Step 10: Run the full gate**

Run: `just verify` Expected: exit 0.

- [ ] **Step 11: Commit — both changes together**

```bash
git add crates/ridl-backend-proto crates/ridl-sem
git commit -m "feat(ridl-backend-proto): project structs, and check their field names

Tier 1 begins: a struct becomes a proto3 message numbered by the typl
section 7.4 ordinals, with a tombstone emitting proto reserved. A named
scalar inlines to its backing scalar, and the unit, range and step that
proto3 has no construct for are emitted as a comment. An optional field
takes the proto3 optional keyword, because proto3 represents absence
structurally (ADR-0013 decision 7).

The ridl-sem change ships in this commit rather than its own, because
ADR-0016 decision 4 requires it: struct fields join the pinned transform
and RIDL-149 in the commit that starts projecting them, so that the rule
and its application change together. Two fields whose names collide after
the transform are now an error, the same fail-closed rule already applied
to interface members and interaction parameters.

Measured over the workspace before and after: no existing name collides,
so the new error rejects nothing that exists."
```

---

### Task 4: Tier 1 — enums and enum sets

**Files:**

- Modify: `crates/ridl-backend-proto/src/lib.rs`
- Modify: `crates/ridl-backend-proto/src/tests.rs`

**Interfaces:**

- Consumes: `screaming_snake_case` and `type_name` from Task 2; `proto_scalar`
  from Task 3.
- Produces: nothing new for later tasks.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_enum_prefixes_its_values_and_gains_a_zero_member() {
    // proto3 scopes enum values as siblings of the enum, so two enums in one
    // package could otherwise both declare OK. And the first value must be 0.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "GearPosition".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    v2::EnumValue { name: "PARK".to_string(), value: 1, doc: String::new() },
                    v2::EnumValue { name: "DRIVE".to_string(), value: 2, doc: String::new() },
                ],
                reserved: vec![v2::Reserved { ordinal: 3, ..Default::default() }],
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains(
        "enum GearPosition {\n  \
         GEAR_POSITION_UNSPECIFIED = 0;\n  \
         GEAR_POSITION_PARK = 1;\n  \
         GEAR_POSITION_DRIVE = 2;\n  \
         reserved 3;\n}"
    ), "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_that_already_declares_zero_gains_no_synthetic_member() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Mode".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    v2::EnumValue { name: "OFF".to_string(), value: 0, doc: String::new() },
                    v2::EnumValue { name: "ON".to_string(), value: 1, doc: String::new() },
                ],
                reserved: Vec::new(),
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(!generated.proto_source.contains("MODE_UNSPECIFIED"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("MODE_OFF = 0;"),
        "got:\n{}", generated.proto_source);
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_value_outside_int32_is_refused() {
    // proto3 enum values are int32; typl admits int64.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Wide".to_string(),
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![v2::EnumValue {
                    name: "HUGE".to_string(),
                    value: i64::from(i32::MAX) + 1,
                    doc: String::new(),
                }],
                reserved: Vec::new(),
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("int32"), "got: {}", error.message);
}

#[test]
fn a_const_is_not_emitted() {
    // ADR-0013 decision 5: neither proto3 nor FlatBuffers has a constant
    // declaration, and no instance of a typl constant ever crosses a wire.
    // A wire backend may emit one as a comment and must not encode it as an
    // enum, which is the mistake this test exists to catch.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "MAX_GEAR".to_string(),
            kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("integer".to_string()),
                value: "6".to_string(),
                regex: None,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(!generated.proto_source.contains("enum MAX_GEAR"),
        "a const must not become an enum:\n{}", generated.proto_source);
    assert!(!generated.proto_source.contains("message MAX_GEAR"),
        "got:\n{}", generated.proto_source);
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_enum_set_becomes_an_integer_with_its_bits_in_a_comment() {
    // A proto enum field holds one value, so it cannot represent a
    // combination of bits. Emitting one would imply a guarantee proto3 does
    // not make (ADR-0013 decision 2).
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Warnings".to_string(),
                kind: Some(v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                    backing_enum: None,
                    bits: vec![
                        v2::EnumValue { name: "LOW_FUEL".to_string(), value: 0, doc: String::new() },
                        v2::EnumValue { name: "DOOR_AJAR".to_string(), value: 1, doc: String::new() },
                    ],
                    width: v2::IntWidth::Uint32 as i32,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Status".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("warnings", 1, named_type("Warnings"))],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(!generated.proto_source.contains("enum Warnings"),
        "an enum set must not become a proto enum:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("uint32 warnings = 1;"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("LOW_FUEL = bit 0"),
        "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}
```

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL.

- [ ] **Step 3: Implement enum and enum-set emission**

An enum emits `enum <Name> { ... }` with every value prefixed by
`screaming_snake_case(&name)`; `<PREFIX>_UNSPECIFIED = 0` is synthesized only
when no declared value is 0; a `Reserved` emits `reserved N;`; a value outside
`i32` fails with a `GenerateError` naming `int32`.

An enum set emits **no declaration**. It resolves to the proto scalar for its
`width` at each use site, and its bit names and positions become a comment
there, one line per bit in the form `LOW_FUEL = bit 0`.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "feat(ridl-backend-proto): project enums and enum sets

A typl enum becomes a proto3 enum keeping its explicitly assigned
numbers, with a retired value emitting proto reserved. Two proto3 rules
shape the output: enum values are scoped as siblings of the enum, so
every value is prefixed with the enum's name or two enums in one package
could both declare OK; and the first value must be zero, so a synthetic
UNSPECIFIED member leads unless a declared value already takes 0. A value
outside int32 is refused, because proto3 enum values are int32 while typl
admits int64.

An enum set does not become a proto enum. A proto enum field holds one
value and cannot represent a combination of bits, so emitting one would
imply a guarantee proto3 does not make - what ADR-0013 decision 2
forbids. It resolves to its integer width at each use site, with the bit
names and positions in a comment."
```

---

### Task 5: Tier 1 — unions, arrays, maps and tuples

**Files:**

- Modify: `crates/ridl-backend-proto/src/lib.rs`
- Modify: `crates/ridl-backend-proto/src/tests.rs`

**Interfaces:**

- Consumes: everything from Tasks 3 and 4.
- Produces: nothing new for later tasks.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_union_becomes_a_message_wrapping_a_oneof() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Payload".to_string(),
            kind: Some(v2::decl::Kind::UnionDef(v2::UnionDef {
                arms: vec![
                    v2::UnionArm {
                        name: "speed".to_string(),
                        ordinal: 1,
                        type_ref: "Speed".to_string(),
                        doc: String::new(),
                    },
                    v2::UnionArm {
                        name: "gearIndex".to_string(),
                        ordinal: 2,
                        type_ref: "GearIndex".to_string(),
                        doc: String::new(),
                    },
                ],
                is_result: false,
                reserved: vec![v2::Reserved { ordinal: 3, ..Default::default() }],
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains(
        "message Payload {\n  \
         oneof value {\n    \
         double speed = 1;\n    \
         sint64 gear_index = 2;\n  \
         }\n  \
         reserved 3;\n}"
    ), "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn an_array_field_is_repeated() {
    let package = struct_package("Trace", "samples", 1, array_of(float64_type()));
    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("repeated double samples = 1;"),
        "got:\n{}", generated.proto_source);
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_map_field_becomes_a_proto_map() {
    let package = struct_package(
        "Index", "byName", 1,
        map_of(v2::PrimitiveType::String, float64_type()),
    );
    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("map<string, double> by_name = 1;"),
        "got:\n{}", generated.proto_source);
    compile_with_protox("veh.common.proto", &generated.proto_source);
}

#[test]
fn a_tuple_field_induces_a_positional_message() {
    // proto3 has no tuple, so one is generated, named for its owner and field.
    let package = struct_package(
        "Reading", "bounds", 1,
        tuple_of(vec![float64_type(), float64_type()]),
    );
    let generated = generate(&package).expect("generate");

    assert!(generated.proto_source.contains(
        "message ReadingBounds {\n  \
         double field_1 = 1;\n  \
         double field_2 = 2;\n}"
    ), "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("ReadingBounds bounds = 1;"),
        "got:\n{}", generated.proto_source);

    compile_with_protox("veh.common.proto", &generated.proto_source);
}
```

Write `struct_package`, `array_of`, `map_of` and `tuple_of` helpers alongside.

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL.

- [ ] **Step 3: Implement the composite mappings**

A union becomes `message <Name>` wrapping `oneof value`, with each arm's field
number being its `ordinal` and each arm name going through `snake_case`; a
retired arm emits `reserved N;` at the message level, outside the `oneof`,
because proto3 does not admit `reserved` inside one.

An array becomes `repeated <element>`. A map becomes `map<K, V>`; proto3
restricts a map key to an integral or string type, so a key outside that set
fails with a `GenerateError` rather than emitting something `protoc` rejects.

A tuple induces a message named `<OwnerType><FieldName>` in CamelCase, with
positional fields `field_1`, `field_2`, … numbered from 1. Induced messages are
collected during the walk and emitted after the declarations that induced them,
the way `ridl-backend-rust` collects `InducedTuple`.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "feat(ridl-backend-proto): project unions, arrays, maps and tuples

A union becomes a message wrapping a oneof, with each arm numbered by its
typl ordinal and a retired arm emitting proto reserved at the message
level - proto3 does not admit reserved inside a oneof. An array becomes
repeated, and a map becomes a proto map, with a key type proto3 cannot
carry refused rather than emitted.

proto3 has no tuple, so a tuple field induces a message named for its
owner and field, with positional fields numbered from 1."
```

---

### Task 6: Cross-package imports and the `ridl.std` well-known types

**Files:**

- Modify: `crates/ridl-backend-proto/src/lib.rs`
- Modify: `crates/ridl-backend-proto/src/tests.rs`

**Interfaces:**

- Consumes: everything above; `ridl_ir::v2` helpers for referenced packages —
  `crates/ridl-ir/src/lib.rs` near line 477 has "Every package named by a type
  reference in `package`", which is the traversal to reuse rather than rewrite.
- Produces: nothing new for later tasks.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_cross_package_reference_emits_an_import() {
    let package = struct_package_in(
        "veh.cluster", "Reading", "value", 1, named_type("veh.common.Speed"),
    );
    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("import \"veh.common.proto\";"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("veh.common.Speed value = 1;"),
        "got:\n{}", generated.proto_source);
}

#[test]
fn ridl_std_duration_maps_onto_the_protobuf_well_known_type() {
    // ridl.std is version-locked to the compiler binary and excluded from IR
    // dumps, so it gets no emitted file of its own.
    let package = struct_package_in(
        "veh.cluster", "Window", "span", 1, named_type("ridl.std.Duration"),
    );
    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("import \"google/protobuf/duration.proto\";"),
        "got:\n{}", generated.proto_source);
    assert!(generated.proto_source.contains("google.protobuf.Duration span = 1;"),
        "got:\n{}", generated.proto_source);
    assert!(!generated.proto_source.contains("import \"ridl.std.proto\";"),
        "got:\n{}", generated.proto_source);
}
```

`compile_with_protox` is not called in the first test: the import names a file
the temp directory does not hold. Task 7 covers cross-package compilation over
the real corpus, where both files exist.

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-proto --locked` Expected: FAIL.

- [ ] **Step 3: Implement imports**

Collect every referenced package during the walk into a `BTreeSet<String>` so
the import block is deterministic and sorted — ADR-0016 decision 6's determinism
property applies to the whole emission, not only to numbers. Emit the block
after `package ...;` and before the first declaration.

Map `ridl.std.Duration` to `google.protobuf.Duration` with
`import "google/protobuf/duration.proto";`, and `ridl.std.Timestamp` to
`google.protobuf.Timestamp` with `import "google/protobuf/timestamp.proto";`.
Any other `ridl.std` member fails with a `GenerateError` naming it, so a future
addition to the standard package cannot pass through silently untranslated.

Every other package `p` emits `import "p.proto";` and its types are referenced
by their fully qualified name.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-proto --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "feat(ridl-backend-proto): emit imports for referenced packages

A cross-package type reference emits an import naming the other package's
file, matching the artifact naming the TypeScript backend already relies
on in package and workspace mode. The import block is sorted, because
ADR-0016 decision 6's determinism property covers the whole emission and
not only the numbers.

ridl.std gets no emitted file: it is version-locked to the compiler
binary and already excluded from IR dumps. Its two members map onto the
protobuf well-known types instead. Any other ridl.std member is refused,
so a later addition to the standard package cannot pass through silently
untranslated."
```

---

### Task 7: The cruise-control fixture and corpus-wide validation

The story's acceptance criterion. No cruise-control package exists in the
repository, and the baseline corpus declares only named scalars and interfaces,
so it exercises tier 2 and almost none of tier 1.

**Files:**

- Create: `crates/ridl-backend-proto/tests/fixtures/cruise.ridl`
- Create: `crates/ridl-backend-proto/tests/corpus.rs`
- Create: `crates/ridl-backend-proto/tests/snapshots/` — `insta` writes beside
  the test file, so an integration test's snapshots land here. The unit-test
  snapshots of the other backends live in `src/snapshots/` because their tests
  are in `src/tests.rs`; both conventions are correct for their location.
- Modify: `crates/ridl-backend-proto/Cargo.toml` — `ridlc` as a dev-dependency,
  for the source-to-IR compile path

**Interfaces:**

- Consumes: `generate`; the compile path used by `crates/ridl/tests/` to go from
  source text to a checked `v2::Package` — read that harness and reuse it rather
  than writing a second one.
- Produces: the golden snapshot every later change is diffed against.

- [ ] **Step 1: Write the fixture**

`crates/ridl-backend-proto/tests/fixtures/cruise.ridl` must exercise every row
of design §3.1, so that each has a living example:

```ridl
package veh.cruise

type Speed      : km/h    [0.0..250.0 step 0.5]
type Percent    : integer [0..100]
type Delta      : integer [-100..100]

enum EngageState {
  OFF    = 0
  ARMED  = 1
  ACTIVE = 2
}

enumset Warnings {
  LOW_FUEL
  DOOR_AJAR
}

struct Setpoint {
  target    : Speed
  tolerance : Percent?
  trim      : Delta
  reserved 4
}

union Command {
  engage   : Setpoint
  disengage: Percent
}

interface CruiseControl {
  signal  currentSpeed : Speed @10ms
  signal  state        : EngageState @10ms
  event   warning      : Warnings @[100ms..1s]
  command setTarget(target : Speed)
  reserved legacyMode
}
```

Adjust the syntax to what the checker actually accepts — run
`cargo run -p ridl -- check crates/ridl-backend-proto/tests/fixtures/cruise.ridl`
and fix until it reports no diagnostic. The declarations above are the coverage
requirement, not a syntax reference.

- [ ] **Step 2: Write the failing test**

`crates/ridl-backend-proto/tests/corpus.rs`:

```rust
//! The story's acceptance check: the cruise-control package emits valid
//! proto3, and its text is pinned so a later change has to be looked at.

#[test]
fn the_cruise_control_package_emits_valid_proto3() {
    let package = compile_fixture("cruise.ridl");
    let generated = ridl_backend_proto::generate(&package).expect("generate");
    compile_with_protox("veh.cruise.proto", &generated.proto_source);
    insta::assert_snapshot!(generated.proto_source);
}

#[test]
fn the_baseline_corpus_emits_valid_proto3() {
    let package = compile_fixture("../../../ridl/tests/baseline-corpus/cluster.ridl");
    let generated = ridl_backend_proto::generate(&package).expect("generate");
    compile_with_protox("corpus.baseline.proto", &generated.proto_source);
    insta::assert_snapshot!(generated.proto_source);
}
```

Write `compile_fixture` over the existing source-to-IR harness, and move
`compile_with_protox` somewhere both the unit tests and this integration test
can reach it.

- [ ] **Step 3: Run and confirm it fails**

Run: `cargo test -p ridl-backend-proto --locked --test corpus` Expected: FAIL —
no snapshot is accepted yet.

- [ ] **Step 4: Review and accept the snapshots**

Run: `cargo insta review`

**Read every line of the emitted schema before accepting.** This is the artifact
the story delivers, and the snapshot is what future changes are compared
against, so an error accepted here becomes the baseline. Check specifically:
every field number matches the source ordinal; every retired slot emits
`reserved`; no enum set became a `proto` enum; the constraint comments carry the
unit, the range and the step.

- [ ] **Step 5: Run the full gate**

Run: `just verify` Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "test(ridl-backend-proto): the cruise-control package emits valid proto3

The story's acceptance criterion. No cruise-control package existed, and
the baseline corpus declares only named scalars and interfaces, so it
exercises the identity table and almost none of the typl surface. The
fixture declares one of every construct design section 3.1 maps, so each
row has a living example.

Both packages are compiled with protox and their emitted text is
snapshotted."
```

---

### Task 8: The stability property, driven from the classifier

ADR-0016 decision 6 property 3 — "if `ridl-diff` returns compatible, no number
already assigned may move" — is the load-bearing property, and the note requires
it be driven from the classifier rather than from hand-written examples:
"Hard-coding example deltas would test the examples; driving it from the
classifier tests the rule."

**Files:**

- Create: `crates/ridl-backend-proto/tests/stability.rs`
- Modify: `crates/ridl-backend-proto/Cargo.toml` — add `proptest` and
  `ridl-diff` as dev-dependencies

**Interfaces:**

- Consumes:
  `ridl_diff::diff_packages(old: &Package, new: &Package) ->
  DiffReport` and
  `ridl_diff::Verdict`; `generate`.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

```rust
//! ADR-0016 decision 6, property 3: a compatible change never moves a number
//! already assigned. Driven from `ridl-diff`'s own classifier, so the property
//! is tested rather than a list of examples.

use proptest::prelude::*;

/// Every `name = number` pair the schema assigns, keyed by the enclosing
/// message or enum so two declarations cannot share a key. A number that moves
/// between two schemas is exactly a change to this map's values.
///
/// This reads the emitted *text* rather than the emitter's own data, so a bug
/// in the emitter cannot hide itself in the assertion.
fn assigned_numbers(schema: &str) -> std::collections::BTreeMap<String, u32> {
    let mut out = std::collections::BTreeMap::new();
    let mut scope = String::new();
    for line in schema.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("message ").or_else(|| line.strip_prefix("enum ")) {
            scope = rest.trim_end_matches(" {").trim().to_string();
            continue;
        }
        if line == "}" {
            scope.clear();
            continue;
        }
        // Both `double current_speed = 1;` and `GEAR_POSITION_PARK = 1;` end
        // in `= <digits>;`. `reserved 3;` deliberately does not match: a
        // tombstone assigns no name.
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        let Some(digits) = right.trim().strip_suffix(';') else {
            continue;
        };
        let Ok(number) = digits.trim().parse::<u32>() else {
            continue;
        };
        let Some(name) = left.split_whitespace().last() else {
            continue;
        };
        out.insert(format!("{scope}.{name}"), number);
    }
    out
}

proptest! {
    #[test]
    fn a_compatible_change_never_moves_an_assigned_number(
        delta in compatible_delta_strategy()
    ) {
        let (old, new) = delta;
        let report = ridl_diff::diff_packages(&old, &new);
        prop_assume!(report.verdict == ridl_diff::Verdict::Compatible);

        let old_numbers = assigned_numbers(
            &ridl_backend_proto::generate(&old).expect("generate old").proto_source
        );
        let new_numbers = assigned_numbers(
            &ridl_backend_proto::generate(&new).expect("generate new").proto_source
        );

        for (name, number) in &old_numbers {
            if let Some(moved) = new_numbers.get(name) {
                prop_assert_eq!(
                    number, moved,
                    "{name} moved from {number} to {moved} under a change \
                     ridl-diff calls compatible"
                );
            }
        }
    }
}
```

Write `compatible_delta_strategy` as a proptest `Strategy` producing a
`(v2::Package, v2::Package)` pair by generating a package and applying one
mutation drawn from the edits `ridl-diff` classifies: appending an interaction
at the next ordinal, retiring one with a tombstone, widening a declared range,
adding a struct field at the next ordinal, adding an enum value. Read
`crates/ridl-diff/src/classify.rs` for the full set the classifier calls
compatible, and take the mutations from there rather than from memory.

`prop_assume!` discards a mutation the classifier calls breaking, so the
strategy need not be perfectly accurate — but a strategy that produces mostly
breaking changes tests very little, so check the pass rate with
`PROPTEST_VERBOSE=1` and adjust if most cases are discarded.

- [ ] **Step 2: Run and confirm it fails**

Run: `cargo test -p ridl-backend-proto --locked --test stability` Expected: FAIL
— the strategy and the parser do not exist yet.

- [ ] **Step 3: Implement the parser and the strategy**

- [ ] **Step 4: Run and confirm it passes**

Run: `cargo test -p ridl-backend-proto --locked --test stability` Expected:
PASS, with the default 256 cases.

If it fails, **do not weaken the property** — a counterexample is the point of
the test. Report the shrunk case: it means either the projection moves a number
under a compatible change (a real defect in this story) or `ridl-diff`
classifies something as compatible that is not (a defect one layer down, which
is more serious and must be reported rather than worked around).

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-proto
git commit -m "test(ridl-backend-proto): pin the stability property from the classifier

ADR-0016 decision 6 property 3: if ridl-diff returns compatible, no
number already assigned may move. This is the load-bearing property of a
projection, and the schema-projection note requires it be driven from the
classifier - hard-coding example deltas would test the examples, while
driving it from ridl-diff tests the rule.

The check parses the emitted text rather than reading the emitter's own
data, so a bug in the emitter cannot hide itself in the assertion."
```

---

### Task 9: Documentation and the roadmap

**Files:**

- Modify: `docs/ROADMAP.md` — the E9.8 row and the Epic 9 status paragraph
- Modify: `docs/book/cli-reference.md` — the `--emit` values
- Modify: `AGENTS.md` — the crate count and list
- Modify: `README.md` — the crate list, if it enumerates them
- Modify: `docs/wip/README.md` — mark the design note's story landed

**Interfaces:**

- Consumes: nothing.
- Produces: nothing.

- [ ] **Step 1: Update the crate inventory**

`AGENTS.md` currently reads "eleven crates under `crates/`" and lists them.
`ridl-backend-proto` makes twelve. Update both the count and the list, and check
`README.md` for the same enumeration.

- [ ] **Step 2: Update the CLI reference**

Add `proto` to the documented `--emit` values in `docs/book/cli-reference.md`,
matching how `rust`, `c-header` and `typescript` are described there. State what
it emits: the typl surface and the interaction identity table, and not a
`service` block.

- [ ] **Step 3: Update the roadmap**

Mark E9.8 landed in the Epic 9 table and add a status paragraph recording what
shipped, in the style of the existing E9.1–E9.6 paragraph. Record the two
decisions the design took that are not in an ADR — constraints as comments, and
the emit ceiling for this story — and the conflict left for E9.11.

- [ ] **Step 4: Verify the docs gates**

Run: `just check && just link-check && just book-check` Expected: all exit 0.

- [ ] **Step 5: Run the whole gate**

Run: `just verify` Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add AGENTS.md README.md docs/
git commit -m "docs(docs): record the proto3 backend

The workspace gains a twelfth crate, the CLI gains an emit value, and the
E9.8 roadmap row lands. The status paragraph records the two decisions
the design took outside an ADR - constraint information is carried as
comments, and E9.8's ceiling is tier 1 and tier 2 with no service block -
and the conflict left for E9.11: ADR-0013 decision 2 says a wire backend
emits no service block, while ADR-0016 decision 10 describes the
dispatcher as one service definition per provided interface."
```

---

## After the plan

Before opening the PR:

1. `just verify` exits 0.
2. Run the review, per the standing order: a spec-and-quality review over the
   branch, findings recorded as PR comments, fixes for what survives scoring.
3. `docs/wip/` gardening is **not** part of this story: the design note stays
   there while E9.9 to E9.11 are open, exactly as the schema-projection note
   does. The design/plan pair for this story is archived when the story lands,
   the way E9.7's pair was.

Two follow-ups this story deliberately does not do, recorded so they are not
lost:

- **`ridl-backend-rust` emits struct field names verbatim**, so a multi-word
  typl field reaches generated Rust as `pub sensorId: i64` and draws
  `non_snake_case` at every consumer, since the backend emits no `allow`
  attribute. Design §8 records it; file it as an issue.
- **The ADR-0013 decision 2 versus ADR-0016 decision 10 conflict** over the
  `service` block must be resolved by an ADR amendment before E9.11 writes an
  emitter against either reading.
