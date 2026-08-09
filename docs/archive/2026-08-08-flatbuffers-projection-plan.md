# FlatBuffers Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit valid FlatBuffers schemas from IR v2 — the typl surface and the
interaction identity table — as roadmap story E9.9.

**Architecture:** A new crate `ridl-backend-flatbuffers` walks a `v2::Package`
and writes `.fbs` text directly, the way `ridl-backend-proto` writes `.proto`.
It inherits ADR-0017 decision 1's API: `generate_with(package, others)`, with
`generate(package)` as the single-package convenience. Validity is established
by compiling every emitted schema with `planus-translation` inside the test
suite.

**Tech Stack:** Rust 1.95.0, `ridl-ir`, `planus-translation` 1.3.0 (pure-Rust
FlatBuffers compiler, test-only), `insta`, `proptest`.

**Design of record:**
[`2026-08-08-flatbuffers-projection-design.md`](2026-08-08-flatbuffers-projection-design.md).
Every structural claim in it was verified against `flatc` 25.12.19 and `planus`
1.3.0; the section references below point at those measurements.

## Global Constraints

- **Never push to `main`.** All work lands on `e99-flatbuffers-projection`
  through a PR. Squash merge.
- **The toolchain is pinned** — rustc 1.95.0 via `rust-toolchain.toml`.
- **`just verify` before any PR.** Clippy runs with `-D warnings`.
- **`planus-translation` is a dev-dependency and must stay one.** It is the
  test-time validity oracle, not part of emission. Making it a normal dependency
  would put a schema compiler in every build and break `just wasm-check`.
- **`flatc` must NOT become a dependency of anything.** It was used to author
  the design and may be used to spot-check by hand; the repository must not
  require it.
- **Conventional Commits**, linted by git-std. A new crate adds its own scope to
  `.git-std.toml`, which is an explicit list.
- **Prose is plain and literal** in comments, commit messages and docs.
- **No wildcard arms over `Emit`** — `ridlc`'s matches are deliberately
  exhaustive so the compiler names every site a new variant must reach.
- **Every emitted identifier goes through `ridl_ir::name::snake_case`** where
  the target namespace is snake_case. Never write a second transform.
- **Never write a test that skips the validity check.** If a schema cannot be
  compiled standalone, make it compilable (write its siblings to the same
  directory) or say why in the report — do not silently omit the check. This is
  the single instruction that caused E9.8's costliest defect.

---

### Task 1: Crate skeleton, `Emit::Flatbuffers`, and the planus harness

**Files:**

- Create: `crates/ridl-backend-flatbuffers/Cargo.toml`
- Create: `crates/ridl-backend-flatbuffers/src/lib.rs`
- Create: `crates/ridl-backend-flatbuffers/src/tests.rs`
- Modify: `crates/ridlc/src/lib.rs` — the `Emit` enum (~line 272), its
  `ir_dump_suffix` match, and the emit dispatch beside `Emit::Proto` (~line 753)
- Modify: `crates/ridlc/Cargo.toml`, `Cargo.toml`, `.git-std.toml`

**Interfaces:**

- Consumes: `ridl_ir::v2::Package`.
- Produces: `generate(&v2::Package) -> Result<Generated, GenerateError>` and
  `generate_with(&v2::Package, &[&v2::Package]) -> Result<Generated,
  GenerateError>`,
  where `Generated { pub fbs_source: String }` and
  `GenerateError { pub message: String }`. Also
  `tests::compile_with_planus(file_name: &str, source: &str)`, the shared
  validity assertion.

- [ ] **Step 1: Write the failing test**

```rust
use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as a FlatBuffers schema with planus, panicking with the
/// compiler's own message on failure. This is the story's acceptance check:
/// every test that emits a schema runs it through here.
pub(crate) fn compile_with_planus(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if planus_translation::translate_files(&[path]).is_none() {
        panic!("emitted schema is not a valid FlatBuffers schema:\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package { name: name.to_string(), ..Default::default() }
}

#[test]
fn an_empty_package_emits_a_valid_namespace_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(generated.fbs_source, "namespace veh.common;\n");
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}
```

`translate_files` returns `Option`; it prints its own diagnostics to stderr. If
its signature differs in 1.3.0, read the crate's docs and adapt — the contract
this harness must provide is "panic with the compiler's complaint on invalid
input", not a particular call shape.

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL — the
crate does not exist.

- [ ] **Step 3: Create the manifest**

```toml
[package]
name = "ridl-backend-flatbuffers"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
ridl-ir = { path = "../ridl-ir" }

# planus-translation is TEST-ONLY and must stay here. It is the validity
# oracle, not part of emission: making it a normal dependency would put a
# schema compiler in every build and break `just wasm-check`.
[dev-dependencies]
insta.workspace = true
planus-translation = "1.3.0"
tempfile = "3.15.0"
```

Add `planus-translation` to the root `[workspace.dependencies]` if the
repository's convention is to pin every third-party version there — check how
`protox` is declared for `ridl-backend-proto` and follow it exactly.

