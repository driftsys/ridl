# Deployment Descriptor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** emit one FlatBuffers deployment descriptor per lowered rsdl
deployment, which a runtime node reads to learn every instance, what it offers,
what it maps, and its placement backend keys.

**Architecture:** a second hand-written schema in `ridl-descriptor`
(`schema/deployment.fbs`, identifier `RDLD`) with generated planus accessors, a
`no_std` `finish`/`verify` half and a `std` lowering that projects
`ridl_ir::v2::System` plus the codegen deployment section
(`ridl_ir::codegen::lower_deployment`) and embeds the catalog descriptors.
`ridlc` writes it under `--emit deployment`; `ridl describe` reads either
descriptor.

**Tech Stack:** Rust 2024 at the `rust-toolchain.toml` pin, planus 1.3,
prost-generated IR (`ridl_ir::v2`, `ridl_ir::codegen::v1`).

**Spec:** `docs/wip/2026-11-01-deployment-descriptor-design.md` (rulings R-1 to
R-9). Read it before any task.

## Global Constraints

- File identifier `RDLD` at bytes 4..8; suffix `.deployment.binfb`; file name
  `<deployment package>.<DeploymentName>.deployment.binfb`; emit value
  `deployment`; FlatBuffers namespace `ridl.deployment`;
  `deployment::SCHEMA_VERSION = 1` (R-1, R-9).
- `ridl-descriptor` keeps building with `--no-default-features` (no `std`) and
  with only `dedup`: the accessors, `finish` and `verify` of the deployment
  schema are in the `no_std` half; `lower` and `to_json` are under `std` (R-7).
  `just wasm-check` and `just compat-check` (rust-version 1.88) cover this.
- The schema is append-only (R-9): field order in each table is the order the
  design's R-3 lists.
- The catalog `.catalog.binfb` output and the catalog hash do not change. No
  existing test changes its expected bytes.
- Prose in comments and docs is plain, literal English; no story ids in
  `crates/`, `docs/book/`, `docs/design/` (`just story-id-check`).
- Implementers run focused crate tests only (`cargo test -p <crate> --locked`);
  the full gate runs in CI.
- Commits: Conventional Commits with scopes from `.git-std.toml`
  (`ridl-descriptor`, `ridlc`, `ridl`, `ridl-ir`, `docs`, `xtask`), each ending
  with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

