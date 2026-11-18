# Plan — service declaration names (driftsys/ridl#1390)

Working memory for the branch `feat/1390-pascalcase-services`, stacked on
`fix/1389-deployment-file-names` (PR #1391). The decisions are recorded in
[ADR-0034](../decisions/ADR-0034-service-declaration-names.md) (_Proposed_);
this plan is how they are built. It is removed by gardening before the pull
request merges (`docs/wip/` is not shipped).

Line numbers below are from the branch head `276331b1` and drift as tasks land;
search for the quoted symbol, not the number.

## 0. Decisions as built, and the refinements to the brief

The brief's decisions D1 to D10 map to ADR-0034 decisions 1 to 10. Four were
refined against the code; the reasons are in ADR-0034 and repeated here:

1. **D5 — the new code is RIDL-142, not a new number.** ADR-0008 decision 21
   (2026-07-25) reserved RIDL-142 for exactly the uncoded error
   `check_service_name` raises (`crates/ridl-sem/src/check.rs:4550`). The
   catalogue lists it as reserved (`crates/ridl-core/src/diag.rs:653-658`,
   `:803`). Minting another number would leave a reservation for an error that
   no longer exists.
2. **D1/D5 — a one-segment name must be `CamelCase_id`, enforced.** The checker
   does not enforce a case rule on interface names, but the old form accepted a
   one-segment lowercase service (`service doors {` in
   `crates/ridl/tests/baseline_gate.rs:1521`, `service cabin {` in
   `crates/ridl-sem/src/check.rs:16575`). Read as a declared name, `doors` would
   move its global name from `doors` to `<package>.doors` with no diagnostic.
   And rsdl reads a reference's role from its case (`form_of`,
   `crates/ridl-sem/src/rsdl/collect.rs:418-458`): a lowercase one-segment name
   is read as a package path, so such a service could never be offered. RIDL-142
   therefore covers a dotted name and a one-segment name that is not
   `CamelCase_id` (`is_upper_camel`, `crates/ridl-sem/src/rsdl/mod.rs:697`).
3. **D5 — the suggestion rule is total.** "The segments after the package" is
   undefined when the dotted name does not start with the package
   (`service veh.hvac.cabin` in package `veh.cluster`,
   `xtask/tests/freeze-fixtures/generated/veh-cluster/cluster/services.ridl:116`)
   and when the join clashes (`service alpha.face : Face`,
   `crates/ridl-rt-conformance/fixtures/identity/alpha/face.ridl:17`). Rule:
   drop the longest leading run shared with the package (keep the last segment
   when nothing is left), split at `_`, upper-case each part's first letter,
   join. The result is taken when a declaration of the package has it or when an
   earlier RIDL-142 declaration of the same package (file order, then source
   order) was already given it — old-form declarations bind nothing, so
   `veh.cabin.control` and `control` in `veh.cabin` would otherwise both get
   `Control`. While taken, try `<result>Service`, `<result>Service2`,
   `<result>Service3`, …; the rule is total, so every RIDL-142 has a fix-it.
4. **D7 — `instances.lock` keys move too.** A lone service's implicit component
   is keyed by the service's name followed by `/Unit`
   (`crates/ridlc/tests/corpus/instance-numbers/instances.lock:5`,
   `veh.ego.status/Unit 3`). The migration therefore also runs
   `ridl lock <path> --rename veh.ego.status=veh.ego.Status`, which is the
   existing component rename (`Edit::RenameComponent`,
   `crates/ridl/src/lock.rs:488-500`). No new command.

5. **D3 — `ridl_references` is out of scope (orchestrator decision).** MCP
   `ridl_resolve` resolves a service (kind `service`) in this change;
   `ridl_references` reports nothing new for a service, and listing the
   components that offer one is left to a later change, tracked by its own
   issue.

Three further facts the brief did not state, found in the code:

- **IR `Service.name` keeps the global name.** It stays the one key read by the
  catalog, the lock key (`LockKey::Service(shape.name)`,
  `crates/ridl-sem/src/check.rs:482`), the descriptors, the codegen model
  (`dotted_name(&service.name)`, `crates/ridl-ir/src/codegen/lower.rs:1689`) and
  `ridl diff`. D4's "no string split" is met by comparing with a join
  (`format!("{package}.{name}") == service.name`), never by splitting.
- **The catalog hash changes for a unit with an inline shape.** The hash input
  names an inline shape by its catalog name, which is the service's global name
  (`crates/ridl-ir/src/catalog_hash.rs:40-55`, `Package::catalog_name`,
  `crates/ridl-ir/src/lib.rs:932-938`). The cabin example has no inline shape,
  so `freeze/wire.txt:1` (`catalog-hash veh.cabin …`) does not change.
- **The VS Code grammar needs no change.** `ridl.tmLanguage.json:71` matches the
  keyword `service` only, and `rsdl.tmLanguage.json:37` the keywords
  `offers|requires`; neither scopes the name.

## 1. How the gates stay meaningful between tasks

The checker change of task 3 makes every source in the old form an error. The
decision: **each task migrates the inline sources and fixtures of every crate it
changes, so that crate's tests are green at the end of the task; the corpora and
trees that no changed crate owns move in the task that first needs them (the
`crates/ridlc/tests/corpus/**` entries in task 5, because the backend tests read
them) or in task 7.** Two consequences fixed by this rule:

- Task 3 must add `SymbolKind::Service` arms to the exhaustive matches in
  `crates/ridl-lsp/src/complete.rs:961-968` and `hover.rs:937-945`, and the IR's
  global name breaks the language server's inline-shape lookups (H1 sites in
  section 2.6). So task 3 changes `ridl-lsp`, fixes those lookups, and migrates
  `crates/ridl-lsp/tests/server.rs` (old-form services at `:2144`, `:2146`,
  `:2755`, `:2923-2927`, `:4746`; hover assertions on `veh.adas.cruise` at
  `:2498`, `:3214`, `:3221`). The new language-server features (definition,
  rename, completion of services as symbols, the quick-fix test) stay in task 6.
  `crates/ridl/src/property.rs` matches `SymbolKind` with a `_ =>` arm
  (`:820-848`) and is not touched.
- The literal guard `codes_written_as_string_literals_are_all_catalogued`
  (`crates/ridl-core/src/diag.rs:2696`) scans every `.rs` file of the workspace,
  so task 3 removes every `"RIDL-140"` literal outside `DEFERRED_OR_RETIRED`:
  `crates/ridl/tests/servers.rs:251-262`,
  `crates/ridl-lsp/tests/server.rs:4749-4870`,
  `crates/ridlc/tests/{check_source.rs,workspace_passes.rs}`,
  `crates/ridl-sem/src/workspace.rs`, and the `RIDL_PROFILE_CODES` row in
  `crates/ridlc/tests/corpus.rs`.

Every task ends with `cargo build --workspace --all-targets --locked`,
`just lint`, `just fmt-check` and `just check` green, plus exactly these test
commands:

| Task | Must be green                                                                                                                                                                                                                                                                                                                                                                           | Exempt until (exact targets)          |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| 2    | `cargo test -p ridl-syntax`                                                                                                                                                                                                                                                                                                                                                             | none                                  |
| 3    | `cargo test -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-lsp -p ridl-fmt`; `cargo test -p ridlc --test check_source --test workspace_passes`; `cargo test -p ridlc --test payload_limits -- --exact <the two task-3 tests>`; the exact corpus list below; `cargo test -p ridl-cli --test servers`; `cargo test -p ridl-cli --test diff_cli -- --exact <the task-3 inline span test>` | see the table below                   |
| 4    | task 3's list, plus `cargo test -p ridl-ir -p ridl-descriptor -p ridl-diff`; `cargo test -p ridl-cli --test lock_cli --test instance_lock_cli --test baseline_gate`; `cargo test -p ridlc --test payload_limits`                                                                                                                                                                        | as task 3, less the targets now green |
| 5    | task 4's list, plus every backend crate, `ridlc-gen-model`, `ridlc-gen-rust`, `cargo test -p ridlc` (all targets), `cargo test -p xtask`, `just freeze-check` (with the notice for the four removed rows)                                                                                                                                                                               | as task 4, less the targets now green |
| 6    | task 5's list, plus `cargo test -p ridl-mcp`, `just vscode-verify`                                                                                                                                                                                                                                                                                                                      | as task 5, less `ridl-mcp`            |
| 7    | `just verify`                                                                                                                                                                                                                                                                                                                                                                           | none                                  |

The ridl corpus tests task 3 runs, by exact name (the broad `showcase` filter is
not used, because it also selects
`rsdl_showcase_provokes_exactly_the_expected_codes` at `corpus.rs:1052`, whose
`rsdl-diag-showcase` entry moves in task 5):

```sh
cargo test -p ridlc --test corpus -- --exact \
  showcase_provokes_exactly_the_expected_codes \
  ridl_profile_codes_match_the_catalogue \
  every_ridl_profile_code_has_a_living_example \
  showcase_pins_every_severity \
  showcase_pins_the_interaction_boundary_narrowings \
  typl_and_ridl_catalogue_entries_match_the_reference_tables \
  every_ridl_code_the_reference_marks_retired_is_in_the_retired_list
INSTA_GLOB_FILTER='ridl-diag-showcase/*' cargo insta test -p ridlc --test corpus \
  -- --exact corpus_entries_compile_to_reviewed_snapshots
```

Exempt targets, each red only because its input is in the old form:

| Target (exact)                                                                                                                                                                                                    | Red from | Green in | Input that moves                                                                |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------- | ------------------------------------------------------------------------------- |
| `cargo test -p ridlc` targets not listed for task 3 (`corpus` other tests, `deployment_descriptor`, `cabin_example`, …)                                                                                           | 3        | 5        | `crates/ridlc/tests/corpus/**` except `ridl-diag-showcase`                      |
| `cargo test -p ridl-ir`, `-p ridl-diff`                                                                                                                                                                           | 3        | 4        | inline sources in `crates/ridl-ir/src/lib.rs`, `crates/ridl-diff/src/system.rs` |
| `cargo test -p ridl-backend-rust --test podbuf_conformance --test proto3_conformance`, `-p ridl-backend-proto --test corpus`, `-p ridl-backend-flatbuffers --test corpus`, `-p ridl-backend-podbuf --test corpus` | 3        | 5        | the ridlc corpora they read                                                     |
| `cargo test -p xtask`, `just freeze-check`                                                                                                                                                                        | 3        | 5        | frozen copies; `freeze/diagnostics.txt`                                         |
| `cargo test -p ridl-mcp`                                                                                                                                                                                          | 3        | 6        | `crates/ridl-mcp/tests/fixtures/{ws,ws-diag,ws-rsdl,ws-v2}`                     |
| `cargo test -p ridl-cli` targets other than those listed above (incl. `book_examples`, `identity_fixture`)                                                                                                        | 3        | 7        | inline sources, `baseline-corpus`, `docs/book`, the identity fixture            |
| `cargo test -p ridl-transport-ws --test debug_gateway`, `-p ridl-loopback` (any test that compiles a source rather than reading the committed descriptor)                                                         | 3        | 7        | the identity fixture                                                            |
| `just demo`, `just compat-check`, `just cabin-ws-demo`, `just ts-e2e`, `cargo xtask eval`                                                                                                                         | 3        | 7        | `examples/cabin`, `evals/tasks/*`                                               |

For each task from 3 to 6, run
`cargo test --workspace --no-fail-fast 2>&1 |
tee target/1390-task<N>.log` once
and confirm that every failing test is in the exempt table and fails on
RIDL-142, RSDL-310, RSDL-312, RSDL-602, RSDL-702, RSDL-903, or an insta diff
whose new side shows one of those. Any other failure is a defect of the task.

