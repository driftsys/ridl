# Workspace-aware MCP tools, follow-up — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ridl_references` and `ridl_dependencies` count rsdl component
uses, and close the gaps a review of #668 found: duplicated snapshot helpers,
outputs no test pins, and a set of small defects.

**Architecture:** The review tools read the lowered rsdl system
(`WorkspaceOutput.system`) in addition to the package IR (spec §4.4). The
snapshot helpers that `ridl diff` and the baseline commands share move to one
public copy in `ridlc`. The rest are local changes in `ridl-mcp`, `ridl-core`
and the docs.

**Tech Stack:** Rust (toolchain pinned in `rust-toolchain.toml`), `rmcp` 3.3.0,
pbjson serde for the IR, `serde_json`.

**Spec:**
[`2026-10-03-mcp-workspace-tools-design.md`](2026-10-03-mcp-workspace-tools-design.md),
§4.4 (new) and §6.3, plus the rest of the spec as built by #668. The first plan,
[`2026-10-03-mcp-workspace-tools-plan.md`](2026-10-03-mcp-workspace-tools-plan.md),
describes the code this plan changes. Where this plan and the spec disagree, or
the code does not match what this plan says, stop and ask.

## Before you start

- Work on a branch `feat/1a-mcp-followup` from `main` (which contains #668 and
  this plan). Run `./bootstrap` once after creating the worktree. One pull
  request; at least one commit per task.
- Commit scopes allowed: those in `.git-std.toml`. This plan's commit messages
  use `ridl-mcp`, `ridlc` and `docs`; use `ridl-core` or `adr` if you split a
  commit by crate.
- Per task: `cargo test -p <crate> --locked`, `cargo fmt --all`,
  `cargo clippy -p <crate> --all-targets -- -D warnings`. Before the pull
  request: `just verify`.

## Global Constraints

- Every Global Constraint of the first plan still holds: read-only and offline
  tools, no change to `ridl-lsp`, the language or the IR schema, byte-identical
  `ridl check` and `ridl diff` output, the `Result<CallToolResult, ErrorData>`
  tool form.
- The tool surface changes only by addition (spec §7.2). The changes to
  `crates/ridl-mcp/tests/tools.json` in this plan are: the `kind` field on a
  reference (a required property, plus a `ReferenceKind` entry under `$defs`),
  and `description` texts on input fields. No field is removed or renamed, and
  no tool result for an existing input changes except that `ridl_references` and
  `ridl_dependencies` now also report rsdl uses, which is the point of §4.4.
- Every existing test keeps its assertions, except where a task says otherwise.

## Review Focus

1. A workspace whose `system` lists a component from another package: the
   system's package `depends_on` that package (Task 1,
   `system_package_depends_on_member_component_packages`).
2. `ridl_dependencies` with `package` set: the one package returned still lists
   the dependents found across the whole workspace (Task 1,
   `one_package_keeps_its_workspace_dependents`).
3. A workspace with `.rsdl` files and no `system`: the note appears, and no
   result claims rsdl uses (Task 1, `no_lowered_system_draws_the_rsdl_note`).
4. `ridl diff` and the baseline commands after the helpers move: byte-identical
   output, including the fail-closed refusals of #230 and #339 (Task 2, the
   existing CLI tests unchanged).
5. Source-mode `ridl_check` with `overlays`: a tool error, not a silent ignore
   (Task 4, `source_mode_refuses_overlays`).

---

### Task 1: rsdl uses in `ridl_references` and `ridl_dependencies`

**Files:**

- Create: `crates/ridl-mcp/tests/fixtures/ws-rsdl/` (below)
- Modify: `crates/ridl-mcp/src/refs.rs` (`Reference`, `references`,
  `dependencies`), `crates/ridl-mcp/src/snapshot.rs` (the notes, and
  `TempWorkspace::copy`, which gains a fixture-name parameter so tests can copy
  `ws-rsdl`), `crates/ridl-core/src/diag.rs` (`iter_files` becomes `pub`),
  `crates/ridl-mcp/tests/tools.json`, `crates/ridl-mcp/README.md`