- [ ] **Step 4: Write the minimal implementation**

```rust
//! IR v2 package to a FlatBuffers schema (roadmap story E9.9, ADR-0013
//! decision 2).
//!
//! The second wire backend. The emit ceiling is two tiers — the typl surface
//! and the interaction identity table — and nothing above them. No
//! `rpc_service`, no reply carriers, no store.
//!
//! Three rules here differ from the proto3 backend and are not
//! interchangeable with it: a union is isolated in a wrapper table because a
//! native union owns two id slots; a struct is always emitted as a `table`
//! because a FlatBuffers `struct` fabricates a value after a compatible
//! append; and enum values are scoped to their enum rather than to the
//! namespace, so no value prefixing is emitted.

use ridl_ir::v2;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub fbs_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the FlatBuffers schema for `package`, resolving foreign
/// references against `others` (ADR-0017 decision 1).
pub fn generate_with(
    package: &v2::Package,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    let _ = others;
    let mut out = String::new();
    out.push_str(&format!("namespace {};\n", package.name));
    Ok(Generated { fbs_source: out })
}

/// [`generate_with`] with no other packages — the single-package case. A
/// foreign reference fails rather than emitting an unresolvable name.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_with(package, &[])
}
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: PASS.

- [ ] **Step 6: Wire `Emit::Flatbuffers` into `ridlc`**

Add the variant, its `ir_dump_suffix` arm (it is a code emit, so it joins the
`None` arm), and a dispatch arm. **Read the `Emit::Proto` arm at
`crates/ridlc/src/lib.rs:753` and follow it exactly** — it already threads
`others` into `generate_with`, which this backend needs identically. Add
`ridl-backend-flatbuffers` to `crates/ridlc/Cargo.toml` and to `.git-std.toml`'s
scope list.

The `Emit` matches are wildcard-free by design. Expect the compiler to name
several sites beyond `src/lib.rs` — E9.8 found two more matches plus an `--emit`
value list in `crates/ridlc/tests/cli.rs` and a help doc comment in
`crates/ridlc/src/main.rs`. Fix each; never add a wildcard.

- [ ] **Step 7: Verify end to end**

Run: `cargo test --workspace --locked` Run:
`cargo run -p ridl -- build crates/ridl/tests/baseline-corpus/cluster.ridl --emit flatbuffers --out-dir /tmp/fbsout`
Expected: writes `.fbs` artifacts. Inspect one by eye.

- [ ] **Step 8: Commit**

```bash
git add crates/ridl-backend-flatbuffers crates/ridlc Cargo.toml Cargo.lock .git-std.toml
git commit -m "feat(ridl-backend-flatbuffers): add the crate and wire the emit

The second wire backend. This commit establishes the crate, the
Emit::Flatbuffers value and the planus validity check; the mapping
arrives in the commits that follow.

planus-translation is a dev-dependency and must stay one: it is the
test-time validity oracle, not part of emission, and making it a normal
dependency would put a schema compiler in every build and break
wasm-check."
```

---

### Task 2: Tier 2 — the interaction identity table

**Files:**

- Modify: `crates/ridl-backend-flatbuffers/src/lib.rs`, `src/tests.rs`

**Interfaces:**

- Consumes: `v2::Package::shapes()`, which yields
  `InterfaceShape { name, interface, service }` and covers named interfaces and
  a service's inline shape uniformly — for an inline shape `name` is the dotted
  service address, because `Interface.name` is empty there. It is defined in
  `crates/ridl-ir/src/lib.rs` around line 452. **Use it; do not write a second
  traversal.**
- Produces: `fn type_name(dotted: &str) -> String` (a dotted address to one
  CamelCase identifier) and `fn screaming_snake_case(name: &str) -> String`
  (built on the pinned `ridl_ir::name::snake_case`), both used by Task 4.

- [ ] **Step 1: Write the failing tests**

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
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    // FlatBuffers scopes enum values to their enum, so values are NOT
    // prefixed with the enum name — unlike the proto3 backend.
    assert!(generated.fbs_source.contains(
        "enum VehicleStatusOrdinal : uint {\n  \
         CURRENT_SPEED = 1,\n  \
         DOOR_OPENED = 2,\n  \
         TYRE_PRESSURE = 4,\n}"
    ), "got:\n{}", generated.fbs_source);

    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn two_enums_may_share_a_value_name() {
    // The scoping difference from proto3, pinned so nobody reintroduces
    // prefixing by copying the proto backend.
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![
            v2::Interface {
                name: "Alpha".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
            v2::Interface {
                name: "Beta".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("enum AlphaOrdinal : uint {"));
    assert!(generated.fbs_source.contains("enum BetaOrdinal : uint {"));
    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
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
    assert!(generated.fbs_source.contains("enum CorpusBaselineHvacOrdinal : uint {"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("corpus.baseline.fbs", &generated.fbs_source);
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
```

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL — no enum
is emitted.

- [ ] **Step 3: Implement the identity table**

