# Layout inputs for backend plugins — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** a backend plugin computes a deployment's shared-memory layouts and
socket message layouts from its `CodegenRequest` alone, and a test plugin proves
it over `examples/cabin`.

**Architecture:** the request gains a deployment section emitted from the
lowered system IR by `ridl_ir::codegen::lower_deployment` and selected by
`ridl build --deployment`; the model gains per-encoding size states, the
reservation and the table budget; rsdl gains the `depth`, `slots` and `budget`
keys on the `deployment` declaration and on placement lines, lowered into
dedicated IR fields; a toolchain-held table carries the known bindings'
overheads.

**Tech Stack:** Rust (pinned by `rust-toolchain.toml`), protobuf through
`protox`, `prost` and `pbjson`, `clap`, `insta` snapshots in the corpus, `just`
recipes as the gate.

**Spec:** `docs/wip/2026-10-05-layout-inputs-design.md` (the design; its
decision numbers D-1 to D-12 are cited below). Executors read both.

## 1. The stage driver

This section is the driver prompt of lane S (the lanes plan,
`2026-09-13-step1-lanes-plan.md`). A fresh session runs one stage. Paste the
stage's line into the session and point it here.

**Stages and tasks.**

| Stage | Stories                                      | Tasks       | Starts when                                  | Model                                                              |
| ----- | -------------------------------------------- | ----------- | -------------------------------------------- | ------------------------------------------------------------------ |
| S2a   | E17.1 the system in the codegen model (#716) | 1 to 6      | this plan is merged                          | Sonnet stage agent, one Sonnet implementer per task, Opus reviewer |
| S2b   | E17.2 the size states (#717)                 | 7 to 10     | this plan is merged; in parallel with S2a    | Sonnet stage agent and implementers, Opus reviewer                 |
| S2c   | E17.3 the binding overheads (#718)           | 11; then 12 | 11 with S2a; 12 when driftsys/ridl#265 lands | Sonnet                                                             |
| S3    | E17.4 the rsdl sizing keys (#719)            | 13 to 18    | S2a merged                                   | Fable for Tasks 14, 16 and 17; Sonnet for the rest; Opus reviewer  |
| S4    | E17.5 the proof and the records (#720)       | 19 to 21    | S2a, S2b, S2c (Task 12) and S3 merged        | Opus for Task 19 and its fixture; Sonnet for the rest              |

**Per stage.** Start from `main` in a worktree named after the stage, run
`./bootstrap`, and read `AGENTS.md`, the spec and this plan before the first
edit. One pull request per story; each task is one or more commits inside it.
Run the review (`~/.claude/commands/review.md`) pass 1 and pass 2 on the pull
request, fix what survives, merge when CI is green, and close the story's issue
from the pull request body. Then stop; the next stage is a fresh session.

**Rules that bind every stage** (from the handoff and the lanes plan §6):

- `model.proto`, `plugin.proto` and `system.proto` change additively only (IR
  stability D-5). Every in-tree backend, `ridlc-gen-model` and `ridlc-gen-rust`
  keep passing. The stage that changes the model (S2a, S2b) files a heads-up
  issue on `driftsys/ridlc-gen-kotlin` naming the new fields.
- Lane S holds the rsdl grammar and checker files while S3 runs
  (`crates/ridl-syntax/src/parser.rs`, `crates/ridl-sem/src/rsdl/`). No other
  lane changes them in that window.
- `repr(C)` (driftsys/ridl#317) is not a gate: no channel gets that encoding and
  no size state exists for it.
- Shipped docs and rustdoc name no story id, no `epic E<n>`, no `lane <Letter>`
  (`just story-id-check`); prose is plain and literal.
- A pull request body writes "closes #N" only for the issue it closes; a negated
  closing keyword still closes.
- Every decision taken on Sebastien's behalf is appended to the spec's §8 under
  a new `DD-n` with the stage that took it.
- The gate before a pull request is `just verify`. A stage agent runs it itself
  and reads the tail of the output, not the whole log.

**Hand-back.** A stage agent that waits on its review driver stays in the
session until the ledger is back; it does not hand back with the review pending.
S4 gardens: the spec and this plan move to `docs/archive/` through the
`sdd-gardening` skill after Task 21, and their README entries are rewritten.

## Global Constraints

- Field numbers are the spec's §5, verbatim; no other number is used.
- Order of every repeated field is the spec's D-2 order; two builds of one
  workspace write the same bytes.
- `slots` default is 16; `slots` range is 1 to 65536; `depth` range is 1 to
  4294967295; `budget` range is 1 to 18446744073709551615; `budget` default is
  none.
- The derived depth is `ceil(max_us / min_us)` in exact integer arithmetic.
- Encoding per crossing: same machine gives `ENCODING_FLATBUFFERS`, the two
  other crossings give `ENCODING_PROTO3`.
- Diagnostic codes: RSDL-709 (error), RSDL-805 lint `depth-below-bound`
  (warning), RSDL-806 lint `depth-underivable` (warning).
- The toolchain's generated JSON reader stays strict; `ignore_unknown_fields` is
  never set.
- Commit messages are Conventional Commits with a scope from `.git-std.toml`
  (`ridl-ir`, `ridlc`, `ridl`, `ridl-sem`, `ridl-syntax`, `ridl-core`,
  `ridl-descriptor`, `docs`, `roadmap`, `adr`, `repo`); a docs-only change under
  `docs/wip/` uses `docs(docs)`.

## Review Focus

1. A workspace with no `system`, or several deployments and no `--deployment`,
   must build a request byte for byte equal to today's. Pinned in Task 5.
2. A redundant provider set (a component with two instances offering one
   service, RSDL-409) gives two channels per member, each with its own consumer
   links. Pinned in Task 4.
3. An event with an explicit half-open range and no declared `depth` gives an
   absent depth with source `UNDERIVABLE` and one RSDL-806 per link, never a
   panic. Pinned in Tasks 4 and 17.
4. `budget = 0`, `budget = 18446744073709551616` and `slots = 65537` each draw
   RSDL-709 and block only their deployment. Pinned in Task 14.
5. Reordering machines or placement lines in the source does not change the
   request's bytes. Pinned in Task 4.

---

## 2. Stage S2a — E17.1, the system in the codegen model

### Task 1: The cabin system

**Files:**

- Modify: `examples/cabin/cabin.ridl` (add one `service`)
- Create: `examples/cabin/system.rsdl`
- Test: `crates/ridlc/tests/cabin_example.rs`

**Interfaces:**

- Produces: the workspace `examples/cabin` with one system `Vehicle`, three
  components, one deployment `Bench`. Later tasks' tests read this deployment by
  name.

- [ ] **Step 1: Write the failing test** in `cabin_example.rs`:

```rust
#[test]
fn cabin_lowers_one_deployment_with_three_links() {
    // compile_workspace over examples/cabin, then lower_workspace_system
    let system = system_of_cabin(); // helper in this file: compile_workspace + lower_workspace_system
    assert_eq!(system.deployments.len(), 1);
    let bench = &system.deployments[0];
    assert_eq!(bench.name, "Bench");
    let crossings: Vec<i32> = bench.links.iter().map(|l| l.crossing).collect();
    assert_eq!(crossings, vec![CROSSING_SAME_MACHINE, CROSSING_SAME_MACHINE, CROSSING_DIFFERENT_MACHINE]);
    assert_eq!(system.regions.len(), 1);
    assert_eq!(system.regions[0].catalog, "veh.cabin");
    assert_eq!(system.regions[0].interfaces.len(), 2);
}
```

- [ ] **Step 2: Run it**:
      `cargo test -p ridlc --test cabin_example cabin_lowers 2>&1 | tail -5`.
      Expected: FAIL, no system (0 deployments).

- [ ] **Step 3: Add to `cabin.ridl`**, after the two interfaces, documented:

```ridl
/// The cabin's one service: both interfaces, provided by one component.
service veh.cabin.control : Cabin, Horn
```

- [ ] **Step 4: Create `examples/cabin/system.rsdl`** (every declaration
      documented with `///`, as `cabin.ridl` is):

```rsdl
package veh.cabin

system Vehicle { Climate, Panel, Telemetry }

component Climate   { offers veh.cabin.control }
component Panel     { requires Cabin
                      requires Horn }
component Telemetry { requires Cabin }

deployment Bench for Vehicle {
  machine Hpc     { Climate, Panel }
  machine Gateway { Telemetry }
}
```

Keep the declaration order above: Panel's two links are the same-machine pair,
Telemetry's is the different-machine link; neither machine is `external`.

- [ ] **Step 5: Run the test and the demo**: the Step 1 test passes;
      `just demo 2>&1 | tail -3` passes (the service changes the Rust crate's
      emitted surface; if the consumer no longer compiles, fix the consumer and
      say so in the commit body);
      `cargo test -p ridlc --test corpus 2>&1 | tail -3` passes.

- [ ] **Step 6: Commit**:
      `feat(repo): give examples/cabin a system and a deployment`.

### Task 2: `deployment.proto` and the request field

**Files:**

- Create: `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`
- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto` (field 6),
  `crates/ridl-ir/build.rs` (the `protos` array)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

**Interfaces:**

- Produces: `v1::Deployment`, `v1::Region`, `v1::RegionInterface`,
  `v1::InterfaceKey`, `v1::Instance`, `v1::Endpoint`, `v1::Channel`,
  `v1::Consumer`, `v1::Depth`, `v1::Binding`, enums `v1::Crossing`,
  `v1::Encoding`, `v1::ValueSource`;
  `v1::CodegenRequest::deployment:
  Option<Deployment>`. Field numbers and
  names: spec §5.1.

- [ ] **Step 1: Write the failing tests** in `tests.rs`:

```rust
#[test]
fn a_request_without_a_deployment_serializes_as_before() {
    let request = v1::CodegenRequest { schema: SCHEMA.into(), toolchain: "t".into(), model: Some(v1::Model::default()), options: vec![], artifact_base: "a".into(), deployment: None };
    let json = request_to_json(&request).unwrap();
    assert!(!json.contains("deployment"));
}
#[test]
fn a_request_with_a_deployment_round_trips() {
    let deployment = v1::Deployment { system: "veh.cabin.Vehicle".into(), name: "Bench".into(), ..Default::default() };
    let request = v1::CodegenRequest { deployment: Some(deployment), ..minimal_request() };
    let json = request_to_json(&request).unwrap();
    assert_eq!(request_from_json(&json).unwrap(), request);
}
```

- [ ] **Step 2: Run**: `cargo test -p ridl-ir a_request_with 2>&1 | tail -5`.
      Expected: FAIL to compile (no field `deployment`).

- [ ] **Step 3: Write `deployment.proto`** from spec §5.1 verbatim (the header
      comment follows `model.proto`'s: what the file is, the order rule, the
      numbering scheme), add `optional Deployment deployment = 6;` to
      `CodegenRequest` with a comment citing D-1, add the file to the `protos`
      array in `build.rs`.

- [ ] **Step 4: Run** the two tests and `cargo test -p ridl-ir 2>&1 | tail -3`.
      Expected: PASS. `cargo clippy -p ridl-ir --all-targets -- -D warnings`
      clean; if `large_enum_variant` fires, box as `build.rs` does for other
      fields.

- [ ] **Step 5: Commit**:
      `feat(ridl-ir): add the deployment section to the codegen request schema`.

### Task 3: `ceil_ratio` over exact-decimal microseconds

**Files:**

- Create: `crates/ridl-ir/src/codegen/depth.rs`
- Modify: `crates/ridl-ir/src/codegen.rs` (`mod depth;`)

**Interfaces:**

- Produces: `pub fn ceil_ratio(max_us: &str, min_us: &str) -> Option<u32>`.
  `None` when either string is not `digits[.digits]`, when `min_us` is zero, or
  when the quotient exceeds `u32::MAX`.

- [ ] **Step 1: Write the failing tests** in `depth.rs`'s `mod tests`:

```rust
assert_eq!(ceil_ratio("1000000", "100000"), Some(10));   // @[100ms..1s]
assert_eq!(ceil_ratio("1000000", "300000"), Some(4));    // 3.33 rounds up
assert_eq!(ceil_ratio("500.5", "0.25"), Some(2002));
assert_eq!(ceil_ratio("10000", "10000"), Some(1));
assert_eq!(ceil_ratio("1", "0"), None);
assert_eq!(ceil_ratio("1e3", "1"), None);
assert_eq!(ceil_ratio("4294967296", "1"), None);
```

- [ ] **Step 2: Run**: `cargo test -p ridl-ir depth:: 2>&1 | tail -5`. Expected:
      FAIL (module missing).

- [ ] **Step 3: Implement**: split each operand at `.`, pad the fractional parts
      to the same length, parse both as `u128`, compute `(max + min - 1) / min`,
      convert with `u32::try_from(..).ok()`.

- [ ] **Step 4: Run**: PASS. **Step 5: Commit**:
      `feat(ridl-ir): derive an event ring depth from its timing bounds`.

### Task 4: The deployment emitter

**Files:**

- Create: `crates/ridl-ir/src/codegen/deployment.rs`
- Modify: `crates/ridl-ir/src/codegen.rs`
  (`mod deployment; pub use deployment::lower_deployment;`)
- Test: `crates/ridl-ir/src/codegen/deployment.rs` (`mod tests`)

**Interfaces:**

- Consumes: `ceil_ratio` (Task 3); `v2::System`, `v2::Package`.
- Produces:
  `pub fn lower_deployment(system: &v2::System, name: &str, packages: &[&v2::Package]) -> Option<v1::Deployment>`.
  `None` when no deployment of `system` is named `name`. `packages` holds every
  package of the workspace, `ridl.std` included; the function finds a route's
  interface by (catalog = package name, interface name, `inline`) and the member
  by ordinal.

**The algorithm** (D-2, D-4, D-5), since the signature does not determine it:

1. `regions`: copy `system.regions` in order; `RegionInterface.service` from the
   IR.
2. `instances`: one per `deployment.placements` entry, in that order. `external`
   from the component's flag; `offers` = the `RegionInterface`s whose `service`
   is one the component's `offers` lines name, as `InterfaceKey`s; `maps` = the
   `Grant` of that component, its `regions`.
3. `channels`: for each `deployment.routes` entry, for each of its `producers`
   (in order): one channel. `kind` from the member's definition. `consumers`:
   every `deployment.links` entry whose `interface` is the route's interface and
   whose `producer` equals this producer, mapped to
   `Consumer { crossing, encoding: encoding_of(crossing), .. }`, sorted by
   (component, instance). For an event: `depth` per consumer =
   `Depth {
   value: ceil_ratio(max, min), source: DERIVED }` or
   `{ value: None, source:
   UNDERIVABLE }` when either bound is absent or
   `ceil_ratio` is `None`; the channel's `depth` = the max of the consumers'
   values, absent if any is absent. For a command or query:
   `slots: Some(16), slots_source: DEFAULT,
   budget: None, budget_source: UNSPECIFIED`.
   For a signal or fixed: no sizing fields. Task 16 adds the declared sources.
4. `bindings`: `bindings::KNOWN` (Task 11); until Task 11 lands, empty.

- [ ] **Step 1: Write the failing tests** with a `fixture()` helper that builds,
      by struct literals, one `v2::Package` (`veh.cabin`-like: an interface with
      an event `@[100ms..1s]`, a query, a signal) and one `v2::System` with a
      deployment of two machines, a provider with two instances (`primary`,
      `backup`) and two consumers, one per machine:

```rust
#[test] fn an_unknown_deployment_name_gives_none()
#[test] fn each_producer_instance_of_a_route_is_one_channel()      // 2 instances -> 2 channels per member
#[test] fn a_same_machine_link_is_flatbuffers_and_a_different_machine_link_is_proto3()
#[test] fn an_event_channel_derives_its_depth_and_the_ring_depth_is_the_max()  // 10, DERIVED
#[test] fn a_half_open_event_has_an_absent_underivable_depth()
#[test] fn a_call_channel_carries_sixteen_default_slots_and_no_budget()
#[test] fn a_signal_channel_carries_no_sizing()
#[test] fn instances_list_what_they_offer_and_what_they_map()
#[test] fn reordering_placements_in_the_system_does_not_change_the_bytes()   // permute system.deployments[0].placements, compare request_to_json
```

- [ ] **Step 2: Run**: `cargo test -p ridl-ir deployment:: 2>&1 | tail -5`.
      Expected: FAIL (function missing).

- [ ] **Step 3: Implement `lower_deployment`** per the algorithm, with private
      helpers `encoding_of(crossing: i32) -> i32`,
      `member_kind(packages, route) -> (v1::Kind, Option<&v2::Timing>)`,
      `depth_of(timing: Option<&v2::Timing>) -> v1::Depth`.

- [ ] **Step 4: Run** the module's tests and
      `cargo test -p ridl-ir 2>&1 | tail -3`. PASS.

- [ ] **Step 5: Commit**:
      `feat(ridl-ir): emit the deployment section from the lowered system`.

### Task 5: Selection and wiring in `ridlc` and the two binaries

**Files:**

- Modify: `crates/ridlc/src/lib.rs` (`run_build_with`, `codegen_request`,
  `write_emits`, the caller near line 933), `crates/ridl/src/main.rs` (`Build`),
  `crates/ridlc/src/main.rs` (`Build`),
  `crates/ridlc-gen-model/tests/parity.rs:83`,
  `crates/ridlc-gen-rust/tests/parity.rs:97`, `docs/book/cli-reference.md` (the
  two `build` help transcripts)
- Test: `crates/ridlc/tests/cli.rs`, `crates/ridlc/tests/codegen_model.rs`

**Interfaces:**

- Consumes: `lower_deployment` (Task 4).
- Produces:
  `pub fn codegen_request(base: &str, package: &v2::Package, others: &[&v2::Package], options: Vec<v1::BackendOption>, deployment: Option<v1::Deployment>) -> v1::CodegenRequest`;
  `pub fn run_build_with(entry, out_dir, emits, plugins, plugin_timeout, frozen, apply_lints, deployment: Option<&str>) -> io::Result<CliRun>`;
  `pub fn select_deployment(system: Option<&v2::System>, name: Option<&str>, packages: &[&v2::Package]) -> Result<Option<v1::Deployment>, UnknownDeployment>`
  where `UnknownDeployment { requested: String, known: Vec<String> }`. Rule
  (D-11): `name` given and found gives `Some`; given and not found gives `Err`;
  absent and exactly one deployment gives `Some`; otherwise `None`.

- [ ] **Step 1: Write the failing tests**:

```rust
// codegen_model.rs
#[test] fn a_request_for_cabin_carries_its_one_deployment_without_a_flag()   // compile examples/cabin; select_deployment(.., None, ..) is Some, name "Bench", 5 channels
#[test] fn a_workspace_without_a_system_builds_the_request_of_today()        // the corpus entry with no system: codegen_request(.., None) json has no "deployment" key
#[test] fn two_deployments_and_no_flag_select_none()                          // rsdl-appendix-a has Production and Bench
#[test] fn an_unknown_name_lists_the_known_deployments()                      // Err { requested: "Nope", known: ["Bench", "Production"] }
// cli.rs
#[test] fn build_with_an_unknown_deployment_exits_two_and_names_the_known_ones() // ridl build examples/cabin --deployment Nope -> exit 2, stderr contains "Bench"
```

- [ ] **Step 2: Run**:
      `cargo test -p ridlc --test codegen_model --test cli deployment 2>&1 | tail -8`.
      FAIL.

- [ ] **Step 3: Implement**: add the parameter to `codegen_request` and
      `run_build_with`; in `run_build_with`, after the system is lowered, call
      `select_deployment` once and pass the result to every `write_emits` call,
      which hands it to `codegen_request`. An `Err` exits 2 with
      `error: no deployment named`Nope`; the system declares: Bench, Production`.
      Add `#[arg(long, value_name = "NAME")] deployment: Option<String>` with
      the doc comment "The deployment to carry in each codegen request. With one
      deployment in the workspace it is selected without this flag; with
      several, none is carried unless named." to both `Build` structs. Update
      the two parity tests to pass `None`. Paste the new `--help` transcripts
      into `docs/book/cli-reference.md` (the `cli_reference` tests compare
      them).

- [ ] **Step 4: Run**
      `cargo test -p ridlc -p ridl-cli -p ridlc-gen-model -p ridlc-gen-rust 2>&1 | tail -3`.
      PASS.

- [ ] **Step 5: Commit**:
      `feat(ridlc): select a deployment and carry it in every codegen request`.

### Task 6: The record and the heads-up

**Files:**

- Modify: `docs/design/codegen-plugins.md` (new section "The deployment
  section"), `docs/technotes/rsdl-implementation.md` (the "What is not built
  yet" list: the request now reads the lowered system)
- Create: a heads-up issue on `driftsys/ridlc-gen-kotlin`

- [ ] **Step 1: Write the section**: what the section carries (D-2), the order
      rule, the encoding rule (D-4), the depth rule (D-5) with the sources, the
      default slots, the selection (D-11), and that a request with no deployment
      is unchanged. No story ids.
- [ ] **Step 2: Run**
      `just link-check 2>&1 | tail -2 && just doc-path-check 2>&1 | tail -2 && just story-id-check 2>&1 | tail -2 && just check 2>&1 | tail -2`.
      PASS.
- [ ] **Step 3: File the heads-up** with
      `gh issue create --repo driftsys/ridlc-gen-kotlin` naming
      `CodegenRequest.deployment = 6` and the new messages, and that a lenient
      reader ignores them. Also file, on driftsys/ridl, the follow-up for a
      `codegen-request` emit (D-12), and the spec's §10 items for `repr(C)` and
      the per-member grain if they are not filed yet.
- [ ] **Step 4: Commit**:
      `docs(docs): record the deployment section of the codegen request`.

## 3. Stage S2b — E17.2, the size states

### Task 7: Move the sizer into `ridl-ir`

**Files:**

- Create: `crates/ridl-ir/src/projection/size.rs` (and `size/proto3.rs`,
  `size/flatbuffers.rs`, moved from `crates/ridl-descriptor/src/size.rs` and
  `size/`)
- Modify: `crates/ridl-descriptor/src/lib.rs`, `src/lower.rs` (map the moved
  types onto the generated FlatBuffers enums), delete `src/size.rs` and
  `src/size/`
- Test: `crates/ridl-ir/src/projection/size.rs` (`mod tests`, the moved tests
  plus the cause tests)

**Interfaces:**

- Produces, in `ridl_ir::projection::size`:
  `pub enum Encoding { Proto3, FlatBuffers, ReprC }`,
  `pub enum SizeState { Bounded(u32), Unbounded(UnboundedCause), Absent(AbsentCause) }`,
  `pub enum AbsentCause { EncodingUndefined, NoMessage, RefusedMember, NoBound, Overflow, Unresolved }`,
  `pub fn size_state(shape: &PayloadShape<'_>, ctx: &Ctx<'_>, encoding: Encoding) -> SizeState`
  (the shape, not only a name: `PayloadShape::Params` with zero or several
  entries, `Return` with a fallible, and a stream field give
  `Absent(EncodingUndefined)`), `Ctx::new`, `PayloadShape`, `string_max_bytes`.
  `UnboundedCause` is `ridl_ir::codegen::fb_unbounded`'s cause type,
  re-exported.

- [ ] **Step 1: Write the failing cause tests** in the new module (the moved
      tests keep their assertions, rewritten to `SizeState::Absent(_)` where
      they asserted `None`):

```rust
#[test] fn a_named_scalar_has_no_proto3_message()        // Absent(NoMessage), FlatBuffers Bounded
#[test] fn a_stream_payload_is_undefined_in_both_encodings()
#[test] fn two_parameters_are_undefined_in_both_encodings()
#[test] fn a_fallible_reply_is_undefined_in_both_encodings()
#[test] fn an_unbounded_string_has_no_bound()            // Absent(NoBound) in both
#[test] fn repr_c_is_absent_with_encoding_undefined()
```

- [ ] **Step 2: Run**: `cargo test -p ridl-ir size:: 2>&1 | tail -5`. FAIL.
- [ ] **Step 3: Move the files**, add the enums, make `ridl-descriptor` map
      `Absent(_)` to "no row" and the other two states as before.
- [ ] **Step 4: Run** `cargo test -p ridl-ir -p ridl-descriptor 2>&1 | tail -3`
      and `cargo test -p ridlc --test corpus 2>&1 | tail -3` (the catalog
      snapshots are unchanged). PASS.
- [ ] **Step 5: Commit**:
      `refactor(ridl-ir): move the payload sizer out of ridl-descriptor`.

### Task 8: `Payload.sizes`

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (`SizeAbsent`,
  `AbsentCause`, `SizeState`, `PayloadSizes`, `Payload.sizes = 3`, spec §5.2),
  `crates/ridl-ir/src/codegen/lower.rs:916` (`payload`)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

- [ ] **Step 1: Write the failing tests**:

```rust
#[test] fn a_payload_carries_both_size_states()                 // sizes.flatbuffers == Bounded(flatbuffers_max_size), sizes.proto3 Bounded for a struct
#[test] fn field_two_still_equals_the_flatbuffers_bounded_value()
#[test] fn an_unbounded_payload_keeps_its_cause_in_the_state()  // unbounded.cause == fb_unbounded's
```

- [ ] **Step 2: Run**: FAIL. **Step 3: Implement**: `payload()` calls
      `size::size_state(&PayloadShape::Named(reference), &ctx, Proto3)` and
      `FlatBuffers`, maps to `v1::SizeState`; `flatbuffers_max_size` stays
      computed as today. **Step 4: Run** `cargo test -p ridl-ir 2>&1 | tail -3`,
      then `cargo test -p ridlc --test corpus 2>&1 | tail -3` and review the
      `codegen` snapshot diffs with `cargo insta review` (only `sizes` keys are
      added). **Step 5: Commit**:
      `feat(ridl-ir): carry a size state per encoding on every payload`.

### Task 9: Request and reply size states for every shape

**Files:**

- Modify: `model.proto` (`CommandShape.request_sizes = 4`,
  `QueryShape.request_sizes = 6`, `QueryShape.reply_sizes = 7`), `lower.rs`
  (`interaction`, near 739)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

- [ ] **Step 1: Tests**:
      `a_two_parameter_command_has_an_undefined_request_size`,
      `a_one_parameter_command_request_sizes_equal_its_payload_sizes`,
      `a_fallible_query_reply_is_undefined`,
      `a_zero_parameter_query_request_is_undefined`.
- [ ] **Step 2: FAIL. Step 3: Implement** with `PayloadShape::Params(&params)`
      and `PayloadShape::Return(&ret)`. **Step 4: PASS**, snapshots reviewed.
      **Step 5: Commit**:
      `feat(ridl-ir): size every request and reply shape, absent with a cause where no codec defines it`.

### Task 10: Reservation and table budget

**Files:**

- Modify: `model.proto` (`ReservationState`, `Reservation`,
  `Interaction.reservation = 8`, `Interface.table_budget = 9`), `lower.rs`
  (`interaction`, `interface`)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

**Rule (D-8):** per encoding, a member's reservation is the saturating `u64` sum
of the bounded sizes of its payloads: signal, event, fixed: the one payload;
command: the request; query: the request then the reply. The first payload whose
state is not `bounded` makes the reservation `unsized` naming
`"<member>.<role>: <type>"`. The interface's table budget is the saturating sum
over its live members in `MEMBERS` order, `unsized` naming the first unsized
member.

- [ ] **Step 1: Tests**: `a_query_reserves_request_plus_reply`,
      `a_command_reserves_its_request_only`,
      `a_table_budget_sums_every_live_member`,
      `one_unsized_member_makes_the_budget_unsized`,
      `a_tombstone_does_not_count`.
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS**, snapshots reviewed.
      Also update `docs/design/catalog-descriptor.md` "The size states" (the
      model now carries the states too) and `docs/design/codegen-plugins.md`.
      **Step 5: Commit**:
      `feat(ridl-ir): tabulate the reservation and the table budget per encoding`.
      File the Kotlin heads-up for Tasks 8 to 10 (one issue).

## 4. Stage S2c — E17.3, the binding overheads

### Task 11: The binding table and its plumbing

**Files:**

- Create: `crates/ridl-ir/src/codegen/bindings.rs`
- Modify: `crates/ridl-ir/src/codegen/deployment.rs` (write `bindings`)
- Test: `deployment.rs` tests

**Interfaces:**

- Produces:
  `pub struct Known { pub name: &'static str, pub version: &'static str, pub frame_header_max_bytes: Option<u32>, pub envelope_bytes: Option<u32> }`,
  `pub const KNOWN: &[Known] = &[];`, `pub fn bindings() -> Vec<v1::Binding>`
  (name order).

- [ ] **Step 1: Test** `the_binding_list_is_the_known_table_in_name_order`
      (asserts it is empty today and equals `bindings()`).
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS. Step 5: Commit**:
      `feat(ridl-ir): carry the known transport bindings' overheads in the deployment section`.

### Task 12: The WebSocket row (waits on driftsys/ridl#265)

**Files:**

- Modify: `bindings.rs` (one row), the binding's test crate
- Test: a test beside the binding that frames one message of `examples/cabin`'s
  `Cabin.temperature` and asserts
  `frame.len() <= frame_header_max_bytes + envelope_bytes + proto3_bound`.

- [ ] **Step 1**: read the binding document's header layout (frame specification
      §10 item 2) and compute the maximum header size and the envelope size by
      hand; write both into the row with the document's version.
- [ ] **Step 2**: the test above; FAIL, then PASS. **Step 3: Commit**:
      `feat(ridl-ir): add the WebSocket binding's header and envelope sizes`.
      Update `docs/design/codegen-plugins.md`.

## 5. Stage S3 — E17.4, the rsdl sizing keys

### Task 13: The three codes

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (three rows after RSDL-708 and
  RSDL-804; the lint-name list near line 1978), `docs/book/lints.md` (two rows),
  `docs/specification/rsdl-language-reference.md` §16.1 (three rows),
  `crates/ridlc/tests/corpus.rs` (`RSDL_PROFILE_CODES`),
  `crates/ridlc/tests/corpus/rsdl-diag-showcase/` (a living example of each;
  RSDL-805 and 806 need an event with timing in the showcase's `ridl` package:
  add one if the showcase has none)

- [ ] **Step 1**: add the rows; run
      `cargo test -p ridl-core diag 2>&1 | tail -3` and
      `cargo test -p ridlc --test corpus rsdl 2>&1 | tail -5`; the showcase
      tests FAIL until the examples exist (they pass only once Tasks 14 and 17
      raise the codes; land this task's rows and the showcase sources together
      with Task 17 if the gate cannot be satisfied in between, and say so in the
      pull request).
- [ ] **Step 2: Commit**:
      `feat(ridl-core): add RSDL-709, RSDL-805 and RSDL-806`.

### Task 14: Parse and check the keys (Fable)

**Files:**

- Modify: `crates/ridl-sem/src/rsdl/attrs.rs` (`AttrSite::Placement`, `allows`,
  `RSDL_KEYS`, `ReadAttrs.sizing`, a reader `sizing_key`),
  `crates/ridl-sem/src/rsdl/collect.rs:325` (machine body lines read with
  `AttrSite::Placement`; `member_ref` keeps the `sizing`),
  `crates/ridl-sem/src/rsdl/mod.rs` (`MemberRef.sizing`,
  `DeploymentDecl.sizing`)
- Test: `crates/ridl-sem/src/rsdl/mod.rs` `mod tests`

**Interfaces:**

- Produces:
  `pub struct Sizing { pub depth: Option<u32>, pub slots: Option<u32>, pub budget: Option<u64> }`
  on `DeploymentDecl` and `MemberRef`;
  `fn sizing_key(key: &str, value: Option<ast::AttrValue>, site: Site, reporter: &mut Reporter, into: &mut Sizing)`
  raising RSDL-709 with the message
  `` `<key>` takes an integer from <lo> to <hi>, written `<key> = <n>`; `<text>` is not one (rsdl reference §5) ``.

- [ ] **Step 1: Tests** (the `check_topology` helper and `codes`):

```rust
#[test] fn the_sizing_keys_are_read_on_a_deployment_and_a_placement_line()   // no codes; DeploymentDecl.sizing and MemberRef.sizing hold the values
#[test] fn a_sizing_key_out_of_range_is_rsdl_709_and_blocks_its_deployment()  // "slots = 65537", "budget = 0", "budget = 18446744073709551616", "depth = 0", "depth = -1", "slots = (1, 2)", "slots = \"8\""
#[test] fn a_sizing_key_on_a_requires_line_or_a_machine_is_form_107()
#[test] fn a_sizing_key_twice_in_one_block_is_form_108()
#[test] fn a_sizing_key_on_an_instance_that_consumes_nothing_draws_nothing()
```

- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS**:
      `cargo test -p ridl-sem rsdl 2>&1 | tail -3`. **Step 5: Commit**:
      `feat(ridl-sem): read the depth, slots and budget keys on deployments and placement lines`.

### Task 15: `Sizing` in the system IR

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/system.proto` (`Sizing`,
  `Deployment.sizing = 15`, `Placement.sizing = 5`),
  `crates/ridl-sem/src/rsdl/lower.rs:232-320` (`deployment`: the two `sizing`
  fields)
- Test: `crates/ridl-sem/src/rsdl/lower.rs` tests; the corpus `system` snapshot
  (unchanged unless the appendix declares a key)

- [ ] **Step 1: Test**
      `declared_sizing_values_reach_the_deployment_and_its_placements`. **Step
      2: FAIL. Step 3: Implement. Step 4: PASS. Step 5: Commit**:
      `feat(ridl-ir): carry the declared sizing values in the system IR`.

### Task 16: Precedence in the emitter (Fable)

**Files:**

- Modify: `crates/ridl-ir/src/codegen/deployment.rs`
- Test: its `mod tests`

**Rule (D-6):** per consumer link, `depth` = the placement's `sizing.depth`,
else the deployment's, else derived; `slots` = placement, else deployment, else
16; `budget` = placement, else deployment, else none. A declared value has
source `DECLARED`.

- [ ] **Step 1: Tests**:
      `a_placement_value_takes_precedence_over_the_deployment_value`,
      `a_deployment_value_applies_to_every_link`,
      `a_declared_depth_replaces_the_derived_one_and_the_ring_depth_is_the_max`,
      `a_declared_budget_has_source_declared`.
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS. Step 5: Commit**:
      `feat(ridl-ir): resolve the declared sizing values into each consumer link`.

### Task 17: RSDL-805 and RSDL-806 (Fable)

**Files:**

- Modify: `crates/ridl-sem/src/rsdl/mod.rs` (`check_system`: a pass over the
  deployments' links that reads each consumed interface's events through
  `check_package(db, ws, pkg, std).ir` and
  `ridl_ir::codegen::depth::ceil_ratio`)
- Test: `mod tests`

- [ ] **Step 1: Tests**:
      `a_declared_depth_below_the_bound_is_rsdl_805_once_per_link_and_event`
      (message names the event, the value and the bound),
      `a_half_open_event_with_no_declared_depth_is_rsdl_806`,
      `a_declared_depth_silences_rsdl_806`,
      `a_depth_at_or_above_the_bound_draws_nothing`.
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS**, then the showcase tests
      of Task 13 PASS. **Step 5: Commit**:
      `feat(ridl-sem): warn on a depth below the contract bound and on an underivable depth`.

### Task 18: The rsdl reference and the book

**Files:**

- Modify: `docs/specification/rsdl-language-reference.md` §3.4, §5, §13 (spec §6
  text), `docs/book/rsdl.md` (one compiled fence showing `[ slots = 8 ]` on a
  placement line, named `allow=` nothing), `docs/design/codegen-plugins.md`

- [ ] **Step 1**: write the text;
      `just link-check && just story-id-check && just check` tails clean;
      `cargo test -p ridl-cli --test book_examples 2>&1 | tail -3` PASS.
- [ ] **Step 2: Commit**:
      `docs(rsdl): specify the depth, slots and budget keys`.

## 6. Stage S4 — E17.5, the proof and the records

### Task 19: The test plugin and the hand-checked fixture (Opus)

**Files:**

- Create: `crates/ridlc/tests/layout.rs`,
  `crates/ridlc/tests/fixtures/cabin-layout.json`
- Test: `layout.rs`

**Interfaces:**

- Produces, inside the test file: `struct LayoutBackend;` implementing
  `ridl_ir::codegen::Backend` (`language() -> "layout"`), whose `generate` reads
  only the request and emits one file `layout.json` with, per region:
  `{ catalog, slots: [{ member, offset, size }], bytes }`, and per proto3
  consumer link: `{ interface, member, consumer, max_message_bytes }`. Its
  layout rule, stated in the file's doc comment: slots in member ordinal order;
  a slot is the FlatBuffers bounded size rounded up to 8, times the ring depth
  for an event, times `slots` times the reservation for a call;
  `max_message_bytes = frame_header_max_bytes + envelope_bytes + proto3 bound`
  for the WebSocket binding row, `null` when the row is absent.

- [ ] **Step 1: Write the fixture by hand**, each number with a comment line
      deriving it from the request (the FlatBuffers bounds from
      `projection::flatbuffers::max_size` as the model prints them; the depth
      10; the 16 slots; the reservation sums; the header and envelope sizes).
- [ ] **Step 2: Write the test**: compile `examples/cabin`, build the request
      for `veh.cabin` with `select_deployment(.., None, ..)`, run
      `LayoutBackend.generate(&request)`, parse `layout.json`, and `assert_eq!`
      it to the fixture. Also assert two requests from two `compile_workspace`
      runs are byte-equal, and that each channel's consumer count equals the
      number of links of that interface in the system IR.
- [ ] **Step 3: Run**: `cargo test -p ridlc --test layout 2>&1 | tail -5`. PASS,
      or the fixture's derivation is wrong: fix the fixture only when the
      derivation comment was wrong, never to match the output.
- [ ] **Step 4: Commit**:
      `test(ridlc): a layout plugin computes cabin's regions and messages from the request alone`.

### Task 20: The as-built records

**Files:**

- Modify: `docs/design/codegen-plugins.md` (as built),
  `docs/design/catalog-descriptor.md` ("Not built" confirmed),
  `docs/technotes/rsdl-implementation.md`,
  `docs/technotes/walking-skeleton-architecture.md` (where the emitter lives),
  `docs/ROADMAP.md` (the E17 rows move to the landed record,
  `docs/archive/roadmap-landed-record.md`), `docs/BACKLOG.md` (P1 done)

- [ ] **Step 1**: write; `just verify 2>&1 | tail -5` clean. **Step 2: Commit**:
      `docs(roadmap): record the layout inputs for backend plugins as built`.

### Task 21: Garden

- [ ] Run the `sdd-gardening` skill: the spec and this plan move to
      `docs/archive/`, `docs/wip/README.md` and `docs/archive/README.md` are
      updated, the lanes plan's lane S row says "closed", the spec's §8
      delegated decisions are summarised for Sebastien in the pull request body.
- [ ] Commit: `docs(docs): garden the layout-inputs design and plan`.
