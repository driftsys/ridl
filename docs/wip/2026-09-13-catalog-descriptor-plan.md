# Catalog Descriptor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

## Re-baseline 2026-10

Stage D1 of lane E16
([`2026-10-03-lane-e16-catalog-descriptor-driver.md`](2026-10-03-lane-e16-catalog-descriptor-driver.md))
audited this plan against `main` at 440dfb59 on 2026-10-03 and amended it in
place. The task numbering is unchanged, so the issues' task references stay
valid. Each change below names the record or the driver §4 answer that caused
it.

- **Interface numbers are copied from the IR (Tasks 3, 8; §4 answer 3).** Epic
  15 landed: `ridl-sem` folds `interfaces.lock` into `Interface.number`,
  `Interface.provisional` and `Package.retired` (`number_interfaces` in
  `crates/ridl-sem/src/check.rs`). Task 3 no longer assigns numbers; it copies
  them, rejects a number of 0 as an internal error, and the descriptor's
  `retired` list is `Package.retired`.
- **The hash is over the protobuf binary of the reduced package, and its
  decision record comes first (Task 4; §4 answer 4; ADR-0014 decision 9 as
  amended 2026-09-22).** Canonical protobuf JSON is now the IR's canonical
  encoding; the hash departs from it on purpose, and Task 4 gains step 0, the
  decision record (the determinism rule, the reason, the golden-hash test). The
  numbering is no longer appended after the binary: the reduced package's
  interfaces carry `number` and `provisional` themselves. #275's criterion joins
  the tests (§4 answer 11).
- **Three size states, every size a `uint32` (Tasks 1, 2, 5, 8, 10; §4 answer
  5).** `MaxSize` carries `encoding`, `state` (bounded or unbounded),
  `bytes: uint32` and `cause` (the codegen model's `FbUnboundedCause`, same
  members and values). An encoding with no row is absent.
- **The descriptor defines no wire shape (Tasks 5 to 8; §4 answer 6).** The
  induced single-field message, the induced request message and the `ok`/`err`
  union are removed. Only a payload that is one named type is sized, through
  `ridl_ir::projection::flatbuffers::max_size` (ADR-0019 decision 8) and
  ADR-0017's projection. A request with zero or more than one parameter, an
  inline `T | E` reply and a stream payload have absent sizes.
- **No `match` narrowing (Task 5; §4 answer 7; #665).** `ascii_only` and its
  tests are removed; a string counts 4 bytes per scalar value.
- **Task 7 calls the projection's bound (E11.7 design D-6,
  `docs/design/flatbuffers-codec.md`).** The hand-rolled FlatBuffers charges
  written on 2026-09-13 are deleted; the stage K8 note of 2026-09-21 that said
  not to implement them is folded into the task's text. One ridl-ir change comes
  with it: the cause of a missing bound, computed in
  `crates/ridl-ir/src/codegen/unbounded.rs`, becomes public.
- **planus (Task 1; §4 answer 8).** The three planus crates are already
  workspace dependencies (root `Cargo.toml`); Task 1 narrows their comment and
  extends `xtask/tests/oracle_boundary.rs` instead of adding them.
- **Publishing (Task 1; §4 answer 9; ADR-0007 decision 14 as amended
  2026-09-21).** `ridl-descriptor` is a workspace dependency entry with a path
  and a version, declared `ridl-descriptor.workspace = true` by `ridlc` and
  `ridl`, and published after `ridl-ir` and before `ridlc` in
  `.github/workflows/crates-io-release.yml`.
- **No `stream` flag (Tasks 1, 2, 8, 10; §4 answer 10).** A stream payload's
  `type_name` is the spelled `<T>` and its sizes are absent. #336 appends the
  flag and the per-element bound.
- **IR facts that moved (Tasks 3 to 8).** `Package` has a fifth field,
  `retired`, so every `Package` literal takes `..Default::default()`;
  `ServiceShape` has no `id` field (field 1 is reserved); `Package::shapes()` is
  at `crates/ridl-ir/src/lib.rs:605` and `InterfaceShape` carries `name`,
  `interface`, `service` and a `visibility()` method; `ridl_ir::v2::to_binary`
  exists and is documented as a derived encoding.
- **CLI facts that moved (Tasks 9, 11).** `Emit` is at
  `crates/ridlc/src/lib.rs:359-436` with eight values (`codegen-model` is the
  eighth, so `catalog` is the ninth); `ir_dump_suffix` is at :484; `write_emits`
  is at :1454 and already receives `others`. `Command` in
  `crates/ridl/src/main.rs` is at :71-210 with nine variants (`lock`, `lsp` and
  `mcp` postdate the plan); the dispatch is at :252. The book census in Task 11
  step 5 is recounted against `docs/book/cli-reference.md` as of today and
  carries every item of #326.
- **Records (Task 12).** The crate count in `AGENTS.md` is nineteen, not
  thirteen. The roadmap has Epic 16 rows (E16.1 to E16.6) in place of the "Epic
  14" row the plan told Task 12 to add. The design's §7 items that §4 answers 1
  and 2 settle are disposed of in this re-baseline, not in Task 12. The book
  harness compiles `ridl`, `typl` and `rsdl` fences.
- **Dependencies that already exist.** `sha2`, `serde_json`, `insta`, `syn` and
  `prettyplease` are workspace dependencies; `xtask` already depends on `syn`
  and `prettyplease`; `crates/ridl` has no `insta` dev-dependency yet.
- **Left to later stages, on purpose.** The roadmap and
  `docs/design/interaction-face.md` attribute the Rust backend's `None` sizes to
  E16.2; D5 amends them to E16.4 (driver §3, D5). The port's catalog check and
  what `new` does on a mismatch are D3's (driver §4, last paragraph).

Decisions this re-baseline took that §4 does not settle, reported to Sebastien
in D1's final report:

1. The hash does not cover `Package.retired`: the reduced package clears it. The
   hash is over the interfaces, their numbers and the types they reach (rsdl
   note D-8); the retired list is carried beside it.