**Interfaces:**

- `Reference` gains `pub kind: ReferenceKind`, serialised in lower case:

  ```rust
  #[derive(Serialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  #[schemars(crate = "rmcp::schemars")]
  #[serde(rename_all = "snake_case")]
  pub enum ReferenceKind { Declaration, Interface, Service, Component }
  ```

  `Item::Decl` → `Declaration`, `Item::Interface` → `Interface`, `Item::Service`
  → `Service`, a component → `Component`.

- New, in `refs.rs`:

  ```rust
  /// Every (component package, component name, required interface) triple of
  /// the lowered system, per spec §4.4: declared components only, required
  /// interfaces that are present and not inline, as `{catalog}.{name}`.
  fn component_requires(system: &ridl_ir::v2::System) -> Vec<(String, String, String)>;

  /// (system package, member component package) pairs, per spec §4.4.
  fn system_member_packages(system: &ridl_ir::v2::System) -> Vec<(String, String)>;
  ```

- The sort order of references stays (package, declaration, interaction); add
  `kind` as the last key so equal triples have a defined order.

The no-system note (spec §6.3, §4.4), exact text:
``"rsdl uses were not counted, because no system was lowered: the workspace declares no `system`, or an error in its closure blocked the lowering; run ridl_check on the same path to see which"``.
It is added in `snapshot` when `output.system.is_none()` and any file the load
read ends in `.rsdl`. The loaded paths are in `output.sources`; make
`SourceMap::iter_files` (`crates/ridl-core/src/diag.rs`, about line 1532) `pub`
for this, a one-word API addition. Do not load the workspace a second time.

**The fixture** `ws-rsdl/`, checked with `ridl check`: exit 0, two RSDL-409
warnings (the book example draws the same two):

`ws-rsdl/ridl.toml`:

```toml
[workspace]
members = ["climate", "cabin"]
```

`ws-rsdl/climate/ridl.toml` and `ws-rsdl/cabin/ridl.toml`: `[package]` with
`name = "veh.climate"` and `name = "veh.cabin"`, `version = "1.0.0"`.

`ws-rsdl/climate/climate.ridl`:

```ridl
package veh.climate

type Temperature: integer [-40..85]

type FanLevel: integer [0..7]

type Heated: boolean

interface Climate {
  signal cabinTemperature: Temperature @[100ms..1s]
  command setFan(level: FanLevel) @[..50ms]
}

interface Seats {
  signal heated: Heated @[100ms..1s]
}

service veh.climate.control: Climate

service veh.climate.seats: Seats

service veh.climate.diag {
  query readTemperature(): Temperature @[..100ms]
}
```

`ws-rsdl/cabin/cabin.rsdl`:

```rsdl
package veh.cabin
import veh.climate.Climate
import veh.climate.Seats

/// Runs the climate loop. Two copies, one per zone controller.
component ClimateControl [ instances = (front, rear) ] {
  offers veh.climate.control
  requires Seats
}

component SeatHeating {
  offers veh.climate.seats
}

component Dashboard {
  requires Climate
  requires Seats
}

/// The phone app. No implementation in this workspace.
component PhoneApp [ external ] {
  requires Climate
  requires veh.climate.diag
}

system Cabin {
  ClimateControl
  SeatHeating
  Dashboard
  PhoneApp
  veh.climate.diag
}
```

These fixture files sit under `crates/`, outside `docs/book/`, so the book
harness does not compile them.

- [ ] **Step 1: Create the fixture** and confirm
      `cargo run -q -p ridl-cli -- check crates/ridl-mcp/tests/fixtures/ws-rsdl`
      exits 0 with exactly two RSDL-409 warnings.

- [ ] **Step 2: Write the failing tests** in `refs.rs`:

- `references_from_components`: target `veh.climate.Seats` → exactly, in order:
  `(veh.cabin, ClimateControl, component, null)`,
  `(veh.cabin, Dashboard, component, null)`, and the service reference from
  `veh.climate` with kind `service` (read its IR name once and write it as a
  literal). Every component reference has `location: null`.