- A deployment whose closure reaches a catalog of a dependency unit (not one of
  the build's own units): the descriptor embeds that unit's catalog and its hash
  equals the region's hash. Pinned in Task 3
  (`a_dependency_unit_catalog_is_embedded`), over
  `crates/ridlc/tests/corpus/rsdl-appendix-a` if it reaches one, else a two-unit
  fixture written for the test.
- A workspace with a system but no `deployment`, and a deployment blocked by an
  RSDL-7xx error: no file, exit 0 (blocked: ADR-0022 decision 8). Pinned in
  Task 3.
- An instance that offers nothing and maps nothing (an external consumer, a
  component with no lines): listed with empty `offers` and `maps`, not dropped.
  Pinned in Task 2.
- A list-valued and a flag backend key (`linux.cpuset = (2, 3)`, `shm.pinned`):
  carried with `list` and with no value. Pinned in Task 2 over
  `crates/ridlc/tests/corpus/rsdl-diag-showcase` or a hand-built `System`.
- `ridl describe` on a deployment descriptor whose embedded catalog was altered
  after writing: exit 2 naming the catalog. Pinned in Task 3.

---

### Task 1: The schema, accessors, `finish` and `verify`

**Files:**

- Create: `crates/ridl-descriptor/schema/deployment.fbs`
- Create: `crates/ridl-descriptor/src/deployment.rs` (module root: constants,
  `finish`, `verify`, `VerifyError` reuse or own error), and
  `crates/ridl-descriptor/src/deployment/generated.rs` (generated, committed)
- Modify: `crates/ridl-descriptor/src/lib.rs` (add `pub mod deployment;`)
- Modify: `xtask/src/descriptor.rs` (generate both schemas; the staleness test
  `committed_generated_accessors_match_the_schema` covers both, or a second test
  beside it)
- Test: `crates/ridl-descriptor/tests/deployment_round_trip.rs`,
  `crates/ridl-descriptor/tests/deployment_verify.rs`

**Interfaces:**

- Produces:
  - `ridl_descriptor::deployment::{FILE_IDENTIFIER: [u8; 4] = *b"RDLD",
    FILE_SUFFIX: &str = ".deployment.binfb", SCHEMA_VERSION: u32 = 1}`
  - the generated owned and `Ref` types re-exported from
    `ridl_descriptor::deployment`: `Deployment`, `EmbeddedCatalog`,
    `DeployedInterface`, `Machine`, `Component`, `Instance`, `InterfaceKey`,
    `Link`, `Endpoint`, `Crossing`, `Route`, `Surface`, `SurfaceDirection`,
    `Attribute`, `AttributeValue`
  - `pub fn finish(deployment: &Deployment) -> Vec<u8>` (same header swap as
    `ridl_descriptor::finish`; factor the swap into one private helper used by
    both, do not copy it)
  - `pub fn verify(bytes: &[u8]) -> Result<DeploymentRef<'_>, VerifyError>`

- [ ] **Step 1: Write `deployment.fbs`.** Tables and fields in exactly the order
      of design R-3. Exact field names:
  - `Deployment { version: uint32; system: string; name: string; package:
    string; catalogs: [EmbeddedCatalog]; interfaces: [DeployedInterface];
    machines: [Machine]; components: [Component]; instances: [Instance];
    links: [Link]; routes: [Route]; surface: [Surface]; system_attributes:
    [Attribute]; attributes: [Attribute]; }`,
    `root_type Deployment;`, `file_identifier "RDLD";`
  - `EmbeddedCatalog { name: string; hash: [ubyte]; bytes: [ubyte]
    (nested_flatbuffer: "ridl.descriptor.Catalog"); }`
    — if planus 1.3 rejects the `nested_flatbuffer` attribute or the
    cross-namespace include, drop the attribute and keep `[ubyte]`, and say so
    in a schema comment.
  - `InterfaceKey { catalog: string; number: uint32; }` (a table)
  - `DeployedInterface { catalog: string; number: uint32; name: string;
    inline: bool; provisional: bool; service: string; }`
  - `Machine { name: string; external: bool; labels: [string]; attributes:
    [Attribute]; }`
  - `Component { name: string; implicit: bool; external: bool; instances:
    [string]; labels: [string]; attributes: [Attribute]; }`
  - `Instance { component: string; instance: string; machine: string;
    external: bool; attributes: [Attribute]; offers: [InterfaceKey]; maps:
    [string]; }`
  - `Endpoint { component: string; instance: string; machine: string; }`
  - `enum Crossing: ubyte { SameMachine = 0, DifferentMachine = 1, OffBoard =
    2 }`
  - `Link { catalog: string; number: uint32; service: string; consumer:
    Endpoint; producer: Endpoint; crossing: Crossing; attributes: [Attribute];
    }`
  - `Route { catalog: string; number: uint32; ordinal: uint32; interface:
    string; member: string; service: string; producers: [Endpoint]; }`
  - `enum SurfaceDirection: ubyte { ExternalConsumes = 0, ExternalOffers = 1 }`
  - `Surface { link: uint32; direction: SurfaceDirection; }`
  - `Attribute { namespace: string; key: string; value: AttributeValue; }`
  - `AttributeValue { scalar: string; list: [AttributeValue]; }` Header comment:
    the evolution rule copied from `catalog.fbs`, and that the file is the
    deployment descriptor of `docs/design/deployment-descriptor.md` (that record
    is created in Task 5; until then cite the wip design path). Mark `required`
    the same kinds of fields the catalog schema marks required (names and the
    root's lists).
- [ ] **Step 2: Extend `cargo xtask descriptor-codegen`** to generate
      `src/deployment/generated.rs` from the new schema with the same header,
      path relativisation and rustfmt; run it; commit the output.
- [ ] **Step 3: Write the failing tests.**
  - `deployment_round_trip.rs`: `a_deployment_round_trips` (build an owned
    `Deployment` with one of every table and an `AttributeValue` list nested two
    deep, `finish`, `verify`, read every field back);
    `the_identifier_is_at_bytes_4_to_8` (`&bytes[4..8] == b"RDLD"`).
  - `deployment_verify.rs`: `a_short_buffer_is_refused` (`&[0; 7]`),
    `a_catalog_descriptor_is_refused` (bytes of `ridl_descriptor::finish` of a
    minimal catalog → the foreign-identifier variant),
    `a_wrong_version_is_refused` (`version = 2` → the version variant),
    `a_truncated_buffer_is_refused` (drop the last 8 bytes),
    `an_embedded_catalog_that_is_not_a_catalog_is_refused` (an
    `EmbeddedCatalog.bytes` of 16 zero bytes → an error naming the catalog), and
    the reverse in the catalog verifier: `ridl_descriptor::verify` refuses a
    deployment descriptor (foreign identifier).
- [ ] **Step 4: Implement `finish` and `verify`.** `verify` checks length ≥ 8,
      identifier, `read_as_root`, `version == SCHEMA_VERSION`, walks every
      vector and nested table (the catalog verifier's walk is the pattern), and
      calls `ridl_descriptor::verify` on every `EmbeddedCatalog.bytes`. Add one
      `VerifyError` variant for an embedded catalog failure carrying the catalog
      name and the inner error, if the existing enum cannot carry it. It is
      `#[non_exhaustive]`-compatible only if the enum already is; check
      `freeze/` for a dump of `VerifyError` before adding a variant, and run
      `cargo xtask freeze update` if a dump changes.
- [ ] **Step 5: Run** `cargo test -p ridl-descriptor --locked`,
      `cargo test -p ridl-descriptor --locked --no-default-features`,
      `cargo test -p xtask --locked descriptor` — all PASS.
- [ ] **Step 6: Commit**
      `feat(ridl-descriptor): the deployment descriptor schema
      and its verifier`.

### Task 2: The lowering and the JSON view

**Files:**

- Create: `crates/ridl-descriptor/src/deployment/lower.rs`,
  `crates/ridl-descriptor/src/deployment/describe.rs`
- Test: `crates/ridl-descriptor/tests/deployment_lower.rs` (hand-built `System`)

**Interfaces:**

- Consumes: Task 1's owned types, `finish`.
- Produces:
  - `pub fn lower(system: &ridl_ir::v2::System, deployment: &str, packages:
    &[&ridl_ir::v2::Package], catalogs: &BTreeMap<String, Vec<u8>>) ->
    Result<Vec<u8>, DeploymentLowerError>`
    (under `std`)
  - `pub enum DeploymentLowerError { UnknownDeployment(String),
    MissingCatalog(String), HashMismatch { catalog: String } }`
    with `Display` and `std::error::Error`
  - `pub fn to_json(deployment: DeploymentRef<'_>) -> planus::Result<Value>`
    (under `std`)

- [ ] **Step 1: Write the failing tests** in `deployment_lower.rs`, over a
      hand-built `System` of two components (a provider offering one service
      that lists one interface of catalog `veh`, a consumer requiring it), one
      deployment on two machines, a placement line with `shm.service = 3`, a
      flag `shm.pinned`, and a list `linux.cpuset = (2, 3)`, and a third
      component with no lines. `catalogs` maps `veh` to a catalog buffer built
      with `ridl_descriptor::finish` whose hash is the region's.
  - `the_instances_carry_offers_maps_and_placement_keys`: the provider instance
    has `offers == [("veh", 1)]`, `maps == []`, and its attributes include
    `shm.service` with scalar `"3"`; the consumer has `maps == ["veh"]`.
  - `an_instance_with_no_lines_is_listed_with_empty_lists`.
  - `flag_and_list_keys_are_carried`: `shm.pinned` has no value; `linux.cpuset`
    has a list of scalars `"2"`, `"3"`.
  - `the_embedded_catalog_is_the_given_bytes`.
  - `an_unknown_deployment_is_an_error`, `a_missing_catalog_is_an_error`,
    `a_catalog_whose_hash_differs_from_the_region_is_an_error`.
  - `two_lowerings_write_the_same_bytes`.
  - `to_json_names_every_field`: the JSON of the descriptor has the keys of R-3
    and `catalogs[0].hash` as an array of 32 numbers (the catalog `describe`
    convention); `bytes` is not printed in full — print its length as
    `bytes_len` and the embedded catalog's JSON under `catalog`.
- [ ] **Step 2: Run**
      `cargo test -p ridl-descriptor --locked --test
      deployment_lower` —
      FAIL (no `lower`).
- [ ] **Step 3: Implement `lower`.** Find the deployment by name in
      `system.deployments` (else `UnknownDeployment`). Call
      `ridl_ir::codegen::lower_deployment(system, deployment, packages)` and
      take `instances` (component, instance, machine, external, offers, maps)
      from its result, so the two emitters cannot disagree; add each instance's
      attributes from the matching `Placement`. Regions: from `system.regions`
      with their hash; for each, take `catalogs[name]` (else `MissingCatalog`),
      compute its hash by reading the embedded buffer's `hash` field after
      `ridl_descriptor::verify` (else `HashMismatch`). Everything else is a
      field-by-field projection of `System` and the `Deployment` in the order
      R-3 states. `link` of a `Surface` is the index of the equal `Link` in the
      deployment's link list.
- [ ] **Step 4: Implement `to_json`** following `ridl_descriptor::describe`'s
      conventions (key order as the schema's, `null` for an absent field).
- [ ] **Step 5: Run** `cargo test -p ridl-descriptor --locked` — PASS.
- [ ] **Step 6: Commit**
      `feat(ridl-descriptor): lower a deployment descriptor
      from the system IR`.

### Task 3: `--emit deployment` and `ridl describe`

**Files:**

- Modify: `crates/ridlc/src/lib.rs` (`Emit::Deployment`, the per-unit catalog
  bytes shared by `write_catalogs` and the deployment emit, the write), and
  every `match` over `Emit` it touches
- Modify: `crates/ridl/src/main.rs` (`run_describe` dispatches on bytes 4..8)
- Modify: `docs/book/cli-reference.md` (the `--emit` values and `describe`),
  `freeze/cli.txt` via `cargo xtask freeze update`
- Test: `crates/ridlc/tests/deployment_descriptor.rs`, and the describe cases in
  the existing `ridl` CLI test file that covers `describe` (find it with
  `grep -rln describe crates/ridl/tests`)

**Interfaces:**

- Consumes:
  `ridl_descriptor::deployment::{lower, FILE_SUFFIX, verify,
  to_json}`,
  Task 2.

- [ ] **Step 1: Write the failing tests** in `deployment_descriptor.rs` (reuse
      the `system_and_packages` approach of `crates/ridlc/tests/layout.rs` or
      drive the `ridlc` binary as `crates/ridlc/tests/cli.rs` does):
  - `cabin_writes_one_descriptor_per_deployment`:
    `ridlc build --emit
    deployment` over `examples/cabin` writes
    `<pkg>.Bench.deployment.binfb` (and one per other deployment); each
    verifies; the `Climate` instance's offers name the cabin interface.
  - `the_embedded_catalog_equals_the_standalone_file`: with
    `--emit catalog
    --emit deployment`, each embedded `bytes` equals the
    `.catalog.binfb` written beside it.
  - `a_dependency_unit_catalog_is_embedded` (Review Focus 1).
  - `no_deployment_writes_no_file` (a system with no deployment) and
    `a_blocked_deployment_writes_no_file` (a deployment with an RSDL-7xx error;
    reuse a corpus of `crates/ridlc/tests/corpus/rsdl-diag-showcase` if one is
    blocked, else a fixture), both exit 0 or the build's normal exit for those
    diagnostics.
  - `single_file_mode_writes_no_deployment_descriptor`.
  - `ridl describe` tests: a deployment descriptor prints JSON with
    `"name": "Bench"`; a descriptor with a byte flipped inside an embedded
    catalog exits 2 and stderr names the catalog; a catalog descriptor still
    prints as before.
- [ ] **Step 2: Run** them — FAIL.
- [ ] **Step 3: Implement.** Compute the catalog bytes per unit once (own units
      with their `compatible` lists as now; every other unit a region names,
      lowered over the same scope with the compatible list the build has for it,
      empty if none); `write_catalogs` writes the own units from that map; the
      deployment emit lowers each deployment of `System.deployments` with the
      whole map. A lowering error is an I/O error (exit 2), as for the catalog.
      Single-file mode and a build with no system write nothing. `run_describe`:
      if `bytes[4..8] == b"RDLD"` use the deployment verifier and `to_json`,
      else the catalog path unchanged; a verify error of an embedded catalog
      exits 2 with the catalog's name in the message.
- [ ] **Step 4: Run**
      `cargo test -p ridlc --locked --test
      deployment_descriptor`,
      `cargo test -p ridl --locked`, then `cargo xtask
      freeze update` and
      check that only `freeze/cli.txt` changed (and only by the new value) —
      PASS.
- [ ] **Step 5: Update `docs/book/cli-reference.md`**: add `deployment` to the
      `--emit` list with its file name, and the deployment descriptor to
      `ridl describe`; run `just book-check link-check doc-path-check`.
- [ ] **Step 6: Commit** `feat(ridlc): emit the deployment descriptor`.

### Task 4: The placement's backend keys in the codegen deployment section

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`
  (`message
  Attribute`, `message AttributeValue`, `message AttributeList` with
  the IR's shapes; `Instance.attributes = 7`)
- Modify: `crates/ridl-ir/src/codegen/deployment.rs` (`instances`)
- Modify: `docs/design/codegen-plugins.md` §7 "What the section carries", item 3
- Modify: the codegen-protocol freeze dump via `cargo xtask freeze update`
- Test: the existing deployment section tests in
  `crates/ridl-ir/src/codegen/deployment.rs` or `crates/ridlc/tests/layout.rs`

**Interfaces:**

- Produces: `v1::Instance.attributes: Vec<v1::Attribute>` — Task 2's lowering
  may then read attributes from here instead of from `Placement`; leave Task 2's
  code as it is unless the change is one line.

- [ ] **Step 1: Write the failing test**
      `an_instance_carries_its_placement_line_keys`: a placement with
      `shm.service = 3` gives
      `Instance.attributes == [Attribute { namespace:
      "shm", key: "service", value: scalar "3" }]`;
      an instance placed with no keys has an empty list.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement**: add the messages and the field, copy the
      placement's attributes in `instances`.
- [ ] **Step 4: Run** `cargo test -p ridl-ir --locked`,
      `cargo test -p ridlc
      --locked --test layout --test codegen_model`,
      `cargo xtask freeze update`; the dump changes only by the new field —
      PASS.
- [ ] **Step 5: Update `codegen-plugins.md` §7** item 3: "the backend keys of
      the placement line that placed the instance".
- [ ] **Step 6: Commit**
      `feat(ridl-ir): carry the placement's backend keys in
      the codegen deployment section`.

### Task 5: Records

**Files:**

- Create: `docs/design/deployment-descriptor.md` (as-built record: the artifact,
  the contents, the instance number rule R-4, where the code is, the tests
  table, open items)
- Modify: `docs/design/catalog-descriptor.md` ("Only the catalog half…" and "Not
  built": the system descriptor is built as the deployment descriptor; D-9 row),
  `docs/decisions/ADR-0022-rsdl-system-in-the-ir.md` (an amendment line in
  Status and a decision 12: the deployment descriptor is the second emitter over
  `System`), `docs/technotes/rsdl-implementation.md` ("What is not built yet"),
  `docs/decisions/README.md` if it summarises ADR-0022's decisions, `AGENTS.md`
  (the ADR-0022 reading line names the new record),
  `crates/ridl-descriptor/schema/deployment.fbs` header path,
  `docs/book/SUMMARY.md` only if design records are listed there.
- [ ] **Step 1:** write and amend the records; cite #1176.
- [ ] **Step 2: Run** `just link-check doc-path-check story-id-check book-check`
      and `prim lint` — PASS.
- [ ] **Step 3: Commit** `docs(docs): record the deployment descriptor`.

Gardening (moving this plan and the design to `docs/archive/`) runs at branch
finish with `sdd-gardening`, not in this task.