2. A reduced interface takes `InterfaceShape::visibility()`, so a declared
   interface and an inline shape hash alike (#326, third minor item).
3. A stream payload's `type_name` is the spelled `<T>`.
4. A request with zero parameters has absent sizes, like one with more than one:
   neither is one named type.
5. A proto3 state is bounded only for a struct or a union payload. ADR-0017
   decision 1 inlines a named scalar and rejects a wrapper message, so a named
   scalar, an enum or an enum set has no proto3 root form, and its proto3 state
   is absent until a record defines one.
6. The FlatBuffers cause comes from `ridl_ir::codegen::unbounded::attribute`,
   made public as `ridl_ir::codegen::fb_unbounded`, not from lowering the whole
   codegen model.
7. `lower` returns a `Result`; `ridlc` reports a zero interface number as an
   internal error with exit 2.

**Goal:** Emit the catalog descriptor of
`2026-09-13-runtime-descriptors-design.md` — a FlatBuffers file per package that
an engine reads without decoding — from `ridlc build --emit catalog`, verify it
before access, print it with `ridl describe`, and carry a size state per payload
for proto3 and FlatBuffers.

**Architecture:** A new crate `ridl-descriptor` holds the hand-written schema
`catalog.fbs`, the Rust accessors generated from it by
`cargo xtask descriptor-codegen` (planus, pure Rust, committed and drift-tested
like the AST), the lowering from the protobuf IR, the copy of the interface
numbers the IR carries, the catalog hash, the size states and the verifier.
`ridlc` gains the `catalog` emit; `ridl` gains the `describe` subcommand. The
system descriptor is out of scope (it waits for the rsdl lowering's descriptor
story).

**Tech Stack:** Rust 1.98.1 (edition 2024), `planus` 1.3.0 runtime,
`planus-translation` 1.3.0 + `planus-codegen` 1.3.0 in `xtask` only (all three
already in `[workspace.dependencies]`), `prost` IR types and
`projection::flatbuffers::max_size` from `ridl-ir`, `sha2` for the hash,
`serde_json` for `describe`, `insta` for snapshots.

**Spec:** `docs/wip/2026-09-13-runtime-descriptors-design.md` (D-1 to D-10, §3,
§4, §7); it cites `docs/wip/2026-09-12-rsdl-rewrite-decisions.md` D-7 and D-8,
`docs/decisions/ADR-0010-cli-conventions.md`,
`docs/decisions/ADR-0014-ir-encodings.md` decision 4,
`docs/decisions/ADR-0017-proto3-projection-rules.md`,
`docs/decisions/ADR-0019-flatbuffers-projection-rules.md`.

## Global Constraints

- Toolchain pin `channel = "1.98.1"` (`rust-toolchain.toml`); every cargo
  command runs `--locked`; `just build` is the gate (ADR-0009).
- A crate lives at `crates/<crate-name>/` and adds its own scope to
  `.git-std.toml` (AGENTS.md); commits are Conventional Commits with a scope
  from that list.
- No external code generator: the IR compiles through `protox`, so the
  descriptor accessors are generated by `cargo xtask descriptor-codegen` with
  planus and committed; a drift test fails when they are stale (the AST
  precedent in `xtask/src/codegen.rs`).
- The toolchain may depend on planus: `ridl-descriptor` on the `planus` runtime,
  `xtask` on `planus-translation` and `planus-codegen`, and `ridlc` and `ridl`
  through `ridl-descriptor`. `ridl-rt` and every generated package must not
  (driver §4 answer 8); `xtask/tests/oracle_boundary.rs` checks it.
- Every walk over a package's interfaces goes through `Package::shapes()`
  (`crates/ridl-ir/src/lib.rs:605`): a `service` with an inline body carries its
  `Interface` in its shape list, outside `Package.interfaces`, and the item
  `InterfaceShape<'_>` carries the identity `name` (the interface's own, or the
  owning service's dotted global name) beside `interface: &Interface`, with
  `visibility()` answering the owning service's visibility for an inline shape.
  The corpus fixture `baseline-corpus/cluster.ridl` has one of each, so it
  yields two shapes.
- The interface numbers, the provisional flags and the retired list are the IR's
  (`Interface.number`, `Interface.provisional`, `Package.retired`, folded from
  `interfaces.lock` by `ridl-sem`). The descriptor copies them and computes no
  numbering (driver §4 answer 3).
- Every size is a `uint32`, as in `ridl_rt::contract::EncodedSizes` and the
  codegen model (driver §4 answer 5).
- `ridl-descriptor` is published: a `[workspace.dependencies]` entry with a path
  and a version, no `publish = false`, published after `ridl-ir` and before
  `ridlc` (driver §4 answer 9; ADR-0007 decision 14).
- `ridl-descriptor` joins the `wasm-check` list in `justfile` and must compile
  for `wasm32-unknown-unknown` with `--no-default-features`: no file I/O inside
  the crate; `ridlc` and `ridl` do the reading and writing.
- Schema evolution is append-only: fields are only ever added at the end of a
  table, never removed or reordered (spec D-3).
- Every reader checks the file identifier and the version before the first field
  read, and rejects the buffer as a whole (spec D-8); the toolchain reports a
  rejection with exit code 2 and the cause named (ADR-0010 decision 1).
- Prose in comments, commit messages and docs is plain and literal (AGENTS.md).
- Book fences: a fence whose language word is `ridl`, `typl` or `rsdl` is
  compiled by `crates/ridl/tests/book_examples.rs`; a `json` transcript fence is
  not checked (CONTRIBUTING.md).

## Dispositions of the spec's open items, settled 2026-10-03

Sebastien settled these on 2026-10-03 (driver §4). State them in the PR
description of the stage that applies each; none is open.

- **Crate (answer 1):** `ridl-descriptor` beside `ridl-ir`, not inside it, so
  only the crates that read or write a catalog take a FlatBuffers dependency. It
  uses `ridl-ir`'s IR types and `ridl_ir::projection::flatbuffers::max_size`,
  and copies neither.
- **File identifier and extension (answer 2):** `file_identifier "RDLC"`; the
  emit flag value is `catalog` and the artifact is `<base>.catalog.binfb`,
  following ADR-0014 decision 4 (plain-English flag value, encoding-bearing
  extension, as `.ir.binpb`).
- **Interface numbers (answer 3):** the descriptor copies `Interface.number`,
  `Interface.provisional` and `Package.retired` from the IR and computes no
  numbering. `--emit catalog` writes a catalog when an interface is provisional,
  and the descriptor flags that interface (spec D-4). A number of 0 is rejected
  as an internal error. The hash covers the numbers, so it changes when a
  provisional number changes.
- **Catalog hash (answer 4):** SHA-256 over the protobuf binary of a reduced
  package (every interface shape under its identity name with its number and
  provisional flag, the reachable type declarations with canonical names, doc
  strings blanked), not over the canonical JSON. The decision record (an
  ADR-0014 amendment or a new ADR) is written before the code, with the
  determinism rule for the binary — fields in field-number order, list elements
  in the order the writer holds them, no `map<>` field (the IR schema has none),
  fields at their default omitted — the reason (canonical JSON emits every
  non-`optional` field at its default, so each additive IR field would change
  every catalog hash at a toolchain upgrade, and the read-back bound behind
  ADR-0014 decision 9's amendment does not apply to bytes that are only hashed),
  and a golden-hash test in CI. Derived, never recorded (rsdl note D-7, D-8).
- **Sizes (answer 5):** each payload and encoding has one of three states:
  absent (no row), bounded with a byte count, or unbounded with a cause (the
  codegen model's `FbUnboundedCause`). Every size is a `uint32`. The `repr(C)`
  column is present in the `Encoding` enum and has no row until E11.12 (#317).
- **Payload shapes follow the codecs (answer 6):** the descriptor does not
  define a wire shape. It sizes a payload that is one named type through the
  existing projections: ADR-0019 decision 8 for FlatBuffers, and ADR-0017's
  projection of the same type for proto3. A request with zero or more than one
  parameter and an inline `T | E` reply have absent sizes until the frame
  specification and a codec define their encoding.
- **No `match` narrowing (answer 7):** a string counts 4 bytes per scalar value
  in both columns, as `max_size` and §3.11 of
  `2026-09-12-release-scope-and-plugin-system-design.md` do. The checker and the
  generated Rust code do not agree on what a pattern matches until the design of
  `2026-10-01-portable-match-patterns-design.md` (approach A, #597) is
  implemented; #665 records the narrowing, to be built once, in one function
  that both bounds call.
- **planus (answer 8):** the toolchain may depend on planus; `ridl-rt` and every
  generated package must not. The root `Cargo.toml` comment says that, and
  `xtask/tests/oracle_boundary.rs` checks `ridl-rt` and the dependency closure
  of the generated crate.
- **Publishing (answer 9):** `ridl-descriptor` is published to crates.io, after
  `ridl-ir` and before `ridlc`.
- **Streams (answer 10):** a stream payload has absent sizes and the descriptor
  has no `stream` flag. #336 adds the per-element bound and the flag, as an
  append to the schema.
- **#275 (answer 11):** one identity. The catalog hash is the schema hash over
  the IR; the hash is the same whether a build emits proto3, FlatBuffers or
  both. #275 closes when #378 merges.

## File Structure

```
crates/ridl-descriptor/
  Cargo.toml                  crate manifest: planus, ridl-ir, sha2, serde_json
  schema/catalog.fbs          the hand-written schema (spec D-3, D-4, D-6)
  src/lib.rs                  constants, re-exports, `verify`, `VerifyError`
  src/generated.rs            planus output — never edited by hand
  src/number.rs               `numbered_shapes` — the IR's numbers, copied (§4 answer 3)
  src/hash.rs                 `reachable_decls`, `reduced_package`, `catalog_hash` (rsdl D-8)
  src/size.rs                 `Ctx`, `PayloadShape`, `named_payload`, `SizeState`, `size_state`
  src/size/proto3.rs          proto3 state of a named-type payload (ADR-0017 projection)
  src/size/flatbuffers.rs     FlatBuffers state, through `ridl_ir::projection::flatbuffers::max_size`
  src/lower.rs                IR → `Catalog` → finished bytes (spec D-4)
  src/describe.rs             `CatalogRef` → strict JSON (spec D-9)
  tests/round_trip.rs         builder → verify → read every field
xtask/src/descriptor.rs       `generate`, `write_generated`, drift test
xtask/src/main.rs             the `descriptor-codegen` task
xtask/tests/oracle_boundary.rs   the planus boundary for `ridl-rt` and the generated crate
crates/ridl-ir/src/codegen.rs     `pub fn fb_unbounded` (Task 7)
crates/ridlc/src/lib.rs       `Emit::Catalog`, the `write_emits` arm
crates/ridl/src/main.rs       `Command::Describe`, `run_describe`
crates/ridl/tests/describe_cli.rs   `--emit catalog` writes `<base>.catalog.binfb`;
                                    exit codes, JSON snapshot, byte stability
docs/decisions/                the hash decision record (Task 4 step 0);
                               ADR-0010's `ridl describe` exit-code row
docs/book/cli-reference.md, docs/book/getting-started.md
                              `--emit catalog`, `ridl describe`, the census
AGENTS.md, README.md, docs/technotes/walking-skeleton-architecture.md,
.git-std.toml, Cargo.toml, justfile, .github/workflows/crates-io-release.yml
```

Types that cross task boundaries (defined once, used verbatim later):

```rust
// ridl_descriptor (lib.rs)
pub const SCHEMA_VERSION: u32 = 1;
pub const FILE_IDENTIFIER: [u8; 4] = *b"RDLC";
pub const FILE_SUFFIX: &str = ".catalog.binfb";
pub use generated::ridl::descriptor::{
    Catalog, CatalogRef, Encoding, Interface, InterfaceRef, Kind, MaxSize, MaxSizeRef,
    Member, MemberRef, Payload, PayloadRef, RetiredInterface, RetiredInterfaceRef,
    SizeState as SizeStateTag, Timing, TimingMode, TimingRef, UnboundedCause,
};
pub enum VerifyError { TooShort(usize), WrongIdentifier([u8; 4]), WrongVersion(u32), Invalid(planus::Error) }
pub fn verify(bytes: &[u8]) -> Result<CatalogRef<'_>, VerifyError>;
// ridl_descriptor::number
pub struct Numbered { pub name: String, pub number: u32, pub provisional: bool }
pub struct ZeroNumber(pub String);
pub fn numbered_shapes(package: &ridl_ir::v2::Package) -> Result<Vec<Numbered>, ZeroNumber>;
// ridl_descriptor::hash
pub fn reachable_decls<'a>(package: &'a Package, others: &[&'a Package]) -> BTreeMap<String, &'a Decl>;
pub fn reduced_package(package: &Package, others: &[&Package]) -> Package;
pub fn catalog_hash(package: &Package, others: &[&Package]) -> [u8; 32];
// ridl_descriptor::size
pub struct Ctx<'a> { /* private: wraps ridl_ir::projection::flatbuffers::Packages<'a> */ }
impl<'a> Ctx<'a> { pub fn new(package: &'a Package, others: &'a [&'a Package]) -> Self; pub fn resolve(&self, name: &str) -> Option<&'a Decl>; pub fn packages(&self) -> Packages<'a>; }
pub enum PayloadShape<'a> { Named(&'a str), Field(&'a FieldType), Params(&'a [Param]), Return(&'a ReturnType) }
pub fn named_payload<'a>(shape: &PayloadShape<'a>) -> Option<&'a str>;
pub enum SizeState { Bounded(u32), Unbounded(UnboundedCause) }
pub fn size_state(type_name: &str, ctx: &Ctx<'_>, encoding: Encoding) -> Option<SizeState>;
pub fn string_max_bytes(constraint: Option<&Constraint>) -> u64;
// ridl_descriptor::lower
pub enum LowerError { ZeroNumber(String) }
pub fn lower(package: &Package, others: &[&Package]) -> Result<Vec<u8>, LowerError>;
// ridl_descriptor::describe
pub fn to_json(catalog: CatalogRef<'_>) -> planus::Result<serde_json::Value>;
```

---

### Task 1: The crate, the schema, and the generated accessors

**Files:**

- Create: `crates/ridl-descriptor/Cargo.toml`
- Create: `crates/ridl-descriptor/schema/catalog.fbs`
- Create: `crates/ridl-descriptor/src/lib.rs`
- Create: `crates/ridl-descriptor/src/generated.rs` (by the xtask, then
  committed)
- Create: `xtask/src/descriptor.rs`
- Modify: `xtask/src/main.rs:13-24` (the task match)
- Modify: `xtask/Cargo.toml` (dependencies)
- Modify: `Cargo.toml` (`[workspace.dependencies]`: the `ridl-descriptor` entry;
  the planus comment block, about lines 55-64)
- Modify: `.git-std.toml:13-66` (scope `ridl-descriptor`)
- Modify: `justfile:152` (the `wasm-check` crate list)
- Modify: `xtask/tests/oracle_boundary.rs` (the planus boundary, §4 answer 8)
- Modify: `.github/workflows/crates-io-release.yml` (the publish order, §4
  answer 9)
- Test: `crates/ridl-descriptor/tests/round_trip.rs`

**Interfaces:**

- Produces: the module `ridl_descriptor::generated::ridl::descriptor` with the
  owned tables `Catalog`, `Interface`, `RetiredInterface`, `Member`, `Payload`,
  `MaxSize`, `Timing`, the zero-copy views `CatalogRef<'a>` … `TimingRef<'a>`,
  the enums `Kind`, `Encoding`, `SizeState`, `UnboundedCause`, `TimingMode`; the
  constants `SCHEMA_VERSION`, `FILE_IDENTIFIER`, `FILE_SUFFIX`.

- [ ] **Step 1: The workspace entry, the planus boundary, the scope, the gates**

`planus`, `planus-codegen` and `planus-translation` are already in
`[workspace.dependencies]` of the root `Cargo.toml`, all at 1.3.0; add nothing.
Rewrite the comment block above them (it says both are dev-dependencies of
`ridl-backend-rust` only and that nothing the workspace ships depends on either)
to what §4 answer 8 decided: the toolchain may depend on planus —
`ridl-descriptor` on the runtime, `xtask` on `planus-translation` and
`planus-codegen` to generate the accessors, `ridlc` and `ridl` through
`ridl-descriptor` — and `ridl-rt` and every generated package must not. Keep the
sentence that ADR-0020 decision 5 permits one FlatBuffers runtime crate in
`ridl-rt` under its `flatbuffers` feature, and say that nothing uses that
permission today. Keep the E11.7 D-12 citation for `ridl-backend-rust`'s
dev-dependency.

In the same table, add the workspace entry, in the same form as `ridl-ir`'s
(`ridl-ir = { path = "crates/ridl-ir", version = "<workspace version>" }`):

```toml
ridl-descriptor = { path = "crates/ridl-descriptor", version = "<the version ridl-ir's entry carries>" }
```

In `xtask/tests/oracle_boundary.rs`, extend the `BOUNDARIES` table with the
edges answer 8 forbids: (`ridl-rt`, `planus`), (`ridl-rt`, `planus-codegen`),
(`ridl-rt`, `planus-translation`) — and these three are forbidden as any edge
kind, dev included, because `ridl-rt` is what a generated package links. Then
add a second check over the generated crate's dependency closure:
`examples/cabin` is its own cargo workspace (AGENTS.md), so run
`cargo metadata --format-version 1 --locked` with its manifest path (after
`just demo` has generated the crate, or over the committed consumer manifest if
no generation is needed to resolve) and assert no package named `planus`,
`planus-codegen` or `planus-translation` is in the resolve graph. Read the
file's existing rationale comment first and extend it; the file's shape is the
one to keep. If the closure check cannot run inside `cargo test` without a
generated crate, make it a `just` recipe member of `demo` instead and say so in
the test file's comment — do not drop it.

In `.git-std.toml`, add `"ridl-descriptor",` to `scopes` after `"ridl-ir",`. In
`justfile`, in the `wasm-check` recipe, add `-p ridl-descriptor` after
`-p ridl-ir` on the first `cargo check` line. In
`.github/workflows/crates-io-release.yml`, add `publish ridl-descriptor` after
`publish ridl-ir` and extend the dependency-order comment above the list with
one sentence: `ridl-descriptor` depends on `ridl-ir` only, and `ridlc` depends
on it.

- [ ] **Step 2: Write the crate manifest**

`crates/ridl-descriptor/Cargo.toml` (compare with `crates/ridl-ir/Cargo.toml`
for the fields the published crates share — `rust-version`, `readme`, keywords —
and copy what is there; no `publish = false`):

```toml
[package]
name = "ridl-descriptor"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "The catalog descriptor an engine reads: schema, accessors, lowering, sizes, verifier"

[dependencies]
# The FlatBuffers runtime the generated accessors call into. The schema
# compiler (planus-translation, planus-codegen) is an xtask dependency only:
# the accessors are generated by `cargo xtask descriptor-codegen` and committed.
planus.workspace = true
ridl-ir.workspace = true
serde_json.workspace = true
sha2.workspace = true
```

- [ ] **Step 3: Write the schema**

`crates/ridl-descriptor/schema/catalog.fbs`:

```fbs
// The catalog descriptor: what a catalog contains and how it is numbered
// (docs/wip/2026-09-13-runtime-descriptors-design.md, D-3, D-4, D-6).
//
// Evolution is append-only. A field is only ever added at the END of its
// table, so every field id already written keeps its meaning. Never remove,
// reorder or retype a field; never renumber an enum member. A reader built
// against an older schema ignores fields it does not know.
//
// Regenerate the Rust accessors after every edit: `cargo xtask descriptor-codegen`.

namespace ridl.descriptor;

// The five interaction kinds: the table under ridl §3, "The Interaction Model".
enum Kind : ubyte { Signal = 0, Event = 1, Command = 2, Query = 3, Fixed = 4 }

// The core encodings: proto3 and FlatBuffers (ADR-0018 decision 3) and
// repr(C) (ADR-0020 decision 1, via ADR-0018's 2026-09-12 amendment). A new
// encoding is appended.
enum Encoding : ubyte { Proto3 = 0, FlatBuffers = 1, ReprC = 2 }

// ridl §9 timing, as the IR carries it (ridl.ir.v2.TimingMode).
enum TimingMode : ubyte { Unspecified = 0, StrictPeriodic = 1, Range = 2 }

// Whether a row's payload has a finite bound under its encoding.
enum SizeState : ubyte { Bounded = 0, Unbounded = 1 }

// Why a payload has no finite bound: the codegen model's FbUnboundedCause
// (ridl.codegen.v1), same members, same values. A new cause is appended
// there and here together.
enum UnboundedCause : ubyte { Unspecified = 0, Member = 1, Untyped = 2, Layout = 3, Aggregate = 4, Exempt = 5 }

// One row of the size table: the state of a payload under one encoding
// (D-6, D-7). An encoding with no row is absent: this toolchain version
// computed no state for it. Bounded: `bytes` is the maximum encoded size,
// envelope and framing excluded. Unbounded: `bytes` is 0 and `cause` says why.
table MaxSize {
  encoding: Encoding;
  bytes: uint32;
  state: SizeState = Bounded;
  cause: UnboundedCause = Unspecified;
}

// One payload of an interaction (D-6): a signal, an event and a fixed have
// one ("value" / "occurrence" / "value"); a command has one ("request");
// a query has two ("request", "response").
table Payload {
  role: string (required);
  // The canonical type name of a payload that is one named type; otherwise
  // a spelling of the shape: `<T>` for a stream, `(a: T, b: U)` for a
  // request of several parameters, `T | E` for a fallible reply.
  type_name: string (required);
  // One row per encoding that has a state; empty when every encoding is
  // absent (a stream, a request of zero or several parameters, a `T | E`
  // reply, until a record defines their encoding).
  max_sizes: [MaxSize] (required);
}

table Timing {
  mode: TimingMode;
  min_us: string;
  max_us: string;
}

table Member {
  name: string (required);
  // Position in the interface body, one sequence regardless of kind (ridl §11).
  ordinal: uint32;
  kind: Kind;
  payloads: [Payload] (required);
  // Absent when the interaction declares none (a fixed never does).
  timing: Timing;
}

table Interface {
  name: string (required);
  // The interface number, copied from the IR's Interface.number, which
  // ridl-sem folds from interfaces.lock (rsdl note D-7). Never 0.
  number: uint32;
  // Copied from the IR's Interface.provisional: the number is not frozen by
  // a lock yet; `ridl baseline` refuses it, an engine decides (D-9, §3).
  provisional: bool = false;
  members: [Member] (required);
  // Ordinals held by a `reserved` tombstone, so the ordinal space is complete.
  reserved_ordinals: [uint32] (required);
}

// A retired interface, copied from the IR's Package.retired: name and number
// held forever, so an engine can refuse a peer that still speaks it (D-4).
table RetiredInterface {
  name: string (required);
  number: uint32;
}

table Catalog {
  // The schema version of this file; a reader rejects one it does not know.
  version: uint32;
  // The package name (vocabulary note V-16).
  name: string (required);
  // SHA-256 over the interfaces, their numbers and the types they reach
  // (rsdl note D-8); 32 bytes.
  hash: [ubyte] (required);
  // The toolchain version that wrote the file.
  toolchain: string (required);
  interfaces: [Interface] (required);
  retired: [RetiredInterface] (required);
}

root_type Catalog;
file_identifier "RDLC";
file_extension "binfb";
```

- [ ] **Step 4: Write the xtask generator**

`xtask/Cargo.toml` `[dependencies]` gains (`syn` and `prettyplease` are already
there):

```toml
planus-codegen.workspace = true
planus-translation.workspace = true
```

`xtask/src/descriptor.rs`:

```rust
//! Generates the catalog-descriptor accessors
//! (`crates/ridl-descriptor/src/generated.rs`) from
//! `crates/ridl-descriptor/schema/catalog.fbs` with planus, the pure-Rust
//! FlatBuffers toolchain. No `flatc` binary is involved, for the same reason
//! the IR compiles through `protox`: the build must not depend on a tool the
//! toolchain pin does not cover (ADR-0009).
//!
//! The drift test below fails whenever the committed output is stale, so the
//! accessors can never silently diverge from the schema.

use std::fs;
use std::path::PathBuf;

/// The header the generated file starts with. The lints are allowed because
/// the output is planus's, not ours, and `-D warnings` must not turn a
/// generator style change into a build failure.
const HEADER: &str = "//! Generated by `cargo xtask descriptor-codegen` from \
`crates/ridl-descriptor/schema/catalog.fbs`. Do not edit.\n\
#![allow(clippy::all, dead_code, unused_imports, missing_docs)]\n\n";

fn descriptor_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/ridl-descriptor")
        .canonicalize()
        .expect("the ridl-descriptor crate directory exists")
}

pub(crate) fn schema_path() -> PathBuf {
    descriptor_crate_dir().join("schema/catalog.fbs")
}

pub(crate) fn generated_path() -> PathBuf {
    descriptor_crate_dir().join("src/generated.rs")
}

/// Compiles the schema and returns the formatted Rust source.
pub(crate) fn generate() -> String {
    let declarations = planus_translation::translate_files(&[schema_path()])
        .expect("catalog.fbs is a valid FlatBuffers schema (planus printed why not)");
    let raw = planus_codegen::generate_rust(&declarations, false)
        .expect("planus generates Rust for catalog.fbs");
    let file = syn::parse_file(&raw).expect("planus output is valid Rust");
    let mut out = String::from(HEADER);
    out.push_str(&prettyplease::unparse(&file));
    out
}

pub(crate) fn write_generated() -> PathBuf {
    let path = generated_path();
    fs::write(&path, generate()).expect("write generated.rs");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed accessors must match what the schema produces.
    #[test]
    fn committed_generated_accessors_match_the_schema() {
        let fresh = generate();
        let committed = fs::read_to_string(generated_path()).unwrap_or_default();
        assert!(
            fresh == committed,
            "crates/ridl-descriptor/src/generated.rs is stale — run `cargo xtask descriptor-codegen`",
        );
    }
}
```

`xtask/src/main.rs`: add `mod descriptor;` after `mod codegen;`, extend the
module doc comment's task list, and add the arm:

```rust
Some("descriptor-codegen") => {
    let path = descriptor::write_generated();
    println!("wrote {}", path.display());
    ExitCode::SUCCESS
}
```

and change the usage line to
`eprintln!("usage: cargo xtask <codegen|descriptor-codegen>");`.

- [ ] **Step 5: Write the library root, with a temporary empty generated file**

`crates/ridl-descriptor/src/lib.rs`:

```rust
//! The catalog descriptor: the FlatBuffers file per package that an engine
//! reads without decoding (`docs/wip/2026-09-13-runtime-descriptors-design.md`).
//!
//! `schema/catalog.fbs` is the schema; `generated.rs` holds the accessors
//! planus generates from it (`cargo xtask descriptor-codegen`). This crate
//! does no I/O: `ridlc` writes the bytes `lower` returns, and `ridl describe`
//! hands the bytes it read to `verify`.

pub mod generated;

pub use generated::ridl::descriptor::{
    Catalog, CatalogRef, Encoding, Interface, InterfaceRef, Kind, MaxSize, MaxSizeRef,
    Member, MemberRef, Payload, PayloadRef, RetiredInterface, RetiredInterfaceRef,
    Timing, TimingMode, TimingRef,
};

/// The schema version this toolchain writes and accepts.
pub const SCHEMA_VERSION: u32 = 1;

/// The FlatBuffers file identifier, at bytes 4..8 of every catalog descriptor.
pub const FILE_IDENTIFIER: [u8; 4] = *b"RDLC";

/// The artifact suffix: `<base>.catalog.binfb` (ADR-0014 decision 4's
/// convention — a plain-English flag value, an encoding-bearing extension).
pub const FILE_SUFFIX: &str = ".catalog.binfb";
```

Create `crates/ridl-descriptor/src/generated.rs` with the single line
`//! placeholder, replaced by cargo xtask descriptor-codegen` so the workspace
resolves, then run the generator:

Run: `cargo xtask descriptor-codegen` Expected:
`wrote /.../crates/ridl-descriptor/src/generated.rs`, and
`grep -c 'pub struct CatalogRef' crates/ridl-descriptor/src/generated.rs` prints
`1`.

- [ ] **Step 6: Write the failing round-trip test**

`crates/ridl-descriptor/tests/round_trip.rs`:

```rust
//! Spec §4 "Schema round trip": build a descriptor with the Rust builder,
//! finish it with the file identifier, read every field back.

use planus::ReadAsRoot;
use ridl_descriptor::{
    Catalog, CatalogRef, Encoding, Interface, Kind, MaxSize, Member, Payload,
    RetiredInterface, SizeStateTag, Timing, TimingMode, UnboundedCause, FILE_IDENTIFIER,
    SCHEMA_VERSION,
};

fn sample() -> Catalog {
    Catalog {
        version: SCHEMA_VERSION,
        name: "veh.cluster".to_owned(),
        hash: vec![7u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![Interface {
            name: "VehicleStatus".to_owned(),
            number: 1,
            provisional: true,
            members: vec![Member {
                name: "currentSpeed".to_owned(),
                ordinal: 1,
                kind: Kind::Signal,
                payloads: vec![Payload {
                    role: "value".to_owned(),
                    type_name: "Speed".to_owned(),
                    max_sizes: vec![
                        MaxSize {
                            encoding: Encoding::Proto3,
                            bytes: 0,
                            state: SizeStateTag::Unbounded,
                            cause: UnboundedCause::Member,
                        },
                        MaxSize {
                            encoding: Encoding::FlatBuffers,
                            bytes: 40,
                            state: SizeStateTag::Bounded,
                            cause: UnboundedCause::Unspecified,
                        },
                    ],
                }],
                timing: Some(Box::new(Timing {
                    mode: TimingMode::StrictPeriodic,
                    min_us: Some("100000".to_owned()),
                    max_us: None,
                })),
            }],
            reserved_ordinals: vec![5],
        }],
        retired: vec![RetiredInterface { name: "LaneAssist".to_owned(), number: 2 }],
    }
}

#[test]
fn every_field_reads_back() {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(&sample(), Some(FILE_IDENTIFIER)).to_vec();

    assert_eq!(&bytes[4..8], &FILE_IDENTIFIER);
    let catalog = CatalogRef::read_as_root(&bytes).expect("a finished buffer reads");
    assert_eq!(catalog.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.name().unwrap(), "veh.cluster");
    assert_eq!(catalog.hash().unwrap().len(), 32);
    assert_eq!(catalog.toolchain().unwrap(), "0.0.0");

    let interfaces = catalog.interfaces().unwrap();
    assert_eq!(interfaces.len(), 1);
    let interface = interfaces.get(0).unwrap();
    assert_eq!(interface.name().unwrap(), "VehicleStatus");
    assert_eq!(interface.number().unwrap(), 1);
    assert!(interface.provisional().unwrap());
    assert_eq!(interface.reserved_ordinals().unwrap().iter().collect::<Vec<u32>>(), vec![5]);

    let member = interface.members().unwrap().get(0).unwrap();
    assert_eq!(member.name().unwrap(), "currentSpeed");
    assert_eq!(member.ordinal().unwrap(), 1);
    assert_eq!(member.kind().unwrap(), Kind::Signal);
    let timing = member.timing().unwrap().expect("timing is present");
    assert_eq!(timing.mode().unwrap(), TimingMode::StrictPeriodic);
    assert_eq!(timing.min_us().unwrap(), Some("100000"));
    assert_eq!(timing.max_us().unwrap(), None);

    let payload = member.payloads().unwrap().get(0).unwrap();
    assert_eq!(payload.role().unwrap(), "value");
    assert_eq!(payload.type_name().unwrap(), "Speed");
    let sizes = payload.max_sizes().unwrap();
    assert_eq!(sizes.get(0).unwrap().state().unwrap(), SizeStateTag::Unbounded);
    assert_eq!(sizes.get(0).unwrap().cause().unwrap(), UnboundedCause::Member);
    assert_eq!(sizes.get(1).unwrap().encoding().unwrap(), Encoding::FlatBuffers);
    assert_eq!(sizes.get(1).unwrap().state().unwrap(), SizeStateTag::Bounded);
    assert_eq!(sizes.get(1).unwrap().bytes().unwrap(), 40);

    let retired = catalog.retired().unwrap().get(0).unwrap();
    assert_eq!(retired.name().unwrap(), "LaneAssist");
    assert_eq!(retired.number().unwrap(), 2);
}

#[test]
fn the_owned_form_round_trips_through_the_view() {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(&sample(), Some(FILE_IDENTIFIER)).to_vec();
    let view = CatalogRef::read_as_root(&bytes).unwrap();
    let owned: Catalog = view.try_into().expect("a valid view converts");
    assert_eq!(owned.interfaces[0].members[0].payloads[0].max_sizes[1].bytes, 40);
}
```

If the generated `Timing` field in `Member` is `Option<Timing>` rather than
`Option<Box<Timing>>`, drop the `Box::new` — read the generated struct, the
owned form is whatever planus wrote.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p ridl-descriptor --locked` Expected: both tests PASS. Run:
`cargo test -p xtask --locked` —
`committed_generated_accessors_match_the_schema` PASS, and the extended
`oracle_boundary` test PASS.

- [ ] **Step 8: Run the wasm, lint and publish gates for the new crate**

Run:
`just wasm-check && cargo clippy -p ridl-descriptor -p xtask --all-targets -- -D warnings && cargo fmt --all --check && cargo publish -p ridl-descriptor --dry-run --locked`
Expected: all four succeed; the dry run is what ADR-0007 decision 14's
workspace-entry rule exists for. If clippy reports inside `generated.rs`, extend
the `#![allow(...)]` list in `HEADER` and regenerate; do not edit the generated
file.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock .git-std.toml justfile .github/workflows/crates-io-release.yml xtask crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): add the catalog descriptor schema and its generated accessors"
```

---

### Task 2: The verifier

**Files:**

- Modify: `crates/ridl-descriptor/src/lib.rs`
- Test: `crates/ridl-descriptor/tests/verify.rs`

**Interfaces:**

- Produces: `pub enum VerifyError`,
  `pub fn verify(bytes: &[u8]) -> Result<CatalogRef<'_>, VerifyError>`. Every
  later reader (`ridl describe`, tests) calls `verify`, never
  `CatalogRef::read_as_root` directly.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-descriptor/tests/verify.rs`:

```rust
//! Spec D-8: every reader checks the identifier and the version first, then
//! walks the whole buffer through checked accessors; a buffer that fails is
//! rejected as a whole.

use ridl_descriptor::{verify, Catalog, VerifyError, FILE_IDENTIFIER, SCHEMA_VERSION};

fn minimal(version: u32) -> Vec<u8> {
    let catalog = Catalog {
        version,
        name: "p".to_owned(),
        hash: vec![0u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![],
        retired: vec![],
    };
    let mut builder = planus::Builder::new();
    builder.finish(&catalog, Some(FILE_IDENTIFIER)).to_vec()
}

#[test]
fn a_finished_buffer_verifies() {
    let bytes = minimal(SCHEMA_VERSION);
    let catalog = verify(&bytes).expect("verifies");
    assert_eq!(catalog.name().unwrap(), "p");
}

#[test]
fn a_buffer_shorter_than_the_header_is_too_short() {
    assert!(matches!(verify(&[0, 0, 0]), Err(VerifyError::TooShort(3))));
}

#[test]
fn a_foreign_identifier_is_rejected_before_any_read() {
    let mut bytes = minimal(SCHEMA_VERSION);
    bytes[4..8].copy_from_slice(b"RDLS");
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongIdentifier(id)) if &id == b"RDLS"));
}

#[test]
fn an_unknown_version_is_rejected() {
    let bytes = minimal(SCHEMA_VERSION + 1);
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongVersion(v)) if v == SCHEMA_VERSION + 1));
}