- `an_inline_require_is_not_a_reference`: target `veh.climate.Temperature` → no
  reference has declaration `PhoneApp`.
- `one_package_keeps_its_workspace_dependents`: `dependencies` with
  `package: "veh.climate"` returns one package whose `dependents` is
  `["veh.cabin"]`. (Review Focus 2.) A component cannot require the same
  interface twice (RSDL-309), so no test covers that case.
- `dependencies_count_component_requires`:
  `veh.cabin.depends_on ==
  ["veh.climate"]`,
  `veh.climate.dependents == ["veh.cabin"]`.
- `system_package_depends_on_member_component_packages`: a temporary copy of
  `ws-rsdl` (through `TempWorkspace::copy("ws-rsdl")`) with `"ops"` added to the
  root `members`, the `system Cabin` block removed from `cabin.rsdl`, and
  `ops/ridl.toml` (`name = "veh.ops"`) plus `ops/ops.rsdl`:

  ```rsdl
  package veh.ops
  import veh.climate.Climate
  import veh.cabin.Dashboard
  import veh.cabin.ClimateControl
  import veh.cabin.SeatHeating

  component Monitor {
    requires Climate
  }

  system Ops {
    Monitor
    Dashboard
    ClimateControl
    SeatHeating
  }
  ```

  (checked: two RSDL-409 warnings, no error). `veh.ops.depends_on` is
  `["veh.cabin", "veh.climate"]`. (Review Focus 1.)
- `no_lowered_system_draws_the_rsdl_note`: an overlay on `cabin.rsdl` that
  removes the `system` block → `workspace.notes` contains the exact note above,
  and `references(veh.climate.Seats)` has no `component` reference. (Review
  Focus 3.)
- `existing_references_carry_their_kind`: on `ws`, `references(Reading)` reports
  `(fx.a, Outcome)` with kind `declaration` and `(fx.b, Status)` with kind
  `interface`.
- Every existing `refs.rs` test keeps its assertions; tests that compare whole
  `Reference` values gain the `kind` field.

- [ ] **Step 3: Run and see them fail**: `cargo test -p ridl-mcp --locked refs`

- [ ] **Step 4: Implement**, then update `tests/tools.json` (the pin test prints
      the new JSON; check the only change is the `kind` property) and the
      README's `ridl_references` and `ridl_dependencies` sections: the `kind`
      field, that rsdl components count, the no-system note, and that
      `depends_on` can name a qualifier that is not a workspace package (spec
      §4.3).

- [ ] **Step 5: Run**:
      `cargo test -p ridl-mcp --locked && cargo test -p
      ridl-cli --locked --test servers`.
      Expected: pass.

- [ ] **Step 6: Commit**:
      `feat(ridl-mcp): count rsdl component uses in references and dependencies`

---

### Task 2: One copy of the snapshot helpers

**Files:**

- Modify: `crates/ridlc/src/diff_side.rs`, `crates/ridl/src/main.rs`

`crates/ridl/src/main.rs` still defines helpers that #668 copied into
`ridlc::diff_side`, because `load_baseline` and `load_published` use them.
Compare the two files and list every function defined in both. Share the ones
whose behaviour is the same in both copies (expect the predicates and listing
helpers: `is_ir_json`, `has_ir_json_name`, `is_non_json_ir`, `is_source_file`,
`is_source_dir`, `files_matching`, `ir_json_files`, `first_non_json_ir_in`,
`first_nested_snapshot_dir`, and `snapshot_files` if its errors map cleanly).
Keep in `main.rs` the helpers whose message text depends on the caller:
`refuse_nested_snapshot_directory` and `refuse_artifact_directory` take the
remedy text from their caller, and `main.rs`'s `load_snapshots` takes a
`parse_remedy` argument, so the baseline commands print words that the
`DiffSideError` messages do not. List in the pull request which helpers you
shared and which you kept, and why.

