# Instance Numbers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** ridl gives every instance of the workspace's system a locked number in
a root `instances.lock`, carries it in the system IR, the codegen deployment
section and the deployment descriptor, generates it as a Rust constant, and
`ridl diff` reports a changed locked number as breaking.

**Architecture:** The interface lock's line table becomes generic over its key,
so `instances.lock` reuses its parser, writer, edits and merge. The loader reads
the root `instances.lock` into `LoadedWorkspace`; a numbering step in `ridl-sem`
(the analogue of `number_unit`) runs once over the checked system and returns a
map from key to `(number, provisional)`. `lower_system` takes that map; every
artifact reads the number from the lowered `Component.numbers`.

**Tech Stack:** Rust workspace (`ridl-core`, `ridl-sem`, `ridl-ir` with prost,
`ridl-descriptor` with planus, `ridl-backend-rust`, `ridlc`, `ridl`,
`ridl-diff`, `ridl-lsp`), insta snapshots, `cargo xtask descriptor-codegen`,
`cargo xtask freeze update`.

**Spec:**
[`2026-11-02-instance-numbers-design.md`](2026-11-02-instance-numbers-design.md),
including its section "Settled after the builder check". Context:
[`2026-11-02-instance-identity-decisions.md`](2026-11-02-instance-identity-decisions.md),
`docs/design/deployment-descriptor.md`.

## Global Constraints

- File name `instances.lock`, one per workspace, beside the root `ridl.toml`; a
  bare source file reads it from the file's own directory.
- Header line, byte for byte:
  `# instances.lock — written by ridl lock; do not edit by hand.`
- Key: `<component qualified name>/<instance name>`, exactly one `/`; the
  instance is a camelCase name or `Unit`.
- Range: 1 to 2^31 − 1 (2147483647). 0 is never valid. Negative numbers are
  reserved; nothing in this plan writes or accepts one.
- Provisional numbers count up from `next` (from 1 with no lock) in byte order
  of the key's text (`component/instance`), never in declaration, file or
  placement order.
- Lock rules reused unchanged: `next` never lowered, retired entries never
  removed or reissued, only plain `ridl lock` allocates, a missing lock is not
  an error.
- `ridl lock` never writes into a remote unit or the cache (ADR-0002).
- Appended fields only: `system.proto` `Component.numbers = 15`;
  `deployment.proto` `Instance` `int32 number = 8; bool provisional = 9;`;
  `deployment.fbs` `Instance` `number: int32; provisional: bool;` at the end of
  the table. `SCHEMA_VERSION` stays 1.
- No instance number in a catalog, a catalog hash, a data frame, or the
  generated interface code of a unit.
- Rust constant type `i32`, name SCREAMING_SNAKE_CASE of the instance name
  (`UNIT` for the unit instance), module path the component's qualified name
  with each segment through `snake_case` then `module_segment`, under a crate
  root module `instances`.
- New diagnostic codes: RSDL-314 (live `instances.lock` entry with no instance)
  and RSDL-315 (`instances.lock` is malformed), both errors, in rsdl §16.1 under
  §7.
- Repository rules: Conventional Commits with scopes from `.git-std.toml`; plain
  literal English; no story id (`E19.x`) in code, rustdoc, `docs/design/`,
  `docs/book/` or `docs/technotes/` (`just story-id-check`); every cited `docs/`
  path exists (`just doc-path-check`); every commit ends with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; implementers run
  `cargo test -p <crate>` for the crates they touch and CI runs the full gate; a
  red test fails on an assertion, not on a compile error, where the code allows
  it (add the new signature as a stub returning a wrong value first).

## Review Focus

1. **Provisional order of services with a common prefix.** Lone services
   `veh.cruise` and `veh.cruise.x`: the derived `Ord` of a two-field key orders
   `veh.cruise/Unit` first, the key's bytes order `veh.cruise.x/Unit` first (`.`
   is below `/`). The spec says bytes. Pinned in Task 2
   (`the_provisional_order_is_the_key_text`).
2. **A closure with errors.** A workspace with a typo in a `system` line must
   not draw RSDL-314 on every lock entry. Pinned in Task 2
   (`a_closure_with_errors_reports_no_orphan`).
3. **A component rename with retired entries.** `--rename Old=New` must move the
   retired `Old/*` lines too, or a later instance `New/x` can be given the
   retired number's name without its history. Pinned in Task 5
   (`rename_a_component_moves_every_entry_with_its_number`).
4. **The merge driver on `instances.lock`.** A user who registered the driver
   for both files runs the same `ridl lock merge %O %A %B %L`; the driver must
   read keys with `/` and write the instance header. Pinned in Task 5
   (`the_merge_driver_merges_two_instance_allocations`).
5. **Two components with one constant path.** A component `veh.adas.LaneKeep`
   and a lone service `veh.adas.lane_keep` both map to
   `instances::veh::adas::lane_keep::UNIT`. The build must refuse with an error
   that names both keys, not write a crate that does not compile. Pinned in Task
   6 (`two_keys_with_one_constant_path_are_refused`).

---

### Task 1: The instance lock file

The line table of `interfaces.lock` becomes generic over its key; the new module
holds the instance key and the `instances.lock` constants. Pure text in, table
out, no `fs` outside `read`/`write`, so the `wasm32` build keeps compiling.

**Files:**

- Modify: `crates/ridl-core/src/interface_lock.rs` (the table, entry, edit
  error, merge outcome, `parse`, `merge`, `render`, `allocate`)
- Create: `crates/ridl-core/src/instance_lock.rs`
- Modify: `crates/ridl-core/src/lib.rs` (`pub mod instance_lock;`)
- Modify only if the compiler requires it: `crates/ridl/src/lock.rs`,
  `crates/ridl-sem/src/check.rs` (existing callers keep the `InterfaceLock`
  name)
- Test: `crates/ridl-core/src/instance_lock.rs` (`mod tests`); the existing
  tests of `interface_lock.rs` stay unchanged and green

**Interfaces:**

- Consumes: nothing new.
- Produces, in `ridl_core::interface_lock`:
  - `pub trait TableKey: Clone + Eq + Ord + Hash + fmt::Display + FromStr<Err = InvalidLockKey> { const FILE_NAME: &'static str; const HEADER: &'static str; const MAX_NUMBER: u32; }`
    with `impl TableKey for LockKey` (`FILE_NAME`, `HEADER` as today,
    `MAX_NUMBER = u32::MAX - 1`).
  - `pub struct LockTable<K = LockKey> { pub next: u32, pub entries: Vec<LockEntry<K>> }`,
    `pub type InterfaceLock = LockTable<LockKey>;`,
    `pub struct LockEntry<K = LockKey>` (fields unchanged, `key: K`),
    `impl<K> Default for LockTable<K>` (`next 1`).
  - `pub enum LockEditError<K = LockKey> { NoLiveEntry(K), KeyAlreadyLive(K), NoEntryUnder(String), Exhausted }`
    — `NoEntryUnder` names a component with no entry, `Exhausted` is an
    allocation above `K::MAX_NUMBER`.
  - `pub enum MergeOutcome<K = LockKey> { Clean(LockTable<K>), Conflict { text: String } }`.
  - `pub fn parse(text: &str) -> Result<InterfaceLock, LockError>` (unchanged)
    and
    `pub fn parse_table<K: TableKey>(text: &str) -> Result<LockTable<K>, LockError>`;
    a number above `K::MAX_NUMBER`, or `next` above `K::MAX_NUMBER + 1`, is
    malformed.
  - `pub fn merge<K: TableKey>(base: &LockTable<K>, ours: &LockTable<K>, theirs: &LockTable<K>, marker_size: usize) -> MergeOutcome<K>`;
    `render` and the conflict text write `K::HEADER`.