Emit one `enum <ShapeName>Ordinal : uint { ... }` per shape from
`package.shapes()`, values at their ordinals in declaration order, value names
through `screaming_snake_case`.

Three differences from the proto3 backend, each verified against `flatc` and
each easy to get wrong by copying it:

1. **No value prefixing.** FlatBuffers scopes enum values inside their enum.
2. **No synthesized zero member.** A FlatBuffers enum declaration needs none.
3. **An explicit underlying type is required** — `: uint`, since ridl ordinals
   are `u32`.

A retired ordinal contributes no enum value: a FlatBuffers enum has no
`reserved` construct, and the identity table is a name-to-number map rather than
a wire layout, so a gap is simply a gap. Record that in a comment on the
function.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "feat(ridl-backend-flatbuffers): emit the interaction identity table

Tier 2 of ADR-0013 decision 2: one enum per interface shape, keyed by the
ridl section 11 ordinal, interface-wide and kind-blind.

Three things differ from the proto3 backend and are pinned by tests.
FlatBuffers scopes enum values to their enum rather than to the
namespace, so no value prefixing is emitted; a FlatBuffers enum
declaration needs no zero member, so none is synthesized; and an explicit
underlying type is required, which is uint because ridl ordinals are u32."
```

---

### Task 3: Tier 1 — named scalars and struct tables

**Files:**

- Modify: `crates/ridl-backend-flatbuffers/src/lib.rs`, `src/tests.rs`

**Interfaces:**

- Consumes: `type_name`, `screaming_snake_case` from Task 2.
- Produces: `fn fbs_scalar(td: &v2::TypeDef) -> &'static str`,
  `fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String`, and
  `fn resolve_field_type(..)`, all used by Tasks 4 and 5.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_struct_emits_a_table_with_ids_one_below_the_ordinal() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "SensorReading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("currentSpeed", 1, float64_type()),
                    field_member("sensorId", 2, int64_type()),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains(
        "table SensorReading {\n  \
         current_speed: double (id: 0);\n  \
         sensor_id: long (id: 1);\n}"
    ), "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_fixed_layout_struct_is_still_a_table() {
    // typl Appendix D permits a FlatBuffers `struct` here. The design
    // withdraws that: a FlatBuffers struct fabricates a value from padding
    // after a compatible append, which makes ADR-0016 decision 6 property 3
    // unsatisfiable. The flag must not change the emitted construct.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Packed".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member("a", 1, int64_type())],
                fixed_layout: true,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("table Packed {"),
        "got:\n{}", generated.fbs_source);
    assert!(!generated.fbs_source.contains("struct Packed"),
        "a fixed-layout struct must not become a FlatBuffers struct:\n{}",
        generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_retired_field_holds_its_slot_as_a_deprecated_field() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![v2::Decl {
            name: "Reading".to_string(),
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member("value", 1, float64_type()),
                    reserved_member(2),
                    field_member("trim", 3, int64_type()),
                ],
                fixed_layout: false,
            })),
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    // The tombstone keeps id 1 occupied so `trim` stays at id 2.
    assert!(generated.fbs_source.contains("(id: 1, deprecated)"),
        "got:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("trim: long (id: 2);"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn every_width_takes_its_narrow_flatbuffers_type() {
    // The full uint8..uint64 palette is the reason to choose this target
    // (typl Appendix D, ADR-0013 decision 4). A width silently widened here
    // is the defect this test exists to catch.
    let package = widths_package();
    let generated = generate(&package).expect("generate");
    for expected in [
        "u8_field: ubyte (id: 0);",
        "u16_field: ushort (id: 1);",
        "u32_field: uint (id: 2);",
        "u64_field: ulong (id: 3);",
        "i8_field: byte (id: 4);",
        "i16_field: short (id: 5);",
        "i32_field: int (id: 6);",
        "i64_field: long (id: 7);",
        "f32_field: float (id: 8);",
        "f64_field: double (id: 9);",
    ] {
        assert!(generated.fbs_source.contains(expected),
            "missing `{expected}` in:\n{}", generated.fbs_source);
    }
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_const_is_not_emitted() {
    // ADR-0013 decision 5: neither proto3 nor FlatBuffers has a constant
    // declaration, and no instance of a typl constant ever crosses a wire.
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
    assert!(!generated.fbs_source.contains("MAX_GEAR"),
        "a const must not be emitted:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_named_scalar_inlines_and_leaves_its_constraint_in_a_comment() {
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
    assert!(!generated.fbs_source.contains("table Speed"),
        "a named scalar must inline:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("// Speed"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}
```

Write `field_member`, `reserved_member`, `float64_type`, `int64_type`,
`named_type`, `speed_type_def` and `widths_package` alongside. `widths_package`
declares one struct with ten fields at ordinals 1 to 10, each typed by a named
scalar whose `TypeDef.width` is a different one of the ten typl widths. **Read
`crates/ridl-backend-proto/src/tests.rs` for the exact shapes** — it has
equivalents for all six, and reusing their construction avoids re-deriving the
IR by guesswork. Note that `speed_type_def` there deliberately omits `step`,
because a declared step makes the checker derive f32; keep that property.

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL — no table
is emitted.

- [ ] **Step 3: Implement scalar and table emission**

`fbs_scalar` maps a resolved width to FlatBuffers' full palette, which is the
whole point of this target (typl Appendix D, ADR-0013 decision 4):

| typl width | FlatBuffers |
| ---------- | ----------- |
| `uint8`    | `ubyte`     |
| `uint16`   | `ushort`    |
| `uint32`   | `uint`      |
| `uint64`   | `ulong`     |
| `int8`     | `byte`      |
| `int16`    | `short`     |
| `int32`    | `int`       |
| `int64`    | `long`      |
| `float32`  | `float`     |
| `float64`  | `double`    |

Boolean maps to `bool`, string to `string`, bytes to `[ubyte]`. **Read the
generated `v2::IntWidth` / `v2::FloatWidth` variant names before writing the
match** — an earlier story found the plan's sketched names wrong twice, and
`ridl-backend-proto`'s `proto_scalar` is the working cross-reference for how
these fields are actually read.

Emit each struct as `table <Name>`, one field per member, `id = ordinal − 1`,
names through `ridl_ir::name::snake_case`, constraint comments on their own line
above the field. **A `fixed_layout` struct is emitted as a `table` like any
other** — do not branch on the flag. A `Reserved` member emits a placeholder
field at its slot marked `deprecated`; its declared type is inert because a
deprecated field generates no accessor, so `ubyte` is fine.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-flatbuffers --locked`, then
`cargo test --workspace --locked`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "feat(ridl-backend-flatbuffers): project structs as tables

Tier 1 begins. A struct becomes a FlatBuffers table with an explicit
id on every field, id = ordinal - 1, and a tombstone holding its slot as
a deprecated field so later ids do not move.

A fixed-layout struct is emitted as a table like any other. typl
Appendix D permits a FlatBuffers struct there, but appending a struct
field is a compatible change in typl, and after one the struct form
fabricates a value from padding while the table form reports the field
absent - so the struct form makes ADR-0016 decision 6 property 3
unsatisfiable, and silently.

Widths use the full uint8 to uint64 palette, which is the reason to
choose this target."
```

---

### Task 4: Tier 1 — enums, enum sets, and the enum-typed field rule

**Files:**

- Modify: `crates/ridl-backend-flatbuffers/src/lib.rs`, `src/tests.rs`

**Interfaces:**

- Consumes: everything from Task 3.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_enum_keeps_its_values_unprefixed_with_an_explicit_underlying_type() {
    let package = enum_package("GearPosition", vec![("PARK", 1), ("DRIVE", 2)]);
    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains(
        "enum GearPosition : long {\n  PARK = 1,\n  DRIVE = 2,\n}"
    ), "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_field_typed_by_an_enum_without_a_zero_member_takes_null() {
    // flatc: "default value of `0` for field `g` is not part of enum `Gear`".
    // FlatBuffers cannot mark a scalar or enum field (required) either, so
    // `= null` is the honest rendering — it never fabricates a reading.
    let package = enum_and_field("Gear", vec![("PARK", 1)], "g");
    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("g: Gear = null (id: 0);"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_field_typed_by_an_enum_declaring_zero_needs_no_null() {
    let package = enum_and_field("Mode", vec![("OFF", 0), ("ON", 1)], "m");
    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("m: Mode (id: 0);"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn an_enum_set_becomes_an_integer_with_its_bits_in_a_comment() {
    // A FlatBuffers enum field holds one value, so it cannot represent a
    // combination of bits. Emitting one would imply a guarantee the target
    // does not make (ADR-0013 decision 2).
    let package = enum_set_and_field("Warnings", vec![("LOW_FUEL", 0), ("DOOR_AJAR", 1)]);
    let generated = generate(&package).expect("generate");
    assert!(!generated.fbs_source.contains("enum Warnings"),
        "an enum set must not become a FlatBuffers enum:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("LOW_FUEL = bit 0"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}
```

Write `enum_package`, `enum_and_field` and `enum_set_and_field` as small fixture
builders over `v2::EnumDef`, `v2::EnumValue`, `v2::EnumSetDef` and
`v2::StructDef`. Confirm the field names against the generated types first.

The underlying type in the first test is `long` because typl enum values are
`int64` (`EnumValue.value` is `int64`). If you choose to narrow it to the
smallest type that fits the declared values, that is a defensible alternative —
but then say so in the report and adjust the test, because narrowing makes the
underlying type a function of the values, and a later value could widen it,
which is a wire change under a compatible edit. **The safe default is `long`.**

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL.

- [ ] **Step 3: Implement**

An enum emits `enum <Name> : long { ... }` with values unprefixed, in
declaration order. A retired enum value contributes nothing — FlatBuffers has no
`reserved` for enums.

An enum set emits **no declaration**; it resolves to the FlatBuffers scalar for
its width at each use site, with the bit names and positions as a comment.

An enum-typed field appends `= null` when the enum declares no zero-valued
member, and nothing otherwise. This applies whether or not the typl field is
optional, because FlatBuffers cannot express a required scalar or enum at all.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "feat(ridl-backend-flatbuffers): project enums and enum sets

A typl enum becomes a FlatBuffers enum with an explicit underlying type
and unprefixed values, because FlatBuffers scopes enum values to their
enum rather than to the namespace.

A field typed by an enum that declares no zero member takes = null.
FlatBuffers gives every table field a default and refuses one whose
implicit zero is not a member of the enum, and it cannot mark a scalar or
enum field required in any case, so = null is the rendering that never
fabricates a reading.

An enum set does not become a FlatBuffers enum: an enum field holds one
value and cannot represent a combination of bits."
```

---

### Task 5: Tier 1 — unions, arrays, maps, tuples, and the namespace guard

**Files:**

- Modify: `crates/ridl-backend-flatbuffers/src/lib.rs`, `src/tests.rs`

**Interfaces:**

- Consumes: everything above.
- Produces: `struct Namespace` — the name-collision guard described in Step 3.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_union_is_isolated_in_a_wrapper_table() {
    // A native union owns TWO id slots (a hidden _type plus the value), so
    // one in an ordinal-owned slot shifts every later id. The wrapper takes
    // one slot in the parent and keeps the union's two in its own id space.
    let package = union_and_field("Payload", vec![("speed", 1, "Speed"), ("gearIndex", 2, "GearIndex")]);
    let generated = generate(&package).expect("generate");

    assert!(generated.fbs_source.contains("union PayloadUnion { Speed, GearIndex }"),
        "got:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains(
        "table Payload {\n  value: PayloadUnion (id: 1);\n}"
    ), "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_union_field_takes_exactly_one_slot_in_its_parent() {
    let package = struct_with_union_between_scalars();
    let generated = generate(&package).expect("generate");
    // before: id 0, union: id 1, after: id 2 — the mapping is intact.
    assert!(generated.fbs_source.contains("after: long (id: 2);"),
        "the union must not consume its neighbour's slot:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn an_array_field_is_a_vector() {
    let package = struct_package("Trace", "samples", 1, array_of(float64_type()));
    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("samples: [double] (id: 0);"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_map_field_is_a_vector_of_entry_tables_with_no_key_attribute() {
    // FlatBuffers has no map. (key) is deliberately NOT emitted: it obliges
    // the producer to sort, and typl §12.2 gives a map no ordering.
    let package = struct_package("Index", "byName", 1,
        map_of(v2::PrimitiveType::String, float64_type()));
    let generated = generate(&package).expect("generate");

    assert!(generated.fbs_source.contains("table IndexByNameEntry {"),
        "got:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("by_name: [IndexByNameEntry] (id: 0);"),
        "got:\n{}", generated.fbs_source);
    assert!(!generated.fbs_source.contains("(key)"),
        "(key) must not be emitted:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_tuple_field_induces_a_positional_table() {
    let package = struct_package("Reading", "bounds", 1,
        tuple_of(vec![float64_type(), float64_type()]));
    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains(
        "table ReadingBounds {\n  field_1: double (id: 0);\n  field_2: double (id: 1);\n}"
    ), "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn a_generated_name_colliding_with_a_declared_type_is_refused() {
    // Wrapper tables and entry tables mint names into the namespace scope,
    // which FlatBuffers shares across tables, structs, enums and unions —
    // `flatc` rejects a repeat with "datatype already exists".
    //
    // Here a declared struct `ReadingBounds` collides with the table induced
    // by the tuple field `bounds` on struct `Reading`, whose generated name
    // is `<Owner><Field>` = `ReadingBounds`.
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![
            v2::Decl {
                name: "Reading".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member(
                        "bounds", 1,
                        tuple_of(vec![float64_type(), float64_type()]),
                    )],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "ReadingBounds".to_string(),
                kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![field_member("x", 1, float64_type())],
                    fixed_layout: false,
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("ReadingBounds"), "got: {}", error.message);
}
```

Write the fixture builders alongside. `struct_with_union_between_scalars` must
place a scalar at ordinal 1, a union-typed field at ordinal 2 and a scalar at
ordinal 3, so the middle test actually proves the slot arithmetic.
`declared_type_named` must construct a package where a declared type's name
equals a name the emitter will generate — pick the collision so it is genuine
and note which generated name it targets.

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL.

- [ ] **Step 3: Implement**

A union emits two declarations: `union <Name>Union { Arm, ... }` listing the arm
**types**, and `table <Name> { value: <Name>Union (id: 1); }`. The wrapper's
inner ids are 0 (the implicit `_type`) and 1 (the value); a union declared
`(id: N)` owns `N-1` and `N`, which is why the value takes id 1 and not 0. A
retired arm contributes nothing to the union list.

An array is `[T]`. A map is a vector of a generated entry table
`<Owner><Field>Entry { key: K; value: V; }` with **no `(key)`**. A tuple induces
`<Owner><Field> { field_1: T (id: 0); ... }`.

`Namespace` is the name guard ADR-0017 decision 4 requires, modelled on
**FlatBuffers'** scopes, not proto3's:

- **namespace scope** — every table, struct, enum and union name shares one
  space. Register declared names first, then every generated name (union
  wrappers, entry tables, induced tuples, identity-table enums). A repeat
  returns a `GenerateError` naming both sources.
- **table scope** — one table's field names.
- **enum scope** — one enum's value names. This is where FlatBuffers differs
  from proto3, which is why values are not prefixed.

Do **not** lift `ridl-backend-proto`'s `SymbolScope`: its package scope includes
enum values and would over-refuse here.

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-flatbuffers --locked`, then
`cargo test --workspace --locked`, then
`cargo clippy --workspace --all-targets -- -D warnings`. Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "feat(ridl-backend-flatbuffers): project unions, arrays, maps and tuples

A union is isolated in a wrapper table holding a native union. A native
union owns two id slots - a hidden discriminant plus the value - so one
placed in an ordinal-owned slot shifts every later id and flatc refuses
the schema. The wrapper takes one slot in the parent and keeps the two
inside its own id space, where contiguity binds nothing else.

A map is a vector of generated entry tables with no (key). The attribute
obliges the producer to write the vector sorted and nothing checks that
at read time, while typl section 12.2 gives a map no ordering at all.

The namespace guard models FlatBuffers' own scopes: type names share one
space, field names are per table, and enum values are per enum. The proto
backend's guard is not reusable, because proto3 scopes enum values as
namespace siblings and this target does not."
```

---

### Task 6: Cross-package references

**Files:**

- Modify: `crates/ridl-backend-flatbuffers/src/lib.rs`, `src/tests.rs`

**Interfaces:**

- Consumes: `ridl_ir::v2::referenced_packages` (in `crates/ridl-ir/src/lib.rs`
  around line 489) — reuse it, do not write a second traversal.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_foreign_named_scalar_inlines_with_no_include() {
    // ADR-0017 decision 1: a named scalar inlines whether local or foreign,
    // so it is never included — there is no declaration to include.
    let (cluster, common) = cross_package_scalar();
    let generated = generate_with(&cluster, &[&common]).expect("generate");
    assert!(!generated.fbs_source.contains("include"),
        "got:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("value: double (id: 0);"),
        "got:\n{}", generated.fbs_source);
    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn a_foreign_table_reference_emits_an_include_and_the_qualified_name() {
    let (cluster, common) = cross_package_struct();
    let common_out = generate(&common).expect("generate common");
    let generated = generate_with(&cluster, &[&common]).expect("generate");

    assert!(generated.fbs_source.contains("include \"veh.common.fbs\";"),
        "got:\n{}", generated.fbs_source);
    assert!(generated.fbs_source.contains("veh.common.Setpoint"),
        "got:\n{}", generated.fbs_source);

    // Compile the pair together so the include actually resolves.
    compile_with_planus_and_siblings(
        "veh.cluster.fbs", &generated.fbs_source,
        &[("veh.common.fbs", &common_out.fbs_source)],
    );
}

#[test]
fn an_unresolvable_foreign_reference_is_refused() {
    let cluster = cross_package_dangling();
    let error = generate(&cluster).expect_err("must refuse");
    assert!(error.message.contains("veh.other"), "got: {}", error.message);
}
```

Write `compile_with_planus_and_siblings` beside `compile_with_planus`: it writes
every sibling into the same temp directory before compiling the entry file, so
the `include` resolves. **Every test that produces a schema calls one of the
two.** The only test here without a validity check is the refusal case, which
produces no schema.

- [ ] **Step 2: Run and confirm they fail**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: FAIL.

- [ ] **Step 3: Implement**

Follow ADR-0017 decision 1 exactly, which `crates/ridl-backend-proto/src/lib.rs`
already implements for proto3 — read its `named_field_type` and its `Packages`
resolver and mirror the structure:

- a foreign **named scalar** or **enum set** inlines; no include;
- a foreign **struct**, **enum** or **union** emits `include "<package>.fbs";`
  and is referenced by its fully qualified name;
- an unresolvable foreign reference is a `GenerateError` naming it;
- includes are collected in a `BTreeSet<String>` so the block is deterministic,
  and emitted **before** the `namespace` line, which is what FlatBuffers
  requires.

`ridl.std` needs no special casing — every one of its members is a named scalar,
so they all inline (ADR-0017 decision 2).

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test -p ridl-backend-flatbuffers --locked` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "feat(ridl-backend-flatbuffers): resolve foreign references

ADR-0017 decision 1, inherited from the proto3 backend: a named scalar
and an enum set inline whether local or foreign, so neither is ever
included, and an include appears only for a foreign table, enum or union
- the kinds that emit a declaration another file can reference. An
unresolvable reference is refused rather than emitted.

ridl.std needs no special casing, because every one of its members is a
named scalar and inlines through the same path."
```

---

### Task 7: The cruise-control fixture and corpus validation

**Files:**

- Create: `crates/ridl-backend-flatbuffers/tests/corpus.rs`
- Create: `crates/ridl-backend-flatbuffers/tests/support/mod.rs`
- Create: `crates/ridl-backend-flatbuffers/tests/snapshots/` (insta writes here)
- Modify: `crates/ridl-backend-flatbuffers/Cargo.toml` — `ridlc` and `ridl-core`
  as dev-dependencies for the source-to-IR path
- Modify: `crates/ridlc/tests/cli.rs` — a `--emit flatbuffers` case

**Interfaces:**

- Consumes: the fixtures `crates/ridl-backend-proto/tests/fixtures/cruise.ridl`
  and `.../cross-package/`, which already exist and already compile clean.
- Produces: the golden snapshots.

- [ ] **Step 1: Write the failing test**

Reuse the **existing** proto fixtures rather than writing new ridl source — they
were built to exercise one of every construct and are already known good.
Reference them by relative path from this crate's `tests/`.

```rust
#[test]
fn the_cruise_control_package_emits_a_valid_flatbuffers_schema() {
    let package = compile_fixture("cruise.ridl");
    let generated = ridl_backend_flatbuffers::generate(&package).expect("generate");
    compile_with_planus("veh.cruise.fbs", &generated.fbs_source);
    insta::assert_snapshot!(generated.fbs_source);
}

#[test]
fn the_cross_package_workspace_emits_valid_flatbuffers_schemas() {
    let (parts, vehicle) = compile_cross_package_fixture();
    let parts_out = ridl_backend_flatbuffers::generate(&parts).expect("parts");
    let vehicle_out =
        ridl_backend_flatbuffers::generate_with(&vehicle, &[&parts]).expect("vehicle");
    compile_with_planus_and_siblings(
        "proto.vehicle.fbs", &vehicle_out.fbs_source,
        &[("proto.parts.fbs", &parts_out.fbs_source)],
    );
    insta::assert_snapshot!(vehicle_out.fbs_source);
}
```

Write `compile_fixture` and `compile_cross_package_fixture` over the existing
source-to-IR harness. **Read `crates/ridl-backend-proto/tests/corpus.rs` and its
`tests/support/mod.rs`** — they do exactly this for proto3, and copying their
structure is correct here.

- [ ] **Step 2: Run and confirm it fails**

Run: `cargo test -p ridl-backend-flatbuffers --locked --test corpus` Expected:
FAIL — no snapshot accepted.

- [ ] **Step 3: Add the `ridlc` CLI test**

Add a `--emit flatbuffers` case to `crates/ridlc/tests/cli.rs`, following the
pattern the `proto` and `typescript` cases already use there — regenerate the
expected output through the library path and assert byte equality, with a
non-vacuity guard.

- [ ] **Step 4: Review and accept the snapshots**

Run: `cargo insta review`

**Read every line before accepting.** Check specifically: every `id` is exactly
one below its typl ordinal; every tombstone emits a `deprecated` placeholder
holding its slot; no enum set became a FlatBuffers enum; no `struct` keyword
appears anywhere; no `(key)` appears anywhere; enum values are unprefixed; and
each union appears as a wrapper table whose `value` field is at `id: 1`.

- [ ] **Step 5: Run the full gate**

Run: `just verify` Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-backend-flatbuffers crates/ridlc/tests/cli.rs
git commit -m "test(ridl-backend-flatbuffers): the cruise-control package emits a valid schema

The story's acceptance criterion, over the fixtures E9.8 already built:
the cruise-control package and the two-package cross-package workspace.
Both are compiled with planus and their emitted text is snapshotted, and
the cross-package pair is compiled together so the include resolves."
```

---

### Task 8: The stability property, driven from the classifier

**Files:**

- Create: `crates/ridl-backend-flatbuffers/tests/stability.rs`
- Modify: `crates/ridl-backend-flatbuffers/Cargo.toml` — `proptest` and
  `ridl-diff` as dev-dependencies

**Interfaces:**

- Consumes: `ridl_diff::diff_packages`; `generate`.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

**Port the sibling that already exists**:
`crates/ridl-backend-proto/tests/stability.rs` implements this property against
the proto3 emitter, with a 14-mutation strategy taken from `ridl-diff`'s
compatible arms and a companion test asserting a 0 % discard rate. Read it and
adapt two things:

1. **The parser.** `assigned_numbers` there scans for `= <digits>;`. FlatBuffers
   assigns ids as `(id: N)`, so the scan changes accordingly — and it must still
   read the emitted **text**, never the emitter's own data, so an emitter bug
   cannot hide inside the check.
2. **The property statement.** Identical: where `diff_packages` returns
   compatible, no id already assigned may move.

Keep the companion discard-rate test. A property test that passes because every
case was discarded proves nothing.

- [ ] **Step 2: Run and confirm it fails**

Run: `cargo test -p ridl-backend-flatbuffers --locked --test stability`
Expected: FAIL — the file does not exist yet.

- [ ] **Step 3: Implement, then prove it has teeth**

After it passes, **temporarily sabotage the id assignment** — for example number
fields positionally instead of by ordinal — confirm the property fails and
shrinks to a named counterexample, then revert and confirm `git status` is
clean. Record the counterexample in the report. A property test nobody has seen
fail is not known to work.

- [ ] **Step 4: Run and confirm it passes**

Run: `cargo test -p ridl-backend-flatbuffers --locked --test stability`
Expected: PASS, with the discard rate reported as 0.

If it fails for real, **do not weaken it** — report the shrunk case. It means
either the projection moves an id under a compatible change or `ridl-diff` calls
something compatible that is not.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-flatbuffers
git commit -m "test(ridl-backend-flatbuffers): pin the stability property

ADR-0016 decision 6 property 3 over this target: where ridl-diff returns
compatible, no id already assigned may move. Driven from the classifier
rather than from hand-written deltas, with a companion test asserting
every generated delta really does classify compatible.

The check parses the emitted text rather than the emitter's own data, so
a bug in the emitter cannot hide itself in the assertion."
```

---

### Task 9: Documentation, the roadmap, and the three record amendments

**Files:**

- Create: `docs/decisions/ADR-0018-flatbuffers-projection-rules.md`
- Modify: `docs/specification/typl-language-reference.md` — Appendix D
- Modify: `docs/decisions/ADR-0013-codegen-backend-scope.md` — decision 6
- Modify: `docs/ROADMAP.md`, `docs/book/cli-reference.md`,
  `docs/book/getting-started.md`, `docs/book/introduction.md`, `AGENTS.md`,
  `README.md`, `CONTRIBUTING.md`, `docs/wip/README.md`

**Interfaces:** none.

- [ ] **Step 1: Mint ADR-0018**

Record the five decisions the design took, each with the measurement behind it:
the union wrapper (superseding the schema-projection note §4.4), the struct
remedy (amending typl Appendix D), the map remedy with no `(key)`, the
width-floor closure, and the FlatBuffers-specific name scopes. Follow
`ADR-0017`'s structure — Status, Context, Decision, Alternatives considered,
Consequences, Open, References — and cite the archived design note as the
reasoning trail.

- [ ] **Step 2: Amend typl Appendix D**

Its FlatBuffers paragraph says a fixed-layout struct "may be emitted as a
FlatBuffers `struct` (inline, zero indirection) instead of a `table`". Record
that this projection does not take that allowance, and why: after a compatible
append the struct form fabricates a value from padding. Keep the `fixed_layout`
flag's description — it stays in the IR for targets where a fixed layout is
safe.

- [ ] **Step 3: Amend ADR-0013 decision 6**

It requires typl §17.11's open question be closed "before a FlatBuffers backend
ships". Record that E9.9 closed it by decision — `ridl-diff` remains the sole
guard for v0.1 — with the measured cost of the alternative (always-widest is
2.2× on a table and 2.6× on a fixed-layout struct) and the forcing case that
reopens §17.11.

- [ ] **Step 4: The routine updates**

`AGENTS.md` and `README.md` say twelve crates; it is now thirteen. Add
`flatbuffers` to the documented `--emit` values in `docs/book/cli-reference.md`,
and to the emit table in `docs/book/getting-started.md` (which counts them — it
will say seven and must say eight) and the sentence in
`docs/book/introduction.md`. Add the crate's scope to `CONTRIBUTING.md`'s
enumerated list. Mark E9.9 landed in `docs/ROADMAP.md` with a status paragraph.

**E9.8 left all four of those book and CONTRIBUTING sites stale and they were
caught at gardening.** Check each by reading it, not by grep alone.

- [ ] **Step 5: Verify**

Run: `just check && just link-check && just book-check && just verify` Expected:
all exit 0.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "docs(docs): record the FlatBuffers backend

ADR-0018 records the five decisions the design took and the measurement
behind each. typl Appendix D gains the reason this projection declines
its fixed-layout-struct allowance. ADR-0013 decision 6's width-floor
precondition is recorded as closed by decision, with the measured cost of
the alternative and the case that reopens typl section 17.11.

The workspace gains a thirteenth crate, the CLI gains an emit value, and
the book's emit table goes from seven entries to eight."
```

---

## After the plan

Before opening the PR:

1. `just verify` exits 0.
2. Run the review over the branch and record findings as PR comments.
3. Garden `docs/wip/` — the design and plan pair is archived once the story
   lands, the way E9.7's and E9.8's were. The schema-projection note stays,
   because E9.10 and E9.11 still read it.

Carried forward, not this story's work:

- The ADR-0013 decision 2 versus ADR-0016 decision 10 conflict over the
  `service` block. Both records carry a Status note. **E9.11 cannot avoid it.**
- `(key)` and sorted-vector lookup, reopenable in E9.11 where a generated
  producer could hold the sortedness obligation.
