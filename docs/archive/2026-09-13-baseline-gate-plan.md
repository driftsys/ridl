# Baseline gate, explicit-baseline read, and composite reorder — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `ridl baseline` from publishing an interaction removal that has
no `reserved` tombstone, stop an explicit `--baseline` path that holds no
snapshot from passing silently, and give a composite body reorder its own diff
category.

**Architecture:** The gate runs inside `run_baseline`, between the clean-compile
check and `publish_baseline`. It reads the snapshots already in the output
directory — the ones publication is about to delete — diffs them against the
freshly built snapshots in the staging directory, and records a new RIDL-408 for
every `InteractionRemoved` it finds. A recorded error makes `CliRun::has_error`
true, so the existing `finish` helper renders the diagnostic and returns exit 1
with no further change. The explicit-baseline refusal extends the ladder
`load_baseline` already runs when a directory holds no snapshot. The reorder
category is a new `Category` variant plus a sequence comparison inside the
branch of `diff_composite` that today emits one whole-container
`ConstraintChanged`.

**Tech Stack:** Rust, one workspace, `cargo` with `--locked`. Diagnostics are
declared through the `diag_codes!` macro in `crates/ridl-core/src/diag.rs`. Diff
categories are declared through the `declare_categories!` macro in
`crates/ridl-diff/src/lib.rs`. Integration tests use a hand-rolled harness over
`std::process::Command` and `CARGO_BIN_EXE_ridl`; there is no `assert_cmd`.

**Spec:**
[`2026-09-13-baseline-gate-design.md`](2026-09-13-baseline-gate-design.md)

## Global Constraints

- Work only inside the worktree `.claude/worktrees/baseline-tombstone-gate` on
  branch `baseline-tombstone-gate`. Never check out another branch there, never
  touch another worktree, never run a formatter in a worktree that is not this
  one.
- Every cargo command passes `--locked`. `just build` is the full local gate
  (ADR-0009); run it before opening a pull request.
- Run one test with `cargo test -p <crate-name> --locked <test_name>`. There is
  no justfile recipe for a single test; the workspace recipe is `just test`.
- Conventional Commits, linted by git-std against `.git-std.toml`. The scopes
  this plan uses are `ridl` (which covers the `crates/ridl` binary as well as
  the language), `ridl-core`, `ridl-diff`, `docs` and `adr`. A commit that spans
  a crate and its records takes the crate scope.
- Never push to `main`. Open a pull request. `gh pr create` has been failing
  with GitHub GraphQL server errors in this repository; the REST endpoint
  `gh api repos/driftsys/ridl/pulls --method POST --input <json>` works, and its
  response must be projected with `--jq` because the raw body is very large.
- Prose — code comments, commit messages, documentation — is plain and literal.
  No idioms and no figures of speech. Technical terms and acronyms stay as they
  are.
- A diagnostic code must be declared through `diag_codes!` before it is written
  anywhere as a string literal, or the catalogue drift test
  `codes_written_as_string_literals_are_all_catalogued` in
  `crates/ridl-core/src/diag.rs` fails.
- `classify`, `explain` and `category_word` each carry
  `#[deny(clippy::wildcard_enum_match_arm)]`, so a new `Category` variant must
  gain a real arm in each of the three. The build fails until it does.

---

## File Structure

**Task 1 — the baseline gate**

- Modify `crates/ridl-core/src/diag.rs` — one new entry, `RIDL_408`, directly
  after `RIDL_407` at line 738.
- Modify `crates/ridl/src/main.rs` — `run_baseline` (lines 482-516) gains the
  gate call; one new private function `untombstoned_removals` beside it.
- Create `crates/ridl/tests/baseline_gate.rs` — four integration tests.
- Modify `docs/specification/ridl-language-reference.md` — a RIDL-408 row in the
  diagnostics table and one sentence in §11.
- Modify `docs/decisions/ADR-0010-cli-conventions.md` — the `ridl baseline`
  row's exit-1 cell in decision 1's table.

**Task 2 — the explicit-baseline input error**

- Modify `crates/ridl/src/main.rs` — `baseline_location` (lines 557-583) reports
  whether the location came from the flag; `load_baseline` (lines 780-817) gains
  the parameter and one more refusal; `desk_check` (line 615) passes it through;
  one new private function `refuse_empty_baseline`.
- Modify `crates/ridl/tests/baseline_desk.rs` — two tests appended.
- Modify `docs/decisions/ADR-0010-cli-conventions.md` — the `ridl check` row's
  exit-2 cell, and decision 1's closing passage on within-cell dates.

**Task 3 — the composite reorder category**

- Modify `crates/ridl-diff/src/lib.rs` — one `MemberReordered` variant in the
  `declare_categories!` invocation, one `category_word` arm.
- Modify `crates/ridl-diff/src/classify.rs` — one `classify` arm, one `explain`
  arm.
- Modify `crates/ridl-diff/src/walk.rs` — the tail of `diff_composite` (lines
  244-253).
- Create `crates/ridl/tests/diff_member_reorder.rs` — two integration tests.
- Modify `docs/book/cli-reference.md` — the diff category list near line 822.

---

### Task 1: The baseline publication gate

**Files:**