- Produces, in `ridl_core::instance_lock`:
  - `pub const FILE_NAME: &str = "instances.lock";`
  - `pub const HEADER: &str = "# instances.lock — written by ridl lock; do not edit by hand.";`
  - `pub const MAX_NUMBER: u32 = 2_147_483_647;`
  - `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct InstanceKey { pub component: String, pub instance: String }`
    — `Display` is `component/instance`; `FromStr` accepts exactly one `/`, a
    left side of one or more identifiers joined by `.`, and a right side that is
    `Unit` or an identifier whose first byte is an ASCII lowercase letter.
  - `impl TableKey for InstanceKey`,
    `pub type InstanceLock = LockTable<InstanceKey>;`
  - `pub fn parse(text: &str) -> Result<InstanceLock, LockError>`
  - `impl LockTable<InstanceKey>`:
    `pub fn rename_component(&mut self, old: &str, new: &str) -> Result<Vec<(InstanceKey, InstanceKey, u32)>, LockEditError<InstanceKey>>`
    (every live and retired `old/*` entry becomes `new/*`, number kept;
    `NoEntryUnder(old)` when there is none; `KeyAlreadyLive` when a live `new/x`
    exists for a live `old/x`; nothing changes on error),
    `pub fn retire_component(&mut self, component: &str) -> Result<Vec<(InstanceKey, u32)>, LockEditError<InstanceKey>>`
    (every live `component/*` entry; `NoEntryUnder` when none is live),
    `pub fn has_component(&self, component: &str, include_retired: bool) -> bool`.
  - `#[cfg(feature = "fs")] pub fn read(dir: &Path) -> std::io::Result<Option<String>>`
    and `pub fn write(dir: &Path, lock: &InstanceLock) -> std::io::Result<()>`,
    as `interface_lock::read`/`write` for `FILE_NAME`.

- [ ] **Step 1: Write the failing tests** in `instance_lock.rs`:

```rust
const EXAMPLE: &str = "\
# instances.lock — written by ridl lock; do not edit by hand.
next 5
veh.adas.Cruise/primary 1
veh.adas.Cruise/backup 2
veh.ego.status/Unit 3
veh.adas.Lane/left 4 retired
";

#[test]
fn the_example_parses_and_renders_byte_for_byte() {
    let lock = parse(EXAMPLE).unwrap();
    assert_eq!(lock.next, 5);
    assert_eq!(lock.entries.len(), 4);
    assert_eq!(lock.render(), EXAMPLE);
}

#[test]
fn a_key_is_a_dotted_component_a_slash_and_an_instance() {
    for good in ["veh.adas.Cruise/primary", "veh.ego.status/Unit", "a.B/x1", "a.B/unit"] {
        assert!(good.parse::<InstanceKey>().is_ok(), "{good}");
    }
    for bad in ["veh.adas.Cruise", "veh.adas.Cruise/", "/primary", "a.B/x/y",
                "veh..C/x", "veh.C/Primary", "veh.C/_x", "veh.C/pri-mary"] {
        assert!(bad.parse::<InstanceKey>().is_err(), "{bad}");
    }
}

#[test]
fn a_number_outside_the_range_is_malformed() {
    let header = format!("{HEADER}\n");
    assert!(parse(&format!("{header}next 2147483648\na.B/x 2147483647\n")).is_ok());
    assert!(parse(&format!("{header}next 2147483649\n")).is_err());
    assert!(parse(&format!("{header}next 2147483649\na.B/x 2147483648\n")).is_err());
    assert!(parse(&format!("{header}next 2\na.B/x 0\n")).is_err());
}

#[test]
fn allocate_refuses_a_number_above_the_range() {
    let mut lock = parse(&format!("{HEADER}\nnext 2147483648\na.B/x 2147483647\n")).unwrap();
    assert_eq!(lock.allocate("a.B/y".parse().unwrap()), Err(LockEditError::Exhausted));
}

#[test]
fn a_component_rename_moves_every_live_and_retired_entry() {
    let mut lock = parse(EXAMPLE).unwrap();
    let moved = lock.rename_component("veh.adas.Lane", "veh.adas.Keep").unwrap();
    assert_eq!(moved.len(), 1);
    let mut lock = parse(EXAMPLE).unwrap();
    lock.rename_component("veh.adas.Cruise", "veh.adas.Speed").unwrap();
    assert!(lock.render().contains("veh.adas.Speed/primary 1\nveh.adas.Speed/backup 2\n"));
    assert_eq!(lock.next, 5);
}

#[test]
fn a_component_retire_retires_every_live_entry() {
    let mut lock = parse(EXAMPLE).unwrap();
    assert_eq!(lock.retire_component("veh.adas.Cruise").unwrap().len(), 2);
    assert!(lock.render().contains("veh.adas.Cruise/primary 1 retired\n"));
    assert_eq!(
        lock.retire_component("veh.adas.Lane"),
        Err(LockEditError::NoEntryUnder("veh.adas.Lane".into()))
    );
}

#[test]
fn two_instance_allocations_merge_with_the_instance_header() {
    let base = parse(&format!("{HEADER}\nnext 2\nveh.C/a 1\n")).unwrap();
    let ours = parse(&format!("{HEADER}\nnext 3\nveh.C/a 1\nveh.C/b 2\n")).unwrap();
    let theirs = parse(&format!("{HEADER}\nnext 3\nveh.C/a 1\nveh.C/c 2\n")).unwrap();
    let MergeOutcome::Clean(merged) = merge(&base, &ours, &theirs, 7) else { panic!() };
    assert_eq!(merged.render(), format!("{HEADER}\nnext 4\nveh.C/a 1\nveh.C/b 2\nveh.C/c 3\n"));
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p ridl-core instance_lock` Expected: FAIL (with stubs that
return `Err`/empty values, on the assertions).

- [ ] **Step 3: Implement** the generic table in `interface_lock.rs` and the new
      module. Keep every existing public name of `interface_lock` usable as
      before through the default type parameters and the `InterfaceLock` alias;
      move no behaviour of the interface lock.

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p ridl-core` and
`cargo check -p ridl-core --no-default-features --target wasm32-unknown-unknown`
Expected: PASS, including every existing `interface_lock` test.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core/src/interface_lock.rs crates/ridl-core/src/instance_lock.rs crates/ridl-core/src/lib.rs
git commit -m "feat(ridl-core): the instance lock file"
```