#[test]
fn a_truncated_buffer_is_rejected() {
    let bytes = minimal(SCHEMA_VERSION);
    let cut = &bytes[..bytes.len() - 6];
    assert!(matches!(verify(cut), Err(VerifyError::Invalid(_))));
}

#[test]
fn a_flipped_offset_is_rejected_as_a_whole() {
    let mut bytes = minimal(SCHEMA_VERSION);
    // The root uoffset at 0..4 points at the table; send it past the end.
    bytes[0..4].copy_from_slice(&(bytes.len() as u32 + 64).to_le_bytes());
    assert!(matches!(verify(&bytes), Err(VerifyError::Invalid(_))));
}

#[test]
fn the_error_names_its_cause() {
    let text = VerifyError::WrongIdentifier(*b"RDLS").to_string();
    assert!(text.contains("RDLS") && text.contains("RDLC"), "{text}");
    let text = VerifyError::WrongVersion(9).to_string();
    assert!(text.contains('9') && text.contains(&SCHEMA_VERSION.to_string()), "{text}");
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked --test verify` Expected: compile
error, `verify` and `VerifyError` not found.

- [ ] **Step 3: Implement `verify`**

Append to `crates/ridl-descriptor/src/lib.rs`:

```rust
use planus::ReadAsRoot;

/// Why a buffer is not a catalog descriptor this toolchain reads.
#[derive(Debug)]
pub enum VerifyError {
    /// Fewer bytes than the root offset and the identifier need.
    TooShort(usize),
    /// Bytes 4..8 are not `FILE_IDENTIFIER`: another kind of file.
    WrongIdentifier([u8; 4]),
    /// A schema version this toolchain does not know.
    WrongVersion(u32),
    /// The FlatBuffers structure is not sound: an offset or a length points
    /// outside the buffer, a required field is missing, an enum tag is unknown.
    Invalid(planus::Error),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort(len) => write!(f, "{len} bytes is shorter than a catalog descriptor header"),
            Self::WrongIdentifier(id) => write!(
                f,
                "not a catalog descriptor: file identifier {:?}, expected {:?}",
                String::from_utf8_lossy(id),
                String::from_utf8_lossy(&FILE_IDENTIFIER),
            ),
            Self::WrongVersion(v) => write!(f, "catalog descriptor version {v}; this toolchain reads version {SCHEMA_VERSION}"),
            Self::Invalid(err) => write!(f, "catalog descriptor is malformed: {err}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Checks the identifier and the version, then walks every table, vector
/// and string once through the checked accessors so that a later read of
/// the returned view cannot fail on a malformed offset.
pub fn verify(bytes: &[u8]) -> Result<CatalogRef<'_>, VerifyError> {
    if bytes.len() < 8 {
        return Err(VerifyError::TooShort(bytes.len()));
    }
    let id: [u8; 4] = bytes[4..8].try_into().expect("four bytes");
    if id != FILE_IDENTIFIER {
        return Err(VerifyError::WrongIdentifier(id));
    }
    let catalog = CatalogRef::read_as_root(bytes).map_err(VerifyError::Invalid)?;
    let version = catalog.version().map_err(VerifyError::Invalid)?;
    if version != SCHEMA_VERSION {
        return Err(VerifyError::WrongVersion(version));
    }
    walk(catalog).map_err(VerifyError::Invalid)?;
    Ok(catalog)
}

/// Touches every field once. planus checks each access; a walk over all
/// of them is the whole-buffer verification of D-8.
fn walk(catalog: CatalogRef<'_>) -> planus::Result<()> {
    catalog.name()?;
    catalog.hash()?;
    catalog.toolchain()?;
    for interface in catalog.interfaces()? {
        let interface = interface?;
        interface.name()?;
        interface.number()?;
        interface.provisional()?;
        for ordinal in interface.reserved_ordinals()? {
            let _ = ordinal;
        }
        for member in interface.members()? {
            let member = member?;
            member.name()?;
            member.ordinal()?;
            member.kind()?;
            if let Some(timing) = member.timing()? {
                timing.mode()?;
                timing.min_us()?;
                timing.max_us()?;
            }
            for payload in member.payloads()? {
                let payload = payload?;
                payload.role()?;
                payload.type_name()?;
                for size in payload.max_sizes()? {
                    let size = size?;
                    size.encoding()?;
                    size.bytes()?;
                    size.state()?;
                    size.cause()?;
                }
            }
        }
    }
    for retired in catalog.retired()? {
        let retired = retired?;
        retired.name()?;
        retired.number()?;
    }
    Ok(())
}
```

If `reserved_ordinals()` yields plain `u32` rather than `Result<u32>`, iterate
with `for _ in ...` — read the generated accessor's type. If `read_as_root` on
the truncated buffer of test 5 succeeds and the walk is what fails, the test
still passes: both paths return `Invalid`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-descriptor --locked --test verify` Expected: 7 tests
PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): verify a descriptor before the first read"
```

---
### Task 3: The interface numbers, copied from the IR

Re-baselined 2026-10-03 (driver §4 answer 3): Epic 15 landed, so the IR
carries every number. This task reads them; it assigns none.

**Files:**

- Create: `crates/ridl-descriptor/src/number.rs`
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod number;`)

**Interfaces:**

- Consumes: `ridl_ir::v2::Package` through `Package::shapes()`
  (`crates/ridl-ir/src/lib.rs:605`), whose item `InterfaceShape<'_>` carries the
  identity `name: &str` — a declared interface's own name, or the owning
  service's dotted global name for an inline shape — and
  `interface: &Interface`, whose `number` and `provisional` fields `ridl-sem`
  folds from `interfaces.lock` (`number_interfaces` in
  `crates/ridl-sem/src/check.rs`, which numbers the declared interfaces and
  the services' inline shapes alike).
- Produces:
  `pub struct Numbered { pub name: String, pub number: u32, pub provisional: bool }`,
  `pub struct ZeroNumber(pub String)` and
  `pub fn numbered_shapes(package: &Package) -> Result<Vec<Numbered>, ZeroNumber>`
  — the one place the descriptor reads a number.

Before writing the tests, read `number_interfaces` in
`crates/ridl-sem/src/check.rs` and confirm that an inline service shape
receives a number and a provisional flag like a declared interface. If it does
not, stop: that contradicts §4 answer 3 and is reported, not worked around.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-descriptor/src/number.rs`:

```rust
//! The interface numbers (rsdl note D-7), copied from the IR. `ridl-sem`
//! folds `interfaces.lock` into `Interface.number` and
//! `Interface.provisional` for every shape, declared or inline, so the
//! descriptor reads them and computes no numbering of its own. A number of
//! 0 never reaches a checked package: here it is an internal error, not data.

use ridl_ir::v2::Package;

/// One interface's number and whether a lock froze it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numbered {
    pub name: String,
    pub number: u32,
    pub provisional: bool,
}