- [ ] **Step 1:** Make the shared `ridlc::diff_side` helpers `pub`, change the
      `main.rs` callers to use them, mapping any error to the same stderr
      message and `ExitCode` as today, and delete the `main.rs` copies of the
      shared ones only.
- [ ] **Step 2:** Move the doc comments of each deleted `main.rs` copy onto its
      `ridlc` function: the #230 and #339 fail-closed reasoning, and why
      `first_nested_snapshot_dir` must not swallow a read error. Keep their
      text; only fix references to names that changed. (The note about #218 was
      on the old `load_diff_side` and is already gone; restore it on
      `ridlc::load_diff_side` from `git show edeec6e^:crates/ridl/src/main.rs`,
      near line 461.)
- [ ] **Step 3:** Run
      `cargo test -p ridlc --locked && cargo test -p ridl-cli
      --locked`.
      Every file under `crates/ridl/tests/` is unchanged and every test passes.
      (Review Focus 4.)
- [ ] **Step 4: Commit**:
      `refactor(ridlc): share one copy of the snapshot helpers with the CLI`

---

### Task 3: Pin the outputs no test checks

**Files:** tests in `crates/ridl-mcp/src/{query,refs,snapshot,explain,diff}.rs`,
`crates/ridl-core/src/workspace.rs`, and the `ws` fixture (additions only).

Each test below fails if the named code is changed as described; check that by
making the change, running the test, and restoring the code.

Fixture additions to `ws/a/a.ridl` (append; keep every existing declaration and
line number above them unchanged): one `internal` declaration, one `enumset`
over `Health`, one `const` and one type whose `match` pattern names that const.
Use the syntax in the typl reference; confirm `ridl check` on `ws` still exits 0
with no diagnostic. Give `interface Status` in `ws/b/b.ridl` a label and a
`@deprecated` reason (and a number, if the grammar has one), in the form the
ridl reference gives (a tag on the same line as the doc comment becomes doc
text, and a bracket form draws FORM-101; check with `ridl check`). This moves
lines in `b.ridl`, so update the line-number assertions that depend on them.
Apply the same additions to `ws-diag/` and `ws-v2/`, so the variants still
differ from `ws/` only by their own edits. Then update every test whose
assertions these additions change (for example a declaration count), and say
which in the commit message.