---

### Task 2: Reading the lock and numbering the instances

The loader reads the root `instances.lock`; `number_instances` gives every
closure instance its number; the drivers report RSDL-314 and RSDL-315.

**Files:**

- Modify: `crates/ridl-core/src/package.rs` (`InstanceLockFile`)
- Modify: `crates/ridl-core/src/workspace.rs` (`LoadedWorkspace`, `load_inner`,
  a `read_instance_lock` beside `read_lock`, the RSDL-315 message)
- Modify: `crates/ridl-core/src/diag.rs` (`RSDL_314`, `RSDL_315`)
- Create: `crates/ridl-sem/src/rsdl/numbering.rs`
- Modify: `crates/ridl-sem/src/rsdl/mod.rs` (module, re-exports),
  `crates/ridl-sem/src/rsdl/closure.rs` (`ClosureComponent::qualified_name`),
  `crates/ridl-sem/src/lib.rs` (re-export)
- Modify: `crates/ridlc/src/lib.rs` (`Compiled`, `WorkspaceOutput`,
  `check_loaded`, `number_workspace_instances`)
- Modify: `crates/ridl-lsp/src/server.rs` (keep the loaded lock, draw the same
  diagnostics after `check_workspace`)
- Modify: every destructuring of `LoadedWorkspace` the compiler flags
  (`crates/ridlc/tests/corpus.rs` and others)
- Modify: `crates/ridlc/tests/corpus.rs` (`RSDL_PROFILE_CODES`: both codes as
  `Elsewhere` with fixture `crates/ridl/tests/instance_lock_cli.rs`)
- Modify: `docs/specification/rsdl-language-reference.md` §7 and §16.1
- Regenerate: `freeze/diagnostics.txt` (`cargo xtask freeze update`; two added
  lines)
- Test: `crates/ridl-sem/src/rsdl/numbering.rs` (`mod tests`),
  `crates/ridl-core/src/workspace.rs` (`mod tests`), Create
  `crates/ridl/tests/instance_lock_cli.rs`

**Interfaces:**

- Consumes: Task 1's `InstanceKey`, `InstanceLock`,
  `instance_lock::{read, parse, FILE_NAME}`.
- Produces:
  - `ridl_core::package::InstanceLockFile { pub path: String, pub text: String, pub lock: InstanceLock }`
    (same shape and role as `PackageLock`).
  - `LoadedWorkspace.instance_lock: Option<InstanceLockFile>` and
    `LoadedWorkspace.instance_lock_dir: PathBuf` — the root manifest's
    directory, or a bare file's directory; `None` lock when the file is absent
    or malformed. A malformed file is RSDL-315 from the loader on the offending
    line (empty range at 0 when there is none), exactly as `read_lock` does
    RIDL-410.
  - `DiagCode::RSDL_314` summary
    ``live `instances.lock` entry with no instance``, `DiagCode::RSDL_315`
    summary `` `instances.lock` is malformed ``, both `Error`.
  - In `ridl_sem::rsdl` (re-exported from `ridl_sem`):
    `pub struct InstanceNumbering { pub numbers: BTreeMap<InstanceKey, (i32, bool)>, pub orphans: Vec<LockEntry<InstanceKey>>, pub overflow: Vec<InstanceKey>, pub any_provisional: bool }`
    with `Default`;
    `pub fn number_instances(system: &CheckedSystem, lock: Option<&InstanceLock>) -> InstanceNumbering`;
    `pub fn instance_lock_diagnostics(numbering: &InstanceNumbering, lock: &InstanceLock, lock_file: FileId, lock_dir: &str) -> Vec<Diagnostic>`.
  - `ClosureComponent::qualified_name(&self) -> String` — `pkg.Name`, or the
    service's dotted name; `Lowering::component_name` calls it.
  - `pub fn ridlc::number_workspace_instances(system: &CheckedSystem, lock: Option<&InstanceLockFile>, sources: &mut SourceMap) -> (InstanceNumbering, Vec<Diagnostic>)`
    — interns the lock file into `sources` and returns the RSDL-314/315
    diagnostics; `check_loaded` and the language server both call it.
  - `Compiled.instances: InstanceNumbering`,
    `WorkspaceOutput.instances: InstanceNumbering`,
    `WorkspaceOutput.instance_lock_dir: PathBuf`.

Rules `number_instances` implements:

- The keys are every instance of every component of `system.closure` (external
  components included). No closure: empty numbering.
- A key with a live entry is `(entry.number, false)`. Every other key, sorted by
  the bytes of `key.to_string()`, takes `next`, `next + 1`, … as `(n, true)`; a
  key whose number would pass 2147483647 goes in `overflow` and not in
  `numbers`.
- `orphans` (live entries with no key) is filled only when
  `!system.closure_has_errors`.

Rules `instance_lock_diagnostics` implements (RSDL-314 messages mirror
`orphan_entry_message` in `crates/ridl-sem/src/check.rs`):

- Orphans grouped by component. A component with orphans and no key in `numbers`
  draws one RSDL-314 on its first entry's line:
  `` `{C}` has live entries in `instances.lock` and is not in the system: run `ridl lock {dir} --rename {C}=New` when the component was renamed, or `ridl lock {dir} --retire {C}` when it is gone ``.
- Every other orphan draws one RSDL-314 on its line:
  `` `{key}` is a live entry of `instances.lock` with no instance in the system: run `ridl lock {dir} --rename {key}={C}/new` when an instance without an entry is this instance under a new name, or `ridl lock {dir} --retire {key}` when the instance is gone ``.
- When `!any_provisional`, both messages name `--retire` alone.
- A non-empty `overflow` draws one RSDL-315 at the empty range at 0:
  `` `instances.lock` has no number left: {n} instances would take a number above 2147483647 ``.

- [ ] **Step 1: Write the failing numbering tests** in `numbering.rs`. Check the
      system from source the way `lower_topology` in the tests of
      `crates/ridl-sem/src/rsdl/lower.rs` does (move that helper into a shared
      `#[cfg(test)]` module of `rsdl` if both files need it), over the
      `SIZED_CLOSURE` text of those tests (components `Lane`, `Panel`, external
      `Backend`, `Pair [ instances = (primary, backup) ]`, and the lone service
      `veh.diag.access`):