/// An interface shape whose IR number is 0: the package was not numbered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroNumber(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::v2::{service_shape, Interface, Service, ServiceShape};

    fn interface(name: &str, number: u32, provisional: bool) -> Interface {
        Interface { name: name.to_owned(), number, provisional, ..Default::default() }
    }

    fn package(interfaces: Vec<Interface>) -> Package {
        Package { name: "p".to_owned(), interfaces, ..Default::default() }
    }

    /// A `service` with an inline body: its `Interface` lives in the shape
    /// list, not in `Package::interfaces`, and its own `name` is empty.
    fn inline_service(name: &str, number: u32) -> Service {
        Service {
            name: name.to_owned(),
            shapes: vec![ServiceShape {
                kind: Some(service_shape::Kind::Inline(interface("", number, true))),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn numbers_and_flags_are_copied_in_shape_order() {
        let numbered =
            numbered_shapes(&package(vec![interface("B", 7, false), interface("A", 2, true)]))
                .unwrap();
        assert_eq!(
            numbered,
            vec![
                Numbered { name: "B".to_owned(), number: 7, provisional: false },
                Numbered { name: "A".to_owned(), number: 2, provisional: true },
            ]
        );
    }

    #[test]
    fn an_inline_service_shape_is_numbered_under_the_service_name() {
        let mut package = package(vec![interface("A", 1, false)]);
        package.services.push(inline_service("p.hvac", 3));
        assert_eq!(
            numbered_shapes(&package).unwrap(),
            vec![
                Numbered { name: "A".to_owned(), number: 1, provisional: false },
                Numbered { name: "p.hvac".to_owned(), number: 3, provisional: true },
            ]
        );
    }

    #[test]
    fn a_zero_number_is_an_internal_error() {
        let package = package(vec![interface("A", 1, false), interface("Z", 0, true)]);
        assert_eq!(numbered_shapes(&package), Err(ZeroNumber("Z".to_owned())));
    }

    #[test]
    fn a_package_without_interfaces_numbers_nothing() {
        assert!(numbered_shapes(&package(vec![])).unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked number` Expected: compile error,
`numbered_shapes` not found.

- [ ] **Step 3: Implement**

Insert above the `#[cfg(test)]` module:

```rust
/// Every interface shape of `package` in [`Package::shapes`] order, with the
/// number and the provisional flag the IR carries.
pub fn numbered_shapes(package: &Package) -> Result<Vec<Numbered>, ZeroNumber> {
    package
        .shapes()
        .map(|shape| {
            if shape.interface.number == 0 {
                return Err(ZeroNumber(shape.name.to_owned()));
            }
            Ok(Numbered {
                name: shape.name.to_owned(),
                number: shape.interface.number,
                provisional: shape.interface.provisional,
            })
        })
        .collect()
}
```

Add `pub mod number;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-descriptor --locked number` Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): copy the interface numbers from the IR"
```
---

### Task 4: The reachable closure and the catalog hash

**Files:**

- Create: `crates/ridl-descriptor/src/hash.rs`
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod hash;`)

**Interfaces:**

- Consumes: `Package::shapes()` (the walk Task 3 reads) and
  `InterfaceShape::visibility()`;
  `ridl_ir::v2::{Package, Decl, FieldType, StructDef, UnionDef, TypeDef, EnumSetDef, ConstDef}`
  and the oneofs `decl::Kind`, `field_type::Kind`, `stream_type::Element`,
  `return_type::Kind`; `ridl_ir::v2::to_binary` (the derived binary encoding,
  ADR-0014 decision 9 as amended; hashed on purpose, see step 0).
- Produces:
  `pub fn reachable_decls<'a>(package: &'a Package, others: &[&'a Package]) -> BTreeMap<String, &'a Decl>`
  keyed by canonical name (`Name` for this package, `pkg.Name` for another);
  `pub fn reduced_package(package: &Package, others: &[&Package]) -> Package`,
  the exact input of the hash, public so the decision record's rule is testable;
  `pub fn catalog_hash(package: &Package, others: &[&Package]) -> [u8; 32]`.

Re-baselined 2026-10-03 (driver §4 answers 4 and 11): the numbering is no longer
appended after the binary, because the reduced package's interfaces carry
`number` and `provisional` themselves; the hash input is the protobuf binary,
not the canonical JSON; a golden-hash test pins it; #275's criterion is tested
here.

- [ ] **Step 0: Write the decision record**

Before any code, write the decision record §4 answer 4 asks for — an amendment
to `docs/decisions/ADR-0014-ir-encodings.md` decision 9, or a new ADR if the
amendment would not fit decision 9's subject; read ADR-0014's `## Status` and
its amendment convention first and follow it. The record states:

- what is hashed: SHA-256 over `ridl_ir::v2::to_binary` of the reduced package —
  name; every interface shape under its identity name, with
  `InterfaceShape::visibility()`, the IR's `number` and `provisional`, and its
  interactions; the reachable declarations under canonical names, in
  canonical-name order; doc strings blanked; `services` and `retired` empty;
- the determinism rule for the binary: fields in field-number order, list
  elements in the order the writer holds them, no `map<>` field (the IR schema
  has none; a `map<>` added later must not enter the reduced package), fields at
  their default omitted. Check the first clause against the `encode_raw` prost
  generates for `Package` (`cargo expand` or the generated file under
  `target/`): if prost writes fields in declaration order rather than
  field-number order, the record states what prost does and `ir.proto` must keep
  declaring fields in number order;
- the reason: canonical JSON emits every non-`optional` field at its default, so
  each additive IR field would change every catalog hash at a toolchain upgrade;
  the read-back bound that moved the canonical label (decision 9's 2026-09-22
  amendment) does not apply to bytes that are only hashed;
- what is not covered, and why: `Package.retired` (the hash is the identity of
  the interfaces, their numbers and the types they reach, rsdl note D-8; the
  retired list is carried beside it), and doc strings;
- the golden-hash test (step 1, `tests/golden_hash.rs`), so a toolchain change
  that moves a hash fails the gate, and the rule for moving the pinned value:
  only with a change to the IR schema or to this record, named in the commit.

Run `just fmt && just check` over the record. Commit it on its own:
`docs(adr): hash the catalog over the protobuf binary of the reduced package`.

The IR references a type by name string everywhere: `SignalDef.payload`,
`EventDef.payload`, `FieldType::Named`, `UnionArm.type_ref`,
`ConstDef.type_ref`, `EnumSetDef.backing_enum`, `Constraint.pattern_const`,
`StreamType::Named`, `FallibleType.ok`/`err` (`TypeDef.backing.unit` is a UCUM
unit expression, not a reference — `ir.proto:146-150`). A cross-package name is
`pkg.Name`; a same-package name is bare (`ir.proto` header).

- [ ] **Step 1: Write the failing tests**

`crates/ridl-descriptor/src/hash.rs`, test module (the module body comes in step
3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::v2::{
        decl, field_type, service_shape, Decl, Field, FieldType, Interface, RetiredInterface,
        Service, ServiceShape, SignalDef, StructDef, StructMember, struct_member, TypeDef,
        Visibility,
    };

    fn named(name: &str) -> FieldType {
        FieldType { optional: false, kind: Some(field_type::Kind::Named(name.to_owned())) }
    }

    fn struct_decl(name: &str, field_types: &[&str]) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: field_types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| StructMember {
                        member: Some(struct_member::Member::Field(Field {
                            name: format!("f{i}"),
                            ordinal: i as u32 + 1,
                            r#type: Some(named(ty)),
                            ..Default::default()
                        })),
                    })
                    .collect(),
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn scalar_decl(name: &str) -> Decl {
        Decl { name: name.to_owned(), kind: Some(decl::Kind::TypeDef(TypeDef::default())), ..Default::default() }
    }

    fn signal(name: &str, payload: &str) -> Decl {
        Decl {
            name: name.to_owned(),
            ordinal: 1,
            kind: Some(decl::Kind::SignalDef(SignalDef { payload: payload.to_owned(), ..Default::default() })),
            ..Default::default()
        }
    }

    /// A `service` with an inline body carrying `interactions`; its
    /// `Interface` lives in the shape list, not in `Package::interfaces`.
    fn inline_service(name: &str, interactions: Vec<Decl>) -> Service {
        Service {
            name: name.to_owned(),
            visibility: Visibility::Public as i32,
            shapes: vec![ServiceShape {
                kind: Some(service_shape::Kind::Inline(Interface {
                    interactions,
                    number: 2,
                    provisional: true,
                    ..Default::default()
                })),
            }],
            ..Default::default()
        }
    }

    /// `p`: interface `I` (number 1, provisional) with a signal of `Point`;
    /// `Point` has fields of `Coord` (local) and `fw.Unit` (foreign);
    /// `Unused` is declared, not reached.
    fn fixture() -> (Package, Package) {
        let p = Package {
            name: "p".to_owned(),
            decls: vec![
                struct_decl("Point", &["Coord", "fw.Unit"]),
                scalar_decl("Coord"),
                scalar_decl("Unused"),
            ],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![signal("pos", "Point")],
                number: 1,
                provisional: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let fw = Package {
            name: "fw".to_owned(),
            decls: vec![scalar_decl("Unit"), scalar_decl("Other")],
            ..Default::default()
        };
        (p, fw)
    }

    fn hash_of(p: &Package, fw: &Package) -> [u8; 32] {
        catalog_hash(p, &[fw])
    }

    #[test]
    fn the_closure_reaches_local_and_foreign_types_and_nothing_else() {
        let (p, fw) = fixture();
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Coord", "Point", "fw.Unit"]);
    }

    #[test]
    fn the_hash_is_stable_across_runs() {
        let (p, fw) = fixture();
        assert_eq!(hash_of(&p, &fw), hash_of(&p, &fw));
    }

    #[test]
    fn an_unreached_declaration_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.decls.retain(|d| d.name != "Unused");
        assert_eq!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_reached_foreign_type_moves_the_hash() {
        let (p, mut fw) = fixture();
        let before = hash_of(&p, &fw);
        fw.decls[0] = struct_decl("Unit", &[]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_doc_comment_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.decls[0].doc = "documented".to_owned();
        p.interfaces[0].doc = "documented".to_owned();
        assert_eq!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_number_moves_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.interfaces[0].number = 7;
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_provisional_flag_moves_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.interfaces[0].provisional = false;
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_retired_entry_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.retired.push(RetiredInterface { name: "Old".to_owned(), number: 9 });
        assert_eq!(hash_of(&p, &fw), before);
        assert!(reduced_package(&p, &[&fw]).retired.is_empty());
    }

    #[test]
    fn an_inline_service_shape_reaches_its_types_and_moves_the_hash() {
        let (mut p, fw) = fixture();
        p.interfaces.clear();
        let before = hash_of(&p, &fw);
        p.services.push(inline_service("p.hvac", vec![signal("temp", "Point")]));
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Coord", "Point", "fw.Unit"]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// The reduced interface of an inline shape carries the owning service's
    /// visibility, so a declared interface and an inline shape hash alike
    /// (driftsys/ridl#326, third minor item).
    #[test]
    fn an_inline_shape_hashes_the_owning_services_visibility() {
        let (mut p, fw) = fixture();
        p.interfaces.clear();
        p.services.push(inline_service("p.hvac", vec![signal("temp", "Point")]));
        let before = hash_of(&p, &fw);
        p.services[0].visibility = Visibility::Internal as i32;
        assert_ne!(hash_of(&p, &fw), before);
        let reduced = reduced_package(&p, &[&fw]);
        assert_eq!(reduced.interfaces[0].name, "p.hvac");
        assert_eq!(reduced.interfaces[0].visibility, Visibility::Internal as i32);
    }
}
```

And the golden-hash test §4 answer 4 asks for, in
`crates/ridl-descriptor/tests/golden_hash.rs`, over the corpus package's
checked-in IR snapshot
(`crates/ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json`,
read through `ridl_ir::v2::from_json`), so that a toolchain change that moves
the hash fails the gate:

```rust
//! The catalog hash of the corpus package is pinned (driver §4 answer 4).
//! A different value here means the reduced package or the binary encoding
//! changed; the decision record of Task 4 step 0 says when the pin may move.

use std::path::Path;

use ridl_descriptor::hash::catalog_hash;

/// Pinned on the first run; see the decision record for the rule on moving it.
const CORPUS_HASH: &str = "<64 hex digits, taken from the first run>";

#[test]
fn the_corpus_hash_is_pinned() {
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json");
    let text = std::fs::read_to_string(&snapshot).expect("the corpus snapshot is checked in");
    let package = ridl_ir::v2::from_json(&text).expect("the snapshot is canonical IR JSON");
    let hash = catalog_hash(&package, &[]);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, CORPUS_HASH);
}

/// driftsys/ridl#275's criterion (driver §4 answer 11): the hash is a
/// property of the IR, so it does not depend on which wire schema a build
/// emits. The hash takes no emit list; this test states the property where
/// a reader looks for it.
#[test]
fn the_hash_is_the_same_whatever_a_build_emits() {
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json");
    let package = ridl_ir::v2::from_json(&std::fs::read_to_string(&snapshot).unwrap()).unwrap();
    assert_eq!(catalog_hash(&package, &[]), catalog_hash(&package, &[]));
}
```

The second test is weak on its own; Task 9 adds the strong form through the
binary: `--emit proto,catalog`, `--emit flatbuffers,catalog` and
`--emit catalog` write byte-identical descriptors.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked hash` Expected: compile error,
`reachable_decls` and `catalog_hash` not found.

- [ ] **Step 3: Implement the closure and the hash**

Module body above the tests:

```rust
//! The catalog hash (rsdl note D-8): SHA-256 over the interfaces, their
//! numbers and every type they reach, transitively, wherever declared.
//! Derived, never recorded. The input is the protobuf binary of the reduced
//! package, by the decision record Task 4 step 0 wrote; see it for the
//! determinism rule and for why the canonical JSON is not the input.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::v2::{
    decl, field_type, return_type, stream_type, Decl, FieldType, Interface, Package,
};
use sha2::{Digest, Sha256};

/// Every declaration an interface of `package` reaches, keyed by canonical
/// name: bare for this package, `pkg.Name` for another.
pub fn reachable_decls<'a>(package: &'a Package, others: &[&'a Package]) -> BTreeMap<String, &'a Decl> {
    let mut index: BTreeMap<String, &'a Decl> = BTreeMap::new();
    for decl in &package.decls {
        index.insert(decl.name.clone(), decl);
    }
    for other in others {
        for decl in &other.decls {
            index.insert(format!("{}.{}", other.name, decl.name), decl);
        }
    }

    let mut pending: Vec<String> = Vec::new();
    for shape in package.shapes() {
        collect_interface(shape.interface, &mut pending);
    }
    let mut reached: BTreeMap<String, &'a Decl> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        // A primitive spelled as a name, or a name the checker already
        // rejected, has no declaration: nothing more to reach.
        let Some(decl) = index.get(&name) else { continue };
        reached.insert(name, decl);
        collect_decl(decl, &mut pending);
    }
    reached
}

fn collect_interface(interface: &Interface, out: &mut Vec<String>) {
    for interaction in &interface.interactions {
        collect_decl(interaction, out);
    }
}

fn collect_decl(decl: &Decl, out: &mut Vec<String>) {
    match &decl.kind {
        Some(decl::Kind::TypeDef(def)) => collect_type_def(def, out),
        Some(decl::Kind::ConstDef(def)) => out.extend(def.type_ref.clone()),
        Some(decl::Kind::StructDef(def)) => {
            for member in &def.members {
                if let Some(ridl_ir::v2::struct_member::Member::Field(field)) = &member.member
                    && let Some(ty) = &field.r#type
                {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(decl::Kind::EnumDef(_)) | Some(decl::Kind::ReservedSlot(_)) | None => {}
        Some(decl::Kind::EnumSetDef(def)) => out.extend(def.backing_enum.clone()),
        Some(decl::Kind::UnionDef(def)) => out.extend(def.arms.iter().map(|arm| arm.type_ref.clone())),
        Some(decl::Kind::SignalDef(def)) => out.push(def.payload.clone()),
        Some(decl::Kind::EventDef(def)) => out.push(def.payload.clone()),
        Some(decl::Kind::CommandDef(def)) => {
            for param in &def.params {
                if let Some(ty) = &param.r#type {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(decl::Kind::QueryDef(def)) => {
            for param in &def.params {
                if let Some(ty) = &param.r#type {
                    collect_field_type(ty, out);
                }
            }
            match def.return_type.as_ref().and_then(|r| r.kind.as_ref()) {
                Some(return_type::Kind::Value(ty)) => collect_field_type(ty, out),
                Some(return_type::Kind::Fallible(f)) => {
                    out.push(f.ok.clone());
                    out.push(f.err.clone());
                }
                None => {}
            }
        }
        Some(decl::Kind::FixedDef(def)) => {
            if let Some(ty) = &def.payload {
                collect_field_type(ty, out);
            }
        }
    }
}

fn collect_type_def(def: &ridl_ir::v2::TypeDef, out: &mut Vec<String>) {
    // `backing.unit` is a UCUM unit expression, not a type reference.
    if let Some(constant) = def.constraint.as_ref().and_then(|c| c.pattern_const.clone()) {
        out.push(constant);
    }
}

fn collect_field_type(ty: &FieldType, out: &mut Vec<String>) {
    match &ty.kind {
        Some(field_type::Kind::Named(name)) => out.push(name.clone()),
        Some(field_type::Kind::Primitive(_)) | None => {}
        Some(field_type::Kind::InlineScalar(def)) => collect_type_def(def, out),
        Some(field_type::Kind::Tuple(tuple)) => {
            for field in &tuple.fields {
                if let Some(ty) = &field.r#type {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(field_type::Kind::Array(array)) => {
            if let Some(element) = &array.element {
                collect_field_type(element, out);
            }
        }
        Some(field_type::Kind::Map(map)) => {
            for ty in [&map.key, &map.value].into_iter().flatten() {
                collect_field_type(ty, out);
            }
        }
        Some(field_type::Kind::Stream(stream)) => {
            if let Some(stream_type::Element::Named(name)) = &stream.element {
                out.push(name.clone());
            }
        }
    }
}

/// The exact input of the hash: the package name; every interface shape
/// under its identity name (`Package::shapes()` order) with the owning
/// service's visibility for an inline shape, the IR's `number` and
/// `provisional`, and its interactions; the reached declarations under
/// canonical names, in canonical-name order; doc strings blanked; no
/// services and no retired entries.
pub fn reduced_package(package: &Package, others: &[&Package]) -> Package {
    let mut reduced = Package {
        name: package.name.clone(),
        decls: reachable_decls(package, others)
            .into_iter()
            .map(|(canonical, decl)| {
                let mut decl = decl.clone();
                decl.name = canonical;
                blank_docs(&mut decl);
                decl
            })
            .collect(),
        interfaces: package
            .shapes()
            .map(|shape| {
                let mut interface = shape.interface.clone();
                interface.name = shape.name.to_owned();
                interface.visibility = shape.visibility();
                interface
            })
            .collect(),
        services: vec![],
        retired: vec![],
    };
    for interface in &mut reduced.interfaces {
        interface.doc.clear();
        for interaction in &mut interface.interactions {
            blank_docs(interaction);
        }
    }
    reduced
}

/// SHA-256 over the protobuf binary of [`reduced_package`]. The numbers are
/// inside: each reduced interface carries the IR's `number` and
/// `provisional`.
pub fn catalog_hash(package: &Package, others: &[&Package]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ridl_ir::v2::to_binary(&reduced_package(package, others)));
    hasher.finalize().into()
}

fn blank_docs(decl: &mut Decl) {
    decl.doc.clear();
    if let Some(decl::Kind::StructDef(def)) = &mut decl.kind {
        for member in &mut def.members {
            if let Some(ridl_ir::v2::struct_member::Member::Field(field)) = &mut member.member {
                field.doc.clear();
            }
        }
    }
    if let Some(decl::Kind::UnionDef(def)) = &mut decl.kind {
        for arm in &mut def.arms {
            arm.doc.clear();
        }
    }
    if let Some(decl::Kind::EnumDef(def)) = &mut decl.kind {
        for value in &mut def.values {
            value.doc.clear();
        }
    }
}
```

Add `pub mod hash;` to `lib.rs`. The `match` over `decl::Kind` and
`field_type::Kind` is exhaustive on purpose: adding an IR variant is a compile
error here, which is the reminder to decide whether the new variant reaches a
type.

- [ ] **Step 4: Run the tests, then pin the golden hash**

Run: `cargo test -p ridl-descriptor --locked hash` Expected: the 10 unit tests
PASS and `the_corpus_hash_is_pinned` FAILS once, printing the computed hex. Put
that value in `CORPUS_HASH`, re-run: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): derive the catalog hash over the reachable closure"
```

---
### Task 5: The size context, the type leaves, and the string byte capacity

Re-baselined 2026-10-03 (driver §4 answers 5, 6 and 7): `Ctx` wraps the
projection's `Packages`; a payload is sized only when it is one named type
(`named_payload`); a row's state is `SizeState`; the `match` narrowing and
`ascii_only` are gone (#665 records the narrowing).

**Files:**

- Create: `crates/ridl-descriptor/src/size.rs`
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod size;`)

**Interfaces:**

- Consumes:
  `ridl_ir::v2::{Package, Decl, FieldType, Constraint, TypeDef, StructDef, UnionDef, TupleType, ArrayType, MapType, ReturnType, Param, IntWidth, FloatWidth, PrimitiveType}`,
  the oneofs `decl::Kind`, `field_type::Kind`, `return_type::Kind`,
  `backing::Kind`, `type_def::Width`, and
  `ridl_ir::projection::flatbuffers::Packages`
  (`crates/ridl-ir/src/projection/flatbuffers.rs:300`, two public fields).
- Produces: `pub struct Ctx<'a>` with `new`, `resolve` and `packages`;
  `pub enum PayloadShape<'a>`; `pub fn named_payload`; `pub enum SizeState`;
  `pub fn size_state`; `pub fn string_max_bytes`; and, `pub(crate)`, the
  shared leaf model `Scalar`, `Leaf`, `leaf_of_field_type`, `leaf_of_name`,
  `leaf_of_primitive` that Task 6 sizes (Task 7 needs none of it: the
  projection resolves its own types).

- [ ] **Step 1: Write the failing tests**

`crates/ridl-descriptor/src/size.rs`, test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::v2::{
        field_type, return_type, stream_type, FallibleType, Package, Param, PrimitiveType,
        ReturnType, StreamType,
    };

    fn constraint(len_max: Option<u64>, pattern: Option<&str>) -> Constraint {
        Constraint { len_max, pattern: pattern.map(str::to_owned), ..Default::default() }
    }

    fn named(name: &str) -> FieldType {
        FieldType { optional: false, kind: Some(field_type::Kind::Named(name.to_owned())) }
    }

    #[test]
    fn a_string_counts_four_bytes_per_scalar_value_whatever_its_pattern() {
        assert_eq!(string_max_bytes(None), 1024);
        assert_eq!(string_max_bytes(Some(&constraint(Some(17), None))), 68);
        // No `match` narrowing in E16 (driver §4 answer 7; driftsys/ridl#665).
        assert_eq!(string_max_bytes(Some(&constraint(Some(17), Some("^[A-Z0-9]{17}$")))), 68);
    }

    #[test]
    fn only_a_payload_that_is_one_named_type_is_sized() {
        let stream = FieldType {
            optional: false,
            kind: Some(field_type::Kind::Stream(StreamType {
                element: Some(stream_type::Element::Named("Point".to_owned())),
            })),
        };
        let primitive = FieldType {
            optional: false,
            kind: Some(field_type::Kind::Primitive(PrimitiveType::Integer as i32)),
        };
        let one = [Param { name: "at".to_owned(), r#type: Some(named("Point")) }];
        let two = [one[0].clone(), Param { name: "flag".to_owned(), r#type: Some(primitive.clone()) }];
        let value = ReturnType { kind: Some(return_type::Kind::Value(named("Point"))) };
        let fallible = ReturnType {
            kind: Some(return_type::Kind::Fallible(FallibleType {
                ok: "Point".to_owned(),
                err: "Coord".to_owned(),
            })),
        };
        let streamed = ReturnType { kind: Some(return_type::Kind::Value(stream.clone())) };

        assert_eq!(named_payload(&PayloadShape::Named("Point")), Some("Point"));
        assert_eq!(named_payload(&PayloadShape::Field(&named("Point"))), Some("Point"));
        assert_eq!(named_payload(&PayloadShape::Field(&primitive)), None, "not a named type");
        assert_eq!(named_payload(&PayloadShape::Field(&stream)), None, "a stream: §4 answer 10");
        assert_eq!(named_payload(&PayloadShape::Params(&one)), Some("Point"));
        assert_eq!(named_payload(&PayloadShape::Params(&two)), None, "several parameters: §4 answer 6");
        assert_eq!(named_payload(&PayloadShape::Params(&[])), None, "zero parameters");
        assert_eq!(named_payload(&PayloadShape::Return(&value)), Some("Point"));
        assert_eq!(named_payload(&PayloadShape::Return(&fallible)), None, "an inline `T | E`: §4 answer 6");
        assert_eq!(named_payload(&PayloadShape::Return(&streamed)), None);
    }

    #[test]
    fn a_primitive_leaf_has_its_proto3_width() {
        match leaf_of_primitive(PrimitiveType::Integer as i32) {
            Some(Leaf::Scalar(s)) => assert_eq!(s.proto_max(), 10),
            other => panic!("integer is a scalar leaf, got {other:?}"),
        }
        assert!(matches!(leaf_of_primitive(PrimitiveType::String as i32), Some(Leaf::Blob(1024))));
        assert!(matches!(leaf_of_primitive(PrimitiveType::Bytes as i32), Some(Leaf::Blob(256))));
    }

    #[test]
    fn the_repr_c_column_is_absent() {
        let package = Package { name: "p".to_owned(), ..Default::default() };
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked size` Expected: compile error, the
module does not exist.

- [ ] **Step 3: Implement the module**

Module body above the tests:

```rust
//! The size state of a payload, per core encoding (spec D-6, as re-baselined
//! on 2026-10-03): absent (no row), bounded with a byte count, or unbounded
//! with a cause (driver §4 answer 5). A bound is an upper bound: the largest
//! legal value of the payload encodes to at most that many bytes under the
//! projection the wire backend emits (ADR-0017 for proto3, ADR-0019 for
//! FlatBuffers). Envelope and framing are excluded (spec D-7). `repr(C)` has
//! no state until E11.12 (driftsys/ridl#317) defines the layout.
//!
//! The descriptor defines no wire shape (§4 answer 6): only a payload that is
//! one named type is sized, through the projections —
//! `ridl_ir::projection::flatbuffers::max_size` (ADR-0019 decision 8) in
//! `flatbuffers`, and ADR-0017's projection in `proto3`. A request of zero or
//! several parameters, an inline `T | E` reply and a stream payload (§4
//! answer 10) are absent until a record defines their encoding.

pub(crate) mod flatbuffers;
pub(crate) mod proto3;

use std::collections::BTreeMap;

use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::v2::{
    backing, decl, field_type, return_type, type_def, ArrayType, Constraint, Decl, FieldType,
    FloatWidth, IntWidth, MapType, Package, Param, PrimitiveType, ReturnType, StructDef,
    TupleType, TypeDef, UnionDef,
};

use crate::{Encoding, UnboundedCause};

/// Name resolution over a package and the packages it imports, and the
/// projection's view of the same scope.
pub struct Ctx<'a> {
    packages: Packages<'a>,
    index: BTreeMap<String, &'a Decl>,
}

impl<'a> Ctx<'a> {
    pub fn new(package: &'a Package, others: &'a [&'a Package]) -> Self {
        let mut index = BTreeMap::new();
        for decl in &package.decls {
            index.insert(decl.name.clone(), decl);
        }
        for other in others {
            for decl in &other.decls {
                index.insert(format!("{}.{}", other.name, decl.name), decl);
            }
        }
        Self { packages: Packages { package, others }, index }
    }

    /// The declaration a canonical name refers to: bare for this package,
    /// `pkg.Name` for another.
    pub fn resolve(&self, name: &str) -> Option<&'a Decl> {
        self.index.get(name).copied()
    }

    /// The projection's view of the scope, for `max_size`.
    pub fn packages(&self) -> Packages<'a> {
        self.packages
    }
}

/// What is being sized.
pub enum PayloadShape<'a> {
    /// A signal or event payload: a type name.
    Named(&'a str),
    /// A fixed payload.
    Field(&'a FieldType),
    /// A command or query request: the parameters.
    Params(&'a [Param]),
    /// A query response.
    Return(&'a ReturnType),
}

/// The one named type a payload is, or `None` when the payload is not one
/// named type and so has absent sizes (driver §4 answers 6 and 10).
pub fn named_payload<'a>(shape: &PayloadShape<'a>) -> Option<&'a str> {
    match shape {
        PayloadShape::Named(name) => Some(name),
        PayloadShape::Field(ty) => named_field(ty),
        PayloadShape::Params([single]) => single.r#type.as_ref().and_then(named_field),
        PayloadShape::Params(_) => None,
        PayloadShape::Return(ret) => match ret.kind.as_ref()? {
            return_type::Kind::Value(ty) => named_field(ty),
            return_type::Kind::Fallible(_) => None,
        },
    }
}

fn named_field(ty: &FieldType) -> Option<&str> {
    match ty.kind.as_ref()? {
        field_type::Kind::Named(name) => Some(name),
        _ => None,
    }
}

/// One payload's state under one encoding; the row is absent when the
/// toolchain computed no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeState {
    Bounded(u32),
    Unbounded(UnboundedCause),
}

/// The state of the named type `type_name` under `encoding`: `None` when
/// this toolchain computes no state (the `repr(C)` layout is undefined, a
/// name does not resolve, or the projection has no root form for the type).
pub fn size_state(type_name: &str, ctx: &Ctx<'_>, encoding: Encoding) -> Option<SizeState> {
    match encoding {
        Encoding::Proto3 => proto3::state(type_name, ctx),
        Encoding::FlatBuffers => flatbuffers::state(type_name, ctx),
        Encoding::ReprC => None,
    }
}

/// The byte capacity of a `string`: the bound in scalar values (default 256,
/// typl §4.4) times four (design note §3.11; the FlatBuffers projection
/// charges the same). No `match` narrowing (driver §4 answer 7; #665).
pub fn string_max_bytes(constraint: Option<&Constraint>) -> u64 {
    constraint.and_then(|c| c.len_max).unwrap_or(256).saturating_mul(4)
}

/// A scalar as the proto3 projection spells it (`proto_scalar` in the proto
/// backend).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Scalar {
    Bool,
    Int(IntWidth),
    Float(FloatWidth),
}

impl Scalar {
    /// The largest proto3 encoding of the value, tag excluded: a varint for
    /// the integers (`uint32`/`sint32` at most 5 bytes, `uint64`/`sint64`/
    /// `int64` at most 10), fixed for the floats.
    pub(crate) fn proto_max(self) -> u64 {
        match self {
            Self::Bool => 1,
            Self::Int(IntWidth::U8 | IntWidth::U16 | IntWidth::U32 | IntWidth::I8 | IntWidth::I16 | IntWidth::I32) => 5,
            Self::Int(IntWidth::U64 | IntWidth::I64 | IntWidth::Unspecified) => 10,
            Self::Float(FloatWidth::F32) => 4,
            Self::Float(FloatWidth::F64 | FloatWidth::Unspecified) => 8,
        }
    }
}

/// A type resolved to what the projections size.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Leaf<'a> {
    Scalar(Scalar),
    /// A string or bytes value: its maximum byte length.
    Blob(u64),
    /// An enum: whether a member is negative, and the largest magnitude.
    Enum { negative: bool, max_magnitude: u64 },
    EnumSet(IntWidth),
    Struct(&'a StructDef),
    Union(&'a UnionDef),
    Tuple(&'a TupleType),
    Array(&'a ArrayType),
    Map(&'a MapType),
}

pub(crate) fn leaf_of_field_type<'a>(ty: &'a FieldType, ctx: &Ctx<'a>) -> Option<Leaf<'a>> {
    match ty.kind.as_ref()? {
        field_type::Kind::Named(name) => leaf_of_name(name, ctx),
        field_type::Kind::Primitive(primitive) => leaf_of_primitive(*primitive),
        field_type::Kind::InlineScalar(def) => leaf_of_type_def(def),
        field_type::Kind::Tuple(tuple) => Some(Leaf::Tuple(tuple)),
        field_type::Kind::Array(array) => Some(Leaf::Array(array)),
        field_type::Kind::Map(map) => Some(Leaf::Map(map)),
        // A stream has absent sizes (driver §4 answer 10).
        field_type::Kind::Stream(_) => None,
    }
}

pub(crate) fn leaf_of_name<'a>(name: &str, ctx: &Ctx<'a>) -> Option<Leaf<'a>> {
    match ctx.resolve(name)?.kind.as_ref()? {
        decl::Kind::TypeDef(def) => leaf_of_type_def(def),
        decl::Kind::StructDef(def) => Some(Leaf::Struct(def)),
        decl::Kind::UnionDef(def) => Some(Leaf::Union(def)),
        decl::Kind::EnumDef(def) => Some(Leaf::Enum {
            negative: def.values.iter().any(|v| v.value < 0),
            max_magnitude: def.values.iter().map(|v| v.value.unsigned_abs()).max().unwrap_or(0),
        }),
        decl::Kind::EnumSetDef(def) => {
            Some(Leaf::EnumSet(IntWidth::try_from(def.width).unwrap_or(IntWidth::Unspecified)))
        }
        decl::Kind::ConstDef(_)
        | decl::Kind::SignalDef(_)
        | decl::Kind::EventDef(_)
        | decl::Kind::CommandDef(_)
        | decl::Kind::QueryDef(_)
        | decl::Kind::FixedDef(_)
        | decl::Kind::ReservedSlot(_) => None,
    }
}

pub(crate) fn leaf_of_primitive<'a>(primitive: i32) -> Option<Leaf<'a>> {
    match PrimitiveType::try_from(primitive).ok()? {
        PrimitiveType::Boolean => Some(Leaf::Scalar(Scalar::Bool)),
        PrimitiveType::Integer => Some(Leaf::Scalar(Scalar::Int(IntWidth::Unspecified))),
        PrimitiveType::Float => Some(Leaf::Scalar(Scalar::Float(FloatWidth::Unspecified))),
        PrimitiveType::String => Some(Leaf::Blob(256 * 4)),
        PrimitiveType::Bytes => Some(Leaf::Blob(256)),
        PrimitiveType::Unspecified => None,
    }
}

fn leaf_of_type_def<'a>(def: &'a TypeDef) -> Option<Leaf<'a>> {
    let int_width = match def.width {
        Some(type_def::Width::IntWidth(w)) => IntWidth::try_from(w).unwrap_or(IntWidth::Unspecified),
        _ => IntWidth::Unspecified,
    };
    let float_width = match def.width {
        Some(type_def::Width::FloatWidth(w)) => FloatWidth::try_from(w).unwrap_or(FloatWidth::Unspecified),
        _ => FloatWidth::Unspecified,
    };
    match def.backing.as_ref()?.kind.as_ref()? {
        backing::Kind::Primitive(primitive) => match PrimitiveType::try_from(*primitive).ok()? {
            PrimitiveType::Boolean => Some(Leaf::Scalar(Scalar::Bool)),
            PrimitiveType::Integer => Some(Leaf::Scalar(Scalar::Int(int_width))),
            PrimitiveType::Float => Some(Leaf::Scalar(Scalar::Float(float_width))),
            PrimitiveType::String => Some(Leaf::Blob(string_max_bytes(def.constraint.as_ref()))),
            PrimitiveType::Bytes => {
                Some(Leaf::Blob(def.constraint.as_ref().and_then(|c| c.len_max).unwrap_or(256)))
            }
            PrimitiveType::Unspecified => None,
        },
        // A unit-backed scalar carries its own width; the backends project it
        // by that width, integer when an integer width is set, float otherwise.
        backing::Kind::Unit(_) => Some(match def.width {
            Some(type_def::Width::IntWidth(_)) => Leaf::Scalar(Scalar::Int(int_width)),
            _ => Leaf::Scalar(Scalar::Float(float_width)),
        }),
    }
}
```

Create `crates/ridl-descriptor/src/size/proto3.rs` and
`crates/ridl-descriptor/src/size/flatbuffers.rs` each holding a module doc
line for now (`//! proto3 states — Task 6.` / `//! FlatBuffers states — Task
7.`) plus a `state` that the `size_state` arms above compile against:

```rust
use super::{Ctx, SizeState};

pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let _ = (type_name, ctx);
    None
}
```

Tasks 6 and 7 replace these bodies; the `size_state` signature is what Task 8
codes against, so it exists from here.

Add `pub mod size;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-descriptor --locked size` Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): resolve payload types and the string byte capacity"
```
---

### Task 6: The proto3 state

**Files:**

- Modify: `crates/ridl-descriptor/src/size/proto3.rs`

**Interfaces:**

- Consumes: `Ctx`, `SizeState`, `Leaf`, `Scalar`, `leaf_of_field_type`,
  `leaf_of_name` (Task 5).
- Produces:
  `pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState>` and
  `pub(crate) fn varint_len(v: u64) -> u64`.

Re-baselined 2026-10-03 (driver §4 answers 5 and 6; D5 derives this bound "under
ADR-0017 as amended today"). The rules are the proto3 wire format over
ADR-0017's projection: a field number from the IR ordinal (`emit_struct`,
`emit_union` in the proto backend), a struct as a message, a union as a message
with a `oneof`, a named scalar inlined into its field, an enum as a varint, an
enum set as its width's scalar, an array as `repeated` (packed for scalars), a
map as `map<K, V>`, a tuple as an induced message, a nested array or map
refused. A payload is sized only when it is a struct or a union: those are the
two shapes ADR-0017 projects as a message. ADR-0017 decision 1 inlines a named
scalar and rejects a wrapper message per named scalar, so a named scalar, an
enum or an enum set as a payload has no proto3 root form, and its proto3 state
is absent until a record defines one (re-baseline decision 5). proto3 has no
unbounded state: typl bounds every collection, so a message is bounded or, when
the projection refuses a member, absent. A bound above `u32::MAX` is absent, as
in `MAX_ENCODABLE`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/ridl-descriptor/src/size/proto3.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::tests_support::*;

    #[test]
    fn varint_lengths() {
        assert_eq!(varint_len(0), 1);
        assert_eq!(varint_len(127), 1);
        assert_eq!(varint_len(128), 2);
        assert_eq!(varint_len(u64::MAX), 10);
    }

    #[test]
    fn a_struct_payload_is_the_message_itself() {
        // Point { x: Coord, y: Coord }, Coord = integer i16: two sint32
        // fields, tag 1 byte + value 5 bytes each.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Point", &ctx), Some(SizeState::Bounded(12)));
    }

    #[test]
    fn a_named_scalar_has_no_proto3_root_form() {
        // ADR-0017 decision 1 inlines a named scalar and rejects a wrapper
        // message, so a payload of one is absent (driver §4 answer 6).
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Vin", &ctx), None);
        assert_eq!(state("Coord", &ctx), None);
    }

    #[test]
    fn string_array_and_map_fields_inside_a_message() {
        // Bag { tag: Vin @1, flags: [u8; 0..4] @2, index: {string [0..8]: u32; 0..2} @3 }:
        //   tag:   1 + 1 + 17 * 4            = 70
        //   flags: packed, 1 + 1 + 4 * 5     = 22
        //   index: entry (1+1+32) + (1+5) = 40; 2 * (1 + 1 + 40) = 84
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Bag", &ctx), Some(SizeState::Bounded(176)));
    }

    #[test]
    fn a_union_payload_is_its_message_with_the_largest_arm() {
        // Shape = Point @1 | Coord @2: arm a 1 + 1 + 12 = 14, arm b 1 + 5 = 6.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Shape", &ctx), Some(SizeState::Bounded(14)));
    }

    #[test]
    fn an_unresolved_name_is_absent() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Missing", &ctx), None);
    }
}
```

And the shared test fixtures, appended to `crates/ridl-descriptor/src/size.rs`
(used by Task 7 too):

```rust
#[cfg(test)]
pub(crate) mod tests_support {
    use ridl_ir::v2::{
        backing, decl, field_type, type_def, ArrayType, Backing, ConstDef, Constraint, Decl,
        Field, FieldType, IntWidth, MapType, Package, PrimitiveType, StructDef, StructMember,
        struct_member, TypeDef, UnionArm, UnionDef,
    };

    fn scalar_def(primitive: PrimitiveType, width: Option<IntWidth>, constraint: Option<Constraint>) -> TypeDef {
        TypeDef {
            backing: Some(Backing { kind: Some(backing::Kind::Primitive(primitive as i32)) }),
            constraint,
            width: width.map(|w| type_def::Width::IntWidth(w as i32)),
            ..Default::default()
        }
    }

    pub(crate) fn inline(def: TypeDef) -> FieldType {
        FieldType { optional: false, kind: Some(field_type::Kind::InlineScalar(Box::new(def))) }
    }

    pub(crate) fn named(name: &str) -> FieldType {
        FieldType { optional: false, kind: Some(field_type::Kind::Named(name.to_owned())) }
    }

    pub(crate) fn i16() -> FieldType {
        inline(scalar_def(PrimitiveType::Integer, Some(IntWidth::I16), None))
    }

    pub(crate) fn array_u8_max4() -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Array(Box::new(ArrayType {
                element: Some(Box::new(inline(scalar_def(PrimitiveType::Integer, Some(IntWidth::U8), None)))),
                min: 0,
                max: 4,
            }))),
        }
    }

    pub(crate) fn map_str8_u32_max2() -> FieldType {
        let key = inline(scalar_def(
            PrimitiveType::String,
            None,
            Some(Constraint { len_max: Some(8), ..Default::default() }),
        ));
        let value = inline(scalar_def(PrimitiveType::Integer, Some(IntWidth::U32), None));
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Map(Box::new(MapType {
                key: Some(Box::new(key)),
                value: Some(Box::new(value)),
                min: 0,
                max: 2,
            }))),
        }
    }

    /// `Coord = integer [-1000..1000]` (i16); `Point { x: Coord, y: Coord }`;
    /// `Vin = string [17..17] match /^[A-HJ-NPR-Z0-9]{17}$/`;
    /// `Bag { tag: Vin, flags: [u8; 0..4], index: {string [0..8]: u32; 0..2} }`;
    /// `Shape = Point | Coord`; `const LIMIT`.
    pub(crate) fn fixture() -> Package {
        let field = |name: &str, ordinal: u32, ty: FieldType| StructMember {
            member: Some(struct_member::Member::Field(Field {
                name: name.to_owned(),
                ordinal,
                r#type: Some(ty),
                ..Default::default()
            })),
        };
        let arm = |name: &str, ordinal: u32, type_ref: &str| UnionArm {
            name: name.to_owned(),
            ordinal,
            type_ref: type_ref.to_owned(),
            ..Default::default()
        };
        Package {
            name: "p".to_owned(),
            decls: vec![
                Decl {
                    name: "Coord".to_owned(),
                    kind: Some(decl::Kind::TypeDef(scalar_def(PrimitiveType::Integer, Some(IntWidth::I16), None))),
                    ..Default::default()
                },
                Decl {
                    name: "Point".to_owned(),
                    kind: Some(decl::Kind::StructDef(StructDef {
                        members: vec![field("x", 1, named("Coord")), field("y", 2, named("Coord"))],
                        fixed_layout: false,
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "Vin".to_owned(),
                    kind: Some(decl::Kind::TypeDef(scalar_def(
                        PrimitiveType::String,
                        None,
                        Some(Constraint {
                            len_min: Some(17),
                            len_max: Some(17),
                            pattern: Some("/^[A-HJ-NPR-Z0-9]{17}$/".to_owned()),
                            ..Default::default()
                        }),
                    ))),
                    ..Default::default()
                },
                Decl {
                    name: "Bag".to_owned(),
                    kind: Some(decl::Kind::StructDef(StructDef {
                        members: vec![
                            field("tag", 1, named("Vin")),
                            field("flags", 2, array_u8_max4()),
                            field("index", 3, map_str8_u32_max2()),
                        ],
                        fixed_layout: false,
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "Shape".to_owned(),
                    kind: Some(decl::Kind::UnionDef(UnionDef {
                        arms: vec![arm("a", 1, "Point"), arm("b", 2, "Coord")],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "LIMIT".to_owned(),
                    kind: Some(decl::Kind::ConstDef(ConstDef {
                        type_ref: Some("Coord".to_owned()),
                        value: "7".to_owned(),
                        regex: None,
                    })),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
}
```

`i16()`, `inline()` and `named()` stay as written; delete any helper the
compiler reports as unused, because `-D warnings` covers the test modules.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked proto3` Expected: the six tests
compile and every bounded assertion fails against `None` (`varint_len` is a
compile error until step 3).

- [ ] **Step 3: Implement**

Replace the body of `crates/ridl-descriptor/src/size/proto3.rs` above the tests:

```rust
//! The proto3 state of a named-type payload under ADR-0017's projection:
//! bounded for a struct (a message) and a union (a message with a `oneof`);
//! absent for a named scalar, an enum and an enum set, which ADR-0017
//! decision 1 inlines into their field and gives no message of their own.

use ridl_ir::v2::{struct_member, ArrayType, FieldType, MapType, StructDef, TupleType, UnionDef};

use super::{leaf_of_field_type, leaf_of_name, Ctx, Leaf, Scalar, SizeState};

/// Nesting deeper than this is treated as unsizable; typl rejects recursion
/// (§7.3), so this only guards against an IR the checker did not see.
const MAX_DEPTH: u32 = 64;

/// The length of `v` as a base-128 varint.
pub(crate) fn varint_len(v: u64) -> u64 {
    if v == 0 { 1 } else { u64::from((64 - v.leading_zeros()).div_ceil(7)) }
}

/// The tag of field `number`; the wire type does not change the length.
fn tag_len(number: u32) -> u64 {
    varint_len((u64::from(number) << 3) | 5)
}

/// A length-delimited field: tag, length varint, payload.
fn delimited(number: u32, payload: u64) -> u64 {
    tag_len(number) + varint_len(payload) + payload
}

/// The proto3 state of the named type `type_name`: its message's bound for
/// a struct or a union; absent for everything else (ADR-0017 decision 1 has
/// no root form for a named scalar, an enum or an enum set), for a name that
/// does not resolve, for a member the projection refuses, and for a bound
/// above `u32::MAX`.
pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let bytes = match leaf_of_name(type_name, ctx)? {
        Leaf::Struct(def) => struct_size(def, ctx, 0)?,
        Leaf::Union(def) => union_size(def, ctx, 0)?,
        Leaf::Scalar(_)
        | Leaf::Blob(_)
        | Leaf::Enum { .. }
        | Leaf::EnumSet(_)
        | Leaf::Tuple(_)
        | Leaf::Array(_)
        | Leaf::Map(_) => return None,
    };
    Some(SizeState::Bounded(u32::try_from(bytes).ok()?))
}

fn field_size(number: u32, ty: &FieldType, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    leaf_field(number, leaf_of_field_type(ty, ctx)?, ctx, depth)
}

fn leaf_field(number: u32, leaf: Leaf<'_>, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match leaf {
        Leaf::Scalar(s) => tag_len(number) + s.proto_max(),
        Leaf::Blob(bytes) => delimited(number, bytes),
        Leaf::Enum { negative, max_magnitude } => {
            tag_len(number) + if negative { 10 } else { varint_len(max_magnitude) }
        }
        Leaf::EnumSet(width) => tag_len(number) + Scalar::Int(width).proto_max(),
        Leaf::Struct(def) => delimited(number, struct_size(def, ctx, depth + 1)?),
        Leaf::Union(def) => delimited(number, union_size(def, ctx, depth + 1)?),
        Leaf::Tuple(tuple) => delimited(number, tuple_size(tuple, ctx, depth + 1)?),
        Leaf::Array(array) => array_field(number, array, ctx, depth + 1)?,
        Leaf::Map(map) => map_field(number, map, ctx, depth + 1)?,
    })
}

fn struct_size(def: &StructDef, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    def.members
        .iter()
        .map(|member| match &member.member {
            Some(struct_member::Member::Field(field)) => field_size(field.ordinal, field.r#type.as_ref()?, ctx, depth),
            Some(struct_member::Member::Reserved(_)) | None => Some(0),
        })
        .sum()
}

/// The largest arm, as a field of the `oneof` at the arm's ordinal.
fn union_size(def: &UnionDef, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    let mut largest = 0;
    for arm in &def.arms {
        largest = largest.max(leaf_field(arm.ordinal, leaf_of_name(&arm.type_ref, ctx)?, ctx, depth)?);
    }
    Some(largest)
}

/// A tuple is an induced message with positional fields 1..n.
fn tuple_size(tuple: &TupleType, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    tuple
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| field_size(i as u32 + 1, f.r#type.as_ref()?, ctx, depth))
        .sum()
}

/// `repeated`: scalars are packed (one tag, one length, the values); every
/// other element repeats tag and length. A nested array or map is refused
/// by the projection (`resolve_field_type` in the proto backend).
fn array_field(number: u32, array: &ArrayType, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    let element = leaf_of_field_type(array.element.as_ref()?, ctx)?;
    let n = array.max;
    Some(match element {
        Leaf::Scalar(s) => delimited(number, n.checked_mul(s.proto_max())?),
        Leaf::Enum { negative, max_magnitude } => {
            delimited(number, n.checked_mul(if negative { 10 } else { varint_len(max_magnitude) })?)
        }
        Leaf::EnumSet(width) => delimited(number, n.checked_mul(Scalar::Int(width).proto_max())?),
        Leaf::Array(_) | Leaf::Map(_) => return None,
        other => n.checked_mul(leaf_field(number, other, ctx, depth)?)?,
    })
}

/// `map<K, V>`: `max` entries, each a message with the key at 1 and the value at 2.
fn map_field(number: u32, map: &MapType, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    let entry = field_size(1, map.key.as_ref()?, ctx, depth)? + field_size(2, map.value.as_ref()?, ctx, depth)?;
    map.max.checked_mul(delimited(number, entry))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-descriptor --locked size` Expected: all size tests PASS
(4 from Task 5, 6 here).

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): derive the proto3 state of a named-type payload"
```

---
### Task 7: The FlatBuffers state, through the projection's bound

Re-baselined 2026-10-03. E11.7's design D-6 (`docs/design/flatbuffers-codec.md`;
the archived reasoning is `docs/archive/2026-09-20-flatbuffers-codec-design.md`)
decided that the FlatBuffers bound has one implementation,
`ridl_ir::projection::flatbuffers::max_size`
(`crates/ridl-ir/src/projection/flatbuffers.rs:416`, landed by stage K2,
driftsys/ridl#458), which the Rust codec's `Payload<FlatBuffers>::MAX_SIZE` is
emitted from. This task calls it and computes no charge of its own: a second
derivation would be two implementations of one rule and a silent disagreement
the moment either changed (driver §2). The hand-rolled charges this task carried
from 2026-09-13, and the stage K8 note of 2026-09-21 that said not to implement
them, are gone; the projection's source and the codec record hold the charges.

**Files:**

- Modify: `crates/ridl-ir/src/codegen.rs` and
  `crates/ridl-ir/src/codegen/unbounded.rs` (expose the cause)
- Modify: `crates/ridl-descriptor/src/size/flatbuffers.rs`
- Modify: `crates/ridl-descriptor/Cargo.toml` (dev-dependency
  `ridl-backend-rust`, for the agreement test)
- Test: `crates/ridl-descriptor/tests/codec_agreement.rs`

**Interfaces:**

- Consumes: `Ctx::resolve`, `Ctx::packages()`, `SizeState` (Task 5);
  `ridl_ir::projection::flatbuffers::{max_size, root_table, MAX_ENCODABLE}`;
  `ridl_ir::codegen::v1::{FbUnbounded, FbUnboundedCause}`.
- Produces:
  `pub fn ridl_ir::codegen::fb_unbounded(package: &v2::Package, decl: &v2::Decl) -> v1::FbUnbounded`
  — `unbounded::attribute` made public under that name, its doc comment kept
  (the attribution is computed over the package alone, as the module comment
  says; the descriptor accepts that); and
  `pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState>`
  in `size::flatbuffers`.

The rule, from the projection's own contract:

1. `ctx.resolve(type_name)` is `None`, or `root_table(decl)` is `None` (a
   constant — ADR-0013 decision 5 — or an interaction: no FlatBuffers root)
   → absent.
2. `max_size(ctx.packages(), decl)` is `Some(n)` → `Bounded(n as u32)`. The
   projection refuses any bound above `MAX_ENCODABLE`, which is `u32::MAX`, so
   the conversion cannot fail; `expect` it with that reason.
3. `max_size` is `None` → `Unbounded(cause)`, where `cause` is
   `fb_unbounded(declaring_package, decl).cause` mapped member by member onto
   the schema's `UnboundedCause` (same members, same values; an unknown value
   maps to `Unspecified`). `fb_unbounded` takes the declaring package: for a
   canonical name with a `pkg.` prefix, find that package in
   `ctx.packages().others` by name.

The projection answers `None` for an unresolved reference, a cycle, a `u64`
overflow and a bound above `MAX_ENCODABLE`; every one of them reaches the
descriptor as an unbounded row with the cause the codegen model would report
for the same declaration, so `ridl describe` and `--emit codegen-model` agree.

- [ ] **Step 1: Expose the cause**

In `crates/ridl-ir/src/codegen.rs`, add
`pub use unbounded::attribute as fb_unbounded;` beside the existing
`pub use lower::lower;`, and change `attribute`'s visibility in
`crates/ridl-ir/src/codegen/unbounded.rs` from `pub(crate)` to `pub`. Run
`cargo test -p ridl-ir --locked` — unchanged. Commit:
`refactor(ridl-ir): expose the FlatBuffers unbounded attribution`.

- [ ] **Step 2: Write the failing tests**

Append to `crates/ridl-descriptor/src/size/flatbuffers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::tests_support::*;
    use crate::size::{size_state, Ctx, SizeState};
    use crate::{Encoding, UnboundedCause};
    use ridl_ir::projection::flatbuffers::{max_size, Packages};
    use ridl_ir::v2::Package;

    #[test]
    fn a_bounded_type_advertises_the_projections_number() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        for name in ["Point", "Vin", "Coord", "Bag", "Shape"] {
            let decl = ctx.resolve(name).unwrap();
            let expected = max_size(Packages { package: &package, others: &[] }, decl)
                .unwrap_or_else(|| panic!("{name} is bounded in the fixture"));
            assert_eq!(
                state(name, &ctx),
                Some(SizeState::Bounded(u32::try_from(expected).unwrap())),
                "{name}"
            );
        }
    }

    #[test]
    fn an_unbounded_type_carries_the_models_cause() {
        // Build the unbounded fixture the way the tests in
        // `crates/ridl-ir/src/codegen/unbounded.rs` build theirs (read them
        // first), so the cause here is the one the codegen model reports.
        let package = unbounded_fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Open", &ctx), Some(SizeState::Unbounded(UnboundedCause::Member)));
    }

    #[test]
    fn a_name_with_no_root_table_or_no_declaration_is_absent() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Missing", &ctx), None);
        // A constant projects no FlatBuffers declaration (ADR-0013 decision 5).
        assert_eq!(state("LIMIT", &ctx), None);
    }

    #[test]
    fn size_state_routes_the_flatbuffers_column() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert!(matches!(size_state("Point", &ctx, Encoding::FlatBuffers), Some(SizeState::Bounded(_))));
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }
}
```

Add `unbounded_fixture()` to `tests_support` in `size.rs`: a package with a
struct `Open` whose one member the projection cannot bound, copied from the
shape `unbounded.rs`'s own tests use for the `Member` cause.

And the test this task owes, the only one that fails when the descriptor and
the generated codec drift: `crates/ridl-descriptor/tests/codec_agreement.rs`.
Take the corpus package's IR snapshot
(`crates/ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json`,
through `ridl_ir::v2::from_json`), run the Rust backend over it in process
(read `crates/ridl-backend-rust/src/lib.rs` for the entry point `ridlc` calls,
and the codec emitter for the exact spelling of the `MAX_SIZE` constant it
writes per type), extract every `MAX_SIZE` value from the generated source
with the type it belongs to, and assert `state(name, &ctx)` is
`Some(SizeState::Bounded(value))` for each. `ridl-backend-rust` becomes a
dev-dependency of `ridl-descriptor` for this test; the normal dependency
graph does not change, which is what `xtask/tests/oracle_boundary.rs` still
checks.

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked flatbuffers` Expected: the four
unit tests compile and every bounded or unbounded assertion fails against
`None`; `codec_agreement` fails the same way.

- [ ] **Step 4: Implement**

Replace the body of `crates/ridl-descriptor/src/size/flatbuffers.rs` above the
tests:

```rust
//! The FlatBuffers state of a named-type payload: the projection's own bound
//! (`ridl_ir::projection::flatbuffers::max_size`, E11.7 design D-6), and
//! the codegen model's cause when there is none. Nothing is derived here.

use ridl_ir::codegen::fb_unbounded;
use ridl_ir::codegen::v1::FbUnboundedCause;
use ridl_ir::projection::flatbuffers::{max_size, root_table, MAX_ENCODABLE};
use ridl_ir::v2::Package;

use super::{Ctx, SizeState};
use crate::UnboundedCause;

pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let decl = ctx.resolve(type_name)?;
    root_table(decl)?;
    let packages = ctx.packages();
    Some(match max_size(packages, decl) {
        Some(bytes) => {
            debug_assert!(bytes <= MAX_ENCODABLE);
            SizeState::Bounded(u32::try_from(bytes).expect("max_size refuses a bound above MAX_ENCODABLE"))
        }
        None => {
            let declaring = declaring_package(type_name, packages.package, packages.others)?;
            SizeState::Unbounded(cause_of(fb_unbounded(declaring, decl).cause))
        }
    })
}

/// The package a canonical name was declared in: this one for a bare name,
/// the import named by the `pkg.` prefix otherwise.
fn declaring_package<'a>(name: &str, package: &'a Package, others: &'a [&'a Package]) -> Option<&'a Package> {
    match name.rsplit_once('.') {
        None => Some(package),
        Some((pkg, _)) => others.iter().copied().find(|other| other.name == pkg),
    }
}

/// The schema's enum mirrors the model's, member for member.
fn cause_of(cause: i32) -> UnboundedCause {
    match FbUnboundedCause::try_from(cause) {
        Ok(FbUnboundedCause::Member) => UnboundedCause::Member,
        Ok(FbUnboundedCause::Untyped) => UnboundedCause::Untyped,
        Ok(FbUnboundedCause::Layout) => UnboundedCause::Layout,
        Ok(FbUnboundedCause::Aggregate) => UnboundedCause::Aggregate,
        Ok(FbUnboundedCause::Exempt) => UnboundedCause::Exempt,
        Ok(FbUnboundedCause::Unspecified) | Err(_) => UnboundedCause::Unspecified,
    }
}
```

The prost variant names of `FbUnboundedCause` follow its proto values
(`FB_UNBOUNDED_CAUSE_MEMBER` → `Member`, and so on); read the generated enum
if the compiler disagrees. A canonical name can contain more than one dot
(`veh.cluster.Speed`): `rsplit_once` keeps everything before the last dot as
the package name, which is how `reachable_decls` built the key.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p ridl-descriptor --locked` Expected: every size test PASS
(4 + 6 + 4), and `codec_agreement` PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): advertise the projection's FlatBuffers bound per payload"
```
---

### Task 8: The lowering

**Files:**

- Create: `crates/ridl-descriptor/src/lower.rs`
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod lower;`)
- Test: `crates/ridl-descriptor/tests/lower.rs`

**Interfaces:**

- Consumes: `numbered_shapes` (Task 3), `catalog_hash` (Task 4), `Ctx`,
  `PayloadShape`, `named_payload`, `size_state`, `SizeState` (Tasks 5-7),
  `verify` (Task 2), the owned tables (Task 1), `Package::shapes()` (the walk
  Task 3 reads, in the same order), `Package.retired`.
- Produces: `pub enum LowerError { ZeroNumber(String) }` (with `Display`:
  "internal error: interface `<name>` has no number") and
  `pub fn lower(package: &Package, others: &[&Package]) -> Result<Vec<u8>, LowerError>`
  — the finished descriptor bytes, identifier included. `ridlc` writes them
  unchanged.

Re-baselined 2026-10-03: the numbers and the retired list come from the IR (§4
answer 3); a payload's rows are the states of its one named type and are empty
otherwise (§4 answers 5, 6, 10); no `stream` flag, a stream's `type_name` is the
spelled `<T>`.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-descriptor/tests/lower.rs`:

```rust
//! Spec D-4: what the catalog descriptor contains, checked through `verify`.

use ridl_ir::projection::flatbuffers::{max_size, Packages};
use ridl_ir::v2::{
    backing, decl, field_type, return_type, service_shape, stream_type, type_def, Backing,
    CommandDef, Decl, EventDef, Field, FieldType, FixedDef, Interface, IntWidth, Package, Param,
    PrimitiveType, QueryDef, Reserved, RetiredInterface, ReturnType, Service, ServiceShape,
    SignalDef, StreamType, StructDef, StructMember, struct_member, Timing, TimingMode, TypeDef,
};
use ridl_descriptor::lower::LowerError;
use ridl_descriptor::{lower, verify, Encoding, Kind, SizeStateTag, SCHEMA_VERSION};

fn i16_def() -> TypeDef {
    TypeDef {
        backing: Some(Backing { kind: Some(backing::Kind::Primitive(PrimitiveType::Integer as i32)) }),
        width: Some(type_def::Width::IntWidth(IntWidth::I16 as i32)),
        ..Default::default()
    }
}

fn named(name: &str) -> FieldType {
    FieldType { optional: false, kind: Some(field_type::Kind::Named(name.to_owned())) }
}

fn interaction(name: &str, ordinal: u32, kind: decl::Kind) -> Decl {
    Decl { name: name.to_owned(), ordinal, kind: Some(kind), ..Default::default() }
}

/// Coord = integer i16; Point { x: Coord, y: Coord }; interface Vehicle with
/// one member of every kind and one reserved slot.
fn package() -> Package {
    let field = |name: &str, ordinal: u32| StructMember {
        member: Some(struct_member::Member::Field(Field {
            name: name.to_owned(),
            ordinal,
            r#type: Some(named("Coord")),
            ..Default::default()
        })),
    };
    Package {
        name: "veh.cluster".to_owned(),
        decls: vec![
            Decl { name: "Coord".to_owned(), kind: Some(decl::Kind::TypeDef(i16_def())), ..Default::default() },
            Decl {
                name: "Point".to_owned(),
                kind: Some(decl::Kind::StructDef(StructDef { members: vec![field("x", 1), field("y", 2)], fixed_layout: false })),
                ..Default::default()
            },
        ],
        interfaces: vec![Interface {
            name: "Vehicle".to_owned(),
            number: 1,
            provisional: true,
            interactions: vec![
                interaction("position", 1, decl::Kind::SignalDef(SignalDef {
                    payload: "Point".to_owned(),
                    timing: Some(Timing { mode: TimingMode::StrictPeriodic as i32, min_us: Some("100000".to_owned()), max_us: None, default_applied: false }),
                    ..Default::default()
                })),
                interaction("doorOpened", 2, decl::Kind::EventDef(EventDef { payload: "Point".to_owned(), timing: None })),
                interaction("moveTo", 3, decl::Kind::CommandDef(CommandDef {
                    params: vec![Param { name: "to".to_owned(), r#type: Some(named("Point")) }],
                    ..Default::default()
                })),
                interaction("nearest", 4, decl::Kind::QueryDef(QueryDef {
                    params: vec![Param { name: "from".to_owned(), r#type: Some(named("Point")) }],
                    return_type: Some(ReturnType { kind: Some(return_type::Kind::Value(named("Point"))) }),
                    ..Default::default()
                })),
                interaction("legacyWheelPhase", 5, decl::Kind::ReservedSlot(Reserved { ordinal: 5, ..Default::default() })),
                interaction("vin", 6, decl::Kind::FixedDef(FixedDef { payload: Some(named("Coord")) })),
                interaction("trace", 7, decl::Kind::QueryDef(QueryDef {
                    params: vec![],
                    return_type: Some(ReturnType {
                        kind: Some(return_type::Kind::Value(FieldType {
                            optional: false,
                            kind: Some(field_type::Kind::Stream(StreamType {
                                element: Some(stream_type::Element::Named("Point".to_owned())),
                            })),
                        })),
                    }),
                    ..Default::default()
                })),
                interaction("moveBoth", 8, decl::Kind::CommandDef(CommandDef {
                    params: vec![
                        Param { name: "a".to_owned(), r#type: Some(named("Point")) },
                        Param { name: "b".to_owned(), r#type: Some(named("Point")) },
                    ],
                    ..Default::default()
                })),
                interaction("tryNearest", 9, decl::Kind::QueryDef(QueryDef {
                    params: vec![Param { name: "from".to_owned(), r#type: Some(named("Point")) }],
                    return_type: Some(ReturnType {
                        kind: Some(return_type::Kind::Fallible(ridl_ir::v2::FallibleType {
                            ok: "Point".to_owned(),
                            err: "Coord".to_owned(),
                        })),
                    }),
                    ..Default::default()
                })),
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The FlatBuffers bound the projection gives `Point`, which the descriptor
/// must advertise unchanged (Task 7).
fn point_fb_bound(package: &Package) -> u32 {
    let decl = package.decls.iter().find(|d| d.name == "Point").unwrap();
    u32::try_from(max_size(Packages { package, others: &[] }, decl).unwrap()).unwrap()
}

#[test]
fn the_descriptor_carries_every_member_of_every_kind() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).expect("the lowering writes a valid descriptor");
    assert_eq!(catalog.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.name().unwrap(), "veh.cluster");
    assert_eq!(catalog.hash().unwrap().len(), 32);
    assert_eq!(catalog.toolchain().unwrap(), env!("CARGO_PKG_VERSION"));
    assert_eq!(catalog.retired().unwrap().len(), 0);

    let interface = catalog.interfaces().unwrap().get(0).unwrap();
    assert_eq!(interface.name().unwrap(), "Vehicle");
    assert_eq!(interface.number().unwrap(), 1);
    assert!(interface.provisional().unwrap());
    assert_eq!(interface.reserved_ordinals().unwrap().iter().collect::<Vec<u32>>(), vec![5]);

    let members = interface.members().unwrap();
    let summary: Vec<(String, u32, Kind, Vec<String>)> = members
        .iter()
        .map(|m| {
            let m = m.unwrap();
            let roles = m.payloads().unwrap().iter().map(|p| p.unwrap().role().unwrap().to_owned()).collect();
            (m.name().unwrap().to_owned(), m.ordinal().unwrap(), m.kind().unwrap(), roles)
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("position".to_owned(), 1, Kind::Signal, vec!["value".to_owned()]),
            ("doorOpened".to_owned(), 2, Kind::Event, vec!["occurrence".to_owned()]),
            ("moveTo".to_owned(), 3, Kind::Command, vec!["request".to_owned()]),
            ("nearest".to_owned(), 4, Kind::Query, vec!["request".to_owned(), "response".to_owned()]),
            ("vin".to_owned(), 6, Kind::Fixed, vec!["value".to_owned()]),
            ("trace".to_owned(), 7, Kind::Query, vec!["request".to_owned(), "response".to_owned()]),
            ("moveBoth".to_owned(), 8, Kind::Command, vec!["request".to_owned()]),
            ("tryNearest".to_owned(), 9, Kind::Query, vec!["request".to_owned(), "response".to_owned()]),
        ]
    );
}

/// The rows of one payload: (encoding, state, bytes).
fn rows(payload: ridl_descriptor::PayloadRef<'_>) -> Vec<(Encoding, SizeStateTag, u32)> {
    payload
        .max_sizes()
        .unwrap()
        .iter()
        .map(|s| {
            let s = s.unwrap();
            (s.encoding().unwrap(), s.state().unwrap(), s.bytes().unwrap())
        })
        .collect()
}

#[test]
fn a_named_type_payload_has_a_row_per_sized_encoding_and_no_repr_c() {
    let package = package();
    let bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let position = catalog.interfaces().unwrap().get(0).unwrap().members().unwrap().get(0).unwrap();
    let payload = position.payloads().unwrap().get(0).unwrap();
    assert_eq!(payload.type_name().unwrap(), "Point");
    assert_eq!(
        rows(payload),
        vec![
            (Encoding::Proto3, SizeStateTag::Bounded, 12),
            (Encoding::FlatBuffers, SizeStateTag::Bounded, point_fb_bound(&package)),
        ]
    );
}

#[test]
fn a_request_of_one_named_parameter_is_sized_and_of_several_is_absent() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = catalog.interfaces().unwrap().get(0).unwrap().members().unwrap();
    let move_to = members.get(2).unwrap().payloads().unwrap().get(0).unwrap();
    assert_eq!(move_to.type_name().unwrap(), "Point");
    assert_eq!(rows(move_to).len(), 2);
    let move_both = members.get(6).unwrap().payloads().unwrap().get(0).unwrap();
    assert_eq!(move_both.type_name().unwrap(), "(a: Point, b: Point)");
    assert!(rows(move_both).is_empty(), "several parameters: driver §4 answer 6");
}

#[test]
fn a_fallible_reply_is_absent() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = catalog.interfaces().unwrap().get(0).unwrap().members().unwrap();
    let reply = members.get(7).unwrap().payloads().unwrap().get(1).unwrap();
    assert_eq!(reply.type_name().unwrap(), "Point | Coord");
    assert!(rows(reply).is_empty(), "an inline `T | E`: driver §4 answer 6");
}

#[test]
fn timing_is_carried_when_declared_and_absent_otherwise() {
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let members = catalog.interfaces().unwrap().get(0).unwrap().members().unwrap();
    let timing = members.get(0).unwrap().timing().unwrap().expect("the signal declares timing");
    assert_eq!(timing.mode().unwrap(), ridl_descriptor::TimingMode::StrictPeriodic);
    assert_eq!(timing.min_us().unwrap(), Some("100000"));
    assert!(members.get(1).unwrap().timing().unwrap().is_none());
    assert!(members.get(4).unwrap().timing().unwrap().is_none(), "a fixed never carries timing");
}

#[test]
fn a_stream_response_has_absent_sizes_and_a_spelled_type_name() {
    // Driver §4 answer 10: no `stream` flag, no per-element bound yet (#336).
    let bytes = lower(&package(), &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let trace = catalog.interfaces().unwrap().get(0).unwrap().members().unwrap().get(5).unwrap();
    let response = trace.payloads().unwrap().get(1).unwrap();
    assert_eq!(response.type_name().unwrap(), "<Point>");
    assert!(rows(response).is_empty());
}

#[test]
fn an_inline_service_shape_is_an_interface_under_the_service_name() {
    let mut package = package();
    package.services.push(Service {
        name: "veh.cluster.hvac".to_owned(),
        shapes: vec![ServiceShape {
            kind: Some(service_shape::Kind::Inline(Interface {
                number: 2,
                provisional: true,
                interactions: vec![interaction(
                    "cabinTemp",
                    1,
                    decl::Kind::SignalDef(SignalDef { payload: "Coord".to_owned(), ..Default::default() }),
                )],
                ..Default::default()
            })),
        }],
        ..Default::default()
    });
    let catalog_bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&catalog_bytes).expect("the lowering writes a valid descriptor");
    let interfaces = catalog.interfaces().unwrap();
    assert_eq!(interfaces.len(), 2);
    let hvac = interfaces.get(1).unwrap();
    assert_eq!(hvac.name().unwrap(), "veh.cluster.hvac");
    assert_eq!(hvac.number().unwrap(), 2);
    assert!(hvac.provisional().unwrap());
    assert_eq!(hvac.members().unwrap().len(), 1);
}

#[test]
fn the_retired_list_is_copied_from_the_ir() {
    let mut package = package();
    package.retired.push(RetiredInterface { name: "LaneAssist".to_owned(), number: 9 });
    let bytes = lower(&package, &[]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let retired = catalog.retired().unwrap().get(0).unwrap();
    assert_eq!(retired.name().unwrap(), "LaneAssist");
    assert_eq!(retired.number().unwrap(), 9);
}

#[test]
fn a_zero_number_is_an_internal_error() {
    let mut package = package();
    package.interfaces[0].number = 0;
    assert_eq!(lower(&package, &[]), Err(LowerError::ZeroNumber("Vehicle".to_owned())));
}

#[test]
fn the_bytes_are_stable_across_runs() {
    assert_eq!(lower(&package(), &[]).unwrap(), lower(&package(), &[]).unwrap());
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl-descriptor --locked --test lower` Expected: compile
error, `lower` not found.

- [ ] **Step 3: Implement**

`crates/ridl-descriptor/src/lower.rs`:

```rust
//! IR → catalog descriptor (spec D-4, D-6, D-9). One lowering writes the
//! whole file; the toolchain version stamped in it is this crate's. The
//! numbers and the retired list are the IR's (driver §4 answer 3).

use std::fmt;

use ridl_ir::v2::{
    decl, field_type, return_type, stream_type, Decl, FieldType, Package, Param, PrimitiveType,
    ReturnType, StreamType,
};

use crate::hash::catalog_hash;
use crate::number::{numbered_shapes, ZeroNumber};
use crate::size::{named_payload, size_state, Ctx, PayloadShape, SizeState};
use crate::{
    Catalog, Encoding, Interface, Kind, MaxSize, Member, Payload, RetiredInterface,
    SizeStateTag, Timing, TimingMode, UnboundedCause, FILE_IDENTIFIER, SCHEMA_VERSION,
};

/// Every encoding the size table has a column for, in `Encoding` order.
const COLUMNS: [Encoding; 3] = [Encoding::Proto3, Encoding::FlatBuffers, Encoding::ReprC];

/// Why a checked package could not be lowered: a defect upstream, not data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// An interface shape whose IR number is 0 (`ridl-sem` numbers every
    /// shape of a checked package, so this is an internal error).
    ZeroNumber(String),
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroNumber(name) => write!(f, "internal error: interface `{name}` has no number"),
        }
    }
}

impl std::error::Error for LowerError {}

impl From<ZeroNumber> for LowerError {
    fn from(err: ZeroNumber) -> Self {
        Self::ZeroNumber(err.0)
    }
}

/// Lowers `package` to the finished descriptor bytes; `others` are the
/// packages it imports, for name resolution and the hash closure.
pub fn lower(package: &Package, others: &[&Package]) -> Result<Vec<u8>, LowerError> {
    let numbered = numbered_shapes(package)?;
    let hash = catalog_hash(package, others);
    let ctx = Ctx::new(package, others);

    let interfaces = package
        .shapes()
        .zip(&numbered)
        .map(|(shape, numbered)| Interface {
            name: shape.name.to_owned(),
            number: numbered.number,
            provisional: numbered.provisional,
            members: shape.interface.interactions.iter().filter_map(|decl| member_of(decl, &ctx)).collect(),
            reserved_ordinals: shape
                .interface
                .interactions
                .iter()
                .filter(|decl| matches!(decl.kind, Some(decl::Kind::ReservedSlot(_))))
                .map(|decl| decl.ordinal)
                .collect(),
        })
        .collect();

    let catalog = Catalog {
        version: SCHEMA_VERSION,
        name: package.name.clone(),
        hash: hash.to_vec(),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        interfaces,
        retired: package
            .retired
            .iter()
            .map(|entry| RetiredInterface { name: entry.name.clone(), number: entry.number })
            .collect(),
    };
    let mut builder = planus::Builder::new();
    Ok(builder.finish(&catalog, Some(FILE_IDENTIFIER)).to_vec())
}

fn member_of(decl: &Decl, ctx: &Ctx<'_>) -> Option<Member> {
    let (kind, payloads, timing) = match decl.kind.as_ref()? {
        decl::Kind::SignalDef(def) => (
            Kind::Signal,
            vec![payload("value", def.payload.clone(), &PayloadShape::Named(&def.payload), ctx)],
            def.timing.as_ref(),
        ),
        decl::Kind::EventDef(def) => (
            Kind::Event,
            vec![payload("occurrence", def.payload.clone(), &PayloadShape::Named(&def.payload), ctx)],
            def.timing.as_ref(),
        ),
        decl::Kind::CommandDef(def) => (Kind::Command, vec![request(&def.params, ctx)], def.timing.as_ref()),
        decl::Kind::QueryDef(def) => {
            let mut payloads = vec![request(&def.params, ctx)];
            if let Some(ret) = &def.return_type {
                payloads.push(response(ret, ctx));
            }
            (Kind::Query, payloads, def.timing.as_ref())
        }
        decl::Kind::FixedDef(def) => {
            let ty = def.payload.as_ref()?;
            (Kind::Fixed, vec![payload("value", spell(ty), &PayloadShape::Field(ty), ctx)], None)
        }
        // A reserved slot is listed under `reserved_ordinals`; a type
        // declaration never sits in an interface body.
        decl::Kind::ReservedSlot(_)
        | decl::Kind::TypeDef(_)
        | decl::Kind::ConstDef(_)
        | decl::Kind::StructDef(_)
        | decl::Kind::EnumDef(_)
        | decl::Kind::EnumSetDef(_)
        | decl::Kind::UnionDef(_) => return None,
    };
    Some(Member {
        name: decl.name.clone(),
        ordinal: decl.ordinal,
        kind,
        payloads,
        timing: timing.map(|t| {
            Box::new(Timing {
                mode: match ridl_ir::v2::TimingMode::try_from(t.mode) {
                    Ok(ridl_ir::v2::TimingMode::StrictPeriodic) => TimingMode::StrictPeriodic,
                    Ok(ridl_ir::v2::TimingMode::Range) => TimingMode::Range,
                    _ => TimingMode::Unspecified,
                },
                min_us: t.min_us.clone(),
                max_us: t.max_us.clone(),
            })
        }),
    })
}

/// The request payload: one parameter is spelled as its type, several as
/// `(a: T, b: U)`; only one named parameter is sized (§4 answer 6).
fn request(params: &[Param], ctx: &Ctx<'_>) -> Payload {
    let name = match params {
        [single] => single.r#type.as_ref().map(spell).unwrap_or_default(),
        _ => format!(
            "({})",
            params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.r#type.as_ref().map(spell).unwrap_or_default()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    payload("request", name, &PayloadShape::Params(params), ctx)
}

fn response(ret: &ReturnType, ctx: &Ctx<'_>) -> Payload {
    let name = match &ret.kind {
        Some(return_type::Kind::Value(ty)) => spell(ty),
        Some(return_type::Kind::Fallible(f)) => format!("{} | {}", f.ok, f.err),
        None => String::new(),
    };
    payload("response", name, &PayloadShape::Return(ret), ctx)
}

/// One row per encoding that has a state; none when the payload is not one
/// named type (§4 answers 6 and 10).
fn payload(role: &str, type_name: String, shape: &PayloadShape<'_>, ctx: &Ctx<'_>) -> Payload {
    let max_sizes = match named_payload(shape) {
        Some(name) => COLUMNS
            .iter()
            .filter_map(|&encoding| size_state(name, ctx, encoding).map(|state| row(encoding, state)))
            .collect(),
        None => Vec::new(),
    };
    Payload { role: role.to_owned(), type_name, max_sizes }
}

fn row(encoding: Encoding, state: SizeState) -> MaxSize {
    match state {
        SizeState::Bounded(bytes) => MaxSize {
            encoding,
            bytes,
            state: SizeStateTag::Bounded,
            cause: UnboundedCause::Unspecified,
        },
        SizeState::Unbounded(cause) => MaxSize { encoding, bytes: 0, state: SizeStateTag::Unbounded, cause },
    }
}

/// A display spelling of a type for the descriptor's `type_name`: the
/// canonical name when there is one, a structural spelling otherwise.
fn spell(ty: &FieldType) -> String {
    match &ty.kind {
        Some(field_type::Kind::Named(name)) => name.clone(),
        Some(field_type::Kind::Primitive(p)) => spell_primitive(*p),
        Some(field_type::Kind::InlineScalar(def)) => match def.backing.as_ref().and_then(|b| b.kind.as_ref()) {
            Some(ridl_ir::v2::backing::Kind::Primitive(p)) => spell_primitive(*p),
            Some(ridl_ir::v2::backing::Kind::Unit(unit)) => unit.clone(),
            None => String::new(),
        },
        Some(field_type::Kind::Tuple(tuple)) => format!(
            "({})",
            tuple.fields.iter().map(|f| format!("{}: {}", f.name, f.r#type.as_ref().map(spell).unwrap_or_default())).collect::<Vec<_>>().join(", ")
        ),
        Some(field_type::Kind::Array(array)) => format!(
            "[{}; {}..{}]",
            array.element.as_deref().map(spell).unwrap_or_default(),
            array.min,
            array.max
        ),
        Some(field_type::Kind::Map(map)) => format!(
            "{{{}: {}; {}..{}}}",
            map.key.as_deref().map(spell).unwrap_or_default(),
            map.value.as_deref().map(spell).unwrap_or_default(),
            map.min,
            map.max
        ),
        Some(field_type::Kind::Stream(stream)) => format!("<{}>", spell_stream(stream)),
        None => String::new(),
    }
}

fn spell_stream(stream: &StreamType) -> String {
    match &stream.element {
        Some(stream_type::Element::Named(name)) => name.clone(),
        Some(stream_type::Element::Primitive(p)) => spell_primitive(*p),
        None => String::new(),
    }
}

fn spell_primitive(primitive: i32) -> String {
    match PrimitiveType::try_from(primitive) {
        Ok(PrimitiveType::Boolean) => "boolean",
        Ok(PrimitiveType::Integer) => "integer",
        Ok(PrimitiveType::Float) => "float",
        Ok(PrimitiveType::String) => "string",
        Ok(PrimitiveType::Bytes) => "bytes",
        _ => "",
    }
    .to_owned()
}
```

If the generated `Member::timing` is `Option<Timing>` rather than
`Option<Box<Timing>>`, drop the `Box::new`. `spell` already spells a stream as
`<T>`; nothing else changes in it. Add `pub mod lower;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-descriptor --locked` Expected: every test in the crate
PASS, including the ten in `lower.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): lower a package to its catalog descriptor"
```

---

### Task 9: `ridlc build --emit catalog`

**Files:**

- Modify: `crates/ridlc/Cargo.toml` (dependency `ridl-descriptor`)
- Modify: `crates/ridlc/src/lib.rs:359-436` (`Emit`, eight values today), the
  `ir_dump_suffix` match at :484 (and `is_ir_dump` near :440,
  `system_dump_suffix` at :510), the `write_emits` function from :1454, which
  already receives `others: &[&Package]`
- Test: `crates/ridl/tests/describe_cli.rs` (created here, extended in Task 11)

**Interfaces:**

- Consumes: `ridl_descriptor::{lower, FILE_SUFFIX}` (Tasks 1, 8).
- Produces: `Emit::Catalog`, spelled `catalog` on the command line, the ninth
  emit value; the artifact `<base>.catalog.binfb` beside the IR emits, written
  only when the package carries at least one interface shape (spec D-1, as
  widened on 2026-10-03).

The `Emit` enum is a shared file (driver §3, D6): run `gh pr list` and check for
an open pull request that touches `crates/ridlc/src/lib.rs` before you start.

- [ ] **Step 1: Write the failing test**

`crates/ridl/tests/describe_cli.rs` (the `TempDir` and `ridl` helpers copied
from `crates/ridl/tests/baseline_desk.rs:11-58`, the convention being one copy
per test file):

```rust
//! `ridl build --emit catalog` and `ridl describe` (spec D-9), through the binary.

use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ridl-describe-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the temporary directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `ridl` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The checked-in corpus package with one declared interface and one
/// inline-form service — two interface shapes
/// (`crates/ridl/tests/baseline-corpus`).
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/baseline-corpus")
}

/// Builds the corpus with `--emit catalog` into `out` and returns the one
/// `*.catalog.binfb` it wrote.
fn build_catalog(out: &Path) -> PathBuf {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        corpus().as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        "catalog".as_ref(),
    ]);
    assert_eq!(code, 0, "build failed: {stderr}");
    let mut found: Vec<PathBuf> = std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".catalog.binfb"))
        .collect();
    assert_eq!(found.len(), 1, "exactly one catalog descriptor: {found:?}");
    found.pop().unwrap()
}

#[test]
fn build_emits_one_catalog_descriptor_with_the_identifier() {
    let out = TempDir::new("emit");
    let file = build_catalog(out.path());
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(&bytes[4..8], b"RDLC");
}

#[test]
fn build_emits_the_same_bytes_twice() {
    let first = TempDir::new("stable-1");
    let second = TempDir::new("stable-2");
    let a = std::fs::read(build_catalog(first.path())).unwrap();
    let b = std::fs::read(build_catalog(second.path())).unwrap();
    assert_eq!(a, b);
}

/// driftsys/ridl#275's criterion, as driver §4 answer 11 gives it to #378:
/// the hash — and the whole descriptor — is the same whether a build emits
/// proto3, FlatBuffers or both beside the catalog.
#[test]
fn build_writes_the_same_descriptor_whatever_else_it_emits() {
    let mut descriptors = Vec::new();
    for (label, emits) in [("alone", "catalog"), ("proto", "proto,catalog"), ("fbs", "flatbuffers,catalog"), ("both", "proto,flatbuffers,catalog")] {
        let out = TempDir::new(label);
        let (code, _, stderr) = ridl(&[
            "build".as_ref(),
            corpus().as_os_str(),
            "--out-dir".as_ref(),
            out.path().as_os_str(),
            "--emit".as_ref(),
            emits.as_ref(),
        ]);
        assert_eq!(code, 0, "{emits}: {stderr}");
        let file = std::fs::read_dir(out.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.to_string_lossy().ends_with(".catalog.binfb"))
            .expect("a catalog descriptor was written");
        descriptors.push(std::fs::read(file).unwrap());
    }
    assert!(descriptors.windows(2).all(|w| w[0] == w[1]));
}
```

Before writing the test, confirm the flag spellings with
`cargo run -p ridl -- build --help` — the `Build` variant's `out_dir` field is
`--out-dir` under clap's derive unless an `#[arg(long = ...)]` renames it; use
whatever `--help` prints.

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p ridl --locked --test describe_cli` Expected: all three tests
FAIL: `catalog` is not a valid `--emit` value (exit 2).

- [ ] **Step 3: Implement the emit**

`crates/ridlc/Cargo.toml` `[dependencies]` gains
`ridl-descriptor.workspace = true` (the workspace entry Task 1 added; the same
form as `ridl-ir.workspace = true` on line 16).

In `crates/ridlc/src/lib.rs`, `pub enum Emit` gains, after `CodegenModel`:

```rust
/// The catalog descriptor an engine reads, written to
/// `<base>.catalog.binfb` — only when the package carries at least one
/// interface shape: a declared `interface`, or a `service` with an inline
/// body (docs/wip/2026-09-13-runtime-descriptors-design.md, D-1, D-9).
Catalog,
```

The `ir_dump_suffix` match gains `Emit::Catalog => None,` (it is not an IR
dump), and so do `is_ir_dump` and `system_dump_suffix` if they match
exhaustively. The `write_emits` match gains:

```rust
Emit::Catalog => {
    if ir.shapes().next().is_some() {
        let bytes = ridl_descriptor::lower(ir, others)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(out_dir.join(format!("{base}{}", ridl_descriptor::FILE_SUFFIX)), bytes)?;
    }
}
```

Read `write_emits`'s error type first: a `LowerError` is an internal error and
must reach the user as "the tool could not answer" — exit 2 with `lower`'s
message (ADR-0010 decision 1) — through whatever error the function already
returns; the `io::Error::other` above assumes an `io::Result`.

Every other exhaustive `match` over `Emit` in the crate (the compiler lists
them) gains a `Catalog` arm that does what the `Flatbuffers` arm does, except
where the arm names an IR dump.

- [ ] **Step 4: Run the tests**

Run:
`cargo test -p ridl --locked --test describe_cli && cargo test -p ridlc --locked`
Expected: PASS; the ridlc CLI and golden suites still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ridlc crates/ridl/tests/describe_cli.rs Cargo.lock
git commit -m "feat(ridlc): emit the catalog descriptor with --emit catalog"
```

---
### Task 10: The JSON view

**Files:**
- Create: `crates/ridl-descriptor/src/describe.rs`
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod describe;`)

**Interfaces:**
- Consumes: `CatalogRef` and the other views (Task 1), `verify` (Task 2), `lower` (Task 8) in the test.
- Produces: `pub fn to_json(catalog: CatalogRef<'_>) -> planus::Result<serde_json::Value>` — the same view `flatc --json --strict-json` gives: schema field names as keys, enums by member name, `hash` as an array of bytes, an absent `timing` as `null`, and every field of a `MaxSize` row, defaults included (`bytes` is 0 and `cause` is `"Unspecified"` where they do not apply).

- [ ] **Step 1: Write the failing test**

`crates/ridl-descriptor/src/describe.rs`, test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        verify, Catalog, Encoding, Interface, Kind, MaxSize, Member, Payload, SizeStateTag,
        UnboundedCause, FILE_IDENTIFIER, SCHEMA_VERSION,
    };

    #[test]
    fn the_view_uses_schema_names_and_enum_members() {
        let catalog = Catalog {
            version: SCHEMA_VERSION,
            name: "p".to_owned(),
            hash: vec![1, 2],
            toolchain: "0.0.0".to_owned(),
            interfaces: vec![Interface {
                name: "I".to_owned(),
                number: 1,
                provisional: true,
                members: vec![Member {
                    name: "m".to_owned(),
                    ordinal: 1,
                    kind: Kind::Query,
                    payloads: vec![Payload {
                        role: "request".to_owned(),
                        type_name: "Point".to_owned(),
                        max_sizes: vec![
                            MaxSize {
                                encoding: Encoding::Proto3,
                                bytes: 12,
                                state: SizeStateTag::Bounded,
                                cause: UnboundedCause::Unspecified,
                            },
                            MaxSize {
                                encoding: Encoding::FlatBuffers,
                                bytes: 0,
                                state: SizeStateTag::Unbounded,
                                cause: UnboundedCause::Layout,
                            },
                        ],
                    }],
                    timing: None,
                }],
                reserved_ordinals: vec![],
            }],
            retired: vec![],
        };
        let mut builder = planus::Builder::new();
        let bytes = builder.finish(&catalog, Some(FILE_IDENTIFIER)).to_vec();
        let json = to_json(verify(&bytes).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "version": 1,
                "name": "p",
                "hash": [1, 2],
                "toolchain": "0.0.0",
                "interfaces": [{
                    "name": "I",
                    "number": 1,
                    "provisional": true,
                    "members": [{
                        "name": "m",
                        "ordinal": 1,
                        "kind": "Query",
                        "payloads": [{
                            "role": "request",
                            "type_name": "Point",
                            "max_sizes": [
                                { "encoding": "Proto3", "bytes": 12, "state": "Bounded", "cause": "Unspecified" },
                                { "encoding": "FlatBuffers", "bytes": 0, "state": "Unbounded", "cause": "Layout" }
                            ]
                        }],
                        "timing": null
                    }],
                    "reserved_ordinals": []
                }],
                "retired": []
            })
        );
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p ridl-descriptor --locked describe`
Expected: compile error, `to_json` not found.

- [ ] **Step 3: Implement**

Module body above the tests:

```rust
//! `ridl describe`'s view of a descriptor (spec D-9): strict JSON built by
//! walking the checked accessors. There is no JSON emit; this is a rendering
//! of the binary, and `flatc --json --strict-json` gives the same view.

use serde_json::{json, Value};

use crate::{CatalogRef, Encoding, Kind, SizeStateTag, TimingMode, UnboundedCause};

pub fn to_json(catalog: CatalogRef<'_>) -> planus::Result<Value> {
    let mut interfaces = Vec::new();
    for interface in catalog.interfaces()? {
        let interface = interface?;
        let mut members = Vec::new();
        for member in interface.members()? {
            let member = member?;
            let mut payloads = Vec::new();
            for payload in member.payloads()? {
                let payload = payload?;
                let mut sizes = Vec::new();
                for size in payload.max_sizes()? {
                    let size = size?;
                    sizes.push(json!({
                        "encoding": encoding_name(size.encoding()?),
                        "bytes": size.bytes()?,
                        "state": state_name(size.state()?),
                        "cause": cause_name(size.cause()?),
                    }));
                }
                payloads.push(json!({
                    "role": payload.role()?,
                    "type_name": payload.type_name()?,
                    "max_sizes": sizes,
                }));
            }
            let timing = match member.timing()? {
                Some(t) => json!({ "mode": timing_mode_name(t.mode()?), "min_us": t.min_us()?, "max_us": t.max_us()? }),
                None => Value::Null,
            };
            members.push(json!({
                "name": member.name()?,
                "ordinal": member.ordinal()?,
                "kind": kind_name(member.kind()?),
                "payloads": payloads,
                "timing": timing,
            }));
        }
        interfaces.push(json!({
            "name": interface.name()?,
            "number": interface.number()?,
            "provisional": interface.provisional()?,
            "members": members,
            "reserved_ordinals": interface.reserved_ordinals()?.iter().collect::<Vec<u32>>(),
        }));
    }
    let mut retired = Vec::new();
    for entry in catalog.retired()? {
        let entry = entry?;
        retired.push(json!({ "name": entry.name()?, "number": entry.number()? }));
    }
    Ok(json!({
        "version": catalog.version()?,
        "name": catalog.name()?,
        "hash": catalog.hash()?.iter().collect::<Vec<u8>>(),
        "toolchain": catalog.toolchain()?,
        "interfaces": interfaces,
        "retired": retired,
    }))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Signal => "Signal",
        Kind::Event => "Event",
        Kind::Command => "Command",
        Kind::Query => "Query",
        Kind::Fixed => "Fixed",
    }
}

fn encoding_name(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Proto3 => "Proto3",
        Encoding::FlatBuffers => "FlatBuffers",
        Encoding::ReprC => "ReprC",
    }
}

fn timing_mode_name(mode: TimingMode) -> &'static str {
    match mode {
        TimingMode::Unspecified => "Unspecified",
        TimingMode::StrictPeriodic => "StrictPeriodic",
        TimingMode::Range => "Range",
    }
}

fn state_name(state: SizeStateTag) -> &'static str {
    match state {
        SizeStateTag::Bounded => "Bounded",
        SizeStateTag::Unbounded => "Unbounded",
    }
}

fn cause_name(cause: UnboundedCause) -> &'static str {
    match cause {
        UnboundedCause::Unspecified => "Unspecified",
        UnboundedCause::Member => "Member",
        UnboundedCause::Untyped => "Untyped",
        UnboundedCause::Layout => "Layout",
        UnboundedCause::Aggregate => "Aggregate",
        UnboundedCause::Exempt => "Exempt",
    }
}
```

If `reserved_ordinals()?` yields a vector of `u32` directly, `.iter().collect()` is right; if it yields `Result<u32>` items, collect with `.collect::<planus::Result<Vec<u32>>>()?`. The same for `hash()`.

- [ ] **Step 4: Run the test**

Run: `cargo test -p ridl-descriptor --locked describe`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor
git commit -m "feat(ridl-descriptor): render a descriptor as strict JSON"
```
---

### Task 11: `ridl describe`

**Files:**

- Modify: `crates/ridl/Cargo.toml` (dependency `ridl-descriptor`; dev-dependency
  `insta`)
- Modify: `crates/ridl/src/main.rs:71-210` (`Command`, nine variants today), the
  dispatch `match cli.command` at :252, a new `run_describe`
- Modify: `crates/ridl/tests/describe_cli.rs`
- Modify: `docs/decisions/ADR-0010-cli-conventions.md` (the exit-code table
  under decision 1, twelve rows today, and the dated sentences after it)
- Modify: `docs/book/cli-reference.md` and `docs/book/getting-started.md` (the
  census of step 5)

`crates/ridl/src/main.rs`, `docs/book/cli-reference.md` and ADR-0010 are shared
files: the devex track's Spec 0 also changes `ridl check`, the CLI reference and
ADR-0010 (driver §3, D7). Run `gh pr list` and coordinate before you start.

**Interfaces:**

- Consumes: `ridl_descriptor::{verify, describe::to_json, VerifyError}` (Tasks
  2, 10); the `build_catalog` helper of Task 9.
- Produces: the subcommand `ridl describe <FILE>`: exit 0 with the JSON on
  stdout; exit 2 with `error: <path>: <cause>` on stderr for a missing or
  unreadable path, a foreign file, an unknown version, or a malformed buffer
  (ADR-0010 decision 1; spec D-8).

- [ ] **Step 1: Write the failing tests**

Append to `crates/ridl/tests/describe_cli.rs`:

```rust
#[test]
fn describe_prints_the_descriptor_as_json() {
    let out = TempDir::new("describe");
    let file = build_catalog(out.path());
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(json["version"], 1);
    insta::assert_snapshot!("corpus_catalog", stdout);
}

#[test]
fn describe_reports_a_missing_path_with_exit_2() {
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), "/nonexistent/x.catalog.binfb".as_ref()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("error: /nonexistent/x.catalog.binfb: "), "{stderr}");
}

#[test]
fn describe_rejects_a_foreign_file_before_any_read() {
    let out = TempDir::new("foreign");
    let file = out.path().join("ir.binpb");
    std::fs::write(&file, b"\x08\x01\x12\x03abc").unwrap();
    let (code, _, stderr) = ridl(&["describe".as_ref(), file.as_os_str()]);
    assert_eq!(code, 2);
    assert!(stderr.contains("not a catalog descriptor"), "{stderr}");
}

#[test]
fn describe_rejects_a_truncated_and_a_flipped_descriptor() {
    let out = TempDir::new("corrupt");
    let file = build_catalog(out.path());
    let bytes = std::fs::read(&file).unwrap();

    let truncated = out.path().join("truncated.catalog.binfb");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    let (code, _, stderr) = ridl(&["describe".as_ref(), truncated.as_os_str()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("malformed"), "{stderr}");

    let mut flipped = bytes.clone();
    flipped[0..4].copy_from_slice(&(bytes.len() as u32 + 64).to_le_bytes());
    let path = out.path().join("flipped.catalog.binfb");
    std::fs::write(&path, &flipped).unwrap();
    let (code, _, stderr) = ridl(&["describe".as_ref(), path.as_os_str()]);
    assert_eq!(code, 2, "{stderr}");
}
```

`crates/ridl/Cargo.toml`: `[dependencies]` gains
`ridl-descriptor.workspace = true` (`serde_json.workspace = true` is already
there, line 39); `[dev-dependencies]` gains `insta.workspace = true` (the
workspace entry exists; `crates/ridlc` already uses it).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p ridl --locked --test describe_cli` Expected: the four new
tests FAIL — `describe` is an unknown subcommand (clap exits 2 with a usage
message, so the two "exit 2" tests fail on the stderr text).

- [ ] **Step 3: Implement the subcommand**

In `crates/ridl/src/main.rs`, `enum Command` gains:

```rust
/// Print a catalog descriptor as strict JSON, after verifying it.
Describe {
    /// The `<base>.catalog.binfb` file `ridl build --emit catalog` wrote.
    path: PathBuf,
},
```

The dispatch in `main` gains
`Command::Describe { path } => run_describe(&path),` and the function:

```rust
/// `ridl describe`: read, verify (identifier, version, whole-buffer walk),
/// render. Every failure is "the tool could not answer" — exit 2 with the
/// cause named (ADR-0010 decision 1; the descriptors note, D-8).
fn run_describe(path: &Path) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    let catalog = match ridl_descriptor::verify(&bytes) {
        Ok(catalog) => catalog,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    match ridl_descriptor::describe::to_json(catalog) {
        Ok(json) => {
            println!("{}", serde_json::to_string_pretty(&json).expect("a JSON value serializes"));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {}: catalog descriptor is malformed: {err}", path.display());
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 4: Run the tests, then accept the snapshot**

Run: `cargo test -p ridl --locked --test describe_cli` Expected: the snapshot
test fails once with a new snapshot under
`crates/ridl/tests/snapshots/describe_cli__corpus_catalog.snap`. Read it: two
interfaces — `VehicleStatus`, then the inline shape of the service
`corpus.baseline.hvac` — each with the number `ridl-sem` gave it (the corpus has
no `interfaces.lock`, so both are `"provisional": true`) and its members in
ordinal order; per payload, a `max_sizes` list holding the states the toolchain
computed: two rows for a payload that is one named type, none for a stream, a
request of zero or several parameters or a `T | E` reply; no `ReprC` row
anywhere. Then run `cargo insta accept` (or move the `.snap.new` file) and
re-run: all PASS.

- [ ] **Step 5: Record the exit-code row and the book census**

Recounted on 2026-10-03 against `docs/book/cli-reference.md` and
`docs/decisions/ADR-0010-cli-conventions.md` as they stand on `main`, with every
item of driftsys/ridl#326 folded in. `ridl` has nine subcommands today (`check`,
`baseline`, `build`, `test`, `fmt`, `diff`, `lock`, `lsp`, `mcp`); `describe` is
the tenth, and the ninth that takes a path (`lsp` and `mcp` take none). `--emit`
has eight values; `catalog` is the ninth. Re-run every count below against the
file at the time you edit it: `just fmt`, `just check`, `just link-check` and
`just book-check` count nothing and re-run no `--help`, so a stale number passes
every gate.

In `docs/decisions/ADR-0010-cli-conventions.md`, under decision 1:

- append to the exit-code table, after the `ridl lock merge` row, in the same
  column format:

  ```markdown
  | `ridl describe` | the descriptor was printed | — | a missing or unreadable path; a file that is not a catalog descriptor; a version this toolchain does not read; a malformed buffer |
  ```

- the sentence "across the eight subcommands the two binaries expose today"
  before the table, and "The claim is scoped to these eight, constructed this
  way, on this date" after it, are dated (2026-07-27) and stay. Follow the
  precedent the later rows set: the paragraph "`ridl lsp` and `ridl mcp` earned
  their rows on 2026-09-13, when they were added and checked by
  `crates/ridl/tests/servers.rs`" gains a sibling paragraph: "**`ridl describe`
  earned its row on the date of its pull request, when it was added and
  checked** by the four `describe` tests in
  `crates/ridl/tests/describe_cli.rs`," (write the date out), which drive the
  exit-0 outcome and every exit-2 cause above against the built `ridl` binary."
- "A ninth subcommand earns a row here when it is added and checked, not by
  inheriting this table" is stale already (`lsp`, `mcp` and `lock` are the ninth
  to eleventh); make it "A later subcommand earns a row here when it is added
  and checked, not by inheriting this table".
- decision 6 ("Of the eight subcommands, only `ridl fmt`...") and the
  Consequences ("six of the other seven") are dated findings of 2026-07-27 and
  stay.

In `docs/book/cli-reference.md`:

- the literal `ridl --help` transcript under `## ridl` (lines 57-77, whose
  `Commands:` block the page says is literal binary output): paste the new
  transcript from the built binary, which gains the `describe` line;
- under `### ridl build`, the `--emit <EMIT>` `Possible values:` block (about
  line 493): add the `- catalog:` line with the text `--help` prints;
- under `### ridlc build`, both the inline comma list "The artifacts to emit:
  `rust` (default), ..., `codegen-model`" (about line 1652) and the
  `Possible values:` block below it: add `catalog`;
- "Of the nine subcommands that take a path" (about line 1008) becomes "Of the
  ten subcommands that take a path", and the list that follows it is unchanged
  (`ridl describe` names the path in every exit-2 message, so it joins
  `ridl fmt` on the right side of that sentence — say so in one clause);
- "Seven of the nine subcommands that take a path also share a lesser-known gap"
  (about line 1774) becomes "Seven of the ten subcommands that take a path", and
  the list of the seven is unchanged;
- under `## How ridl and ridlc relate`, the sentence "`ridl baseline`,
  `ridl test`, `ridl fmt`, `ridl diff`, and `ridl lock` have no `ridlc`
  counterpart at all" gains `ridl describe`;
- in the table under `## Exit codes across the toolchain`, add a `ridl describe`
  row after `ridl lock merge` with the three cells of the ADR-0010 row above;
  the `ridl fmt` row's wording about the other subcommands, if it counts them,
  is recounted;
- under `## ridl`, after `### ridl mcp` (the last `ridl` subcommand section,
  before `## ridlc`), add `### ridl describe` with one paragraph ("Prints a
  catalog descriptor written by `ridl build --emit catalog` as JSON after
  verifying it; a file that is not a descriptor, or is malformed, is rejected as
  a whole with exit code 2.") and a `json` fence holding the first twenty lines
  of the snapshot from step 4. The fence's language word is `json`, which the
  book harness does not compile (CONTRIBUTING.md, "Writing examples in the
  book"); the transcript is kept current by hand.

In `docs/book/getting-started.md`, under "What the compiler produces" (about
line 783): the sentence "Seven emit targets exist today" and the seven-row
`--emit` table below it. Add a `catalog` row (`<package>.catalog.binfb`, "the
catalog descriptor an engine reads — interfaces, numbers, members, sizes") and
make the count word match the row count. `codegen-model` is absent from that
table today; whether to add it is not this task's question — count the rows that
are there.

- [ ] **Step 6: Run the docs gates**

Run: `just fmt && just check && just link-check && just book-check` Expected:
all pass. They do not check the counts above; re-read each edited sentence.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl docs/decisions/ADR-0010-cli-conventions.md docs/book/cli-reference.md docs/book/getting-started.md Cargo.lock
git commit -m "feat(ridl): add ridl describe for the catalog descriptor"
```

---

### Task 12: The records, the book, and the full gate

**Files:**

- Modify: `AGENTS.md` (the crate count and list in the first section)
- Modify: `README.md` (the crate list, if it has one)
- Modify: `docs/technotes/walking-skeleton-architecture.md` (both places that
  describe `cargo xtask`: the summary near lines 35-37 and the `xtask` entry
  near line 188; `CONTRIBUTING.md` has no generated-code section;
  `docs/decisions/ADR-0007-e1-execution.md:29` also describes
  `cargo xtask codegen`, and stays as a dated record)

The roadmap needs no row: Epic 16's rows E16.1 to E16.6 exist, and D8's
gardening marks them landed. The design note's §7 dispositions and the
`docs/wip/README.md` entry were updated by the re-baseline of 2026-10-03.

**Interfaces:**

- Consumes: everything above.
- Produces: shipped docs that describe the system as built; `just verify` green.

- [ ] **Step 1: Update the crate inventory**

In `AGENTS.md`, change "nineteen crates" to "twenty crates" and insert
`` `ridl-descriptor` `` after `` `ridl-ir` `` in the list. Grep `README.md` for
`ridl-ir`; where the crates are listed, add `ridl-descriptor` beside it with the
one-line purpose "the catalog descriptor an engine reads".

- [ ] **Step 2: Document the generator**

In `docs/technotes/walking-skeleton-architecture.md`, the `xtask` entry near
line 188 ("`cargo xtask codegen`, the typed-AST generator over `family.ungram`")
gains the second generator and its rule, as one paragraph after it:
"`cargo xtask descriptor-codegen` regenerates
`crates/ridl-descriptor/src/generated.rs` from
`crates/ridl-descriptor/schema/catalog.fbs` with planus; run it after every
schema edit. The xtask test `committed_generated_accessors_match_the_schema`
fails while the committed file is stale. The schema is append-only: add fields
at the end of a table, never remove or reorder one." The summary of `xtask` near
lines 35-37 names the second task in one clause.

- [ ] **Step 3: Run the full gate**

Run: `just verify` Expected: `lint-commits` valid for every commit on the
branch; `build` passes every member, including wasm-check (with
`ridl-descriptor` in the list), compat-check and demo.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md README.md docs/technotes
git commit -m "docs(docs): record the catalog descriptor crate and its generator"
```

- [ ] **Step 5: Open the pull request**

The PR description names the §4 answers of the driver the stage applied, by
number, and the decisions of the "Re-baseline 2026-10" section it relied on, and
asks for the review lane as the repository runs it (four seats: executable lines
are present).

---

## Self-review

Re-read on 2026-10-03 after the re-baseline.

**Spec coverage.** D-1 (catalog per package with at least one interface shape:
Task 9's guard over `Package::shapes()`); D-2 (FlatBuffers, IR untouched: Task
1); D-3 (hand-written, `version`, `file_identifier`, append-only: Task 1's
schema header and Task 12's technote paragraph; the `flatc`/`flatcc`
cross-compiler check is deferred by the spec until a C engine exists); D-4
(identity, interfaces with the IR's number and provisional flag, the IR's
retired entries, members with ordinal, kind, payload type, timing, reserved
ordinals: Tasks 3, 4, 8); D-5 is the system descriptor, out of scope; D-6 (one
row per payload and encoding that has a state, three states, the bounds from the
projections, the string capacity at four bytes per scalar value: Tasks 5-8; the
conformance refutation lands with E11.7/E11.8/E11.12 as the spec says); D-7
(nothing added for payload layouts, transport, envelope); D-8 (Task 2, Task 11);
D-9 (Task 9, Task 11; the JSON is a rendering, no JSON emit); D-10 is generated
code: the Rust backend's `NUMBER` and `PROVISIONAL` constants exist, its zero
`CatalogHash` is retired by D3 and its `None` sizes by D5 (driver §3). §3: an
unclaimed namespace and the rsdl errors are the system descriptor's; a
provisional number is data (Task 8 copies the flag, nothing refuses it); a zero
number is an internal error (Tasks 3, 8, 9). §4: schema round trip (Task 1),
golden files (Task 4's golden hash, Task 11's snapshot and byte-stability
tests), verifier rejection (Tasks 2 and 11), book example (Task 11), max-size
conformance deferred with the codecs, codec agreement (Task 7).

**Placeholders.** Task 5 step 3 creates `proto3.rs` and `flatbuffers.rs` with a
`state` returning `None` so `size_state` compiles before Tasks 6 and 7 fill
them; their tests pin every state. No other step defers content.

**Type consistency.** `Numbered { name, number, provisional }` and `ZeroNumber`
(Task 3) are what Task 8's `lower` consumes; `catalog_hash(package, others)`
(Task 4) takes no numbering; `Ctx::new(package, others)`,
`PayloadShape::{Named, Field, Params, Return}`, `named_payload` and
`size_state(name, ctx, encoding) -> Option<SizeState>` (Task 5) are what Tasks
6, 7 and 8 call; `verify(bytes) -> Result<CatalogRef, VerifyError>` (Task 2) is
what Tasks 8, 10 and 11 read through;
`lower(...) -> Result<Vec<u8>, LowerError>` (Task 8) is what Task 9 writes;
`to_json(CatalogRef) -> planus::Result<Value>` (Task 10) is what Task 11 prints;
`FILE_SUFFIX = ".catalog.binfb"` (Task 1) is what Task 9 joins and Task 11's
tests glob; the schema's `SizeState` is re-exported as `SizeStateTag` so it does
not collide with `size::SizeState`.