- **Snapshots.** Accept only snapshots of the targets the task must turn green.
  Never accept a snapshot whose new side shows RIDL-142 for an old-form input.
- Every commit passes the pre-push hook (`just pre-push`), which runs no cargo
  test, so the branch can be pushed at any task.
- The pull request is squash-merged; its title is the task 3 subject and its
  body carries the `BREAKING CHANGE:` footer of task 3 (D10).

## 2. Every code site that takes, builds or splits a service name

`(T)` = the task that changes it. "Join" means the site compares with
`format!("{package}.{name}")`. "No change" means the site reads the IR's global
name and stays correct.

### 2.1 Syntax

| Site                                         | What it does                                                                         | Change                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------------------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/ridl-syntax/family.ungram:268-300`   | `ServiceDef = 'service' name:DottedName …`, `DottedName` comments ("reverse-domain") | (T2) comments only; tree unchanged                                                                                                                                                                                                                                                                                                                                                                                |
| `crates/ridl-syntax/src/parser.rs:1225-1310` | `service_def`, `dotted_name`                                                         | (T2) doc comments only; error text "expected a service name" stays                                                                                                                                                                                                                                                                                                                                                |
| `crates/ridl-syntax/src/ast.rs:475-498`      | `DottedName::segments`, `text` ("`lowercase_id` segments")                           | (T2) docs; add `ServiceDef::declared_name()` and `is_camel_case_id`                                                                                                                                                                                                                                                                                                                                               |
| `crates/ridl-syntax/src/ast.rs:520-532`      | `InterfaceShape::identity()`: an inline shape's identity is the written dotted text  | (T3, H1) becomes `identity(&self, package: &str)`: an interface's own name, or `format!("{package}.{declared}")` for an inline shape, `None` for the old form. The signature change makes the compiler list every caller: `crates/ridl/src/main.rs:2941`, `crates/ridlc/src/design_lints/sites.rs:87`, `crates/ridl-sem/src/lint.rs:130`, `crates/ridl-lsp/src/inlay.rs:303`, and the tests at `ast.rs:1379-1438` |

### 2.2 Semantics — declaration, namespace, catalog

| Site                                                     | What it does                                                                                                                                                                 | Change                                                                                                            |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `crates/ridl-sem/src/resolve.rs:43-51`                   | `enum SymbolKind`                                                                                                                                                            | (T3) add `Service`                                                                                                |
| `crates/ridl-sem/src/resolve.rs:808-870`                 | `enum Declaration`, `declarations()`, `name()`, `name_range()`, `symbol()`                                                                                                   | (T3) add `Declaration::Service(ServiceDef)`, bound only when `declared_name()` is `Some`                          |
| `crates/ridl-sem/src/resolve.rs:120-145`                 | local symbols, TYPL-009                                                                                                                                                      | (T3) automatic through `declarations()`                                                                           |
| `crates/ridl-sem/src/rsdl/collect.rs:60-145`             | `check_declaration_names`: TYPL-009 across profiles                                                                                                                          | (T3) automatic through `declarations()`; add the `Declaration::Service` arm at `:73-81`                           |
| `crates/ridl-sem/src/resolve.rs:1065-…`                  | `doc_link_members`                                                                                                                                                           | (T3) a service's members: its inline interactions, or every interaction of its listed interfaces                  |
| `crates/ridl-sem/src/check.rs:215-240`                   | services lowering loop, `lowered_services` first-wins set                                                                                                                    | (T3) `is_winner`-style check on the declared name; old form → RIDL-142 and `continue`                             |
| `crates/ridl-sem/src/check.rs:4155-4210`                 | `lower_service`: `name = significant_text(dotted)`                                                                                                                           | (T3) `name = format!("{package}.{declared}")` (join)                                                              |
| `crates/ridl-sem/src/check.rs:4543-4565`                 | `check_service_name` (uncoded)                                                                                                                                               | (T3) delete; replaced by `report_old_service_name` (RIDL-142 + fix-it)                                            |
| `crates/ridl-sem/src/check.rs:4615-4630`                 | `lower_service_inline`: `interface_name = dotted text`                                                                                                                       | (T3) the global name (join)                                                                                       |
| `crates/ridl-sem/src/check.rs:995-1015`                  | `resolve_type_path`: an interface in type position                                                                                                                           | (T3) add the `SymbolKind::Service` arm, same uncoded error, "names a service"                                     |
| `crates/ridl-sem/src/check.rs:6245-6255`                 | `is_lowercase_name_segment`                                                                                                                                                  | (T3) delete                                                                                                       |
| `crates/ridl-sem/src/check.rs:6457-6475`                 | kind words ("interface", "an")                                                                                                                                               | (T3) add `Service` → "service", "a"                                                                               |
| every other `match … SymbolKind`                         | `crates/ridl-sem/src/check.rs` (26 uses), `expr.rs` (5), `lint.rs` (2), `crates/ridl/src/property.rs` (2), `crates/ridl-lsp/src/{complete.rs (9), hover.rs (7), nav.rs (1)}` | (T3/T6) the compiler lists the non-exhaustive ones; a service behaves as an interface wherever a type is expected |
| `crates/ridl-sem/src/doc_lint.rs` `carrier_name`         | the missing-docs name of a service                                                                                                                                           | (T3) the declared name; test at `:910-930` expects `Cruise`-style names                                           |
| `crates/ridl-core/src/package.rs:197-290`                | `service_catalog`, RIDL-140                                                                                                                                                  | (T3) key = global name (join of `package.name(db)` and `declared_name()`), skip the old form, emit nothing        |
| `crates/ridl-core/src/package.rs:347-377`                | `package_names` locals                                                                                                                                                       | (T3) add each service's declared name, so a local service shadows an import as in the resolver                    |
| `crates/ridl-core/src/diag.rs:174-194, 653-660, 785-790` | `RETIRED_RIDL_CODES`, catalogue doc, `RIDL_140`                                                                                                                              | (T3) retire 140, declare `RIDL_142`                                                                               |
| `crates/ridlc/tests/corpus.rs:575, :1561`                | `RIDL_PROFILE_CODES`, expected severities                                                                                                                                    | (T3) drop RIDL-140, add `("RIDL-142", Showcase)`                                                                  |
| `crates/ridl-sem/src/workspace.rs:17-110`                | workspace diagnostics start with the catalog's RIDL-140                                                                                                                      | (T3) docs and the test at `:62-110` (no RIDL-140 left)                                                            |

### 2.3 Semantics — rsdl references (D3)

| Site                                                                             | What it does                                                                                             | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/ridl-sem/src/rsdl/collect.rs:418-458`                                    | `form_of`: lowercase-only → `ReferenceForm::Service`                                                     | (T3) lowercase-only → `Unreadable`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `crates/ridl-sem/src/rsdl/mod.rs:581-602`                                        | `enum ReferenceForm { …, Service { name } }`                                                             | (T3) remove `Service`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `crates/ridl-sem/src/rsdl/mod.rs:711-716`                                        | `is_lowercase_segment` ("package or service")                                                            | (T3) doc: package only                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `crates/ridl-sem/src/rsdl/closure.rs:280-298`                                    | `Lookup::declared`: a bare name is a local **component or system** of `from`, else `imports[from][name]` | (T3, H2) a local service `p.S` would lose to an imported component `q.S`, because the local test sees only the component index. Replace by `Lookup::bind(from, package, name) -> Option<Bound>` with `enum Bound { Component(usize), System(usize), Service(String), Interface(InterfaceId) }`: a bare name binds a local declaration of `from` of any kind first (component and system indexes, then `resolutions[from].symbols[name]` with `symbol.package == from`), and only then an import, whose target is looked up in its package with the same rule; a qualified name looks only in that package. Each slot then checks the kind it accepts. `component()`, `system()` and `interface()` become thin wrappers |
| `crates/ridl-sem/src/rsdl/closure.rs:300-325`                                    | `Lookup::interface`                                                                                      | (T3) through `bind`, kind `Interface`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `crates/ridl-sem/src/rsdl/closure.rs:443-466`                                    | `offered_service`: needs `ReferenceForm::Service`                                                        | (T3) `Declared` → `Lookup::bind`, kind `Service`; RSDL-310 message names the form                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `crates/ridl-sem/src/rsdl/closure.rs:470-525`                                    | `required_interface`: `Service` form for an inline shape                                                 | (T3) `Declared` → `Lookup::bind`: an interface, or a service → inline or RSDL-311                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `crates/ridl-sem/src/rsdl/closure.rs:600-641`                                    | `member_target`: `Service` form → implicit component                                                     | (T3) `Declared` → `Lookup::bind`: a component, or a service → implicit (RSDL-504 kept)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `crates/ridl-sem/src/rsdl/closure.rs:29-66`                                      | `ComponentId::Implicit`, `InterfaceId::Inline` docs ("dotted name")                                      | (T3) docs: global name                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `crates/ridl-sem/src/rsdl/closure.rs:332-355`                                    | `service_interfaces`: `rsplit_once('.')` on an **interface** reference                                   | no change (an interface ref, package + `Name`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `crates/ridl-sem/src/rsdl/resolve.rs:30-60, 197-220`                             | catalog entries keyed by the offered name                                                                | no change (keys are global names)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `crates/ridl-sem/src/rsdl/target.rs:160-180, 240-262, 645-660`                   | editor targets by `ReferenceForm::Service`                                                               | (T3) `Declared` → `Lookup::bind`, with the same local-before-import precedence as the checker; a service → `Target::Service(global)`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `crates/ridl-sem/src/rsdl/lower.rs:84, 170-175`                                  | implicit component name = service name                                                                   | no change (global name)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| unknown-name messages: `closure.rs` RSDL-310/312, and the RSDL-602/702/903 sites | —                                                                                                        | (T3) append the hint when every segment is lowercase                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

### 2.4 IR, lock, descriptors, CLI (D4, D7)

| Site                                                                            | What it does                                                                                                                                                         | Change                                                                                                                                                             |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/ridl-ir/proto/ridl/ir/v2/ir.proto:517-519`                              | `Service.name` comment ("dotted global name")                                                                                                                        | (T4) "global name: package, `.`, declared name"                                                                                                                    |
| `crates/ridl-ir/src/lib.rs:776-863`                                             | `InterfaceShape.name` docs ("dotted global name")                                                                                                                    | (T4) docs                                                                                                                                                          |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto:118-130`                      | `DottedName` examples `veh.adas.cruise`                                                                                                                              | (T4) examples `veh.adas.Cruise`; field set unchanged                                                                                                               |
| `crates/ridl-ir/src/codegen/names.rs:18-39`                                     | `dotted_name`, `joined_camel` split a **dotted name** into segments                                                                                                  | no change (segments of a name, not an address split)                                                                                                               |
| `crates/ridl-ir/src/codegen/lower.rs:1072, 1113, 1132, 1684, 1689`              | owner, inline index, identity, model service                                                                                                                         | no change                                                                                                                                                          |
| `crates/ridl-ir/src/catalog_hash.rs:40-55, 129-141`                             | inline shape under its catalog name                                                                                                                                  | no change; the hash value moves with the name                                                                                                                      |
| `crates/ridlc/src/design_lints/limits.rs:19, 75-77, 229`                        | `short_name`: `name.rsplit('.')` of a shape name                                                                                                                     | (T4) an inline shape's short name = the declared name, by `strip_prefix(&format!("{}.", package.name))` on `service.name`; a declared interface keeps its own name |
| `crates/ridlc/src/design_lints/sites.rs:87`                                     | member sites keyed by `shape.identity()` (the written dotted text)                                                                                                   | (T3, H1) key by `identity(&package)`, the global name the IR uses                                                                                                  |
| `crates/ridlc/src/design_lints/limits.rs:246`                                   | `ctx.sites.member(&package.name, shape.name, …)`: IR global name vs the site key; a miss is `continue`, so the payload check of an inline member is skipped silently | (T3, H1) fixed by the `sites.rs` key; regression test: an oversized inline-shape payload still draws RIDL-157                                                      |
| `crates/ridl/src/main.rs:2941`                                                  | `ridl diff` span index for **inline** shapes: key `shape.identity()`                                                                                                 | (T3, H1) key `identity(&package)`; regression test: a diff path inside an inline shape renders with its span                                                       |
| `crates/ridl/src/main.rs:2950-2985`                                             | `ridl diff` span index for named services: key `dotted.text()`                                                                                                       | (T4) key = global name (join)                                                                                                                                      |
| `crates/ridl/src/main.rs:486-507`                                               | `ridl describe <file>`: prints descriptor strings                                                                                                                    | no change (output values change)                                                                                                                                   |
| `crates/ridl-core/src/interface_lock.rs:71-157`                                 | `LockKey::Service`, key parser accepts any identifier case                                                                                                           | no change; test a mixed-case key                                                                                                                                   |
| `crates/ridl/src/lock.rs:205-235, 478-640`                                      | `--rename`, `--retire`, `Edit::RenameInterface`, `Edit::RenameComponent`                                                                                             | no change; tests for the two migrations                                                                                                                            |
| `crates/ridl-sem/src/check.rs:339-360`                                          | retired entries: `rsplit_once('.')` of an **interface** key                                                                                                          | no change                                                                                                                                                          |
| `crates/ridl-descriptor/src/lower.rs:99-230`, `src/deployment/lower.rs:120-295` | copy service strings                                                                                                                                                 | no change (values change)                                                                                                                                          |
| `crates/ridl-diff/src/walk.rs:1163-1263`, `system.rs:1273`                      | service keys, paths `<package>/<service>`                                                                                                                            | no change (values change)                                                                                                                                          |

### 2.5 Backends (D8) — no code change expected

`crates/ridl-backend-proto/src/lib.rs:199, 1401-1433` (`type_name(shape.name)`),
`crates/ridl-backend-flatbuffers/src/lib.rs:241, 1532-1550`,
`crates/ridl-ir/src/codegen/lower.rs:228-261` (induced names from the owner),
`crates/ridl-ir/src/codegen/flatbuffers.rs:582`,
`crates/ridl-backend-ts/src/codec.rs:4022-4045` (`interaction_path`, doc text),
`crates/ridl-backend-rust/src/descriptors.rs:48-70` and `face.rs:156-178` (skip
inline shapes), `crates/ridl-backend-rust/src/codec_podbuf.rs:1069, 1118` and
`crates/ridl-backend-podbuf/src/lib.rs:662-680` (induced tuple names). Each
reads the global name through the IR; task 5 changes their test inputs and
snapshots, and doc comments that say "dotted name".

### 2.6 Language server and MCP (D3)

| Site                                                                                         | What it does                                                                   | Change                                                                                                                                           |
| -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/ridl-lsp/src/nav.rs:520-545`                                                         | `is_definition`: services excluded                                             | (T6) include `SyntaxKind::ServiceDef` (via the declared name)                                                                                    |
| `crates/ridl-lsp/src/hover.rs:93, 1027-1056, 1403`                                           | `service_hover` on a `DottedName`, IR lookup by dotted text                    | (T3) IR lookup by join; render `service Name : …`                                                                                                |
| `crates/ridl-lsp/src/hover.rs:1068-1084`                                                     | `enclosing_shape`: a `ServiceDef` ancestor keyed by its written text           | (T3, H1) key `format!("{package}.{declared}")`; regression test: hover on an interaction of an inline shape                                      |
| `crates/ridl-lsp/src/inlay.rs:303-309`                                                       | `shape_hints`: `ir.shapes().find(                                              | found                                                                                                                                            |
| `crates/ridl-lsp/src/complete.rs:216, 290-300, 345-365`                                      | `offers` completion from `service_catalog` entries (dotted)                    | (T6) complete services as symbols: bare in scope, imported, qualified                                                                            |
| `crates/ridl-lsp/src/rsdl.rs:11-30, 65-66, 239-240`                                          | hover and definition for `Target::Service`                                     | (T6) unchanged shape; `service_site` finds the declaration by join                                                                               |
| `crates/ridl-lsp/src/rsdl.rs:350, 385, 457`                                                  | rename refused for a service                                                   | (T6) allow: rename the declaration, rsdl references, imports and doc links                                                                       |
| `crates/ridl-lsp/src/rsdl.rs:616-655`                                                        | `service_package`, `service_site`, `service_markdown`: `dotted.text() == name` | (T3) `format!("{package}.{declared}") == name`                                                                                                   |
| `crates/ridl-lsp/src/server.rs:162-164, 594-596, 1158-1168, 1707-1709`, `convert.rs:172-192` | quick fixes from `Diagnostic.fixits`                                           | no change: the RIDL-142 fix-it becomes a quick fix                                                                                               |
| `crates/ridl-mcp/src/query.rs:50-62, 117-124`                                                | `Item::Service`; `declarations()` omits services                               | (T6) include services                                                                                                                            |
| `crates/ridl-mcp/src/query.rs:125-149`                                                       | `in_package`: `i.name() == name`                                               | (T6) for a service compare `s.name == format!("{package}.{name}")`                                                                               |
| `crates/ridl-mcp/src/query.rs:183`                                                           | `name.rsplit_once('.')` of a `pkg.Name` input                                  | no change: `Name` holds no `.`                                                                                                                   |
| `crates/ridl-mcp/src/query.rs:245-262`                                                       | `resolve`: `Item::Service => unreachable!`                                     | (T6) kind `service`, visibility `public`                                                                                                         |
| `crates/ridl-mcp/src/lib.rs` `ridl_list_interactions`, `ridl_describe_type`                  | interface input; describe refuses an interface                                 | (T6) accept an inline-shape service; refuse a list-form service naming its interfaces; describe refuses a service                                |
| `crates/ridl-mcp/src/refs.rs:23-110`                                                         | references by type-ref walk; services as referrers                             | no change. A service as `ridl_references` target reports no new kind of use; the offering components are left to a later change (separate issue) |
| `docs/design/mcp-workspace-tools.md:15-30, 60-90`                                            | tool table and rules                                                           | (T6) document the service cases                                                                                                                  |

## 3. D8 — every output built from a service name, before and after

Example declarations, as migrated by the RIDL-142 suggestion:

- `service veh.hvac.cabin { … }` in package `veh.cluster`
  (`xtask/tests/freeze-fixtures/generated/veh-cluster/cluster/services.ridl:116`)
  → `service HvacCabin { … }`, global name `veh.cluster.HvacCabin`.
- `service veh.cluster.wheels { … }` in package `veh.cluster`
  (`…/veh-cluster/cluster/evolution.ridl:31`) → `service Wheels`, global name
  `veh.cluster.Wheels`.
- `service veh.cabin.control: Cabin, Horn` in package `veh.cabin`
  (`examples/cabin/cabin.ridl:97`) → `service Control: Cabin, Horn`, global name
  `veh.cabin.Control`.
- `service veh.ego.status { … }` in package `veh.ego`
  (`crates/ridlc/tests/corpus/instance-numbers/ego/ego.ridl:3`), a lone service
  → `service Status`, global name `veh.ego.Status`.

| Backend / artifact              | Output                                                       | Code site                                                           | Derived as                                                                                                     | Before                                                                                                          | After                                                                                              | Changes                                                                |
| ------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| IR v2                           | `Service.name`                                               | `check.rs` `lower_service`                                          | global name                                                                                                    | `veh.hvac.cabin`                                                                                                | `veh.cluster.HvacCabin`                                                                            | yes                                                                    |
| IR v2                           | `InterfaceShape.name` (inline)                               | `ridl-ir/src/lib.rs:858`                                            | `service.name`                                                                                                 | `veh.hvac.cabin`                                                                                                | `veh.cluster.HvacCabin`                                                                            | yes                                                                    |
| ridl-diff                       | fallible transport identity                                  | `ridl-ir/src/lib.rs:101-116`, `ridl-diff/src/walk.rs:1793`          | `{shape}#{ordinal}:{ok}\|{err}`                                                                                | `veh.hvac.cabin#N:…`                                                                                            | `veh.cluster.HvacCabin#N:…`                                                                        | yes                                                                    |
| ridl-diff                       | path of a service                                            | `walk.rs:1163-1199`                                                 | `<package>/<service.name>`                                                                                     | `veh.cluster/veh.hvac.cabin`                                                                                    | `veh.cluster/veh.cluster.HvacCabin`                                                                | yes                                                                    |
| codegen model                   | `Service.name: DottedName`                                   | `codegen/lower.rs:1689`                                             | `dotted_name(global)`                                                                                          | `dotted veh.hvac.cabin`, `segments [veh,hvac,cabin]`, `joined_camel VehHvacCabin`, `underscored veh_hvac_cabin` | `veh.cluster.HvacCabin`, `[veh,cluster,HvacCabin]`, `VehClusterHvacCabin`, `veh_cluster_HvacCabin` | yes                                                                    |
| codegen model                   | `Interface.identity = Service(DottedName)`                   | `codegen/lower.rs:1132`                                             | as above                                                                                                       | as above                                                                                                        | as above                                                                                           | yes                                                                    |
| codegen model                   | `InducedName` of an inline-shape interaction                 | `codegen/lower.rs:244-245`                                          | Rust `camel_case(joined_camel(owner)) + camel_case(member)`; wire `joined_camel(owner) + joined_camel(member)` | `VehHvacCabinGetClimate…`                                                                                       | `VehClusterHvacCabinGetClimate…`                                                                   | yes                                                                    |
| proto3                          | inline ordinal enum                                          | `ridl-backend-proto/src/lib.rs:1402`                                | `type_name(shape.name) + "Ordinal"`                                                                            | `enum VehHvacCabinOrdinal`, values `VEH_HVAC_CABIN_ORDINAL_CABIN_TEMP = 1`                                      | `enum VehClusterHvacCabinOrdinal`, `VEH_CLUSTER_HVAC_CABIN_ORDINAL_CABIN_TEMP = 1`                 | yes                                                                    |
| proto3                          | induced messages                                             | wire induced name                                                   | as above                                                                                                       | `message VehHvacCabinGetClimateParams` (`freeze/wire.txt:3610-3614`)                                            | `message VehClusterHvacCabinGetClimateParams`                                                      | yes                                                                    |
| FlatBuffers                     | inline ordinal enum                                          | `ridl-backend-flatbuffers/src/lib.rs:1533`                          | `type_name(shape.name) + "Ordinal"`                                                                            | `enum VehHvacCabinOrdinal` (`freeze/wire.txt:743-753`)                                                          | `enum VehClusterHvacCabinOrdinal`                                                                  | yes                                                                    |
| FlatBuffers                     | induced tables                                               | `codegen/flatbuffers.rs:582`                                        | wire induced name                                                                                              | `table VehHvacCabinGetClimateParams` (`wire.txt:826`)                                                           | `table VehClusterHvacCabinGetClimateParams`                                                        | yes                                                                    |
| FlatBuffers / proto3            | `veh.cluster.wheels` → `Wheels`                              | same                                                                | `joined_camel`                                                                                                 | `VehClusterWheelsOrdinal` (`wire.txt:734-736`)                                                                  | `VehClusterWheelsOrdinal`                                                                          | **no**                                                                 |
| catalog hash, fallible identity | `veh.cluster.wheels` → `Wheels` (M4)                         | `catalog_hash.rs:39-55`, `ridl-ir/src/lib.rs:101-116`               | the global name as written                                                                                     | hash over `veh.cluster.wheels`; `veh.cluster.wheels#N:…`                                                        | hash over `veh.cluster.Wheels`; `veh.cluster.Wheels#N:…`                                           | **yes** — only generated identifiers survive a package-prefixed rename |
| Rust crate                      | induced structs, codecs                                      | `freeze/generated/veh-cluster/veh.cluster.rs:521-570` and 410 uses  | Rust induced name                                                                                              | `pub struct VehHvacCabinGetClimateParams`                                                                       | `pub struct VehClusterHvacCabinGetClimateParams`                                                   | yes                                                                    |
| Rust crate                      | podbuf head/view of induced tuples                           | `codec_podbuf.rs:126-138`                                           | `<rust name>PodbufHead`, `…View`                                                                               | `VehHvacCabinGetClimateParamsPodbufHead`                                                                        | `VehClusterHvacCabinGetClimateParamsPodbufHead`                                                    | yes (podbuf-enabled crates)                                            |
| podbuf C header                 | tuple heads                                                  | `ridl-backend-podbuf/src/lib.rs:662-680`                            | `<prefix>_<wire name>_head`                                                                                    | `veh_cluster_VehHvacCabinGetClimateParams_head`                                                                 | `veh_cluster_VehClusterHvacCabinGetClimateParams_head`                                             | yes (when encoded)                                                     |
| Rust crate                      | interaction face, descriptors                                | `face.rs:156-178`, `descriptors.rs:48-70`                           | declared interfaces only                                                                                       | —                                                                                                               | —                                                                                                  | no                                                                     |
| TypeScript                      | `<Interface>Client` (`CabinClient`)                          | `ridl-backend-ts/src/codec.rs:651-668`                              | declared interface name                                                                                        | `CabinClient`                                                                                                   | `CabinClient`                                                                                      | **no**                                                                 |
| TypeScript                      | induced tuple doc comment                                    | `codec.rs:1603-1612`, `:4022-4030`                                  | `name.dotted + "." + member`                                                                                   | ``/** The induced tuple of `veh.hvac.cabin.getClimate`. */``                                                    | `` `veh.cluster.HvacCabin.getClimate` ``                                                           | yes (comment)                                                          |
| catalog descriptor              | `Interface.name` of an inline shape                          | `ridl-descriptor/src/lower.rs:99-129`                               | catalog name = global name                                                                                     | `veh.hvac.cabin`                                                                                                | `veh.cluster.HvacCabin`                                                                            | yes                                                                    |
| catalog hash                    | unit with an inline shape                                    | `catalog_hash.rs:40-55`                                             | SHA-256 of the reduced unit                                                                                    | `H1`                                                                                                            | `H2 ≠ H1`                                                                                          | yes; `examples/cabin` unchanged                                        |
| deployment descriptor           | `DeployedInterface.service`, `Link.service`, `Route.service` | `ridl-descriptor/src/deployment/lower.rs:120-130, 195-220, 286-295` | global name                                                                                                    | `veh.cabin.control`; identity fixture `alpha.face`                                                              | `veh.cabin.Control`; `alpha.FaceService`                                                           | yes                                                                    |
| lowered system / descriptor     | implicit component name                                      | `rsdl/lower.rs:170-175`                                             | global name                                                                                                    | `veh.ego.status`                                                                                                | `veh.ego.Status`                                                                                   | yes                                                                    |
| `interfaces.lock`               | inline shape key                                             | `check.rs:478-485`                                                  | `service:` + global name                                                                                       | `service:veh.hvac.cabin`                                                                                        | `service:veh.cluster.HvacCabin`                                                                    | yes (migrated with `--rename`, number kept)                            |
| `instances.lock`                | implicit component key                                       | rsdl numbering                                                      | global name + `/Unit`                                                                                          | `veh.ego.status/Unit 3`                                                                                         | `veh.ego.Status/Unit 3`                                                                            | yes (migrated with `--rename`, number kept)                            |

ADR-0016's pinned transforms (`snake_case`, `camel_case`, `pascal_case` in
`crates/ridl-ir/src/name.rs:36, :100`) are applied to the same inputs as before;
`joined_camel` (`codegen/names.rs:37`), which ADR-0016 does not pin, is
unchanged. No transform changes; only its input does.

## 4. Tasks

### Task 1 — Records and references (docs first)

**Goal.** ADR-0034 (_Proposed_), the amended references and the dated notes, so
that every later task builds against a written rule.

**Files.** `docs/decisions/ADR-0034-service-declaration-names.md` (new);
`docs/decisions/README.md`;
`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md` (Status, decisions
12, 14, 15, 16); `docs/decisions/ADR-0008-e2-execution.md` (Status, decisions 6,
13, 21, Consequences); `docs/specification/ridl-language-reference.md` (§11,
§14, §14.5, §14.6, §16.4, Appendix C, Appendix G);
`docs/specification/rsdl-language-reference.md` (§1.3, §2, §3.1 to §3.5, §4, §6,
§7, §9, §13, §15, §16.3, §17, Appendix A, Appendix B, Appendix C);
`docs/specification/family-general-form.md` (Shape 3, R2, R7);
`docs/specification/ridl-family-overview.md` (§5 row 44, §6 rsdl §17).

**Tests first.** None (documentation). The fences of the references are not
compiled (they reach the book through `{{#include}}`), so the new form in them
is safe before task 3.

**Commands.** `prim fmt <the files above>`; `just check`; `just link-check`;
`just doc-path-check`; `just story-id-check`; `just book-check`.

**Expected.** All green. No story id in a reference file (the ADRs may hold
them).

**Commit.**

```text
docs(adr): propose ADR-0034 for service declaration names

Record the decisions of driftsys/ridl#1390: a service is declared with one
CamelCase name, its global name is its package followed by that name, and it is
a symbol of its package. The dotted form is RIDL-142, the code ADR-0008
reserved; RIDL-140 is retired. Amend the ridl and rsdl references, the general
form and the overview, and add dated notes to ADR-0015 and ADR-0008.

Refs: #1390
```

### Task 2 — Parser and AST

**Goal.** The tree keeps `ServiceDef { name: DottedName }`; the AST gains the
one accessor every later task uses to tell the new form from the old.

**Files.** `crates/ridl-syntax/src/ast.rs`, `crates/ridl-syntax/src/lib.rs`
(re-export), `crates/ridl-syntax/family.ungram` (comments),
`crates/ridl-syntax/src/parser.rs` (doc comments),
`crates/ridl-syntax/test_data/parser/ok/services_named.ridl` (new) and its
snapshot under `crates/ridl-syntax/tests/snapshots/`.

**Tests first** (in `crates/ridl-syntax/src/ast.rs` `#[cfg(test)]`, or
`crates/ridl-syntax/tests/rsdl_ast.rs` style):

```rust
#[test]
fn a_one_segment_camel_case_service_name_is_its_declared_name() {
    let parse = parse_text("package veh.body\ninterface I {}\nservice Doors : I\n", Profile::Ridl);
    let service = SourceFile::cast(parse.syntax()).unwrap().services().next().unwrap();
    assert_eq!(service.declared_name().map(|t| t.text().to_string()), Some("Doors".into()));
}

#[test]
fn a_dotted_or_lowercase_service_name_has_no_declared_name() {
    for text in ["service veh.body.doors : I", "service doors : I", "service Cabin_2 { }"] {
        let parse = parse_text(&format!("package p\ninterface I {{}}\n{text}\n"), Profile::Ridl);
        let service = SourceFile::cast(parse.syntax()).unwrap().services().next().unwrap();
        assert!(service.name().is_some(), "`{text}` still parses a name");
        assert_eq!(service.declared_name(), None, "`{text}`");
    }
}

#[test]
fn camel_case_id_is_typl_appendix_e() {
    assert!(is_camel_case_id("Doors") && is_camel_case_id("HVAC") && is_camel_case_id("Doors2"));
    assert!(!is_camel_case_id("doors") && !is_camel_case_id("Cabin_2") && !is_camel_case_id(""));
}
```

(Use the crate's existing parse entry point; `parse_text` stands for it.) Add
`test_data/parser/ok/services_named.ridl` with both forms in the new spelling;
keep `test_data/parser/ok/services.ridl` in the dotted form, which pins that the
parser still accepts it (D5).

**Implementation.**

1. `pub fn is_camel_case_id(text: &str) -> bool` in `ast.rs` (an upper-case
   ASCII letter, then ASCII letters and digits — the body of
   `crates/ridl-sem/src/rsdl/mod.rs:697`), re-exported from the crate root.
2. `impl ServiceDef { pub fn declared_name(&self) -> Option<SyntaxToken> }`: the
   one `Ident` of `self.name()?.segments()` when there is exactly one and it
   satisfies `is_camel_case_id`.
3. Rewrite the doc comments of `DottedName::segments`, `service_def`,
   `dotted_name` and the `family.ungram` block: the name is a declared name of
   one segment; the dotted form is parsed so that the checker reports it
   (RIDL-142).

**Commands.** `cargo test -p ridl-syntax`; `cargo insta review` for the new
snapshot only; `just lint`.

**Expected.** Green; one new snapshot; no existing snapshot changes.

**Commit.**

```text
feat(ridl-syntax): add the declared name of a service declaration

ServiceDef::declared_name returns the name of a service declared with one
CamelCase identifier, and nothing for a dotted or lowercase name, which the
parser still accepts so that the checker can report it.

Refs: #1390
```

### Task 3 — Semantics: the symbol, references, RIDL-142, RIDL-140 retired

**Goal.** D2, D3 (ridl and rsdl), D5, D6, and the consumers that the IR's global
name would otherwise break (H1). This is the breaking commit.

**Files.**

- Core and semantics: `crates/ridl-core/src/diag.rs` (and the snapshot
  `crates/ridl-core/src/diag/snapshots/ridl_core__diag__sarif__tests__sarif_shape.snap`,
  whose rule list holds RIDL-140 at `:805`), `crates/ridl-core/src/package.rs`,
  `crates/ridl-sem/src/{resolve.rs,check.rs,doc_lint.rs,expr.rs,lint.rs,workspace.rs}`,
  `crates/ridl-sem/src/rsdl/{mod.rs,collect.rs,closure.rs,target.rs,resolve.rs}`.
- H1 consumers: `crates/ridl-syntax/src/ast.rs` (`InterfaceShape::identity` and
  its tests at `:1379-1438`), `crates/ridl/src/main.rs:2941`,
  `crates/ridlc/src/design_lints/sites.rs:87`,
  `crates/ridl-sem/src/lint.rs:130`,
  `crates/ridl-lsp/src/{inlay.rs,hover.rs,rsdl.rs}`.
- Exhaustive matches: `crates/ridl-lsp/src/complete.rs:961-968`,
  `crates/ridl-lsp/src/hover.rs:937-945`. (`crates/ridl/src/property.rs` has a
  `_ =>` arm and is not touched.)
- Tests and data migrated in this task: every inline source in
  `crates/ridl-core/src/**` and `crates/ridl-sem/src/**` tests;
  `crates/ridl-sem/fixtures/docs/**` (`veh/adas/adas.ridl`,
  `veh/topology/topology.rsdl`); the rsdl tests' `SYSTEM` constant
  (`crates/ridl-sem/src/rsdl/mod.rs:787-…`), which follows the new rsdl Appendix
  A; `crates/ridl-lsp/tests/server.rs` and its fixtures (old-form services at
  `:2144`, `:2146`, `:2755`, `:2923-2927`, `:4746`; hover assertions at `:2498`,
  `:3214`, `:3221`; the RIDL-140 tests at `:4749-4870` become one TYPL-009
  test); the whole `crates/ridlc/tests/corpus/ridl-diag-showcase/` entry —
  `main/{services.ridl,exposure.ridl,inline_shape.ridl,lints.ridl,projection.ridl}`
  (`exposure.ridl:74` carries the RIDL-143 case), `twin/twin.ridl`, `NOTES`
  (`:19` on RIDL-140) and its five snapshots
  (`corpus__{diagnostics,ir,codegen,rust,typescript}@ridl-diag-showcase.snap`);
  `crates/ridlc/tests/corpus.rs` (`RIDL_PROFILE_CODES` `:575`, the severity list
  `:1561`, and `DEFERRED_OR_RETIRED` `:1229`);
  `crates/ridlc/tests/{check_source.rs,workspace_passes.rs,payload_limits.rs}`;
  `crates/ridl/tests/servers.rs:251-262` (the RIDL-140 test is removed: the
  literal guard of `diag.rs:2696` scans every `.rs` file);
  `crates/ridl/tests/diff_cli.rs` (one new test).
- Docs: `docs/technotes/walking-skeleton-architecture.md` (the catalog
  paragraph).

**Tests first.**

In `crates/ridl-sem/src/check.rs` tests (helper `check_ridl(name, text)`,
`:11836`):

```rust
#[test]
fn a_camel_case_service_lowers_under_its_global_name() {
    let checked = check_ridl("veh.body", "package veh.body\ninterface DoorControl {}\nservice Doors : DoorControl\n");
    assert!(errors(&checked).is_empty(), "{:?}", checked.diagnostics);
    assert_eq!(checked.ir.services[0].name, "veh.body.Doors");
}

#[test]
fn a_dotted_service_name_is_ridl_142_with_a_suggested_name() {
    let checked = check_ridl("veh.body", "package veh.body\ninterface DoorControl {}\nservice veh.body.doors.front : DoorControl\n");
    let diagnostic = only(&checked, "RIDL-142");
    assert!(diagnostic.message.contains("service DoorsFront"), "{}", diagnostic.message);
    assert_eq!(diagnostic.fixits[0].replacement, "DoorsFront");
    assert!(checked.ir.services.is_empty(), "the old form binds nothing");
}

#[test]
fn the_suggestion_drops_only_the_shared_package_prefix() {
    for (package, written, suggested) in [
        ("veh.cluster", "veh.hvac.cabin", "HvacCabin"),
        ("veh.cabin", "veh.cabin.control", "Control"),
        ("p", "doors", "Doors"),
        ("p", "cabin_2", "Cabin2"),
        ("a.b", "a.b", "B"),
    ] {
        assert_eq!(suggested_service_name(package, written, &|_| false), suggested);
    }
}

#[test]
fn the_suggestion_appends_service_on_a_clash() {
    let checked = check_ridl("alpha", "package alpha\ninterface Face {}\nservice alpha.face : Face\n");
    assert_eq!(only(&checked, "RIDL-142").fixits[0].replacement, "FaceService");
}

/// ADR-0034 decision 5: old-form declarations bind nothing, so the suggestion
/// must also avoid the names already suggested in the same package.
#[test]
fn colliding_old_form_services_get_distinct_suggestions() {
    let checked = check_ridl("veh.cabin", "package veh.cabin\ninterface I {}\n\
        service veh.cabin.control : I\nservice control : I\nservice veh.cabin.control_ : I\n");
    let suggested: Vec<&str> = checked.diagnostics.iter()
        .filter(|d| d.code.as_str() == "RIDL-142")
        .map(|d| d.fixits[0].replacement.as_str())
        .collect();
    assert_eq!(suggested, ["Control", "ControlService", "ControlService2"]);
}

#[test]
fn a_service_and_an_interface_of_one_name_are_typl_009() {
    let checked = check_ridl("veh.hvac", "package veh.hvac\ntype State: integer [0..1]\n\
        interface Cabin { signal i : State @[100ms..1s] }\nservice Cabin { signal s : State @[100ms..1s] }\n");
    assert_eq!(errors(&checked), ["TYPL-009"]);
}

#[test]
fn a_service_in_a_type_position_is_not_a_type() {
    let checked = check_ridl("p", "package p\ninterface I {}\nservice S : I\nstruct X { s: S }\n");
    assert!(checked.diagnostics.iter().any(|d| d.message.contains("names a service")));
}

#[test]
fn a_doc_link_names_a_service_and_its_member() {
    let checked = check_ridl("p", "package p\ntype V: integer [0..1]\n\
        interface I { signal level: V @[100ms..1s] }\n/// Published as [S]; see [S.level].\nservice S : I\n");
    assert!(!codes(&checked).contains(&"TYPL-401"), "{:?}", checked.diagnostics);
}
```

(`veh.cabin.control_` splits at `_` to `Control` as well; the third collision
pins `Service2`.) Replace `interface_cabin_and_service_cabin_get_two_numbers`
(`:16570-16590`) with the TYPL-009 test above, and rewrite the `service:` keys
in the numbering tests (`:16490-16565`) to `service:veh.hvac.Rear`.

In `crates/ridl-sem/src/rsdl/mod.rs` tests (helper `check(packages)`):

```rust
#[test]
fn offers_resolves_a_service_bare_imported_and_qualified() {
    let ridl = ("adas.ridl", "package veh.adas\ninterface CruiseControl {}\nservice AdaptiveCruise : CruiseControl\n");
    for (import, line) in [
        ("import veh.adas.AdaptiveCruise\n", "offers AdaptiveCruise"),
        ("", "offers veh.adas.AdaptiveCruise"),
    ] {
        let rsdl = format!("package veh.top\n{import}component Cruise {{ {line} }}\nsystem V {{ Cruise }}\n");
        let system = check(&[("veh.adas", &[ridl]), ("veh.top", &[("top.rsdl", rsdl.as_str())])]);
        assert!(errors(&system).is_empty(), "`{line}`: {:?}", system.diagnostics);
    }
}

#[test]
fn a_lone_service_is_a_member_and_a_placement() {
    let ridl = ("diag.ridl", "package veh.diag\ntype R: integer [0..1]\nservice FaultAccess { query read(): R @[..100ms] }\n");
    let rsdl = ("top.rsdl", "package veh.top\nsystem V { veh.diag.FaultAccess }\n\
        deployment D for V { machine M { veh.diag.FaultAccess } }\n");
    let system = check(&[("veh.diag", &[ridl]), ("veh.top", &[rsdl])]);
    assert!(errors(&system).is_empty(), "{:?}", system.diagnostics);
}

/// H2: a bare name binds the package's own declaration before an import, of
/// whatever kind the local declaration is.
#[test]
fn a_local_service_shadows_an_imported_component() {
    let q = ("q.rsdl", "package q\ncomponent S {}\n");
    let p_ridl = ("p.ridl", "package p\ninterface I {}\nservice S : I\n");
    let p_rsdl = ("p.rsdl", "package p\nimport q.S\ncomponent Host { offers S }\nsystem V { Host }\n");
    let system = check(&[("q", &[q]), ("p", &[p_ridl, p_rsdl])]);
    assert!(!codes(&system).contains(&"RSDL-310"), "{:?}", system.diagnostics);
}

#[test]
fn the_old_dotted_reference_names_nothing_and_says_why() {
    let ridl = ("adas.ridl", "package veh.adas\ninterface CruiseControl {}\nservice AdaptiveCruise : CruiseControl\n");
    let rsdl = ("top.rsdl", "package veh.top\ncomponent Cruise { offers veh.adas.cruise }\nsystem V { Cruise }\n");
    let system = check(&[("veh.adas", &[ridl]), ("veh.top", &[rsdl])]);
    let diagnostic = system.diagnostics.iter().find(|d| d.code.as_str() == "RSDL-310").unwrap();
    assert!(diagnostic.message.contains("CamelCase"), "{}", diagnostic.message);
}

#[test]
fn a_component_and_a_service_of_one_name_are_typl_009() {
    let ridl = ("p.ridl", "package p\ninterface I {}\nservice Cruise : I\n");
    let rsdl = ("p.rsdl", "package p\ncomponent Cruise { offers Cruise }\nsystem V { Cruise }\n");
    let system = check(&[("p", &[ridl, rsdl])]);
    assert!(codes(&system).contains(&"TYPL-009"));
}
```

(Whether `import q.S` beside a local `S` also draws an import-collision code is
whatever the resolver does today for a local declaration of the same name; the
test asserts only that the local service is the one `offers` binds.) Add the
same case to the editor target tests of `crates/ridl-sem/src/rsdl/target.rs`
(`:339-…`): the target of `S` on the `offers` line is `Target::Service("p.S")`.

In `crates/ridl-sem/src/rsdl/collect.rs` tests, change
`a_reference_reads_its_role_from_case` (`:478-500`): `veh.diag.access` reads
`Unreadable`, and add `veh.diag.FaultAccess` reading
`Declared { package: Some("veh.diag"), name: "FaultAccess" }`.

In `crates/ridl-core/src/package.rs` tests (`:470-500`): the catalog of two
packages each declaring `service S` has the keys `a.S` and `b.S`; replace the
RIDL-140 test.

H1 regression tests, each failing before the `identity(&package)` change:

- `crates/ridlc/tests/payload_limits.rs`: an inline service
  `service Cabin { signal blob : Huge @[100ms..1s] }` in package `veh.hvac`,
  where `Huge` is above the signal ceiling, draws RIDL-157 on `blob` (today the
  member's site lookup misses and the check is skipped).
- `crates/ridl/tests/diff_cli.rs`: an interaction removed from an inline shape
  between two sources renders with a span inside `service Cabin { … }`.
- `crates/ridl-lsp/tests/server.rs`: hover on an interaction of an inline shape
  shows the interaction's signature; ordinal inlay hints appear on every member
  of an inline shape.

`crates/ridlc/tests/check_source.rs`, replacing
`a_duplicate_service_name_is_ridl_140` (`:72-87`):

```rust
#[test]
fn a_duplicate_service_name_is_typl_009() {
    let source = "package p\n/// An interface.\ninterface I {}\n/// A service.\nservice S : I\n\
                  /// The same service again.\nservice S : I\n";
    let run = ridlc::check_source("dup.ridl", source);
    assert_eq!(codes(&run), ["TYPL-009"], "{:?}", run.diagnostics);
}
```

`crates/ridlc/tests/workspace_passes.rs:12-63`: the two members declare
`service S` in packages `a` and `b` (two global names), and only RSDL-602 is
expected.

**Implementation.**

1. `diag.rs`: add `140` to `RETIRED_RIDL_CODES` with a doc paragraph (ADR-0034
   decision 6); delete `RIDL_140`; declare `RIDL_142` (error, summary "`service`
   declared name is not one CamelCase identifier") between `RIDL_141` and
   `RIDL_143`; fix the catalogue doc (`:653-660`) and the `RIDL_143` doc
   (`:803-806`) that call RIDL-142 reserved. Regenerate the SARIF snapshot. In
   `crates/ridlc/tests/corpus.rs`, add
   `("RIDL", 140, "retired: a service name is in its package's namespace (ADR-0034 decision 6)")`
   to `DEFERRED_OR_RETIRED` (`:1229`), which
   `typl_and_ridl_catalogue_entries_match_the_reference_tables` (`:1437`) needs
   for the reference's retired row; replace `("RIDL-140", Showcase)` with
   `("RIDL-142", Showcase)` at `:575` and the severity row at `:1561`.
2. `ridl-syntax/src/ast.rs`: `InterfaceShape::identity(&self, package: &str)`
   (section 2.1); update its four callers and the tests.
3. `ridl-sem/src/resolve.rs`: `SymbolKind::Service`; `Declaration::Service`;
   `declarations()` yields a `ServiceDef` whose `declared_name()` is `Some`;
   `name()`/`name_range()` read the declared-name token; `symbol()` gives kind
   `Service`, `internal: false`, `is_error: false`.
4. `doc_link_members`: for `SymbolKind::Service`, find the `ServiceDef` by its
   declared name in the symbol's file; inline → its member names; list → the
   members of each listed interface resolved through the package view.
5. `check.rs`:
   - `pub(crate) fn suggested_service_name(package: &str, written: &str, taken: &dyn Fn(&str) -> bool) -> String`:
     steps 1 and 2 of ADR-0034 decision 5 (segments compared one by one, never
     by a string prefix), then the first of `base`, `base + "Service"`,
     `base + "Service2"`, `base + "Service3"`, … for which `taken` is false.
     Total: it always returns a name.
   - Before the services loop, collect the package's old-form declarations in
     the package's file order and source order, and compute their suggestions in
     that order, with `taken(n)` true when the package declares `n`
     (`self.resolution.symbols` holds `n` with `symbol.package` equal to this
     package) or when `n` was suggested to an earlier one. Keep them in a map
     from the `DottedName` range to the suggestion.
   - `fn report_old_service_name(&mut self, service: &ast::ServiceDef)`:
     RIDL-142 at the `DottedName` range, message
     ``"`{written}` is not a service name: a service is declared with one CamelCase name, and its global name is its package followed by that name — write `service {suggested}` (global name `{package}.{suggested}`) (ridl §14.5)"``,
     and one `FixIt` replacing the range with the suggestion, labelled
     ``"rename the service to `{suggested}`"`` (pattern: `doc_lint.rs:103-128`).
   - The services loop (`:215-240`): no `declared_name()` and a non-empty
     `name()` → `report_old_service_name`, `continue`; otherwise skip unless the
     resolver's symbol for the declared name is this declaration (as `is_winner`
     does), then `lower_service`.
   - `lower_service`, `lower_service_inline`: the global name by join.
   - Delete `check_service_name` and `is_lowercase_name_segment`.
   - `resolve_type_path` (`:995-1015`): the `Service` arm.
   - Fill the remaining non-exhaustive `SymbolKind` matches the compiler
     reports, here, in `expr.rs`, `lint.rs`, and in `complete.rs:961-968` and
     `hover.rs:937-945` of `ridl-lsp` (a service is treated as an interface).
6. `doc_lint.rs` `carrier_name`: the declared name for a new-form service.
7. `ridl-core/src/package.rs`: `service_catalog` keys entries by
   `format!("{package_name}.{declared}")` and skips the old form. It reports
   nothing, so remove `ServiceCatalog::diagnostics` and its one consumer,
   `ridl-sem/src/workspace.rs:40` (the workspace diagnostics then start with the
   rsdl system query's). `package_names` adds service names to `locals`.
8. rsdl: remove `ReferenceForm::Service`; `form_of` returns `Unreadable` for
   lowercase-only; replace `Lookup::declared` by `Lookup::bind` (section 2.3,
   H2) and rewrite `offered_service`, `required_interface`, `member_target` and
   `rsdl/target.rs` over it; add
   `fn old_service_hint(reference: &Reference) -> &'static str` returning
   `"; a service is named by its CamelCase name, bare, imported or after its package (ridl §14.5)"`
   when every segment is lowercase, and append it to the RSDL-310, RSDL-312,
   RSDL-602, RSDL-702 and RSDL-903 messages.
9. Language server (H1, and the lookups task 3 breaks): `inlay.rs:303`,
   `hover.rs:1068-1084` (`enclosing_shape`), `hover.rs:1027-1056`
   (`service_hover`) and `rsdl.rs:616-655` compare with the join. Migrate
   `crates/ridl-lsp/tests/server.rs` to the new form; assertions that read the
   catalog (completion after `offers`) expect global names until task 6 adds
   bare-name completion.
10. Showcase: migrate the whole `ridl-diag-showcase` entry, then in
    `main/services.ridl` replace the RIDL-140 pair with one old-form declaration
    that provokes RIDL-142 (`service diag.showcase.legacy : Published`).

**Commands.** Section 1's task-3 row, in this order:

```sh
cargo test -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-lsp -p ridl-fmt
cargo test -p ridlc --test check_source --test workspace_passes
cargo test -p ridlc --test payload_limits -- --exact <the inline RIDL-157 test>
cargo test -p ridlc --test corpus -- --exact <the seven names of section 1>
INSTA_GLOB_FILTER='ridl-diag-showcase/*' cargo insta test -p ridlc --test corpus \
  -- --exact corpus_entries_compile_to_reviewed_snapshots
cargo test -p ridl-cli --test servers
cargo test -p ridl-cli --test diff_cli -- --exact <the inline span test>
cargo build --workspace --all-targets --locked && just lint && just fmt-check
cargo test --workspace --no-fail-fast 2>&1 | tee target/1390-task3.log   # only section 1's exempt targets fail
```

**Expected.** Every command but the last green; the last fails only in the
exempt targets of section 1.

**Commit.**

```text
feat(ridl-sem)!: name a service with one CamelCase identifier

A service is declared with one CamelCase name. Its global name is its package
followed by that name, and it is a symbol of its package: a clash with any
other declaration of the package is TYPL-009, and offers, requires, member and
placement lines, imports and doc links resolve it as an interface reference,
the package's own declaration before an import. A dotted or lowercase name is
RIDL-142, with a suggested name as a fix-it. RIDL-140 is retired: a duplicate
global name needs two packages of one name. Inline-shape lookups in the lints,
the diff spans and the language server use the global name.

BREAKING CHANGE: `service veh.body.doors : I` is refused (RIDL-142). Declare
`service Doors : I` in package `veh.body`; its global name is
`veh.body.Doors` and a member is `veh.body.Doors.member`. Rewrite each rsdl
`offers` line, each lone service in a system, distribution or machine body,
each `requires` line naming an inline-shape service and each doc link to the
new name (bare, imported or qualified). Then, for each inline shape, run
`ridl lock <unit directory> --rename service:<old name>=service:<package>.<Name>`,
once per unit; and for each lone service listed in a system, run
`ridl lock <workspace root> --rename <old name>=<package>.<Name>`. `ridl diff`
against a baseline published before the migration reports every renamed
service as removed and added, which is breaking; publish a new baseline with
`ridl baseline` after migrating.

Refs: #1390
```

### Task 4 — IR, lock keys, descriptors, CLI sites

**Goal.** D4 and D7 pinned by tests; the remaining splitting sites fixed (the
inline-shape lookups were fixed in task 3); the IR comments state the global
name; the inline sources of `ridl-ir` and `ridl-diff` migrated.

**Files.** `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`,
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (comments),
`crates/ridl-ir/src/lib.rs` (docs and tests at `:1006` area),
`crates/ridl-ir/src/codegen/{tests.rs,deployment.rs}` (test data),
`crates/ridl-descriptor/src/number.rs` (tests `:99-200`),
`crates/ridl-descriptor/tests/lower.rs` (`:716`),
`crates/ridlc/src/design_lints/limits.rs`,
`crates/ridlc/tests/payload_limits.rs`, `crates/ridl/src/main.rs`,
`crates/ridl/tests/{lock_cli.rs,instance_lock_cli.rs,baseline_gate.rs}`,
`crates/ridl-core/src/interface_lock.rs` (tests), and the inline sources of
`crates/ridl-ir/src/lib.rs` and `crates/ridl-diff/src/system.rs` (section 1:
exempt from task 3 until this task).

**Tests first.**

`crates/ridl/tests/lock_cli.rs` (helpers `lock(&root, args)`, `read_lock`,
`check`):

```rust
/// ADR-0034 decision 7: an inline shape's entry moves to the global name and
/// keeps its number.
#[test]
fn rename_moves_an_inline_shape_key_to_the_global_name() {
    let dir = TempDir::new("rename-service-key");
    let root = single_unit(&dir, "package veh.hvac\ntype L: integer [0..1]\n\
        service Rear { signal t : L @[100ms..1s] }\n");
    write_lock(&root, "next 2\nservice:veh.hvac.rear 1\n");
    let (code, stdout, stderr) = lock(&root, &["--rename", "service:veh.hvac.rear=service:veh.hvac.Rear"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "renamed service:veh.hvac.rear service:veh.hvac.Rear 1\n");
    assert_eq!(read_lock(&root), format!("{HEADER}next 2\nservice:veh.hvac.Rear 1\n"));
    assert_eq!(check(&root).0, 0);
}
```

`crates/ridl/tests/instance_lock_cli.rs`: a lone service `service Status { … }`
in `veh.ego`, `instances.lock` holding `veh.ego.status/Unit 3`;
`ridl lock <root> --rename veh.ego.status=veh.ego.Status` exits 0 and the file
reads `veh.ego.Status/Unit 3`. Pin the two conditions of `edit()`
(`crates/ridl/src/lock.rs:515-540`) that the migration text states: the instance
rename run with a member directory instead of the root exits 2 with "belongs to
the workspace root", and the `service:` rename run at the root of a two-unit
workspace exits 2 with "name the unit directory".

`crates/ridlc/tests/payload_limits.rs`: a `[limits.interaction]` key
`Cabin.temperature` names the inline service `Cabin` of package `veh.hvac` (no
RIDL-152), and `veh.hvac.Cabin.temperature` is RIDL-152 (written with the
package).

`crates/ridl/tests/` (the `ridl diff` span tests, e.g. `diff_cli.rs`): a
named-form service removed between two snapshots renders its span at the
declaration `service Doors : …` (exercises `main.rs:2950-2985` with the global
name).

**Implementation.**

1. `limits.rs:75-77`: `short_name` takes the shape; for an inline shape return
   `service.name.strip_prefix(&format!("{}.", package.name))`, for a declared
   interface its name. Update the module doc (`:19`).
2. `main.rs:2950-2985`: key `index.shapes` and `index.members` by
   `format!("{package}.{declared}")` from `service.declared_name()`; skip the
   old form.
3. Rewrite the `service:` keys and `v2::Service { name: … }` values in the
   listed tests to global names with a CamelCase last segment
   (`corpus.baseline.Hvac`, `veh.hvac.Rear`).
4. Comments: `ir.proto:517-519`, `model.proto:118-130`, `lib.rs:776-863`.

**Commands.**

```sh
cargo test -p ridl-ir -p ridl-descriptor -p ridl-diff -p ridl-core
cargo test -p ridl-cli --test lock_cli --test instance_lock_cli --test baseline_gate
cargo test -p ridlc --test payload_limits
just lint
```

**Expected.** Green. No freeze command runs in this task: `just freeze-check` is
exempt from task 3 until task 5 (section 1), so it would fail on `diagnostics`
and `accepted-sources` here. Task 5 confirms that `freeze/codegen-protocol.txt`
does not change, since this task edits proto comments only.

**Commit.**

```text
feat(ridlc): key inline shapes and lock entries by the service's global name

Name an inline shape in a [limits.interaction] key by its service's declared
name without splitting the global name, index named services for ridl diff by
the global name, and pin with tests that ridl lock --rename moves a service:
key and an implicit component to the new names with their numbers.

Refs: #1390
```

### Task 5 — Backends, wire identity, frozen surfaces

**Goal.** D8: the backends' test inputs in the new form, snapshots regenerated,
the D8 table verified by tests, and the frozen-surface breaks approved by
ADR-0029 decision 4.

**Files.** `crates/ridl-backend-proto/src/tests.rs` (`:65-100`),
`crates/ridl-backend-flatbuffers/src/tests.rs` (`:92-…`),
`crates/ridl-backend-{rust,ts,proto,flatbuffers,podbuf}/tests/**` inputs and
snapshots, `crates/ridl-backend-rust/tests/generated/*.rs`
(`RIDL_UPDATE_GENERATED=1`), `crates/ridl-backend-rust/src/descriptors.rs:48-70`
and `face.rs:156-160` (comments: "the global name"), `crates/ridlc-gen-model`,
`crates/ridlc-gen-rust` test inputs,
`xtask/tests/freeze-fixtures/sources/examples/cabin/cabin.ridl`,
`xtask/tests/freeze-fixtures/sources/examples/cabin/system.rsdl`,
`xtask/tests/freeze-fixtures/generated/cabin/{cabin.ridl,system.rsdl}`,
`xtask/tests/freeze-fixtures/generated/veh-cluster/cluster/{services.ridl,evolution.ridl}`
(and its `NOTES`), `freeze/{wire.txt,diagnostics.txt,accepted-sources.txt}`,
`freeze/generated/veh-cluster/**`, `freeze/surfaces.toml`,
`docs/book/stability.md`, and every entry of `crates/ridlc/tests/corpus/` except
`ridl-diag-showcase` (moved in task 3): the backend conformance and corpus tests
read `veh-cluster`, `veh-common`, `services-workspace`, `rsdl-appendix-a`,
`rsdl-diag-showcase`, `diag-showcase`, `workspace-two-members` and
`instance-numbers` (`crates/ridl-backend-rust/tests/podbuf_conformance.rs`,
`proto3_conformance.rs`, `crates/ridl-backend-proto/tests/corpus.rs`), with
their `interfaces.lock` and `instances.lock` keys and every
`corpus__*@<entry>.snap`.

**Tests first.**

`crates/ridl-backend-proto/src/tests.rs`, replacing
`an_inline_service_shape_is_named_from_the_service_address`:

```rust
#[test]
fn an_inline_service_shape_is_named_from_the_global_name() {
    let package = v2::Package {
        name: "corpus.baseline".to_string(),
        services: vec![v2::Service {
            name: "corpus.baseline.Hvac".to_string(),
            shapes: vec![v2::ServiceShape { kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                name: String::new(), interactions: vec![signal_decl("cabinTemp", 1)], ..Default::default() })) }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let generated = generate(&package).expect("generate");
    assert!(generated.proto_source.contains("enum CorpusBaselineHvacOrdinal {"));
    assert!(generated.proto_source.contains("CORPUS_BASELINE_HVAC_ORDINAL_CABIN_TEMP = 1;"));
}

#[test]
fn a_global_name_outside_its_old_prefix_moves_the_identifiers() {
    // `service veh.hvac.cabin` in `veh.cluster` becomes `HvacCabin` (ADR-0034 decision 8).
    let generated = generate(&inline_package("veh.cluster", "veh.cluster.HvacCabin")).expect("generate");
    assert!(generated.proto_source.contains("enum VehClusterHvacCabinOrdinal {"));
}
```

The same pair in `crates/ridl-backend-flatbuffers/src/tests.rs`.

The package-prefixed case (M4): `service veh.cluster.wheels` in package
`veh.cluster` becomes `Wheels`. Add to `crates/ridl-ir/src/catalog_hash.rs`
tests:

```rust
/// ADR-0034 decision 8: a rename that keeps every generated identifier still
/// changes the catalog hash input and the fallible-return identity.
#[test]
fn a_package_prefixed_rename_keeps_identifiers_and_moves_the_hash() {
    let before = inline_unit("veh.cluster", "veh.cluster.wheels");
    let after = inline_unit("veh.cluster", "veh.cluster.Wheels");
    assert_ne!(catalog_hash("veh.cluster", &[&before]), catalog_hash("veh.cluster", &[&after]));
    assert_ne!(
        v2::fallible_transport_identity("veh.cluster.wheels", 1, &fallible()),
        v2::fallible_transport_identity("veh.cluster.Wheels", 1, &fallible()),
    );
}
```

and in the proto3 tests,
`generate(&inline_package("veh.cluster", "veh.cluster.Wheels"))` still emits
`enum VehClusterWheelsOrdinal {`. (`inline_unit`, `inline_package` and
`fallible` are small builders in each test module, as
`an_inline_service_shape_is_named_from_the_global_name` builds its package.) In
`crates/ridl-backend-ts` add a codec test whose induced-tuple doc comment reads
`` `veh.cluster.HvacCabin.getClimate` ``. In `ridl-ir`'s codegen tests, assert
the model's `Service.name` of `veh.cluster.HvacCabin` has segments
`["veh", "cluster", "HvacCabin"]` and `joined_camel` `VehClusterHvacCabin`.

**Implementation and the freeze procedure.**

1. Migrate every backend test input with the RIDL-142 suggestion (section 6);
   run `cargo insta test -p <crate> --accept` per backend, then read each
   snapshot diff: a changed line must be a row of the D8 table, nothing else.
2. `RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face`.
3. **Show the break before approving it, with the frozen copies unchanged.** Run
   `cargo xtask freeze update`, then `cargo xtask freeze check`, before touching
   `xtask/tests/freeze-fixtures/**`. Expected failures, with the removed lines:
   `diagnostics` (`RIDL-140 error`) and `accepted-sources` (`examples/cabin ok`:
   the frozen copy of `examples/cabin` declares `service veh.cabin.control`,
   which `ridl check` now refuses, so `xtask/src/freeze/sources.rs:24-37` writes
   no `ok` line for it). That removed line is a break as ADR-0029 decision 3
   defines it (a line of the baseline dump missing at `HEAD`); it is not
   inferred from policy. Record both outputs for the pull request description.
4. **Approve, by the step that applies.** ADR-0029 decision 4's two-step
   procedure, which its bullet "0.11 starts with a pull request that breaks
   surfaces on purpose, with the same two steps" extends to a later pre-1.0
   minor: delete the `[[surface]]` rows of `diagnostics`, `accepted-sources`,
   `wire` and `generated-crate` from `freeze/surfaces.toml`, keep the four names
   in `required`; the pull request title carries `!` (task 3's subject), its
   description states the approval, and it merges only on the maintainer's
   explicit approval. `wire` and `generated-crate` are included because step 5
   removes lines from them.
5. **Then refresh the frozen copies on purpose** (ADR-0029: "A copy is refreshed
   on purpose"): migrate `xtask/tests/freeze-fixtures/**` with the rules of
   section 6 and run `cargo xtask freeze update`. `accepted-sources` holds
   `examples/cabin ok` again; `wire` replaces every `VehHvacCabin…` line
   (`freeze/wire.txt:737-753, 826-…, 3610-…`, 76 lines) with
   `VehClusterHvacCabin…`; `generated-crate` changes
   `freeze/generated/veh-cluster/veh.cluster.rs` (410 uses). The `xtask` unit
   tests that expect `examples/cabin ok` (`xtask/src/freeze/sources.rs:54`,
   `:73`) pass again. Confirm `git diff --exit-code freeze/codegen-protocol.txt`
   (task 4 edited proto comments only). `just freeze-check` then prints the
   notice for the four names and passes; the follow-up pull request records the
   refreshed dumps as the new baseline.
6. `docs/book/stability.md`: a section "Service declaration names" after
   "FlatBuffers enum width adoption" (text in section 7 of this plan), stating
   that the four rows are removed until the follow-up restores them.
7. Migrate the ridlc corpus entries listed under **Files**, run
   `cargo insta test -p ridlc --test corpus --accept`, and read each diff.

**Commands.**

```sh
cargo test -p ridl-ir -p ridl-backend-proto -p ridl-backend-flatbuffers -p ridl-backend-podbuf \
  -p ridl-backend-rust -p ridl-backend-ts -p ridlc-gen-model -p ridlc-gen-rust
cargo test -p ridlc -p xtask
cargo xtask freeze update && just freeze-check
just lint
```

**Expected.** Green; `just freeze-check` passes with the notice for four
surfaces.

**Commit.**

```text
test(ridl-backend-proto): pin inline shape names derived from the global name

The ordinal enums, the induced message and table names and the podbuf heads of
an inline shape derive from the service's global name, package and declared
name. A service whose old dotted name began with its package keeps every
generated identifier; another moves (veh.hvac.cabin in veh.cluster becomes
VehClusterHvacCabin); every renamed inline shape still changes its catalog
hash input and its fallible-return identities. Migrate the backend test
inputs, the ridlc corpora and the frozen copies, and approve the breaks of the
diagnostics, accepted-sources, wire and generated-crate surfaces under
ADR-0029 decision 4: their baseline rows are removed and a follow-up pull
request restores them. Add the migration section to the stability chapter.

Refs: #1390
```

### Task 6 — Language server, MCP server, formatter, VS Code

**Goal.** The tooling features of D2 and D3 that task 3 did not need: go to
definition, rename and completion of services as symbols in the language server,
the RIDL-142 quick-fix test, the MCP resolution of a service, and the `ridl-mcp`
and `ridl-fmt` fixtures. (The language server's lookups by global name and the
migration of `crates/ridl-lsp/tests/server.rs` are task 3.)

**Files.**
`crates/ridl-lsp/src/{nav.rs,hover.rs,complete.rs,rsdl.rs,rename.rs}`,
`crates/ridl-lsp/tests/server.rs` and `crates/ridl-lsp/tests/fixtures/**`,
`crates/ridl-mcp/src/{query.rs,lib.rs}`, `crates/ridl-mcp/tests/**` (fixtures
`ws`, `ws-diag`, `ws-rsdl`, `ws-v2`; `tools.json`), `crates/ridl-mcp/README.md`,
`docs/design/mcp-workspace-tools.md`, `freeze/mcp.txt` (only if a schema
description changes),
`crates/ridl-fmt/test_data/{input,formatted}/{services.ridl,component.rsdl,rsdl_attribute_positions.rsdl}`,
`crates/ridl-fmt/src/lib.rs` (unit tests `:1991-1993`, `:2507`),
`crates/ridl-fmt/tests/properties.rs:109`. VS Code: no change (section 0);
`editors/vscode` tests are run, not edited.

**Tests first** (`crates/ridl-lsp/tests/server.rs`, helpers `code_actions_at`,
`single_edit`, the hover and definition helpers in the file):

```rust
/// ADR-0034 decision 5: the RIDL-142 fix-it is a quick fix.
#[test]
fn ridl_142_offers_the_suggested_name_as_a_quick_fix() {
    // open "package veh.body\ninterface DoorControl {}\nservice veh.body.doors : DoorControl\n"
    // publishDiagnostics carries RIDL-142 on line 2; code_actions_at(line 2)
    // returns one action titled "rename the service to `Doors`" whose edit
    // replaces `veh.body.doors` with `Doors`.
}

#[test]
fn definition_on_an_offers_line_goes_to_the_service_declaration() { /* `offers AdaptiveCruise` → adas.ridl line of `service AdaptiveCruise` */ }

#[test]
fn hover_on_a_service_name_renders_its_global_name() { /* "service veh.adas.AdaptiveCruise : CruiseControl" */ }

#[test]
fn rename_of_a_service_rewrites_the_declaration_imports_offers_and_doc_links() { /* Cruise → Cruising */ }

#[test]
fn completion_after_offers_lists_services_in_scope_bare_and_qualified() { /* AdaptiveCruise, veh.adas.LaneKeeping */ }
```

`crates/ridl-mcp` tests: `ridl_resolve { name: "veh.adas.AdaptiveCruise" }`
returns `kind: "service"`;
`ridl_list_interactions { interface: "FaultAccess", from: "veh.diag" }` returns
the inline shape's interactions; `ridl_list_interactions` on a list-form service
is a request error naming its interfaces. `ridl_references` is not changed:
reporting the components that offer a service is left to a later change
(ADR-0034 decision 3), and no test asserts it.

`crates/ridl-fmt`: `services.ridl` input/formatted pair in the new form
(`service Cruise: CruiseControl`, `service Cabin {` …); run unchanged code.

**Implementation.** Per section 2.6. Rename (`rsdl.rs:350, 385, 457`) uses the
declaration path that interfaces use (`rename.rs`), plus rsdl references.

**Commands.**

```sh
cargo test -p ridl-lsp -p ridl-mcp -p ridl-fmt
just freeze-check                   # mcp surface: only added lines; four notices
just vscode-verify
just lint
```

**Expected.** Green; `freeze/mcp.txt` gains lines at most.

**Commit.**

```text
feat(ridl-lsp): navigate, rename and fix service names

A service is a declaration of its package in the language server: hover, go
to definition, references, rename and completion work on its declaration, on
offers and requires lines, on lone services and in doc links, and the RIDL-142
fix-it is offered as a quick fix. The MCP tools resolve a service and list the
interactions of an inline shape.

Refs: #1390
```

### Task 7 — Migration of every remaining source, then the full gate

**Goal.** Every tracked source and document in the new form; `just verify`
green. Done by a mechanical subagent with the rules of section 6.

**Footprint** (measured on `276331b1`, `git ls-files`, excluding
`docs/archive/`, `docs/wip/`, `CHANGELOG.md`): 83 files hold a service
declaration in the old form (`crates/ridlc` 29, `crates/ridl` 10,
`crates/ridl-sem` 8, `crates/ridl-syntax` 6, `docs` 6, `xtask` 4,
`crates/ridl-rt-conformance` 4, `crates/ridl-mcp` 4, `crates/ridl-lsp` 3,
`crates/ridl-fmt` 3, `evals` 2, `skills` 1, `examples` 1, `crates/ridl-ir` 1,
`crates/ridl-core` 1); 25 tracked `.rsdl` files hold an `offers` line, and 41
files hold `offers <lowercase>.<…>` text (`crates/ridlc` 11,
`crates/ridl-syntax` 7, `crates/ridl-fmt` 5, `crates/ridl` 5, `docs` 4,
`crates/ridl-sem` 4, `xtask` 2, `evals` 2, `crates/ridl-transport-ws` 2, and one
each in `skills`, `examples`, `crates/ridl-rt-conformance`, `crates/ridl-mcp`,
`crates/ridl-lsp`, `crates/ridl-loopback`, `crates/ridl-diff`,
`crates/ridl-core`). Tasks 3 to 6 take the share of their crates and the
`crates/ridlc/tests/corpus/` entries; this task takes the rest:

- `crates/ridlc/tests/*.rs` inline sources not migrated by tasks 3 to 5 (the
  corpus entries moved in tasks 3 and 5).
- `crates/ridl/tests/*.rs` inline sources and
  `crates/ridl/tests/baseline-corpus/cluster.ridl`.
- `crates/ridl-syntax/test_data/lexer/interactions.ridl`,
  `test_data/parser/err/{reserved_in_service_list,reserved_ungrammatical_literal,rsdl_foreign_declarations}.ridl`
  and the rsdl parser fixtures (`rsdl_appendix_a_system`, `rsdl_declarations`,
  `rsdl_attribute_positions`, `rsdl_internal_modifier`,
  `rsdl_one_reference_per_line`, `rsdl_foreign_declarations`); keep
  `test_data/parser/ok/services.ridl` dotted (task 2).
- `crates/ridl-rt-conformance/fixtures/identity/{alpha,beta,mapped,remote}/face.ridl`
  (`service FaceService : Face`, the clash rule), `alpha/system.rsdl`
  (`offers FaceService`, `offers beta.FaceService`, …), its `README.md` prose;
  regenerate `identity.deployment.binfb` by the README procedure with the output
  directory under `target/` (`target/identity-output`), then
  `cargo test -p ridl-cli --test identity_fixture` and the loopback and
  transport tests that read it.
- `crates/ridl-transport-ws/**`, `crates/ridl-loopback/**` test strings.
- `examples/cabin/{cabin.ridl,system.rsdl,server/src/main.rs}`
  (`service
  Control: Cabin, Horn`, `offers Control`), and its README if it
  names the service; `examples/docstore` has no service.
- `docs/book/{getting-started.md,rsdl.md,cli-reference.md,documenting.md}`
  (fences are compiled), `docs/technotes/rsdl-implementation.md`,
  `docs/design/{ridl-transport-ws.md,codegen-plugins.md,deployment-descriptor.md}`,
  `crates/ridl-mcp/README.md`.
- `skills/ridl-authoring/SKILL.md:102-108`
  (`service skill.intro.cabin.control`).
- `evals/tasks/{design-0004,evolve-0003}/start/{cabin.ridl,system.rsdl}` and any
  expected output of those tasks.
- `CONTRIBUTING.md` if it shows a service.

**Tests first.** None new: the suites listed red in section 1 are the tests.

**Commands.**

```sh
git ls-files | xargs grep -nE "(^|[^A-Za-z_])service +[a-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)* *[:{]" \
  | grep -v "^docs/archive/\|^docs/wip/\|CHANGELOG\|test_data/parser/ok/services.ridl\|ridl-diag-showcase/main/services.ridl"
# expected: only the deliberate RIDL-142 inputs of tasks 2, 3 and 6
git ls-files | xargs grep -nE "offers +[a-z][a-z0-9]*\." | grep -v "^docs/archive/\|^docs/wip/"
# expected: only the RSDL-310 hint test of task 3
cargo insta test --workspace --accept   # then read every snapshot diff
just verify
just ts-browser-e2e && just cabin-browser-demo   # CI-only members, run where a browser exists
```

**Expected.** `just verify` green (every `build` member: `demo`, `compat-check`,
`cabin-ws-demo`, `ts-e2e`, `book-check`, `freeze-check` with the four notices
included).

**Commit.**

```text
chore(repo): migrate fixtures, examples and documents to service names

Rename every service declared in the dotted form, as RIDL-142 suggests, and
rewrite the offers lines, lone services, inline-shape requires, doc links and
lock keys that named it, in the compiler corpora, the CLI tests, the
conformance identity fixture and its descriptor, the examples, the book, the
skill and the eval start trees.

Refs: #1390
```

### Follow-up pull request (after the merge)

Re-add the four `[[surface]]` rows (`diagnostics`, `wire`, `generated-crate`,
`accepted-sources`) to `freeze/surfaces.toml` with the merge commit of this pull
request as `baseline` (ADR-0029 decision 4). Commit:
`chore(xtask): restore the frozen-surface rows after the service name break`.

## 5. Pull request

- Title (the squash subject):
  `feat(ridl-sem)!: name a service with one
  CamelCase identifier`.
- Body: the summary, the ADR-0034 acceptance request ("ADR-0034 is _Proposed_;
  this pull request asks the owner to accept it, and its Status becomes Accepted
  before the merge"), the ADR-0029 approval statement for the four surfaces —
  the `cargo xtask freeze check` output of task 5 step 3 as the evidence, and
  the step that applies (decision 4's two-step procedure, extended to a later
  pre-1.0 minor by its "0.11 starts with" bullet) — the D8 table, and the
  `BREAKING CHANGE:` footer of task 3.
- Release: the next minor, 0.20.0 (`Cargo.toml` is at 0.19.0); never a patch.

## 6. Migration rules for the mechanical subagent

1. **Declaration.** Replace the dotted name with the RIDL-142 suggestion
   (`ridl check --format json` on the unit gives it as the fix-it; or apply the
   rule by hand). The suggestion already avoids every declaration of the package
   and every name suggested to an earlier old-form service of the package
   (`Service`, `Service2`, … appended); apply the suggestions of one package in
   one pass, in the order the checker reports them, and check that `TYPL-009` is
   absent afterwards.
2. **Record the mapping** old dotted name → global name per package, and apply
   it to every reference: rsdl `offers`, `requires` of an inline shape, member
   and placement lines (write the bare name when the rsdl file is in the same
   package or imports it, else the global name; do not add imports that would
   collide with a component of the same name), doc links, Rust string literals,
   Markdown.
3. **Lock files.** For each `service:<old>` line in an `interfaces.lock`, and
   each `<old>/<instance>` line in an `instances.lock`, run the
   `ridl lock --rename` command of ADR-0034 decision 7 rather than editing by
   hand: a `service:` rename with the directory of the one unit that holds the
   lock (it fails with "name the unit directory" at the root of a workspace of
   several units), an instance or component rename with the workspace root (it
   fails with "belongs to the workspace root" elsewhere). For a lock written
   inline in a Rust test string, edit the string.
4. **Prose.** Replace "dotted name" of a service with "global name" or "name";
   leave `docs/archive/`, `docs/wip/` and `CHANGELOG.md` untouched.
5. **Keep deliberately old-form inputs**:
   `crates/ridl-syntax/test_data/parser/ok/services.ridl`, the RIDL-142 showcase
   line, the RIDL-142 tests of tasks 3 and 6 and the RSDL-310 hint test.
6. After each area, run that area's `cargo test -p` and stop on a failure that
   is not a pending area.

## 7. Book text — the migration section (task 5 writes it)

Heading `## Service declaration names`, in `docs/book/stability.md` after
"FlatBuffers enum width adoption":

> Release 0.20.0 changes how a service is named (ADR-0034). A service is
> declared with one `CamelCase` name, and its global name is its package
> followed by that name: `service Doors : DoorControl` in package `veh.body` is
> `veh.body.Doors`, and its members are `veh.body.Doors.<member>`. A dotted name
> is RIDL-142, whose fix-it gives the new name. RIDL-140 is retired.
>
> To migrate a workspace:
>
> 1. Run `ridl check`. Apply each RIDL-142 fix-it (the language server offers it
>    as a quick fix).
> 2. Rewrite each rsdl `offers` line, each lone service in a `system`,
>    `distribution` or `machine` body, each `requires` line that names an
>    inline-shape service, and each doc link to a service, with the new name:
>    bare in the same package, after an `import`, or the global name.
> 3. For each inline-shape service, run
>    `ridl lock <unit directory> --rename service:<old name>=service:<package>.<Name>`,
>    once per unit, with the directory of the unit that declares it: an
>    interface edit changes one unit's `interfaces.lock`. For each lone service
>    listed in a system, run
>    `ridl lock <workspace root> --rename <old name>=<package>.<Name>` at the
>    workspace root, which holds `instances.lock`. Both keep their numbers.
> 4. Run `ridl check` and `ridl build`. Regenerate and redeploy every peer
>    together. The descriptor strings and lock keys of every service change.
>    Every renamed inline shape changes the catalog hash of its unit and the
>    transport identities of its fallible returns, because both hold the
>    service's name as written. The generated type and enum names of an inline
>    shape change only when its old name did not begin with its package
>    (`veh.hvac.cabin` in package `veh.cluster` becomes `VehClusterHvacCabin…`;
>    `veh.cluster.wheels` keeps `VehClusterWheels…`).
> 5. `ridl diff` keys a service by its package and global name, so against a
>    baseline published before the migration it reports every renamed service as
>    removed and added, which is breaking. Expect that verdict once, then
>    publish a new baseline with `ridl baseline`.
>
> The `diagnostics`, `accepted-sources`, `wire` and `generated-crate` freeze
> rows are removed until the follow-up restores them.

The CLI reference (`docs/book/cli-reference.md`, section of `ridl lock`) gains
the two `--rename` forms as examples in task 7.