```rust
#[test]
fn with_no_lock_every_instance_is_provisional_in_key_byte_order() {
    let numbering = number_instances(&sized_closure(), None);
    let listed: Vec<(String, i32, bool)> = numbering.numbers.iter()
        .map(|(key, (n, p))| (key.to_string(), *n, *p)).collect();
    let mut by_number = listed.clone();
    by_number.sort_by_key(|(_, n, _)| *n);
    assert_eq!(by_number.iter().map(|(k, n, _)| (k.as_str(), *n)).collect::<Vec<_>>(), [
        ("veh.diag.access/Unit", 1), ("veh.topology.Backend/Unit", 2),
        ("veh.topology.Lane/Unit", 3), ("veh.topology.Pair/backup", 4),
        ("veh.topology.Pair/primary", 5), ("veh.topology.Panel/Unit", 6),
    ]);
    assert!(listed.iter().all(|(_, _, provisional)| *provisional));
}

#[test]
fn a_live_entry_freezes_its_number_and_the_rest_count_from_next() {
    let lock = lock("next 10\nveh.topology.Pair/primary 4\n");
    let numbering = number_instances(&sized_closure(), Some(&lock));
    assert_eq!(numbering.numbers[&key("veh.topology.Pair/primary")], (4, false));
    assert_eq!(numbering.numbers[&key("veh.diag.access/Unit")], (10, true));
    assert_eq!(numbering.numbers[&key("veh.topology.Panel/Unit")], (14, true));
}

#[test]
fn a_retired_entry_does_not_freeze_its_key() {
    let lock = lock("next 8\nveh.topology.Pair/backup 7 retired\n");
    let numbering = number_instances(&sized_closure(), Some(&lock));
    assert_eq!(numbering.numbers[&key("veh.topology.Pair/backup")].1, true);
    assert!(numbering.numbers.values().all(|(n, _)| *n != 7));
}

#[test]
fn the_provisional_order_is_the_key_text() {
    // Two lone services, `veh.cruise` and `veh.cruise.x`, both in the system.
    let numbering = number_instances(&two_lone_services(), None);
    assert_eq!(numbering.numbers[&key("veh.cruise.x/Unit")].0, 1);
    assert_eq!(numbering.numbers[&key("veh.cruise/Unit")].0, 2);
}

#[test]
fn an_orphan_instance_and_an_orphan_component_draw_rsdl_314() {
    let lock = lock("next 10\nveh.topology.Pair/spare 7\nveh.topology.Gone/a 8\nveh.topology.Gone/b 9\n");
    let numbering = number_instances(&sized_closure(), Some(&lock));
    let diagnostics = instance_lock_diagnostics(&numbering, &lock, FileId(0), ".");
    assert_eq!(diagnostics.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(), ["RSDL-314", "RSDL-314"]);
    assert!(diagnostics.iter().any(|d| d.message.contains("--rename veh.topology.Gone=New")));
    assert!(diagnostics.iter().any(|d| d.message.contains("--retire veh.topology.Pair/spare")));
}

#[test]
fn a_closure_with_errors_reports_no_orphan() {
    // SIZED_CLOSURE with `system Vehicle { Lane, Missing }` (RSDL-602).
    let numbering = number_instances(&closure_with_an_error(), Some(&lock("next 2\nveh.topology.Panel/Unit 1\n")));
    assert!(numbering.orphans.is_empty());
}

#[test]
fn numbers_above_the_range_are_overflow_and_rsdl_315() {
    let lock = lock("next 2147483647\n");
    let numbering = number_instances(&sized_closure(), Some(&lock));
    assert_eq!(numbering.overflow.len(), 5);
    let diagnostics = instance_lock_diagnostics(&numbering, &lock, FileId(0), ".");
    assert_eq!(diagnostics.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(), ["RSDL-315"]);
}
```

(The helper `lock` prepends the header and calls `instance_lock::parse`; `key`
parses an `InstanceKey`.)

- [ ] **Step 2: Write the failing loader and CLI tests.** In
      `crates/ridl-core/src/workspace.rs` tests, beside
      `a_lock_beside_the_sources_rides_on_the_package`:
      `an_instance_lock_beside_the_root_manifest_is_loaded` (asserts
      `loaded.instance_lock.unwrap().lock.next == 3` and `instance_lock_dir` is
      the root) and `a_malformed_instance_lock_is_rsdl_315` (text `next x\n`;
      one diagnostic, code `RSDL-315`, lock `None`). In the new
      `crates/ridl/tests/instance_lock_cli.rs` (copy the `TempDir`/`ridl`
      helpers of `lock_cli.rs`), over a one-package workspace with a `system` of
      one component `Cruise [ instances = (primary, backup) ]`: -
      `a_missing_instance_lock_is_not_an_error`: `ridl check` exits 0. -
      `check_reports_an_orphan_instance_entry_as_rsdl_314`: lock holds
      `veh.Cruise/spare 3`; exit 1; stderr contains `error[RSDL-314]`,
      `instances.lock` and `--retire veh.Cruise/spare`. -
      `check_reports_a_malformed_instance_lock_as_rsdl_315`: lock `next x`; exit
      1; stderr contains `error[RSDL-315]`.

- [ ] **Step 3: Run them and see them fail**

Run: `cargo test -p ridl-sem numbering` and
`cargo test -p ridl-core instance_lock` and
`cargo test -p ridl --test instance_lock_cli` Expected: FAIL on the assertions.

- [ ] **Step 4: Implement** the loader read, the two codes, `number_instances`,
      `instance_lock_diagnostics`, `number_workspace_instances` and its calls in
      `check_loaded` (after `check_workspace`, before `drop_remote_non_errors`)
      and in the language server. Add the §16.1 rows (Section `§7`); each rule
      cell starts with the catalogue summary followed by `—` or `(`, as
      `crates/ridlc/tests/corpus.rs` `compare_rule` requires: - RSDL-314:
      `` live `instances.lock` entry with no instance — an entry whose component or instance is not in the system; the message names `--retire` and `--rename` ``
      - RSDL-315:
      `` `instances.lock` is malformed (a line that does not parse, a number outside 1 to 2147483647), or no number is left for a provisional instance ``

      Rewrite §7's opening ("Instances are named, never numbered") to "named in
      source; the toolchain numbers them", with: the file and its place, the
      key, the range (0 invalid, negative numbers reserved for a runtime's own
      use), the provisional rule, and the lock rules of the spec's section 1. In
      the V-02 bullet add that the number is sent at attach only and never in a
      data frame. Run `cargo xtask freeze update` and check that
      `freeze/diagnostics.txt` only gains the two lines.

- [ ] **Step 5: Run the tests**

Run:
`cargo test -p ridl-core && cargo test -p ridl-sem && cargo test -p ridlc --test corpus && cargo test -p ridl --test instance_lock_cli && cargo test -p ridl-lsp`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-core crates/ridl-sem crates/ridlc crates/ridl-lsp crates/ridl/tests/instance_lock_cli.rs docs/specification/rsdl-language-reference.md freeze/diagnostics.txt
git commit -m "feat(rsdl): number every instance of the system from instances.lock"
```

---

### Task 3: The number in the system IR and the codegen deployment section

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/system.proto` (`InstanceNumber`,
  `Component.numbers = 15`)
- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto` (`Instance`
  `number = 8`, `provisional = 9`)
- Modify: `crates/ridl-sem/src/rsdl/lower.rs` (`lower_system` parameter,
  `components`, the gate on `overflow`)
- Modify: `crates/ridl-ir/src/codegen/deployment.rs` (`lower_deployment`), and
  its test `an_instance_carries_its_placement_line_keys` plus
  `crates/ridl-ir/src/codegen/tests.rs` (replace the `shm`/`service` key with
  `linux`/`priority`)
- Modify: `crates/ridlc/src/lib.rs` (`lower_workspace_system`, the build path
  near the `lower_system` call), `crates/ridlc/tests/corpus.rs`,
  `crates/ridl-sem/tests/docs_in_source.rs`
- Create: `crates/ridlc/tests/corpus/instance-numbers/` — a workspace of two
  members: `adas` (package `veh.adas`, a service `veh.adas.cruise`, and
  `component Cruise [ instances = (primary, backup) ] { offers veh.adas.cruise }`,
  the `system Car` and a `deployment Bench` placing every instance on one
  machine) and `ego` (package `veh.ego`, a lone service `veh.ego.status` that
  `Car` lists); a root `instances.lock`: `next 4`, `veh.adas.Cruise/primary 1`,
  `veh.adas.Cruise/backup 2`, `veh.ego.status/Unit 3`. The entry compiles with
  no diagnostic; add it to the corpus runner's list if the runner enumerates
  entries, and accept its snapshot.
- Modify: `docs/specification/rsdl-language-reference.md` §13 (the instance
  numbers from `instances.lock` are an input from outside rsdl, cited once,
  beside the interface numbers), `docs/technotes/rsdl-implementation.md` (the
  `lower_system` signature)
- Regenerate: `freeze/codegen-protocol.txt` (`cargo xtask freeze update`; added
  lines only)
- Test: `crates/ridl-sem/src/rsdl/lower.rs` (`mod tests`),
  `crates/ridl-ir/src/codegen/deployment.rs` (`mod tests`),
  `crates/ridlc/tests/corpus.rs` or a new
  `crates/ridlc/tests/instance_numbers.rs`

**Interfaces:**

- Consumes: Task 2's `InstanceNumbering`, `number_instances`,
  `WorkspaceOutput.instances`, `Compiled.instances`.
- Produces:
  - `message InstanceNumber { string instance = 1; int32 number = 2; bool provisional = 3; }`
    and `repeated InstanceNumber numbers = 15;` on `Component`, one entry per
    name of `instances`, in the same order.
  - `pub fn lower_system(system: &CheckedSystem, packages: &[&v2::Package], numbering: &InstanceNumbering) -> Option<v2::System>`
    — `None` also when `numbering.overflow` is not empty; a closure instance
    missing from `numbering.numbers` is a bug (`expect` with a message).
  - `pub fn ridlc::lower_workspace_system(system: &CheckedSystem, packages: &[&v2::Package], std_ir: &v2::Package, numbering: &InstanceNumbering) -> Option<v2::System>`.
  - `v1::Instance.number: i32`, `v1::Instance.provisional: bool`, read by
    `lower_deployment` from the placed component's `numbers` entry for the
    placement's instance; `(0, true)` when there is none (the descriptor refuses
    0 in Task 4).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-sem/src/rsdl/lower.rs tests
#[test]
fn every_component_carries_its_instance_numbers_in_instance_order() {
    let system = sized_system_numbered(); // SIZED_CLOSURE + a one-machine deployment,
                                          // lowered with number_instances(.., None)
    let pair = system.components.iter().find(|c| c.name == "Pair").unwrap();
    assert_eq!(pair.instances, ["primary", "backup"]);
    let numbers: Vec<(&str, i32, bool)> = pair.numbers.iter()
        .map(|n| (n.instance.as_str(), n.number, n.provisional)).collect();
    assert_eq!(numbers, [("primary", 5, true), ("backup", 4, true)]);
}

#[test]
fn an_overflow_blocks_the_lowering() {
    // the same closure, numbered over a lock `next 2147483647`
    assert!(lower_sized_with_lock("next 2147483647\n").is_none());
}

// crates/ridl-ir/src/codegen/deployment.rs tests
#[test]
fn an_instance_carries_its_number_and_provisional_flag() {
    let mut system = system();
    system.components[0].numbers = vec![v2::InstanceNumber {
        instance: system.deployments[0].placements[0].instance.clone(),
        number: 7, provisional: false,
    }];
    let section = lower_deployment(&system, "prod", &[&package()]).unwrap();
    assert_eq!((section.instances[0].number, section.instances[0].provisional), (7, false));
}
```

Plus, in `crates/ridlc/tests/instance_numbers.rs`,
`the_corpus_system_carries_the_locked_numbers`: `compile_workspace` over
`tests/corpus/instance-numbers`, then for `veh.adas.Cruise` the numbers are
`[("primary", 1, false), ("backup", 2, false)]` and for `veh.ego.status`
`[("Unit", 3, false)]`.

- [ ] **Step 2: Run them and see them fail**

Run:
`cargo test -p ridl-sem lower && cargo test -p ridl-ir deployment && cargo test -p ridlc --test instance_numbers`
Expected: FAIL on the assertions (add the proto fields first so the tests
compile; they then read empty `numbers` and zeros).

- [ ] **Step 3: Implement** the lowering, the section field, and pass
      `&compiled.instances` everywhere `lower_system` and
      `lower_workspace_system` are called. Accept the insta snapshot changes
      that only add `numbers`. Edit §13 and the technote. Run
      `cargo xtask freeze update`; `freeze/codegen-protocol.txt` gains the
      `InstanceNumber` lines, `Component.numbers` and the two `Instance` lines,
      and loses none.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-ir && cargo test -p ridl-sem && cargo test -p ridlc`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir crates/ridl-sem crates/ridlc docs/specification/rsdl-language-reference.md docs/technotes/rsdl-implementation.md freeze/codegen-protocol.txt
git commit -m "feat(ridl-ir): carry the instance number in the system and the deployment section"
```

---

### Task 4: The number in the deployment descriptor

**Files:**

- Modify: `crates/ridl-descriptor/schema/deployment.fbs` (`Instance`: append
  `number: int32;` and `provisional: bool;` after `maps`)
- Regenerate: `crates/ridl-descriptor/src/deployment/generated.rs`
  (`cargo xtask descriptor-codegen`)
- Modify: `crates/ridl-descriptor/src/deployment/lower.rs` (copy both fields
  from the section; an instance number 0 or below is
  `DeploymentLowerError::Internal`)
- Modify: `crates/ridl-descriptor/src/deployment.rs` (`VerifyError`, the walk of
  `instances`)
- Modify: `crates/ridl-descriptor/src/deployment/describe.rs` (JSON keys
  `number` and `provisional`)
- Modify: `crates/ridl-descriptor/tests/deployment_lower.rs` (the `shm.service`
  placement key becomes a `linux.priority` key; read the number instead),
  `crates/ridl-descriptor/tests/deployment_lower_content.rs`,
  `crates/ridl-descriptor/tests/deployment_common.rs`