- Modify: `crates/ridl-core/src/diag.rs:738`
- Modify: `crates/ridl/src/main.rs:482-516`
- Create: `crates/ridl/tests/baseline_gate.rs`
- Modify: `docs/specification/ridl-language-reference.md`
- Modify: `docs/decisions/ADR-0010-cli-conventions.md`

**Interfaces:**

- Consumes: `ridl_diff::diff_sets(&[Package], &[Package]) -> DiffReport` and
  `ridl_diff::Category::InteractionRemoved`, both already public.
  `snapshot_files`, `load_snapshots`, `DeclIndex::build`, `DeclIndex::span_of`
  and `finish`, all already private to `crates/ridl/src/main.rs`.
- Produces: `DiagCode::RIDL_408`, and the private function
  `untombstoned_removals(entry: &Path, out_dir: &Path, staging: &Path, run: &mut CliRun) -> Result<bool, ExitCode>`
  returning `Ok(true)` when at least one refusal was recorded. Task 2 does not
  depend on either.

- [ ] **Step 1: Create the test file with the harness copied from the existing
      one**

Create `crates/ridl/tests/baseline_gate.rs`. Copy the `TempDir` struct with its
`new`, `path`, `write` and `Drop` implementations, the `ridl` helper, the
`MANIFEST` constant and the `package_workspace` helper **verbatim** from
`crates/ridl/tests/baseline_desk.rs:1-62` and `:204-208`. Read those line ranges
and copy what is there; do not retype from memory. Then append the fixtures and
the first test below.

```rust
/// Three events. The second is the one the later fixtures remove.
/// The shape mirrors `BASE` in `baseline_desk.rs:62` — a `type` declaration
/// rather than an enum, and a timing bound on every event — because that
/// fixture is known to compile.
const THREE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `doorClosed` deleted outright. `doorLocked` slides onto ordinal 2.
const BARE_REMOVAL: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `doorClosed` retired in place. `doorLocked` keeps ordinal 3.
const TOMBSTONED_REMOVAL: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
";

/// The published baseline is the only record that a removed interaction's
/// ordinal was ever taken. Replacing it with a snapshot that drops the
/// interaction with no tombstone destroys that record, so publication refuses.
#[test]
fn baseline_refuses_to_publish_an_untombstoned_removal() {
    let dir = TempDir::new("gate-refuses");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a refused publication is a negative answer, not a tool failure:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        stderr.contains("doorClosed"),
        "the message names the interaction that would be dropped:\n{stderr}",
    );
    assert!(
        stderr.contains("reserved doorClosed"),
        "the message names the line that would sanction the removal:\n{stderr}",
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
`cargo test -p ridl --locked baseline_refuses_to_publish_an_untombstoned_removal`

Expected: FAIL. The second `ridl baseline` exits 0 and prints nothing, so the
`assert_eq!(code, 1)` fails with `left: 0, right: 1`.

- [ ] **Step 3: Declare RIDL-408**

In `crates/ridl-core/src/diag.rs`, directly after the `RIDL_407` entry that ends
at line 738, add:

```rust
/// An interaction present in the baseline being replaced is gone from
/// the source with no `reserved` tombstone (ridl §11). Error. Emitted by
/// `ridl baseline` alone. Publication is the last point at which the
/// removal can still be refused, because the snapshot about to be
/// overwritten is the only record that the ordinal was ever taken.
/// Distinct from RIDL-407, which is the desk-time warning that an
/// ordinal moved and which neither classifies nor gates.
RIDL_408 = "RIDL-408", Error,
    "interaction removed without a `reserved` tombstone";