- `diff_uses_std_as_context`: a fixture variant that appends to
  `struct
  Reading` a field whose type is from `ridl.std` (the #598 case);
  `ridl_diff` gives the verdict `ridl diff --format json` gives on the same
  pair, and the test fails when `&[ridlc::std_ir()]` is replaced by `&[]`.
- `resolve_reports_visibility`: the internal declaration → `"internal"`; `Speed`
  → `"public"`.
- `resolve_reports_each_kind`: `Reading` struct, `Health` enum, `Outcome` union,
  `Status` interface, the const, the enumset → their exact `kind` words.
- `list_interactions_reports_the_header`:
  `interface.doc == "The status
  interface."`, plus the label, the deprecation
  reason and `number`.
- `locations_are_complete`: `Speed`'s location equals the full
  `{path, start: {line, column}, end: {line, column}}` of the name `Speed` in
  `a.ridl`, computed in the test from the file text.
- `declaration_references_have_locations`: in `references_to_a_struct`, the
  `Outcome` and `Status` references' locations point at those names.
- `references_through_backing_enums_and_pattern_consts`: `references(Health)`
  contains the enumset; `references(<the const>)` contains the pattern type.
- `no_alias_without_an_alias`: `resolve("Level", from "fx.b").alias` is `None`.
- `an_added_file_sorts_with_the_disk_files` (ridl-core): an overlay `0.typl`
  beside `a.typl` is `files[0]`.
- `an_overlay_replaces_rather_than_adds` (ridl-core): in
  `overlay_replaces_the_text_of_a_file_on_disk`, assert `files.len() == 1`.
- `source_without_profile_is_a_tool_error`: add `{"source": "package p"}` to
  `both_modes_or_neither_is_a_tool_error`'s cases.
- `explain_reports_warning_and_info_severities`: one warning code and one info
  code from the catalogue.
- `snapshot_root_of_a_file_path`: `snapshot("<ws>/b/b.ridl").root` is `<ws>/b`.

- [ ] **Step 1:** Add the fixture lines and the tests; run
      `cargo test -p ridl-mcp --locked` and `cargo test -p ridl-core --locked`.
      Expected: pass. For each test, make its named change in the code, see it
      fail, restore the code.
- [ ] **Step 2: Commit**:
      `test(ridl-mcp): pin the lookup outputs and the
      overlay file order`

---

### Task 4: Small fixes

**Files:** `crates/ridl-mcp/src/{lib,query,types,diff,refs,explain}.rs`,
`crates/ridl-core/src/workspace.rs`, `docs/book/cli-reference.md`,
`docs/decisions/ADR-0005-agent-enablement.md`, `crates/ridl-mcp/README.md`,
`crates/ridl-mcp/tests/tools.json`

- [ ] **Step 1: Source mode refuses overlays.** `ridl_check` with `source` and
      `overlays` is the same tool error as both modes. Test
      `source_mode_refuses_overlays`. (Review Focus 5.)
- [ ] **Step 2: No empty suggestion list.** When an unknown name has no
      suggestion, the message ends after the name, with no `"; suggestions: "`.
      Test `an_unknown_name_with_no_close_match` asserts the exact message for
      `Zzzqqq`.
- [ ] **Step 3: Canonicalise only with overlays.** In `load_package_tree`, the
      `dir.canonicalize()` call runs only when `self.overlays` is not empty, so
      a load with no overlays makes no new system call and has no new error
      path. Existing tests cover the behaviour.
- [ ] **Step 4: `ridl_mcp::check`.** Make it private, or change it to take
      `(&str, Profile)`, so a library caller cannot reach its `expect`.
- [ ] **Step 5: Input descriptions.** Give every input field of every tool a doc
      comment, which schemars turns into the schema `description`: `path` (the
      workspace root, a package directory or a source file; relative to the
      server's working directory), `overlays` and its `path`/`source`, `name`
      (`Name` or `pkg.Name`), `from` (a package name), `interface`, `package`,
      `code`, `old` and `new` (an `.ir.json` file, a snapshot directory such as
      `<root>/.ridl/baseline`, or a source path). One sentence each, from spec
      §4. Update `tests/tools.json`; the only change is the added `description`
      properties.
- [ ] **Step 6: Book link.** In `docs/book/cli-reference.md` (about line 1532),
      replace the relative link to the crate README with
      `https://github.com/driftsys/ridl/blob/main/crates/ridl-mcp/README.md`, as
      the other links that leave the book do.
- [ ] **Step 7: ADR-0005.** In the minimum tool table,
      `ridl_list_interactions(path, name)` becomes
      `ridl_list_interactions(path, interface)`, and the `ridl_references` row
      adds that rsdl components count.
- [ ] **Step 8: README.** Add that `ridl_dependencies.imports` lists the member
      manifest's `[imports]` only, not the workspace root's shared `[imports]`.
- [ ] **Step 9:** Run `cargo test -p ridl-mcp -p ridl-core --locked`, then
      `prim fmt` on changed Markdown and JSON, `just check`,
      `just
      link-check`, `just book-check`. Expected: pass.
- [ ] **Step 10: Commit**:
      `fix(ridl-mcp): refuse source-mode overlays and
      describe every tool input`,
      and a separate `docs(docs): link the MCP README from the book by URL` if
      you prefer to split the docs changes.

---

## Finish

- [ ] Run `just verify`. Expected: exit 0.
- [ ] Open one pull request to `main` titled
      `feat(ridl-mcp): rsdl uses in the review tools, and review follow-ups`.
      List the tasks with their commits, each Review Focus item with its test,
      and every choice you made that the plan did not fix. No closing keyword
      for any issue. Do not edit `docs/ROADMAP.md`. Do not merge.