- Modify: `crates/ridlc/tests/instance_numbers.rs`
- Modify: `docs/design/deployment-descriptor.md` (rewrite "Identifying an
  instance" to describe `instances.lock`, the IR, the section and the descriptor
  field; add `number` and `provisional` to the `instances` row of the field
  table; turn the decision "No number for an instance in ridl" into an
  alternative considered and rejected by the owner on 2026-11-02; correct "The
  Rust backend emits no deployment module" so that it names the
  deployment-independent `instances` module of Task 6)
- Modify: `docs/decisions/ADR-0022-*.md` decision 12 (the lock replaces the
  placement key; `Instance` carries `number` and `provisional`; `System` gains
  `Component.numbers`), and the record's `## Status` amendment line in the form
  the record already uses

**Interfaces:**

- Consumes: Task 3's `v1::Instance.number`, `v1::Instance.provisional`.
- Produces:
  `ridl_descriptor::deployment::Instance { …, number: i32, provisional: bool }`
  (generated owned type), `InstanceRef::number()`, `InstanceRef::provisional()`,
  and
  `VerifyError::InstanceNumber { component: String, instance: String, number: i32 }`
  — the verifier refuses an instance whose number is 0 or negative, with the
  text
  ``instance `{component}/{instance}` has the number {number}; an instance number is 1 to 2147483647``.

- [ ] **Step 1: Write the failing tests**

```rust
// deployment_lower.rs — the system() fixture gives each component `numbers`
#[test]
fn the_instances_carry_their_number_and_provisional_flag() {
    let deployment = owned();
    assert_eq!((deployment.instances[0].number, deployment.instances[0].provisional), (3, false));
    assert_eq!((deployment.instances[1].number, deployment.instances[1].provisional), (1, true));
}

#[test]
fn a_zero_instance_number_is_an_internal_error() {
    let mut system = system();
    system.components[0].numbers.clear();
    assert!(matches!(lower(&system, "Prod", &[], &catalogs()),
        Err(DeploymentLowerError::Internal(text)) if text.contains("number")));
}

#[test]
fn the_verifier_refuses_a_number_that_is_not_positive() {
    // build a Deployment value with Instance { number: 0, .. }, finish() it,
    // and verify(): Err(VerifyError::InstanceNumber { number: 0, .. });
    // the same with number: -1.
}

// deployment_lower_content.rs: extend
// offers_and_maps_equal_the_codegen_section_for_every_placement with
assert_eq!((instance.number, instance.provisional),
           (from_section.number, from_section.provisional), "{}", from_section.instance);
// and update expected() and the JSON expectation with the numbers.
```

And in `crates/ridlc/tests/instance_numbers.rs`,
`the_descriptor_and_the_section_carry_the_locked_number`:
`ridl build --emit
deployment` over the corpus fixture (or the library path the
other `ridlc` tests use), verify the descriptor, and assert that every
instance's `(number, provisional)` equals the section's
(`ridl_ir::codegen::lower_deployment`) and the lock's: `primary` 1, `backup` 2,
`Unit` 3, none provisional.

- [ ] **Step 2: Run them and see them fail**

Run:
`cargo test -p ridl-descriptor && cargo test -p ridlc --test instance_numbers`
Expected: FAIL on the assertions (after the schema regeneration the fields exist
and read 0/false).

- [ ] **Step 3: Implement** lower, verify, JSON; rewrite the design record
      sections and ADR-0022 decision 12. Keep `SCHEMA_VERSION = 1`.

- [ ] **Step 4: Run the tests**

Run:
`cargo test -p ridl-descriptor && cargo test -p ridl-descriptor --no-default-features && cargo test -p ridlc --test instance_numbers && cargo test -p ridl describe`
Expected: PASS. `grep -rn 'shm\.service\|"shm"' crates docs/design` prints
nothing.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-descriptor crates/ridlc/tests/instance_numbers.rs docs/design/deployment-descriptor.md docs/decisions/
git commit -m "feat(ridl-descriptor): carry the instance number in the deployment descriptor"
```

---

### Task 5: `ridl lock` writes `instances.lock`

**Files:**

- Modify: `crates/ridl/src/lock.rs` (allocation, flag parsing, edits, merge
  driver), `crates/ridl/src/main.rs` (help text of `ridl lock`, `--rename`,
  `--retire`, `ridl lock merge`)
- Modify: `crates/ridl/tests/instance_lock_cli.rs`,
  `crates/ridl/tests/remote_units.rs`
- Modify: `docs/book/cli-reference.md` (the `ridl lock` entry and the merge
  driver's `.gitattributes` lines; the quoted `--help` texts must match, which
  `crates/ridl/tests/cli_reference.rs` checks)

**Interfaces:**

- Consumes: Task 1's table edits and merge, Task 2's
  `WorkspaceOutput.instances`, `WorkspaceOutput.instance_lock_dir`, RSDL-314.
- Produces (command behaviour; no new public Rust API):
  - Plain `ridl lock` also allocates every provisional instance, in ascending
    order of its provisional number (so the number written is the number
    checked), and writes `instance_lock_dir/instances.lock` only when it
    allocates. Output lines `allocated veh.Cruise/primary 1`, prefixed exactly
    when interface lines are prefixed, with the root directory relative to
    `PATH`. A workspace whose system numbers no instance writes no file.
  - The instance lock is written only when `PATH` resolves to the root (no
    report scope). A `PATH` inside a member allocates no instance, and an
    instance edit there exits 2 with
    `` `instances.lock` belongs to the workspace root: run `ridl lock <root>` ``.
  - Flag resolution: a key with `/` is an `InstanceKey`. A key without `/` is a
    component when the instance lock has an entry under it (live or retired for
    `--rename`, live for `--retire`), else an interface key; when both the
    interface lock of the target unit and the instance lock hold it, exit 2
    naming both files. `--rename OLD=NEW` refuses a `/` on one side only (exit
    2). Instance edits do not require `PATH` to resolve to one unit.
  - `--retire` of an instance (or a component) the system still has exits 2;
    `--rename OLD=NEW` exits 2 unless every `NEW` key is an instance without an
    entry. Output lines `renamed Old New N`, `retired Key N`, one per entry.
  - `only_lock_orphans` also accepts RSDL-314; an edit run hides RSDL-314 as it
    hides RIDL-409.
  - `ridl lock merge` merges an instance lock when the OURS text, else the
    THEIRS text, holds a line equal to `instance_lock::HEADER`; otherwise an
    interface lock, as today. The CLI surface (`freeze/cli.txt`) does not
    change.

- [ ] **Step 1: Write the failing tests** in `instance_lock_cli.rs` (system with
      `Cruise [ instances = (primary, backup) ]` and the lone service
      `veh.status`, package `veh`): -
      `plain_lock_allocates_every_instance_in_key_byte_order`: stdout contains,
      in this order, `allocated veh.Cruise/backup 1`,
      `allocated veh.Cruise/primary 2`, `allocated veh.status/Unit 3`; the file
      equals
      `"{HEADER}\nnext 4\nveh.Cruise/backup 1\nveh.Cruise/primary 2\nveh.status/Unit 3\n"`.
      - `plain_lock_with_nothing_to_change_writes_nothing`: second run prints no
      `allocated` line, file bytes unchanged. -
      `a_workspace_with_no_system_gets_no_instance_lock`. -
      `plain_lock_refuses_an_orphan_instance_entry`: exit 1, file unchanged. -
      `retire_an_instance_marks_its_line_retired` (instance removed from the
      source first) and `retire_of_an_instance_still_in_the_system_exits_two`. -
      `rename_an_instance_keeps_its_number`:
      `--rename veh.Cruise/backup=veh.Cruise/spare` after the source renames it;
      line `veh.Cruise/spare 1`. -
      `rename_a_component_moves_every_entry_with_its_number`: lock
      `veh.Cruise/primary 2`, `veh.Cruise/backup 1`, `veh.Cruise/old 4 retired`,
      source renames `Cruise` to `Speed`; `--rename veh.Cruise=veh.Speed` exits
      0 and the file holds `veh.Speed/backup 1`, `veh.Speed/primary 2`,
      `veh.Speed/old 4 retired`. -
      `retire_a_component_retires_all_its_instances`. -
      `lock_on_a_member_path_writes_no_instance_lock` and
      `an_instance_edit_on_a_member_path_exits_two`. -
      `the_merge_driver_merges_two_instance_allocations`: the three texts of
      Task 1's merge test on disk; `ridl lock merge base ours theirs 7` exits 0
      and `ours` holds the merged text with the instance header.

      In `remote_units.rs`,
      `lock_numbers_a_remote_component_in_the_root_lock_only`: the remote unit
      of `a_remote_unit_catalog_is_embedded_in_the_deployment` also declares
      `component Source { offers veh.common.probes }` in an `.rsdl` file; the
      consumer's system lists `veh.common.Source` and its own `Sink`; `ridl
      lock` exits 0, the root `instances.lock` holds `veh.common.Source/Unit`
      and `app.Sink/Unit`, and no `instances.lock` exists under the cache.

- [ ] **Step 2: Run them and see them fail**

Run:
`cargo test -p ridl --test instance_lock_cli && cargo test -p ridl --test remote_units lock_numbers`
Expected: FAIL on the assertions.

- [ ] **Step 3: Implement** in `lock.rs`; update the help strings in `main.rs`
      and the `ridl lock` entry of the book: the instance lock, its key, the
      component forms of `--rename` and `--retire`, and the `.gitattributes`
      line `instances.lock merge=ridl-lock` beside the existing one.

- [ ] **Step 4: Run the tests**

Run:
`cargo test -p ridl --test instance_lock_cli --test lock_cli --test remote_units --test cli_reference`
Expected: PASS, every existing `lock_cli` test included.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl docs/book/cli-reference.md
git commit -m "feat(ridl): write instances.lock with ridl lock"
```