```

- [ ] **Step 4: Add the gate to `run_baseline`**

In `crates/ridl/src/main.rs`, change the binding at line 489 from `let run` to
`let mut run`, then insert the gate between the `run.has_error()` block that
ends at line 504 and the `publish_baseline` call that begins at line 506:

```rust
// The published baseline is the only record that a removed interaction's
// ordinal was ever taken. Replacing it with a snapshot that drops the
// interaction with no `reserved` tombstone destroys that record, and a
// later append then reuses the ordinal with nothing to compare against.
// The comparison happens here, against the directory publication is about
// to overwrite (driftsys/ridl#315).
match untombstoned_removals(path, &out_dir, &staging, &mut run) {
    Ok(false) => {}
    Ok(true) => {
        let _ = std::fs::remove_dir_all(&staging);
        return finish(Ok(run));
    }
    Err(code) => {
        let _ = std::fs::remove_dir_all(&staging);
        return code;
    }
}
```

Then add this function directly after `run_baseline`:

```rust
/// Compares the baseline about to be replaced against the snapshots just built
/// and records a RIDL-408 for every interaction the replacement would drop with
/// no `reserved` tombstone. Returns whether any was recorded.
///
/// The published snapshots are read flat from `out_dir`, which is exactly where
/// [`publish_baseline`] writes them. This deliberately does not go through
/// [`load_baseline`], whose discovery rules exist to interpret a user-supplied
/// `--baseline` path: inheriting them would let the comparison become a
/// comparison against nothing in the cases driftsys/ridl#235 describes, and a
/// gate that a directory layout can defeat is not a gate.
///
/// Only the interaction level is covered. The interface level — a removed
/// interface with no service-level tombstone, and an unfrozen interface number
/// — arrives with the lock file, because the rsdl decisions note's D-7 retires
/// the service shape-list slot model a service-level gate would rest on.
fn untombstoned_removals(
    entry: &Path,
    out_dir: &Path,
    staging: &Path,
    run: &mut CliRun,
) -> Result<bool, ExitCode> {
    if !out_dir.is_dir() {
        return Ok(false);
    }
    let published = load_snapshots(&snapshot_files(out_dir)?)?;
    if published.is_empty() {
        return Ok(false);
    }
    let fresh = load_snapshots(&snapshot_files(staging)?)?;

    let report = ridl_diff::diff_sets(&published, &fresh);
    let index = DeclIndex::build(entry);
    let mut refusals = Vec::new();
    for change in &report.changes {
        if change.category != ridl_diff::Category::InteractionRemoved {
            continue;
        }
        let name = change.path.rsplit('/').next().unwrap_or(&change.path);
        refusals.push(Diagnostic {
            code: DiagCode::RIDL_408,
            severity: Severity::Error,
            message: format!(
                "`{name}` is gone from the source but the baseline being replaced still \
                 declares it. Publishing would free its ordinal for a later interaction \
                 to reuse, with nothing left to record that it was ever taken. Retire it \
                 in place with `reserved {name}`."
            ),
            primary: index.span_of(&change.path, &mut run.sources),
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    let refused = !refusals.is_empty();
    run.diagnostics.extend(refusals);
    Ok(refused)
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run:
`cargo test -p ridl --locked baseline_refuses_to_publish_an_untombstoned_removal`

Expected: PASS.

If it fails to compile on `Diagnostic`, `DiagCode` or `Severity` not being in
scope, read the `use` block at the top of `crates/ridl/src/main.rs` —
`desk_check` at line 615 already constructs a `Diagnostic` with all three, so
they are imported; do not add duplicate imports.

- [ ] **Step 6: Write the three remaining tests**

Append to `crates/ridl/tests/baseline_gate.rs`:

```rust
/// The refused run must leave the published record exactly as it was. An exit
/// code alone does not prove that: the refusal happens after a successful
/// compile, one step before the call that deletes the published files.
#[test]
fn a_refused_publication_leaves_the_baseline_byte_identical() {
    let dir = TempDir::new("gate-preserves");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root.join(".ridl").join("baseline").join("veh.cluster.ir.json");
    let before = std::fs::read(&snapshot).expect("the published snapshot is readable");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, _) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "the publication is refused");

    let after = std::fs::read(&snapshot).expect("the published snapshot survives the refusal");
    assert_eq!(
        before, after,
        "a refused publication rewrites nothing",
    );
}

/// The sanctioned retirement is the whole point of the tombstone, so it must
/// publish. `doorLocked` keeps ordinal 3 because the tombstone holds slot 2.
#[test]
fn baseline_publishes_a_tombstoned_removal() {
    let dir = TempDir::new("gate-admits-tombstone");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", TOMBSTONED_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a removal that leaves a tombstone is the sanctioned path:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "the sanctioned path draws no refusal:\n{stderr}",
    );
}

/// With nothing published there is nothing to compare, so the first
/// publication is never refused and draws no diagnostic.
#[test]
fn the_first_publication_is_never_refused() {
    let dir = TempDir::new("gate-first");
    let root = package_workspace(&dir, BARE_REMOVAL);

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "a first publication has no prior record:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-408"),
        "a first publication draws no refusal:\n{stderr}",
    );
}
```

- [ ] **Step 7: Run every test in the file**

Run: `cargo test -p ridl --locked --test baseline_gate`

Expected: 4 tests PASS.

- [ ] **Step 8: Run the existing baseline tests to prove nothing regressed**

Run: `cargo test -p ridl --locked --test baseline_desk`

Expected: every test PASSES. The desk check is untouched, and RIDL-407 keeps its
severity and its single call site.

- [ ] **Step 9: Amend the ridl reference**

In `docs/specification/ridl-language-reference.md`, add a row to the diagnostics
table directly after the `RIDL-407` row at line 1597. Match the surrounding
column widths exactly; `just check` runs `prim` over the file and will report a
malformed table.

```markdown
| RIDL-408 | interaction removed without a `reserved` tombstone, refused at publication (§11) — emitted by `ridl baseline` alone, which refuses to replace a baseline whose record of the ordinal would be lost | error |
```

Then, in §11, directly after the bullet that reads "**`reserved` tombstones**
retire removed interactions by name:" and its fenced example (the example ends
around line 1105), add:

```markdown
- **Publication enforces the tombstone.** `ridl baseline` refuses to replace a
  published baseline when the replacement drops an interaction that the baseline
  declares and the source does not retire with `reserved` — **RIDL-408**, exit 1,
  nothing written. The published snapshot is the only record that the ordinal was
  taken, so publication is the last point at which the removal can be refused.
  RIDL-407 is unchanged: it remains the desk-time warning that an ordinal moved,
  emitted by `ridl check`, and it neither classifies nor gates.
```

- [ ] **Step 10: Amend ADR-0010**

In `docs/decisions/ADR-0010-cli-conventions.md`, in decision 1's table, the
`ridl baseline` row's exit-1 cell currently reads "a package with a diagnostic
error publishes nothing". Change it to:

```markdown
a package with a diagnostic error publishes nothing; a replacement that drops an interaction with no `reserved` tombstone is refused (RIDL-408)
```

Leave every other cell alone. Task 2 amends the `ridl check` row's exit-2 cell —
a different row, because `--baseline` is a flag of `ridl check`.

- [ ] **Step 11: Run the full local gate**

Run: `just build`

Expected: every member passes — toolchain-check, gate-parity, fmt-check,
book-check, link-check, compile, test, lint, wasm-check, check.

If `just check` reports a Markdown formatting difference, run `just fmt` (this
worktree is yours, so the formatter is safe to run here) and re-run
`just build`.

- [ ] **Step 12: Commit**

```bash
test "$(git branch --show-current)" = "baseline-tombstone-gate" || exit 1
git add crates/ridl-core/src/diag.rs crates/ridl/src/main.rs \
        crates/ridl/tests/baseline_gate.rs \
        docs/specification/ridl-language-reference.md \
        docs/decisions/ADR-0010-cli-conventions.md
git commit -m "$(cat <<'MSG'
fix(ridl): refuse to publish a baseline that drops an interaction without a tombstone

`ridl baseline` published unconditionally. An interaction removed with no
`reserved` tombstone was published as if it had never been declared, a later
append reused the freed ordinal, and `ridl diff` reported the append compatible
against that second baseline (driftsys/ridl#315).

The gate runs between the clean compile and the publish. It reads the snapshots
already in the output directory, which is where publication writes and what it
is about to delete, diffs them against the freshly built ones, and records
RIDL-408 for every interaction the replacement would drop with no tombstone. A
recorded error exits 1 and writes nothing, so the published record survives.

It does not reuse `desk_check`. That path interprets a user-supplied
`--baseline` and would let the comparison become a comparison against nothing in
the cases driftsys/ridl#235 describes.

RIDL-407 is unchanged. The reference defines it as non-gating and as meaning
that an ordinal moved, which is the consequence of a removal rather than the
removal itself.

Only the interaction level is covered. The interface level waits for the lock
file, because the rsdl decisions note's D-7 retires the service shape-list slot
model a service-level gate would rest on.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

### Task 2: An explicit baseline that holds no snapshot is an input error

**Files:**

- Modify: `crates/ridl/src/main.rs:557-583` (`baseline_location`), `:615`
  (`desk_check`), `:780-817` (`load_baseline`)
- Modify: `crates/ridl/tests/baseline_desk.rs`
- Modify: `docs/decisions/ADR-0010-cli-conventions.md`

**Interfaces:**

- Consumes: nothing from Task 1.
- Produces: `baseline_location` returns
  `Result<Option<(PathBuf, bool)>, ExitCode>` where the `bool` is true when the
  location came from `--baseline`;
  `load_baseline(location: &Path, explicit: bool)`;
  `desk_check(entry: &Path, location: &Path, explicit: bool, run: &mut CliRun)`.
  Task 3 does not depend on any of them.

**Background the implementer needs.** `load_baseline` already refuses two shapes
of unusable baseline directory: one holding a non-JSON IR artifact, and one
whose snapshots sit exactly one level below the given path
(`first_nested_snapshot_dir`, whose doc comment records the one-level bound as
deliberate and scoped to driftsys/ridl#230). What it does not refuse is
everything else that yields no snapshot — snapshots two or more levels down, or
a directory with nothing in it at all. In those cases it returns an empty vector
and `desk_check` returns `Ok(())` at its second line, so `ridl check` exits 0
with no output. This task adds the last rung to that existing ladder. The search
depth does not change.

- [ ] **Step 1: Write the two failing tests**

Append to `crates/ridl/tests/baseline_desk.rs`:

```rust
/// A baseline path aimed two or more levels above the snapshots falls past
/// both existing refusals and yields no snapshot. Comparing against nothing
/// reports no drift and exits 0, which reads exactly like a clean check, so it
/// is an input error instead (driftsys/ridl#235).
#[test]
fn an_explicit_baseline_holding_no_snapshot_is_an_input_error() {
    let dir = TempDir::new("empty-explicit");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", REORDERED);
    // The snapshots are at `<root>/.ridl/baseline/`, two levels below `root`.
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "the tool could not answer, so it says so instead of passing:\n{stderr}",
    );
    assert!(
        stderr.contains("no `.ir.json` snapshot"),
        "the cause is named:\n{stderr}",
    );
}

/// Auto-discovery keeps its silent skip. With no flag and no published
/// baseline, `ridl check` behaves as it did before the baseline command
/// existed, which is the property `baseline_location` documents.
#[test]
fn auto_discovery_with_no_baseline_stays_silent() {
    let dir = TempDir::new("empty-auto");
    let root = package_workspace(&dir, BASE);

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "a clean check with no baseline succeeds:\n{stderr}");
    assert!(
        stdout.is_empty() && stderr.is_empty(),
        "no baseline means no drift report at all:\nstdout: {stdout}\nstderr: {stderr}",
    );
}
```

- [ ] **Step 2: Run the tests to verify the first fails**

Run:
`cargo test -p ridl --locked --test baseline_desk an_explicit_baseline_holding_no_snapshot_is_an_input_error`

Expected: FAIL with `left: 0, right: 2`.

Run:
`cargo test -p ridl --locked --test baseline_desk auto_discovery_with_no_baseline_stays_silent`

Expected: PASS already. It pins behaviour this task must not break.

- [ ] **Step 3: Add the refusal helper**

In `crates/ridl/src/main.rs`, directly after `load_baseline`, add:

```rust
/// An explicit `--baseline` path that holds no snapshot at the depth the loader
/// reads is an input error, not a silent pass. The caller asserted that a
/// baseline is there. A comparison against nothing reports no drift and exits
/// 0, which is indistinguishable from a clean check — the same failure shape
/// ADR-0010 decision 6 closed for `ridl fmt` (driftsys/ridl#235).
fn refuse_empty_baseline(location: &Path) -> ExitCode {
    eprintln!(
        "error: the baseline `{}` holds no `.ir.json` snapshot; publish one with \
         `ridl baseline --out {}`",
        location.display(),
        location.display(),
    );
    ExitCode::from(2)
}
```

- [ ] **Step 4: Thread the origin through and add the last rung**

Three edits in `crates/ridl/src/main.rs`.

First, `baseline_location` reports where the location came from. Change its
signature and the two arms that return a location:

```rust
fn baseline_location(
    entry: &Path,
    flag: Option<&Path>,
) -> Result<Option<(PathBuf, bool)>, ExitCode> {
```

```rust
Some(explicit) if explicit.exists() => Ok(Some((explicit.to_path_buf(), true))),
```

```rust
None => {
    let default = default_baseline_dir(entry);
    Ok(default.is_dir().then(|| (default, false)))
}
```

Leave the two error arms exactly as they are.

Second, `load_baseline` takes the flag and adds the last rung. Change its
signature, and add the new refusal directly after the
`first_nested_snapshot_dir` block, still inside `if snapshots.is_empty()`:

```rust
fn load_baseline(location: &Path, explicit: bool) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
```

```rust
// The two refusals above name a specific, fixable mistake. This one
// catches every remaining way a directory yields no snapshot —
// snapshots two or more levels down, or an empty directory — and
// refuses rather than comparing against nothing. Auto-discovery is
// exempt: with no flag, "no baseline published yet" is legitimate.
if explicit {
    return Err(refuse_empty_baseline(location));
}
```

Third, `desk_check` takes the flag and passes it on:

```rust
fn desk_check(
    entry: &Path,
    location: &Path,
    explicit: bool,
    run: &mut CliRun,
) -> Result<(), ExitCode> {
    let baseline = load_baseline(location, explicit)?;
```

- [ ] **Step 5: Fix the call site the compiler names**

Run: `cargo build -p ridl --locked`

Expected: one or more type errors at the `baseline_location` call site inside
`run_check` (near `crates/ridl/src/main.rs:456`), because it now yields
`Option<(PathBuf, bool)>` rather than `Option<PathBuf>`. Destructure the tuple
and pass the flag into `desk_check`. Read the surrounding lines and match the
existing control flow; do not restructure it.

Re-run `cargo build -p ridl --locked` until it compiles.

- [ ] **Step 6: Run both tests to verify they pass**

Run: `cargo test -p ridl --locked --test baseline_desk`

Expected: every test PASSES, including the two added in step 1.

- [ ] **Step 7: Run the gate tests from Task 1**

Run: `cargo test -p ridl --locked --test baseline_gate`

Expected: 5 tests PASS. The gate reads `out_dir` directly and never calls
`load_baseline`, so this task must not affect it. If it does, the gate is going
through the loader and step 4 of Task 1 was not followed.

- [ ] **Step 8: Amend ADR-0010**

In `docs/decisions/ADR-0010-cli-conventions.md`, decision 1's table, the
**`ridl check`** row's exit-2 cell currently reads "the given path does not
exist". Change it to:

Take care to edit the right row. Five rows' exit-2 cells read "the given path
does not exist" verbatim — `ridl check`, `ridl build`, `ridl baseline`,
`ridlc check` and `ridlc build` — so a search for that text alone will land on
the wrong one. `--baseline` is a flag of `ridl check` (`main.rs:20`, `:454`),
not of the `ridl baseline` subcommand, and `ridlc` has no baseline flag at all.
The new text is:

```markdown
the given path does not exist, or an explicit `--baseline` holds no `.ir.json` snapshot
```

Then, in decision 6's prose, after the sentence describing the `ridl fmt`
fail-closed change, add one sentence:

```markdown
The same failure shape was closed for an explicit `--baseline` that holds no
snapshot, which reported no drift and exited 0 (driftsys/ridl#235); the search
depth `first_nested_snapshot_dir` records as deliberate is unchanged.
```

Then amend decision 1's closing passage. It currently carries a within-cell date
rule written for one clause — the `ridl baseline` exit-1 cell's tombstone clause
Task 1 added. Your new exit-2 clause is a second clause that postdates the
table's 2026-07-27 construction, so leaving the passage as it stands makes it
certify your clause under a date that does not cover it. That is the exact
defect Task 1's review raised as an Important finding; do not reproduce it.
Replace the passage from "The same discipline applies within a cell:" to the end
of the paragraph with:

```markdown
The same discipline applies within a cell. A clause added to a cell after
2026-07-27 is not covered by that date: it carries its own date and its own
verification, constructed the same way as the original eight — by direct
construction against the built binary. Two clauses on this table postdate the
original construction, both added on 2026-09-13:

- the `ridl baseline` exit-1 cell's `reserved`-tombstone clause, added when
  `ridl baseline` gained the RIDL-408 publication gate, verified via
  `baseline_refuses_to_publish_an_untombstoned_removal` in
  `crates/ridl/tests/baseline_gate.rs`, which asserts the refusal exits 1;
- the `ridl check` exit-2 cell's empty-baseline clause, added when an explicit
  `--baseline` holding no snapshot became an input error (driftsys/ridl#235),
  verified via `an_explicit_baseline_holding_no_snapshot_is_an_input_error` in
  `crates/ridl/tests/baseline_desk.rs`, which asserts the refusal exits 2.
```

- [ ] **Step 9: Run the full local gate**

Run: `just build`

Expected: every member passes.

- [ ] **Step 10: Commit**

```bash
test "$(git branch --show-current)" = "baseline-tombstone-gate" || exit 1
git add crates/ridl/src/main.rs crates/ridl/tests/baseline_desk.rs \
        docs/decisions/ADR-0010-cli-conventions.md
git commit -m "$(cat <<'MSG'
fix(ridl): refuse an explicit baseline that holds no snapshot

`load_baseline` already refused a baseline directory holding a non-JSON IR
artifact, and one whose snapshots sit exactly one level below the given path.
Everything else that yielded no snapshot — snapshots two or more levels down, or
an empty directory — returned an empty vector, and the desk check returned early
with no diagnostic. `ridl check --baseline <path>` then exited 0 with no output,
which is indistinguishable from a clean check (driftsys/ridl#235).

An explicit `--baseline` that holds no snapshot is now exit 2 with the cause
named. The caller asserted that a baseline is there, and the tool cannot answer.
This is the last rung of the ladder those two refusals already form, and it is
the failure shape ADR-0010 decision 6 closed for `ridl fmt`.

The search depth does not change. The one-level bound in
`first_nested_snapshot_dir` records itself as deliberate and scoped to
driftsys/ridl#230, and reopening it is a larger change than this defect needs.

Auto-discovery keeps its silent skip: with no flag and no published baseline,
"no baseline yet" is legitimate. A test pins that.

This also gives driftsys/ridl#234's control something to assert that is not
vacuous.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

- [ ] **Step 11: Open the first pull request**

Tasks 1 and 2 are one pull request. Push the branch and open it against `main`.

```bash
git push -u origin baseline-tombstone-gate
```

`gh pr create` has been failing with GitHub GraphQL server errors. Use the REST
endpoint and project the response:

```bash
cat > /tmp/pr.json <<'JSON'
{"title":"fix(ridl): gate baseline publication on the tombstone rule","head":"baseline-tombstone-gate","base":"main","body":"BODY"}
JSON
gh api repos/driftsys/ridl/pulls --method POST --input /tmp/pr.json \
  --jq '{number, html_url, state}'
```

Write the body from the two commit messages, and end it with:

```markdown
🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

---

### Task 3: The composite body reorder category

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs` (the `declare_categories!` invocation at
  `:118-193`, and `category_word` at `:374`)
- Modify: `crates/ridl-diff/src/classify.rs` (`classify` at `:48-99`, `explain`
  at `:935`)
- Modify: `crates/ridl-diff/src/walk.rs:244-253`
- Create: `crates/ridl/tests/diff_member_reorder.rs`
- Modify: `docs/book/cli-reference.md` near line 822

**Interfaces:**

- Consumes: nothing from Tasks 1 or 2. This task is the second pull request and
  can be implemented before, after, or beside them.
- Produces: `Category::MemberReordered`, whose printed word is
  `member_reordered`.

**Background the implementer needs.** `diff_composite` compares a composite
body's member names as an order-independent set. When a name is present on one
side only it emits `DeclAdded` or `DeclRemoved` and sets a local `structural`
flag. When the sets match but the bodies differ it emits one whole-container
`ConstraintChanged`, which is the right verdict reached through the wrong
category: a reader acting on it looks for a constraint edit that does not exist,
and no per-member detail survives (driftsys/ridl#314).

The carried-debt comment above `diff_composite` (lines 190-207) records that the
comparison never reads the body's `reserved` list, and instructs that this must
not be closed alone because driftsys/ridl#302 records a FlatBuffers
union-discriminant coupling that must move with it. **This task does not touch
that.** The sequence comparison below reads no `reserved` list and changes no
removal matching. Leave the comment exactly as it is.

- [ ] **Step 0: Create this task's own worktree and branch**

This task is the second pull request, so it is built on its own branch cut from
`main` rather than on top of Tasks 1 and 2. Create it before the first edit:

```bash
git -C /Users/sebastientasson/Workspace/driftsys/ridl fetch origin
git -C /Users/sebastientasson/Workspace/driftsys/ridl worktree add \
  .claude/worktrees/member-reordered-category \
  -b member-reordered-category origin/main
```

Every step below runs inside `.claude/worktrees/member-reordered-category`.
Never check out a branch in a worktree you did not create, and never run a
formatter in one.

- [ ] **Step 1: Create the test file**

Create `crates/ridl/tests/diff_member_reorder.rs`. Copy the `TempDir` struct
with its `new`, `path`, `write` and `Drop` implementations, the `ridl` helper
and the `MANIFEST` constant **verbatim** from
`crates/ridl/tests/baseline_desk.rs:1-62`. Read those lines and copy what is
there. Then append:

````rust
fn workspace(dir: &TempDir, relative: &str, source: &str) -> PathBuf {
    dir.write(&format!("{relative}/ridl.toml"), MANIFEST);
    dir.write(&format!("{relative}/report.ridl"), source);
    dir.path().join(relative)
}

const FIELDS: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: DoorState
}
";

/// The two fields swap places. Names and types are untouched.
const SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  latch: DoorState
  door: DoorState
}
";

/// `latch` changes type in place. The names and their order are untouched, so
/// this is the branch `ConstraintChanged` must keep.
const CHANGED_IN_PLACE: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: Count
}
";

/// A body gives one order and that order is wire identity, so a swap is as
/// much a wire change as an interaction reorder and deserves the same kind of
/// category. Reporting it as `constraint_changed` sends the reader looking for
/// a constraint that did not change (driftsys/ridl#314).
#[test]
fn a_swapped_struct_field_reports_member_reordered() {
    let dir = TempDir::new("reorder");
    let old = workspace(&dir, "old", FIELDS);
    let new = workspace(&dir, "new", SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered"),
        "the reorder has its own category:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "the fallback no longer stands in for it:\n{out}",
    );
    assert!(
        out.contains("door") && out.contains("latch"),
        "both moved members are named:\n{out}",
    );
}

/// A member changed in place, with the order untouched, is still
/// `constraint_changed`. This pins the branch the new category must not take
/// over — asserting only the absence of `member_reordered` would pass even if
/// the whole branch stopped reporting anything.
#[test]
fn an_in_place_change_still_reports_constraint_changed() {
    let dir = TempDir::new("in-place");
    let old = workspace(&dir, "old", FIELDS);
    let new = workspace(&dir, "new", CHANGED_IN_PLACE);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "narrowing a member in place is breaking:\n{out}");
    assert!(
        out.contains("constraint_changed"),
        "an in-place change keeps the category it always had:\n{out}",
    );
    assert!(
        !out.contains("member_reordered"),
        "nothing moved, so nothing reorders:\n{out}",
    );
}

- [ ] **Step 2: Run the tests to verify the first fails**

Run: `cargo test -p ridl --locked --test diff_member_reorder`

Expected: `a_swapped_struct_field_reports_member_reordered` FAILS — the output
contains `constraint_changed`, not `member_reordered`.
`an_in_place_change_still_reports_constraint_changed` PASSES already; it pins
behaviour this task must not break.

- [ ] **Step 3: Declare the variant**

In `crates/ridl-diff/src/lib.rs`, inside the `declare_categories!` invocation,
directly after the `DeclRemoved` variant, add:

```rust
/// A surviving composite member whose position in the body changed — a
/// struct field, enum value, enum-set bit or union arm. A body gives
/// one order and that order is wire identity (typl §7.4).
MemberReordered,
````

The macro derives the `CATEGORIES` array length from the variant list, so
nothing else in `lib.rs` changes for the count.

- [ ] **Step 4: Add the word**

In `crates/ridl-diff/src/lib.rs`, in `category_word` near line 374, add the arm
in the same position the variant occupies in the enum:

```rust
Category::MemberReordered => "member_reordered",
```

- [ ] **Step 5: Add the classification and the rule row**

In `crates/ridl-diff/src/classify.rs`, add `Category::MemberReordered` to the
always-breaking OR-list in `classify`, directly after
`Category::InteractionReordered`:

```rust
| Category::MemberReordered
```

Then, in `explain`, add the rule row. Match the surrounding style: the existing
rows write section references without the section sign, as `ridl 11`.

```rust
Category::MemberReordered => concat!(
    "A surviving composite member whose position in the body changed.\n",
    "  breaking    always — a body gives one order and that order is wire\n",
    "              identity, so a reorder moves every member after it and\n",
    "              every later member's wire slot with it (typl 7.4)"
),
```

- [ ] **Step 6: Build and fix whatever the deny lint names**

Run: `cargo build -p ridl-diff --locked`

Expected: it compiles. If `category_from_word` matches on string literals rather
than looping over `CATEGORIES`, the build or the round-trip test will name it;
add `"member_reordered" => Some(Category::MemberReordered),` in that case. If it
loops over `CATEGORIES` comparing `category_word`, no edit is needed.

- [ ] **Step 7: Emit it from `diff_composite`**

In `crates/ridl-diff/src/walk.rs`, replace the tail of `diff_composite` — the
`if !structural { emit(...) }` block at lines 244-253 — with:

```rust
if structural {
    return;
}
// The member names match on both sides, so the difference is either a
// reorder of the body or a member changed in place. A reorder is its own
// category: a body gives one order and that order is wire identity, so
// reporting it as a constraint edit sends the reader looking for a
// constraint that did not change (driftsys/ridl#314). This reads no
// `reserved` list and changes no removal matching, so the carried debt
// above — and driftsys/ridl#302's coupling — stays exactly as it is.
if old_names == new_names {
    emit(
        changes,
        path.to_string(),
        Category::ConstraintChanged,
        None,
        None,
    );
    return;
}
for (index, name) in new_names.iter().enumerate() {
    let was = old_names
        .iter()
        .position(|old| old == name)
        .expect("the name sets are equal, so every new name appears in the old body");
    if was != index {
        emit(
            changes,
            format!("{path}/{name}"),
            Category::MemberReordered,
            Some(format!("position {}", was + 1)),
            Some(format!("position {}", index + 1)),
        );
    }
}
```

Leave the doc comment at lines 190-207 untouched.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p ridl --locked --test diff_member_reorder`

Expected: 2 tests PASS.

- [ ] **Step 9: Run the whole diff crate**

Run: `cargo test -p ridl-diff --locked`

Expected: every test PASSES, including
`every_category_has_a_rule_row_naming_its_verdicts` in
`crates/ridl-diff/src/classify/classify_tests.rs`, which asserts that the new
rule row names a verdict and that `member_reordered` round-trips through
`category_from_word`.

If a test in `crates/ridl-diff/src/tests.rs` that asserted `ConstraintChanged`
on a composite body now sees `MemberReordered`, read it: if its fixture reorders
members, the new category is the correct verdict and the test's expectation
should be updated with a comment naming driftsys/ridl#314. If its fixture does
not reorder, the sequence comparison is wrong — fix step 7, not the test.

- [ ] **Step 10: Update the book**

In `docs/book/cli-reference.md`, add `member_reordered` to the diff category
list near line 822, in the same position the variant occupies in the enum —
after the `decl_removed` entry. Match the surrounding indentation exactly.

- [ ] **Step 11: Run the full local gate**

Run: `just build`

Expected: every member passes, including `book-check` and `link-check`.

- [ ] **Step 12: Commit and open the second pull request**

```bash
test "$(git branch --show-current)" = "member-reordered-category" || exit 1
git add crates/ridl-diff/src/lib.rs crates/ridl-diff/src/classify.rs \
        crates/ridl-diff/src/walk.rs crates/ridl/tests/diff_member_reorder.rs \
        docs/book/cli-reference.md
git commit -m "$(cat <<'MSG'
feat(ridl-diff): give a composite body reorder its own category

Swapping two struct fields was classified breaking, which is the right verdict,
but the only category emitted was `constraint_changed` on the whole container,
with no per-member detail. A reader acting on it looks for a constraint edit
that does not exist (driftsys/ridl#314).

`MemberReordered` is emitted once per member whose position moved, carrying the
old and new positions. A body gives one order and that order is wire identity,
so a field swap is as much a wire change as an interaction reorder and now
classifies under the same kind of category. A member changed in place, with the
order untouched, is still `constraint_changed`.

The sequence comparison reads no `reserved` list and changes no removal
matching, so the carried debt above `diff_composite` and the FlatBuffers
union-discriminant coupling driftsys/ridl#302 guards are untouched.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Self-Review

**Spec coverage.** Every decision in the design has a task.

| Spec                                             | Task                                                                |
| ------------------------------------------------ | ------------------------------------------------------------------- |
| D-1 the gate's position and what it reads        | Task 1 steps 4, and its doc comment states the rejected alternative |
| D-2 RIDL-408 is new, RIDL-407 unchanged          | Task 1 steps 3, 8, 9                                                |
| D-3 explicit baseline with no snapshot is exit 2 | Task 2 steps 3, 4                                                   |
| D-4 `MemberReordered`, #302 untouched            | Task 3 steps 3-7                                                    |
| D-5 interaction level only                       | Task 1 step 4's doc comment; no service-level work anywhere         |
| D-6 two pull requests                            | Task 2 step 11, Task 3 step 12                                      |
| §3 refused publish leaves files identical        | Task 1 step 6, test two                                             |
| §3 first publication publishes                   | Task 1 step 6, test three                                           |
| §4 gate admits an append                         | Not covered — see the gap below                                     |
| §6 reference, ADR-0010, cli-reference            | Task 1 steps 9-10, Task 2 step 8, Task 3 step 10                    |

**One gap found and left deliberately.** The design's §4 lists "the gate admits
an append" as a test. It is not in Task 1, because `ridl diff` already has
coverage for `InteractionAppended` and the gate filters on `InteractionRemoved`
alone — an append produces no `InteractionRemoved`, so the test would assert
that a filter filters. `the_first_publication_is_never_refused` and
`baseline_publishes_a_tombstoned_removal` already prove the gate admits a clean
publish. If the reviewer disagrees, add it beside them using the same shape.

**Placeholder scan.** No "TBD", no "handle edge cases", no "similar to Task N".
Two steps direct the implementer to read existing code rather than quoting it:
Task 1 step 1 and Task 3 step 1 say to copy the test harness verbatim from
`baseline_desk.rs:1-62`, and Task 2 step 5 says to fix the call site the
compiler names. Both are determinate — an exact line range to copy, and a
compiler error to resolve — rather than judgement left open.

**Type consistency.** `untombstoned_removals` returns `Result<bool, ExitCode>`
in the interfaces block, the signature and the call site. `baseline_location`
returns `Result<Option<(PathBuf, bool)>, ExitCode>` in the interfaces block and
in both edited arms. `load_baseline(location, explicit)` and
`desk_check(entry, location, explicit, run)` agree between step 4's signatures
and the call in `desk_check`'s body. `Category::MemberReordered` and the word
`member_reordered` are spelled the same in the variant, the word arm, the
`explain` arm, the `walk.rs` emission, both tests and the book.