---

### Task 6: The Rust constants

**Files:**

- Create: `crates/ridl-backend-rust/src/instances.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (`mod instances;`,
  `pub use instances::generate_instances_module;`)
- Modify: `crates/ridlc/src/lib.rs` (`INSTANCES_FILE`, `render_lib_rs`,
  `write_crate_files`, `crate_file_refusals`, the refusal of a package named
  `instances`, lowering the system whenever crate files are written)
- Regenerate: `freeze/generated/` (`cargo xtask freeze update`; the frozen cabin
  crate gains `__ridl_instances.rs` and one `lib.rs` line)
- Modify: `docs/book/generated-code.md` ("What the crate contains": the
  `instances` module) and the crate-root paragraph of the `ridl build` entry in
  `docs/book/cli-reference.md`
- Test: `crates/ridl-backend-rust/src/instances.rs` (`mod tests`),
  `crates/ridlc/tests/instance_numbers.rs`

**Interfaces:**

- Consumes: Task 3's `v2::Component.numbers`, the lowered system ridlc already
  holds.
- Produces:
  - `pub fn generate_instances_module(system: &v2::System) -> Result<Generated, GenerateError>`
    — one `pub const <SCREAMING>: i32 = <n>;` per entry of every component's
    `numbers`, under nested `pub mod` blocks named by the component's qualified
    name (each segment `module_segment(&ridl_ir::name::snake_case(segment))`),
    formatted with prettyplease like the other generated files. Each constant's
    doc is `` `{key}` in `instances.lock`. ``, plus, when provisional, a second
    paragraph
    ``Provisional: no entry of `instances.lock` holds this number yet, and `ridl lock` writes it.``.
    The module's own doc:
    ``The number of every instance of the system, from `instances.lock`.`` Two
    keys with one constant path are a `GenerateError` whose message names both
    keys.
  - ridlc writes the source to `__ridl_instances.rs` (with the comment preamble)
    and adds `#[rustfmt::skip]`, `#[path = "__ridl_instances.rs"]`,
    `pub mod instances;` to `lib.rs` when the lowered system exists; it removes
    a generated `__ridl_instances.rs` when it does not (as for the `wasm` file);
    a package whose first segment is `instances` with a system is a build error
    naming the package (as for `wasm`).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-backend-rust/src/instances.rs
#[test]
fn every_instance_is_a_constant_under_its_component_path() {
    let source = generate_instances_module(&system_of(&[
        ("veh.adas", "Cruise", &[("primary", 1, false), ("backup", 2, false)]),
        ("", "veh.ego.status", &[("Unit", 3, true)]),
    ])).unwrap().rust_source;
    assert!(source.contains("pub mod adas"));
    assert!(source.contains("pub mod cruise"));
    assert!(source.contains("pub const PRIMARY: i32 = 1;"));
    assert!(source.contains("pub const BACKUP: i32 = 2;"));
    assert!(source.contains("pub mod status"));
    assert!(source.contains("pub const UNIT: i32 = 3;"));
    assert_eq!(source.matches("Provisional:").count(), 1);
}

#[test]
fn two_keys_with_one_constant_path_are_refused() {
    let error = generate_instances_module(&system_of(&[
        ("veh.adas", "LaneKeep", &[("Unit", 1, false)]),
        ("", "veh.adas.lane_keep", &[("Unit", 2, false)]),
    ])).unwrap_err();
    assert!(error.message.contains("veh.adas.LaneKeep/Unit"));
    assert!(error.message.contains("veh.adas.lane_keep/Unit"));
}
```

In `crates/ridlc/tests/instance_numbers.rs`:

- `the_crate_holds_the_locked_constants`: `ridl build --emit rust` over the
  corpus fixture; `lib.rs` contains `pub mod instances;`; `__ridl_instances.rs`
  contains `pub const PRIMARY: i32 = 1;`, `pub const BACKUP: i32 = 2;` and
  `pub const UNIT: i32 = 3;` and no `Provisional:`.
- `the_lock_changes_no_catalog_hash`: build `--emit catalog` with and without
  the fixture's `instances.lock` (copy the fixture to a temp dir); every
  `.catalog.binfb` is byte-identical.
- `a_package_named_instances_is_refused_with_a_system`: exit 1, the message
  names the package.

- [ ] **Step 2: Run them and see them fail**

Run:
`cargo test -p ridl-backend-rust instances && cargo test -p ridlc --test instance_numbers`
Expected: FAIL on the assertions.

- [ ] **Step 3: Implement**, then run `cargo xtask freeze update` and check that
      the only change under `freeze/generated/` is the new file and the `lib.rs`
      line. Update the two book pages.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridl-backend-rust && cargo test -p ridlc` Expected: PASS.
CI's `just demo` compiles and lints the generated crates.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust crates/ridlc freeze/generated docs/book
git commit -m "feat(ridl-backend-rust): generate a constant for every instance number"
```

---

### Task 7: `ridl diff` reports a changed instance number as breaking

**Files:**

- Modify: `crates/ridl-diff/src/system.rs` (`SystemHeading`, `heading_text`,
  `diff_systems`, module doc)
- Modify: `crates/ridl-diff/src/lib.rs` (`DiffReport.system` doc,
  `diff_workspaces_with` verdict, `render_text`, `render_json`)
- Modify: `crates/ridl/tests/diff_cli.rs`
- Modify: `docs/specification/rsdl-language-reference.md` §14,
  `docs/decisions/ADR-0022-*.md` decision 9 (it says every system change carries
  no verdict and the contracts alone decide the exit code; this heading is the
  exception), `docs/book/cli-reference.md` ("At the system")
- Test: `crates/ridl-diff/src/system.rs` (`mod tests`),
  `crates/ridl/tests/diff_cli.rs`

**Interfaces:**

- Consumes: Task 3's `Component.numbers`.
- Produces:
  - `SystemHeading::InstanceNumberChanged`, `heading_text` →
    `"instance number changed"`.
  - `diff_systems` appends, after the existing changes: for each key
    (`component/instance`) whose old number is not provisional and whose new
    number differs, `path = key, before = old number, after = new number`; then
    for each new key that holds a number the old side gave, not provisionally,
    to another key, `path = new key, before = "{n} ({old key})", after = "{n}"`.
    A provisional old number is never reported; a key absent on one side is not
    reported under this heading.
  - `diff_workspaces_with` sets the report verdict to `Verdict::Breaking` when
    any system change has this heading. `render_text` prints the heading as
    `instance number changed (breaking)`; `render_json` adds the key
    `instance_number_changed`, present only when non-empty.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-diff/src/system.rs tests
#[test]
fn a_changed_locked_number_is_listed() {
    let changes = diff_systems(&numbered(&[("veh.C", "a", 1, false)]),
                               &numbered(&[("veh.C", "a", 2, false)]));
    let listed: Vec<_> = changes.iter()
        .filter(|c| c.heading == SystemHeading::InstanceNumberChanged).collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, "veh.C/a");
    assert_eq!((listed[0].before.as_deref(), listed[0].after.as_deref()), (Some("1"), Some("2")));
}

#[test]
fn a_number_given_to_another_instance_is_listed() {
    let changes = diff_systems(&numbered(&[("veh.C", "a", 1, false)]),
                               &numbered(&[("veh.C", "b", 1, false)]));
    let listed: Vec<_> = changes.iter()
        .filter(|c| c.heading == SystemHeading::InstanceNumberChanged).collect();
    assert_eq!(listed[0].path, "veh.C/b");
    assert_eq!(listed[0].before.as_deref(), Some("1 (veh.C/a)"));
}

#[test]
fn a_provisional_old_number_is_not_listed() {
    let changes = diff_systems(&numbered(&[("veh.C", "a", 1, true)]),
                               &numbered(&[("veh.C", "a", 2, false)]));
    assert!(changes.iter().all(|c| c.heading != SystemHeading::InstanceNumberChanged));
}
```

In `diff_cli.rs`, `a_changed_instance_number_is_breaking_and_exits_one`: two
source workspaces equal except for their `instances.lock`
(`veh.demo.Panel/Unit 1` against `veh.demo.Panel/Unit 2`, each with the right
`next`); `ridl diff old new` exits 1 and stdout equals
`"breaking\ninstance number changed (breaking)\n  veh.demo.Panel/Unit: 1 -> 2\n"`;
with `--format json`, `verdict` is `breaking` and `instance_number_changed[0]`
is `{"path": "veh.demo.Panel/Unit", "before": "1", "after": "2"}`. And
`locking_a_provisional_number_unchanged_exits_zero`: old side without
`instances.lock`, new side locking the same number; exit 0 and no
`instance number changed` line.

- [ ] **Step 2: Run them and see them fail**

Run:
`cargo test -p ridl-diff system && cargo test -p ridl --test diff_cli instance`
Expected: FAIL on the assertions.

- [ ] **Step 3: Implement**, then update §14 (a third heading, with its verdict
      and the provisional rule), ADR-0022 decision 9 (one sentence: the instance
      number heading carries `breaking`), and the book's "At the system"
      paragraph and its JSON key list.

- [ ] **Step 4: Run the tests**

Run:
`cargo test -p ridl-diff && cargo test -p ridl --test diff_cli && cargo test -p ridl-mcp`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-diff crates/ridl/tests/diff_cli.rs docs/specification/rsdl-language-reference.md docs/decisions/ docs/book/cli-reference.md
git commit -m "feat(ridl-diff): report a changed instance number as breaking"
```

---

## Decisions this plan takes where the spec leaves a choice

- `instances.lock` shares the interface lock's parser through a key type
  parameter (`LockTable<K>`), in a new module `instance_lock`.
- The two codes are RSDL-314 and RSDL-315, the next free codes of the 3xx group
  ("component and instances"). RSDL-315 also covers a provisional instance that
  would take a number above 2147483647, because the spec makes that an error and
  names no third code.
- The numbering function lives in `crates/ridl-sem/src/rsdl/numbering.rs`,
  because it reads `CheckedSystem`; it mirrors `number_unit` of `check.rs`.
- The loaded lock rides on `LoadedWorkspace`, not on the salsa `Workspace`
  input, so the 100 constructors of `Workspace` stay unchanged; the drivers
  (`check_loaded`, the language server) draw RSDL-314.
- The `instances` module is its own file, `__ridl_instances.rs`; a package named
  `instances` is refused with a system, as a package named `wasm` is refused
  with the `wasm` option.
- `ridl lock merge` tells the two files apart by the instance header line.
- The text heading reads `instance number changed (breaking)`; a reused number
  is reported on the new key with the old holder in `before`.

## Self-review notes

- The spec's section 2 field numbers are fixed here: `Component.numbers = 15`,
  `Instance.number = 8`, `Instance.provisional = 9`.
- External components are numbered: the spec says "every instance of every
  component in the closure".
