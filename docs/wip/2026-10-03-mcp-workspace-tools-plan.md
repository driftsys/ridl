# Workspace-aware MCP tools — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `ridl mcp` from a one-tool checker of pasted text into eight
read-only tools over a workspace on disk, with unsaved-text overlays.

**Architecture:** Overlays are applied inside `ridl-core`'s workspace loader, so
every check and every span sees the overlay text. `ridlc` exposes an
overlay-aware compile and takes over the `ridl diff` side loader from the CLI.
`ridl-mcp` builds one stateless `Snapshot` per call and answers every lookup
with pure functions over the checked IR. `ridl-lsp` is not touched.

**Tech Stack:** Rust (toolchain pinned in `rust-toolchain.toml`), salsa, `rmcp`
3.3.0 (stdio, `#[tool]` macros, `Json<T>` structured output), pbjson serde for
the IR, `serde_json`, `tokio`.

**Spec:**
[`2026-10-03-mcp-workspace-tools-design.md`](2026-10-03-mcp-workspace-tools-design.md).
Read it before Task 1; this plan cites its sections as §N. Where this plan and
the spec disagree, stop and ask.

## Before you start

- Work on a branch from `main` that contains this plan, named
  `feat/1a-mcp-workspace-tools`. Run `./bootstrap` once after creating the
  worktree. One pull request for the whole plan; one commit per task at least.
- Read `AGENTS.md` (commands and conventions), then
  `docs/technotes/walking-skeleton-architecture.md` (which crate owns what).
- The commit scopes allowed are listed in `.git-std.toml`. This plan uses
  `ridl-core`, `ridlc`, `ridl`, `ridl-mcp`, `adr` and `docs`. The commit hook
  rejects any other scope.
- Per task, run the crate's tests (`cargo test -p <crate> --locked`), then
  `cargo fmt --all` and `cargo clippy -p <crate> --all-targets -- -D warnings`.
  Before opening the pull request, run `just verify`; it must pass.

## Global Constraints

- No tool writes a file, and no tool fetches a remote import (D-6). Only
  `ridlc::compile_workspace_with` and `ridlc::load_diff_side` are called; never
  `ridlc::run_check` or anything that reaches `materialize_and_lock`.
- No change to `ridl-lsp`, to the language, to the IR schema, or to any wire
  format.
- `ridl-mcp` must not depend on `ridl-lsp`.
- The source-mode `ridl_check` input `{source, profile}` and its output
  `{diagnostics}` keep their current JSON shape; the existing source-mode tests
  pass unchanged except the one-tool test that Task 4 deletes.
- `ridl check`, `ridl diff` and `ridl diff --explain` keep byte-identical
  stdout, stderr and exit codes. The existing tests in `crates/ridl/tests/`
  (`diff_cli.rs`, `diff_gate.rs`, `diff_member_reorder.rs`, `baseline_desk.rs`,
  `baseline_gate.rs`, `check_json.rs`) pass unchanged.
- Every diagnostic a tool returns is produced by `ridl_core::diag::to_json` and
  converted with `serde_json::to_value`; never built by hand.
- Every `#[tool]` method has the type `Result<CallToolResult, ErrorData>` and
  the attribute
  `#[tool(output_schema =
  rmcp::handler::server::common::schema_for_output::<T>())]`,
  where `T` is the tool's output type. Success is
  `Ok(CallToolResult::structured(serde_json::to_value(&output)?))`. A wrong
  request is `Ok(tool_error.into_result())`, an MCP result with
  `isError:
  true`. Only a compiler panic caught by `spawn_blocking` is
  `Err(ErrorData::internal_error(..))`, as today (spec §6.2). Never return
  `Err(ErrorData)` for a wrong request: it becomes a JSON-RPC protocol error,
  which the agent does not see as a tool result.
- Plain, literal English in comments, commit messages and docs (AGENTS.md).

## Review Focus

1. A relative `path` with an absolute overlay path for the same file, and the
   reverse: the overlay still applies (Task 1, test
   `overlay_matches_a_file_named_by_a_relative_entry`).
2. A bare name that only `ridl.std` declares (`Version`, `Duration`): the lookup
   tools find it, with `location: null` (Task 5, test
   `resolve_finds_a_std_declaration`).
3. A workspace that imports a remote package: the tools answer at once with the
   same `Info` diagnostic `ridl check` gives, and open no network connection
   (Task 4, test `a_remote_import_is_reported_and_not_fetched`).
4. An overlay that empties a file (`source: ""`): no panic, the diagnostics the
   empty file draws, and lookups on other packages still answer (Task 4, test
   `an_empty_overlay_is_checked_like_an_empty_file`).
5. `ridl_diff` with `old` pointing at a missing `.ridl/baseline`: a tool error
   whose text is the CLI's message, not an internal error (Task 7, test
   `diff_with_a_missing_old_side_is_a_tool_error`).

---

## The fixture workspace

Tasks 4 to 8 use this workspace. Task 4 creates it. `ridl check` on it exits 0
with no diagnostic. It has two members, a subdirectory package, every
interaction kind, a cross-package reference, an import alias, and one bare name
(`Level`) that two packages declare.

`crates/ridl-mcp/tests/fixtures/ws/ridl.toml`:

```toml
[workspace]
members = ["a", "b"]
```

`crates/ridl-mcp/tests/fixtures/ws/a/ridl.toml` (and `b/ridl.toml` with
`name = "fx.b"`):

```toml
[package]
name = "fx.a"
version = "1.0.0"
```

`crates/ridl-mcp/tests/fixtures/ws/a/a.ridl`:

```ridl
package fx.a

/// Vehicle speed in kilometres per hour.
type Speed: km/h [0.0..250.0 step 0.5]

type Level: integer [0..100]

/// The health of a subsystem.
enum Health {
  OK = 0
  WARN = 1
  FAIL = 2
}

struct Reading {
  level: Level
  health: Health
}

union Outcome {
  ok: Reading
  fault: Health
}
```

`crates/ridl-mcp/tests/fixtures/ws/a/sub/sub.ridl`:

```ridl
package fx.a.sub

type Gear: integer [0..8]
```

`crates/ridl-mcp/tests/fixtures/ws/b/b.ridl`:

```ridl
package fx.b

import fx.a.Speed
import fx.a.Level as ALevel
import fx.a.Reading
import fx.a.Outcome

/// A window size, local to this package.
type Level: integer [0..10]

/// The status interface.
interface Status {
  signal speed: Speed @10ms
  event reading: Reading @[100ms..1s]
  command setLevel(level: ALevel) @[..50ms]
  query outcome(window: Level): Outcome @[..200ms]
  fixed softwareVersion: Version
}
```

Two variants, each a full copy of `ws/` with one edit set:

- `ws-diag/`: append `type Tag: string` to `a/a.ridl` (draws warning TYPL-103),
  and in `b/b.ridl` change `query outcome(window: Level)` to
  `query outcome(window: Missing)` (draws error TYPL-011).
  `ridl check
  --format json` on it exits 1 with exactly those two diagnostics.
- `ws-v2/`: add `DEAD = 3` as the last value of `enum Health` (compatible), and
  change `command setLevel(level: ALevel)` to
  `command setLevel(level: ALevel, force: Level)` (breaking).
  `ridl diff ws ws-v2 --format json` exits 1.

The fixture files sit under `tests/fixtures/`, so the book-example harness and
the workspace's own `ridl.toml` lookups do not reach them. Confirm with
`cargo run -q -p ridl-cli -- check crates/ridl-mcp/tests/fixtures/ws` (exit 0,
no output) after creating them.

---

### Task 1: Overlays in the workspace loader

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` (`load_workspace` at about line
  69, `Loader` at about 113, `load_package_tree` at about 244, `load_file` at
  about 393)
- Modify: `crates/ridl-core/src/lib.rs` (re-export next to `load_workspace`,
  about line 46)
- Test: the `#[cfg(test)] mod tests` in `crates/ridl-core/src/workspace.rs` (use
  its `TempDir` helper; its `write` creates parent directories)

**Interfaces:**

- Produces, exported at the `ridl_core` crate root:

  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct Overlay { pub path: PathBuf, pub text: String }

  #[derive(Debug)]
  pub enum LoadError {
      Io(io::Error),
      OverlayNotSource(PathBuf),
      OverlayOutsideWorkspace { path: PathBuf, missing_directory: bool },
  }
  // impl Display + std::error::Error for LoadError

  pub fn load_workspace_with(
      db: &mut RidlDatabase,
      entry: &Path,
      overlays: &[Overlay],
  ) -> Result<LoadedWorkspace, LoadError>;
  ```

- `load_workspace(db, entry) -> io::Result<LoadedWorkspace>` keeps its
  signature: it calls `load_workspace_with(db, entry, &[])` and maps
  `LoadError::Io(e)` to `e` (the other variants cannot occur with no overlays).

`LoadError` messages (`Display`), exact text:

- `Io(e)`: `{e}`
- `OverlayNotSource(p)`:
  ``overlay `{p}` is not a `.typl`, `.ridl` or `.rsdl` file``
- `OverlayOutsideWorkspace { missing_directory: true, .. }`:
  ``overlay `{p}` is in a directory that does not exist; create the directory first``
- `OverlayOutsideWorkspace { missing_directory: false, .. }`:
  ``overlay `{p}` is not in a package directory of the workspace loaded from this path``

- [ ] **Step 1: Write the failing tests**

Each test builds a tree with `TempDir`, calls `load_workspace_with`, and runs
`ridlc`-free checks on the result (`loaded.workspace.packages(&db)`, each
package's `files(&db)` paths and `text(&db)`, `loaded.diagnostics` codes,
`loaded.sources`). Tests and their assertions:

- `overlay_replaces_the_text_of_a_file_on_disk`: package `p` with `p/a.typl`
  (`package p` + one type) on disk; overlay for `p/a.typl` with
  `package p\n\ntype Other: integer [0..1]\n`. The loaded `InputFile`'s
  `text(&db)` equals the overlay text, and the `SourceMap` entry for that path
  holds the overlay text, not the disk text.
- `overlay_adds_a_file_to_the_package_of_its_directory`: overlay `p/b.typl`, not
  on disk. Package `p` has two files, in path order `a.typl`, `b.typl`.
- `overlay_in_an_existing_subdirectory_joins_its_package`: `p/sub/` exists on
  disk with `p/sub/c.typl` (`package p.sub`); overlay `p/sub/d.typl` with
  `package p.sub`. Package `p.sub` has both files.
- `an_added_file_with_the_wrong_package_name_draws_typl_002`: overlay `p/b.typl`
  declaring `package q`. `loaded.diagnostics` contains `TYPL-002` with its
  primary span in `b.typl`.
- `an_overlay_in_a_missing_directory_is_refused`: overlay `p/nope/e.typl`.
  `Err(LoadError::OverlayOutsideWorkspace { missing_directory: true, .. })`.
- `an_overlay_outside_the_workspace_is_refused`: overlay for a file in a sibling
  directory of the manifest root that exists on disk.
  `Err(OverlayOutsideWorkspace { missing_directory: false, .. })`.
- `an_overlay_under_a_hidden_directory_is_refused`: `p/.hidden/` exists; overlay
  `p/.hidden/f.typl`. Same error as above.
- `a_non_source_overlay_is_refused`: overlay `p/ridl.toml`.
  `Err(LoadError::OverlayNotSource(_))`.
- `single_file_mode_takes_an_overlay_for_the_entry`: a lone `x.typl` with no
  manifest above it (put the `TempDir` under the system temp directory, as the
  existing single-file tests do); overlay for it replaces its text; an overlay
  for any other path is `OverlayOutsideWorkspace { missing_directory: false }`.
- `overlay_matches_a_file_named_by_a_relative_entry`: load with `entry` given as
  a path relative to the current directory (build it with `pathdiff`-free logic:
  `std::env::current_dir()` and `Path::strip_prefix`; skip the test with an
  early `return` if the temp directory is not under the current directory's
  root) and the overlay path absolute; the overlay applies. (Review Focus 1.)
- `load_workspace_without_overlays_is_unchanged`: the existing loader tests
  still pass; add no assertion here beyond calling
  `load_workspace_with(db,
  root, &[])` and comparing package names and file
  paths with `load_workspace(db, root)` on a fresh database.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p ridl-core --locked workspace::tests::overlay` Expected:
compile errors: `load_workspace_with`, `Overlay`, `LoadError` not found.

- [ ] **Step 3: Implement `load_workspace_with`**

In `Loader`, add the validated overlays and the set of consumed ones. The
overlay key is §5.1 rule 1:

```rust
/// The comparison key for a path: its parent directory canonicalised, joined
/// with its file name. `None` when the parent directory does not exist.
fn overlay_key(path: &Path) -> Option<PathBuf>
```

Make a relative overlay path absolute against `std::env::current_dir()` before
computing the key. Validate every overlay before loading (rule 2, then rule 1's
missing directory). In `load_package_tree`, after `read_dir`, add a path
`dir.join(file_name)` for every overlay whose key's parent equals
`canonicalize(dir)` and whose key matches no file on disk, then sort as today.
In `load_file`, read the text from the overlay whose key equals the file's key,
and mark that overlay consumed; otherwise read the disk. In single-file mode the
same lookup in `load_file` is enough. After the load, the first unconsumed
overlay (in input order) is
`OverlayOutsideWorkspace { missing_directory:
false }`.

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p ridl-core --locked` Expected: all pass, including every
existing loader test.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core
git commit -m "feat(ridl-core): accept unsaved-text overlays in the workspace loader"
```

---

### Task 2: Compile with overlays, and expose manifest imports

**Files:**

- Modify: `crates/ridlc/src/lib.rs` (`WorkspaceOutput` about line 266,
  `compile_workspace` about 332, `load_and_check` about 1142, `check_loaded`
  about 1154, `Compiled`)
- Test: the `#[cfg(test)]` module of `crates/ridlc/src/lib.rs`, or a new
  `crates/ridlc/tests/compile_with_overlays.rs`

**Interfaces:**

- Consumes: `ridl_core::{Overlay, LoadError, load_workspace_with}` (Task 1).
- Produces:

  ```rust
  pub fn compile_workspace_with(
      db: &mut RidlDatabase,
      entry: &Path,
      overlays: &[ridl_core::Overlay],
  ) -> Result<WorkspaceOutput, ridl_core::LoadError>;

  // New public field on WorkspaceOutput, positional like `resolutions`:
  pub imports: Vec<BTreeMap<String, String>>,
  ```

- `compile_workspace(db, entry) -> io::Result<WorkspaceOutput>` keeps its
  signature and calls `compile_workspace_with(db, entry, &[])`.

- [ ] **Step 1: Write the failing tests**

- `an_overlay_reaches_the_checked_ir`: a package on disk declaring
  `type A: integer [0..1]`; overlay adds `type B: integer [0..1]` to the same
  file. `output.checked[0].ir.decls` names `A` and `B`.
- `an_overlay_error_is_a_diagnostic_with_a_span_in_the_overlay`: overlay text
  with an unknown type reference; `output.diagnostics` has one `Error`, and
  `to_json` of it has `span.path` equal to the overlaid file's path and a
  `start.line` that exists only in the overlay text.
- `imports_are_positional_with_checked`: a workspace whose member `b` declares
  `[imports] ext = "https://example.invalid/ext.git"` in its manifest;
  `output.imports[i]` for the index `i` where `checked[i].ir.name == "b"`
  contains key `ext`, and the other member's map is empty.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridlc --locked compile_with` Expected: compile errors for
`compile_workspace_with` and `imports`.

- [ ] **Step 3: Implement**

`load_and_check` takes the overlays and calls `load_workspace_with`.
`check_loaded` pushes `pkg.imports(db).clone()` into a new `imports` vector in
the same loop that fills `resolutions`; `Compiled` and `WorkspaceOutput` carry
it. No other change to `check_loaded`. Update every place that builds a
`WorkspaceOutput` or destructures `Compiled` (the compiler lists them).

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p ridlc --locked` then `cargo test -p ridl-cli --locked`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ridlc
git commit -m "feat(ridlc): compile a workspace with overlays and expose manifest imports"
```

---

### Task 3: Move the diff side loader into `ridlc`

**Files:**

- Modify: `crates/ridl/src/main.rs`: `run_diff` (about line 391), `DiffSide`
  (about 461), `load_diff_side` (about 472) and the helpers only it uses:
  `is_ir_json`, `has_ir_json_name`, `is_non_json_ir`, `is_source_file`,
  `is_source_dir`, `snapshot_files`, `load_snapshots`, `files_matching`,
  `ir_json_files`, `first_non_json_ir_in`, `first_nested_snapshot_dir`,
  `refuse_nested_snapshot_directory`, `refuse_artifact_directory`. Before moving
  a helper, `grep -n` its name in `main.rs`; a helper that another command also
  calls (the baseline commands use some of them) stays in `main.rs` or is made
  `pub` in `ridlc` and called from both.
- Create: `crates/ridlc/src/diff_side.rs` (re-exported from `ridlc`'s root)
- Modify: `crates/ridlc/Cargo.toml` only if a dependency is missing (`ridl-diff`
  is needed if `load_snapshots` keeps using `ridl_diff::load_ir_json`; adding it
  creates no cycle: `ridl-diff` depends only on `ridl-ir`, `serde`,
  `serde_json`)
- Test: `crates/ridlc/src/diff_side.rs` tests module

**Interfaces:**

- Consumes: `compile_workspace_with` (Task 2).
- Produces:

  ```rust
  pub struct DiffSide {
      pub packages: Vec<ridl_ir::v2::Package>,
      pub system: Option<ridl_ir::v2::System>,
  }

  pub enum DiffSideError {
      /// Every refusal the CLI makes today, one variant each, carrying the
      /// paths and values its message needs.
      /* … */
      /// An overlay was given for a side that is not a source path.
      OverlayOnSnapshot(PathBuf),
      /// The overlays were refused by the loader.
      Load(ridl_core::LoadError),
      /// A source side compiled with at least one error-severity diagnostic.
      Compile { diagnostics: Vec<Diagnostic>, sources: SourceMap },
  }
  // impl Display: for every variant except `Compile`, exactly the line the
  // CLI prints today after `error: ` is removed; for `Compile`,
  // "the source side does not compile".

  pub fn load_diff_side(
      db: &mut RidlDatabase,
      path: &Path,
      overlays: &[ridl_core::Overlay],
  ) -> Result<DiffSide, DiffSideError>;
  ```

- The CLI's `run_diff` creates the database, calls
  `ridlc::load_diff_side(db,
  path, &[])`, and on `Err` prints: for `Compile`,
  `ridl_core::diag::render(
  &diagnostics, &sources)` with `eprint!`, as today;
  otherwise `eprintln!("error: {err}")`. Exit code 2 in both cases, as today.
  Copy each message's `format!` string from the current `main.rs` code; do not
  retype them.

- [ ] **Step 1: Write the failing tests** in `diff_side.rs`

- `a_snapshot_file_loads`: write one `.ir.json` by compiling a tiny package with
  `compile_workspace` and `ridl_ir::v2` JSON serialisation
  (`serde_json::
  to_string(&ir)`), then `load_diff_side` on it returns one
  package.
- `a_source_side_with_an_error_is_compile`: a package whose file has an unknown
  type; `Err(DiffSideError::Compile { diagnostics, .. })` with one `Error`.
- `an_overlay_on_a_snapshot_side_is_refused`:
  `Err(DiffSideError::OverlayOnSnapshot(_))`.
- `an_overlay_reaches_a_source_side`: the overlay adds a declaration; the
  returned package's `decls` names it.
- `a_missing_path_is_an_error_whose_text_is_the_cli_text`: `Display` equals
  ``nope: `nope` does not exist`` when called with the relative path `nope` from
  a directory that does not contain it.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridlc --locked diff_side` Expected: compile errors.

- [ ] **Step 3: Move the code and switch the CLI to it**

- [ ] **Step 4: Run the CLI diff tests and the new tests**

Run: `cargo test -p ridlc --locked && cargo test -p ridl-cli --locked` Expected:
all pass, with no change to any file in `crates/ridl/tests/`. If a CLI test
fails, the message or the stream changed: fix the code, not the test.

- [ ] **Step 5: Commit**

```bash
git add crates/ridlc crates/ridl/src/main.rs
git commit -m "refactor(ridlc): move the diff side loader out of the CLI"
```

---

### Task 4: Snapshot, tool errors, and path-mode `ridl_check`

**Files:**

- Create: `crates/ridl-mcp/src/snapshot.rs`, `crates/ridl-mcp/src/types.rs` (the
  shared input and output types)
- Create: the fixture tree (see "The fixture workspace")
- Modify: `crates/ridl-mcp/src/lib.rs` (`CheckParams`, `CheckOutput`, `check`,
  the `ridl_check` tool method, the tests module)
- Modify: `crates/ridl-mcp/Cargo.toml` (add `ridl-ir`, `ridl-sem`, `ridl-diff`
  as workspace dependencies; `ridl-sem` is needed for `Symbol` in Task 5)

**Interfaces:**

- Consumes: `ridlc::compile_workspace_with`,
  `ridl_core::{Overlay,
  LoadError}`.
- Produces, in `types.rs` (outputs derive `Serialize, JsonSchema`, inputs derive
  `Deserialize, JsonSchema`, each with `#[schemars(crate = "rmcp::schemars")]`;
  this applies to every input and output type in Tasks 4 to 7):

  ```rust
  pub struct OverlayInput { pub path: String, pub source: String }
  pub struct Position { pub line: u32, pub column: u32 }
  pub struct Location { pub path: String, pub start: Position, pub end: Position }
  pub struct WorkspaceStatus {
      pub root: String, pub errors: usize, pub warnings: usize, pub notes: Vec<String>,
  }
  ```

- Produces, in `snapshot.rs`:

  ```rust
  pub struct Snapshot {
      pub db: ridl_core::RidlDatabase,
      pub output: ridlc::WorkspaceOutput,
      pub root: PathBuf,
      pub notes: Vec<String>,
  }

  pub enum ToolError {
      /// A wrong request: becomes `CallToolResult::error` with this text.
      Request(String),
      /// A wrong request with data: `CallToolResult::structured_error` of
      /// `{"message": message, …data}`.
      WithData { message: String, data: serde_json::Value },
  }
  impl ToolError { pub fn into_result(self) -> CallToolResult }

  pub fn snapshot(path: &str, overlays: &[OverlayInput]) -> Result<Snapshot, ToolError>;
  impl Snapshot {
      pub fn status(&self) -> WorkspaceStatus;
      /// The 1-based location of `range` in `file`, built with
      /// `ridl_core::diag::line_col`; same shape as `to_json`'s span.
      pub fn location(&self, file: ridl_core::InputFile, range: TextRange) -> Location;
  }
  ```

- `ridl_check`'s new input and output:

  ```rust
  pub struct CheckParams {
      pub source: Option<String>,
      pub profile: Option<Profile>,
      pub path: Option<String>,
      pub overlays: Option<Vec<OverlayInput>>,
  }
  pub struct CheckOutput {
      pub diagnostics: Vec<serde_json::Value>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub workspace: Option<WorkspaceStatus>,
  }
  ```

  The tool method follows the Global Constraints form, with `CheckOutput` as
  `T`. Source mode builds its `CheckOutput` from today's `check` function; the
  text content rmcp writes then parses to the same JSON value as today, which is
  what the existing source-mode tests compare.

Rules:

- `snapshot`: a `path` that does not exist is
  ``Request("`{path}` does not exist")``; a `LoadError` becomes
  `Request(err.to_string())`. The `io::ErrorKind::NotFound` text the loader
  gives for a directory with no manifest is passed through unchanged; append
  `"; pass the workspace root, the directory that holds its ridl.toml"`.
- `root`: the manifest directory that `ridl_core::find_manifest_root` gives for
  the path (for a file, its parent), or the file in single-file mode.
- `notes` (§6.3): when `root/ridl.toml` declares `[package]` and an ancestor of
  `root` holds a `ridl.toml` whose text parses
  (`ridl_core::manifest::
  parse_manifest`) as `[workspace]`, push exactly:
  ``"loaded the package at `{root}` alone, so imports of its sibling workspace members do not resolve (driftsys/ridl#529); pass the workspace root `{ancestor}` as `path` instead"``.
- `status`: `errors` and `warnings` count `output.diagnostics` by severity.
- `ridl_check`: exactly one of `source` and `path`, otherwise
  `Request("pass either`source`with`profile`, or`path`with optional`overlays`")`.
  `profile` is required with `source` and refused with `path`, with the same
  message. Source mode returns `CheckOutput { diagnostics, workspace: None }`
  from today's `check` function, unchanged.

- [ ] **Step 1: Create the fixture** (see "The fixture workspace") and confirm
      `ridl check` exits 0 on `ws/`, 1 on `ws-diag/`.

- [ ] **Step 2: Write the failing tests** in `snapshot.rs` and in `lib.rs`'s
      tests module. Fixture path:
      `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ws")`.

- `snapshot_of_the_fixture_is_clean`: `status().errors == 0`, `warnings == 0`,
  `notes` empty, `output.checked` has three packages (`fx.a`, `fx.a.sub`,
  `fx.b`).
- `path_mode_check_matches_to_json`: on `ws-diag`, the `ridl_check` result's
  `diagnostics` equal `serde_json::to_value(to_json(...))` of
  `ridlc::compile_workspace` on the same path string; codes are
  `["TYPL-103", "TYPL-011"]` in that order.
- `a_member_path_draws_the_529_note`: `snapshot("<ws>/b", &[])` has one note
  that contains `driftsys/ridl#529` and the workspace root path, and its
  `output.diagnostics` contains an error (the import of `fx.a` does not
  resolve).
- `an_overlay_error_is_reported_and_disk_is_unchanged`: overlay on `ws/b/b.ridl`
  with `Speed` replaced by `Missing`; one error; the file on disk still contains
  `signal speed: Speed`.
- `an_empty_overlay_is_checked_like_an_empty_file`: overlay `source: ""` for
  `ws/a/a.ridl`; no panic; the diagnostic codes equal those that
  `ridlc::compile_workspace` gives for a temporary copy of `ws` whose `a/a.ridl`
  is empty. (Review Focus 4.)
- `a_remote_import_is_reported_and_not_fetched`: a temporary copy of `ws` whose
  `b/ridl.toml` adds `[imports]` with `ext = "https://192.0.2.1/ext.git"` (a
  TEST-NET address that does not route) and `b.ridl` adds `import ext.Thing`;
  the call returns within 5 seconds and the diagnostics contain the `Info`
  "remote import" diagnostic. (Review Focus 3.)
- `both_modes_or_neither_is_a_tool_error`: three calls (both, neither, `path`
  with `profile`) each return `isError: true` with the message above.
- Delete `the_server_advertises_exactly_one_tool_named_ridl_check`; Task 8 pins
  the full tool list.

- [ ] **Step 3: Run and see them fail**

Run: `cargo test -p ridl-mcp --locked` Expected: compile errors for the new
modules.

- [ ] **Step 4: Implement** `snapshot.rs`, `types.rs` and the new `ridl_check`.

- [ ] **Step 5: Run and see them pass**

Run:
`cargo test -p ridl-mcp --locked && cargo test -p ridl-cli --locked --test servers`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-mcp
git commit -m "feat(ridl-mcp): check a workspace by path with unsaved overlays"
```

---

### Task 5: Lookup tools — resolve, describe, list interactions

**Files:**

- Create: `crates/ridl-mcp/src/query.rs`
- Modify: `crates/ridl-mcp/src/lib.rs` (three `#[tool]` methods)

**Interfaces:**

- Consumes: `Snapshot`, `ToolError`, `Location`, `WorkspaceStatus`,
  `OverlayInput` (Task 4).
- Produces, in `query.rs`:

  ```rust
  pub struct NameInput {          // shared input of the three tools and Task 6
      pub path: String,
      pub overlays: Option<Vec<OverlayInput>>,
      pub name: String,
      pub from: Option<String>,
  }

  /// A declaration found by name.
  pub struct Found<'a> {
      pub package: String,                 // canonical package name
      pub item: Item<'a>,
      pub symbol: Option<&'a ridl_sem::Symbol>, // None for ridl.std
      pub alias: Option<String>,           // the written name, when it was an alias
  }
  pub enum Item<'a> { Decl(&'a ridl_ir::v2::Decl), Interface(&'a ridl_ir::v2::Interface) }

  pub fn find(snap: &Snapshot, name: &str, from: Option<&str>) -> Result<Found<'_>, ToolError>;

  pub fn resolve(snap: &Snapshot, input: &NameInput) -> Result<ResolveOutput, ToolError>;
  pub fn describe_type(snap: &Snapshot, input: &NameInput) -> Result<DescribeOutput, ToolError>;
  pub fn list_interactions(snap: &Snapshot, input: &NameInput) -> Result<InteractionsOutput, ToolError>;

  pub struct ResolveOutput {
      pub name: String, pub package: String, pub kind: String, pub visibility: String,
      #[serde(skip_serializing_if = "Option::is_none")] pub alias: Option<String>,
      pub location: Option<Location>, pub workspace: WorkspaceStatus,
  }
  pub struct DescribeOutput {
      pub declaration: serde_json::Value,   // serde_json::to_value(&Decl), pbjson form
      pub package: String, pub location: Option<Location>, pub workspace: WorkspaceStatus,
  }
  pub struct InteractionsOutput {
      pub interface: InterfaceHeader,       // name, package, doc, labels, deprecated, number, provisional
      pub interactions: Vec<serde_json::Value>, // serde_json::to_value of each Decl, in order
      pub location: Option<Location>, pub workspace: WorkspaceStatus,
  }
  ```

  `list_interactions` takes `NameInput` with the interface name in `name`; the
  tool's input field is named `interface` (`#[serde(rename = "interface")]` on a
  wrapper type, or a separate input struct with the same fields).

`find` rules (§4.1):

- `pkg.Name` (contains a dot): split at the last dot; search the package of that
  name in `output.checked` and then `output.std_ir`.
- Bare `Name` with `from`: the package index `i` where
  `output.checked[i].ir.name == from` (unknown `from` is
  `Request("no package`{from}`in this workspace; packages: {sorted list}")`);
  look up `output.resolutions[i].symbols[name]`; follow `symbol.package` and
  `symbol.name` to the declaration; `alias` is `Some(name)` when
  `symbol.name != name`.
- Bare `Name` without `from`: every workspace package and `ridl.std` whose IR
  declares `Name` (in `decls` or `interfaces`). One match: found. None:
  `Request` listing up to 10 canonical names that contain `name`, ignoring case,
  sorted. Several: `Request` listing every matching canonical name and saying
  ``pass `pkg.Name` or `from` to choose one``.
- `symbol`: from the resolution of the declaring package, the symbol whose key
  is the declaration's own name and whose `package` is that package. For
  `ridl.std`, `None`, and `location` is `null`.
- `kind` is the `SymbolKind` lowercased (`type`, `const`, `struct`, `enum`,
  `enumset`, `union`, `interface`); for `ridl.std` derive the same words from
  the IR `decl::Kind`. `visibility` is `public` or `internal`.
- `describe_type` on an interface:
  ``Request("`{name}` is an interface; use ridl_list_interactions")``.
- `list_interactions` on a non-interface:
  ``Request("`{name}` is not an interface; use ridl_describe_type")``.
- A package that produced no IR for the found name cannot happen in `find` (it
  searches the IR). If `output.checked` is empty because loading failed,
  `snapshot` already returned the loader's diagnostics; lookups then answer
  `Request("the workspace has errors that stop it from being checked; run ridl_check on the same path")`.

- [ ] **Step 1: Write the failing tests** in `query.rs`, over the fixture:

- `resolve_a_bare_unique_name`: `Speed` → package `fx.a`, kind `type`,
  `location.path` ends with `a/a.ridl`, `location.start.line == 4`.
- `resolve_an_ambiguous_bare_name_lists_candidates`: `Level` → error whose text
  contains `fx.a.Level` and `fx.b.Level`.
- `resolve_a_bare_name_from_a_package_uses_its_view`: `Level` with
  `from: "fx.b"` → package `fx.b`.
- `resolve_an_alias`: `ALevel` with `from: "fx.b"` → package `fx.a`, name
  `Level`, `alias == Some("ALevel")`.
- `resolve_a_canonical_name`: `fx.a.sub.Gear` → package `fx.a.sub`.
- `resolve_finds_a_std_declaration`: `Version` → package `ridl.std`, `location`
  is `None`. (Review Focus 2.)
- `an_unknown_name_suggests_close_names`: lookups are case-sensitive, so `speed`
  is not found; the error lists `fx.a.Speed` as a suggestion, because
  suggestions ignore case.
- `describe_a_struct`: `Reading` → `declaration` has key `structDef` (pbjson
  camelCase oneof) and its members name `level` and `health`; `doc` absent or
  empty.
- `describe_keeps_the_doc`: `Speed` → `declaration.doc` equals
  `"Vehicle speed in kilometres per hour."` (check the exact stored form in the
  IR first; the test asserts what the IR stores).
- `describe_an_interface_is_refused`: `Status` → error naming
  `ridl_list_interactions`.
- `list_every_interaction_kind`: `Status` → five interactions in source order,
  whose single oneof keys are `signalDef`, `eventDef`, `commandDef`, `queryDef`,
  `fixedDef`; ordinals 1 to 5.

- [ ] **Step 2: Run and see them fail**: `cargo test -p ridl-mcp --locked query`

- [ ] **Step 3: Implement `query.rs` and the three tool methods.** Each method
      runs `snapshot` and the query in `spawn_blocking` and returns in the
      Global Constraints form.

- [ ] **Step 4: Run and see them pass**: `cargo test -p ridl-mcp --locked`

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-mcp
git commit -m "feat(ridl-mcp): add the resolve, describe and list-interactions tools"
```

---

### Task 6: Review tools — references and dependencies

**Files:**

- Create: `crates/ridl-mcp/src/refs.rs`
- Modify: `crates/ridl-mcp/src/lib.rs` (two `#[tool]` methods)

**Interfaces:**

- Consumes: `find`, `NameInput`, `Snapshot` (Tasks 4, 5).
- Produces:

  ```rust
  /// Every canonical type reference one declaration holds, each qualified with
  /// `own_package` when written bare. Interactions report their own name.
  pub fn references_of(own_package: &str, item: Item<'_>) -> Vec<(Option<String>, String)>;

  pub fn references(snap: &Snapshot, input: &NameInput) -> Result<ReferencesOutput, ToolError>;
  pub fn dependencies(snap: &Snapshot, input: &DependenciesInput) -> Result<DependenciesOutput, ToolError>;

  pub struct ReferencesOutput { pub target: String, pub references: Vec<Reference>, pub workspace: WorkspaceStatus }
  pub struct Reference { pub package: String, pub declaration: String, pub interaction: Option<String>, pub location: Option<Location> }
  pub struct DependenciesInput { pub path: String, pub overlays: Option<Vec<OverlayInput>>, pub package: Option<String> }
  pub struct DependenciesOutput { pub packages: Vec<PackageDeps>, pub workspace: WorkspaceStatus }
  pub struct PackageDeps { pub name: String, pub imports: Vec<String>, pub depends_on: Vec<String>, pub dependents: Vec<String> }
  ```

`references_of` walks exactly the fields §4.3 lists (`ridl_references`). A
reference string is canonical when it contains a dot whose prefix names a
package of the workspace or `ridl.std`; otherwise it is bare and is qualified as
`{own_package}.{name}`. A primitive or inline scalar has no reference. A unit
string (`TypeDef.backing.unit`) is not a reference for this tool.

`references`: resolve the target with `find`, then collect, for each workspace
package and each declaration and interface, the pairs whose reference equals the
target; one `Reference` per (declaration, interaction) pair; sort by package,
declaration, interaction. `location` is the enclosing package-level
declaration's symbol location.

`dependencies`: `depends_on` of a package is the sorted set of package prefixes
of its references, excluding itself and `ridl.std`; `dependents` is the inverse;
`imports` is the sorted keys of `output.imports[i]`. With `package`, only that
package; unknown package is
`Request("no package`{p}`in this workspace; packages: {sorted list}")`.

- [ ] **Step 1: Write the failing tests** in `refs.rs`, over the fixture:

- `references_to_a_struct`: `Reading` → references `(fx.a, Outcome, null)`,
  `(fx.b, Status, reading)`; `(fx.a, Reading, …)` is absent (a declaration does
  not refer to itself).
- `references_through_an_alias`: `fx.a.Level` → contains `(fx.a, Reading, null)`
  and `(fx.b, Status, setLevel)`; and `fx.b.Level` → only
  `(fx.b, Status, outcome)`.
- `a_declaration_referring_twice_is_reported_once`: overlay `a.ridl` adding
  `struct Pair { x: Health  y: Health }` (check the struct field syntax used in
  `Reading`); `Health` references contain `Pair` exactly once.
- `dependencies_of_the_fixture`: `fx.b.depends_on == ["fx.a"]`,
  `fx.a.dependents == ["fx.b"]`, `fx.a.sub` has both empty, no package lists
  `ridl.std`.
- `dependencies_of_an_unknown_package_is_an_error`.

- [ ] **Step 2: Run and see them fail**: `cargo test -p ridl-mcp --locked refs`

- [ ] **Step 3: Implement `refs.rs` and the two tool methods**
      (`ridl_references` takes `NameInput`; `ridl_dependencies` takes
      `DependenciesInput`).

- [ ] **Step 4: Run and see them pass**: `cargo test -p ridl-mcp --locked`

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-mcp
git commit -m "feat(ridl-mcp): add the references and dependencies tools"
```

---

### Task 7: `ridl_explain` and `ridl_diff`

**Files:**

- Create: `crates/ridl-mcp/src/explain.rs`, `crates/ridl-mcp/src/diff.rs`
- Modify: `crates/ridl-mcp/src/lib.rs` (two `#[tool]` methods)

**Interfaces:**

- Consumes: `ridl_core::diag::ALL_CATALOGS` (`&[(&str, &[CatalogEntry])]`,
  `CatalogEntry { code, severity, summary }`),
  `ridl_diff::{category_from_word,
  category_word, explain}`,
  `ridlc::{load_diff_side, DiffSideError, std_ir}`,
  `ridl_diff::{diff_workspaces, render_json}`, `ToolError`, `OverlayInput`.
- Produces:

  ```rust
  pub struct ExplainInput { pub code: String }
  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum ExplainOutput {
      Diagnostic { code: String, severity: String, summary: String },
      DiffCategory { category: String, text: String },
  }
  pub fn explain(input: &ExplainInput) -> Result<ExplainOutput, ToolError>;

  pub struct DiffInput { pub old: String, pub new: String, pub overlays: Option<Vec<OverlayInput>> }
  pub fn diff(input: &DiffInput) -> Result<serde_json::Value, ToolError>;
  ```

  `ridl_diff`'s output schema is
  `schema_for_output::<serde_json::Map<String, serde_json::Value>>()`, and the
  structured result is the object `render_json` writes, parsed back with
  `serde_json::from_str`.

Rules:

- `explain`: `severity` is the lower-case form `to_json` uses for the same
  `Severity`. An unknown input:
  ``Request("`{code}` is neither a diagnostic code (FORM-, TYPL-, RIDL-, RSDL-, MANI-) nor a ridl diff category word")``.
  Matching is exact (case-sensitive), like `ridl diff --explain`.
- `diff`: `load_diff_side` for `old` with no overlays, then for `new` with the
  overlays;
  `diff_workspaces(&old.packages, old.system.as_ref(),
  &new.packages, new.system.as_ref(), &[ridlc::std_ir()])`;
  `render_json`. `DiffSideError::Compile` is
  `ToolError::WithData { message: "the {old|new}
  side does not compile", data: {"diagnostics": to_json values} }`;
  every other `DiffSideError` is `Request(err.to_string())`.

- [ ] **Step 1: Write the failing tests**

- `explain_a_diagnostic_code`: `TYPL-002` → `Diagnostic` with severity `error`
  and the catalogue's summary.
- `explain_a_diff_category`: `payload_changed` → `DiffCategory` whose text
  equals `ridl_diff::explain(category_from_word("payload_changed").unwrap())`.
- `explain_an_unknown_code_is_an_error`: `TYPL-9999` and `nonsense`.
- `diff_matches_the_cli`: `diff(ws, ws-v2)` equals
  `serde_json::from_str(render_json(...))` computed in the test with the same
  calls, and its `verdict` is `"breaking"` (check the exact word `render_json`
  writes).
- `diff_with_an_overlay_on_new`: `old = ws`, `new = ws`, overlay `ws/a/a.ridl`
  adding `DEAD = 3` to `Health` → verdict compatible, not identical.
- `diff_with_a_missing_old_side_is_a_tool_error`: `old = "<ws>/.ridl/baseline"`
  (absent) → `Request` whose text equals the CLI's message for a missing path
  without its `error:` prefix. (Review Focus 5.)
- `diff_with_a_side_that_does_not_compile_carries_diagnostics`:
  `new =
  ws-diag` → `WithData` whose `data.diagnostics` has codes `TYPL-103`,
  `TYPL-011`.

- [ ] **Step 2: Run and see them fail**:
      `cargo test -p ridl-mcp --locked explain diff`

- [ ] **Step 3: Implement both modules and tool methods.**

- [ ] **Step 4: Run and see them pass**: `cargo test -p ridl-mcp --locked`

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-mcp
git commit -m "feat(ridl-mcp): add the explain and diff tools"
```

---

### Task 8: End-to-end tests and the pinned tool list

**Files:**

- Modify: `crates/ridl/tests/servers.rs` (uses the rmcp child-process client
  already; reuse its helpers)
- Create: `crates/ridl-mcp/tests/tools.json` (the pinned `tools/list` result)
- Modify: `crates/ridl-mcp/src/lib.rs` tests module (the pin test)

**Interfaces:**

- Consumes: the eight tools as built in Tasks 4 to 7. Tool names, exactly:
  `ridl_check`, `ridl_explain`, `ridl_resolve`, `ridl_describe_type`,
  `ridl_list_interactions`, `ridl_references`, `ridl_dependencies`, `ridl_diff`.

- [ ] **Step 1: Write the tests**

In `lib.rs` tests:

- `the_tool_list_is_pinned`: serialise the server's tool list (the same list
  `tools/list` returns, through `tool_router`) with `serde_json::to_value`, sort
  by name, and compare with `include_str!("../tests/tools.json")` parsed. On
  mismatch the assertion message prints the actual JSON pretty and says: "the
  MCP tool surface is an external contract (ADR-0005 §7); if this change only
  adds, replace tests/tools.json with the JSON above". Create `tests/tools.json`
  from the first run's output, then read it once to confirm every tool has an
  `outputSchema` and an input schema with the fields this plan names.

In `servers.rs`, against the real `ridl mcp` binary:

- `path_mode_check_equals_the_cli`: `ridl_check { path: <ws-diag> }` structured
  `diagnostics` equal the array `ridl check --format json
  <ws-diag>` prints,
  element for element, with the same path string on both sides.
- `diff_tool_equals_the_cli`: `ridl_diff { old: <ws>, new: <ws-v2> }` equals the
  object `ridl diff --format json <ws> <ws-v2>` prints.
- `every_tool_leaves_the_tree_unchanged`: copy `ws` into a temp directory;
  record every file's relative path, size and modification time (recursive);
  call each of the eight tools once on the copy (use `Speed`, `Status`, and
  `old = new = copy`); record again; the two records are equal, and no
  `ridl.lock` or `.ridl/` appears.
- `a_wrong_request_is_is_error_not_a_protocol_error`: `ridl_resolve` with an
  unknown name returns a result with `is_error == Some(true)`, not a JSON-RPC
  error.

- [ ] **Step 2: Run**:
      `cargo test -p ridl-mcp --locked && cargo test -p
      ridl-cli --locked --test servers`.
      Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ridl-mcp crates/ridl/tests/servers.rs
git commit -m "test(ridl-mcp): pin the tool surface and compare with the CLI end to end"
```

---

### Task 9: Server instructions, registration and documentation

**Files:**

- Modify: `crates/ridl-mcp/src/lib.rs` (`get_info` instructions string, about
  line 173)
- Create: `.mcp.json`
- Modify: `crates/ridl-mcp/README.md`, `docs/book/cli-reference.md` (section
  `ridl mcp`, about line 1504), `docs/decisions/ADR-0005-agent-enablement.md`
  (`## Status` and the tool table at about line 120)

- [ ] **Step 1: Instructions string.** Replace it with text that names the eight
      tools in one line each and says: "Pass the workspace root (the directory
      that holds its ridl.toml) as `path`. Tools are read-only: they never write
      files and never fetch remote imports. Use `overlays` to check unsaved
      text." Update any test that asserts the old string.

- [ ] **Step 2: `.mcp.json`** at the repository root, exactly:

```json
{
  "mcpServers": {
    "ridl": {
      "command": "ridl",
      "args": ["mcp"]
    }
  }
}
```

- [ ] **Step 3: `crates/ridl-mcp/README.md`.** One subsection per tool: its
      input fields, its result fields, and its tool errors, taken from spec §4
      and §6. Add the §7.1 list of what path-mode `ridl_check` does not report
      and why, and the root-discovery note (#529). Keep the existing
      host-configuration section.

- [ ] **Step 4: `docs/book/cli-reference.md`, section `ridl mcp`.** Replace the
      sentence that names one tool with the list of eight tools, one line each,
      and a link to the crate README for the schemas. The command still takes no
      flag.

- [ ] **Step 5: ADR-0005.** In the minimum tool table, change the inputs to
      paths (`ridl_check(path | source)`, the others `(path, name)`), add rows
      for `ridl_references` (the declarations that use a declaration) and
      `ridl_dependencies` (each package's dependencies and dependents), and
      change "interactions with kinds" to name the IR kinds `signal`, `event`,
      `command`, `query`, `fixed`. In `## Status`, add one paragraph: "Amended
      2026-10-03 by the workspace-aware MCP tools design (piece 1a): the tools
      take a workspace path with optional unsaved overlays, and
      `ridl_references` and `ridl_dependencies` are added." Leave the status
      itself as Proposed.

- [ ] **Step 6: Gates.** Run `prim fmt` on the changed Markdown and JSON, then
      `just check`, `just link-check`, `just doc-path-check`, `just book-check`.
      Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add .mcp.json crates/ridl-mcp docs/book/cli-reference.md docs/decisions/ADR-0005-agent-enablement.md
git commit -m "docs(ridl-mcp): document the workspace tools and register the server"
```

---

## Finish

- [ ] Run `just verify`. Expected: exit 0. A failure in a gate this plan did not
      touch is reported in the pull request, not worked around.
- [ ] Open one pull request to `main` titled
      `feat(ridl-mcp): workspace-aware MCP tools (1a)`. Its body lists the
      tasks, the Review Focus items and the test that covers each, and says that
      ROADMAP E8.6 and E8.7 are marked delivered by the gardening pass, not by
      this pull request. Do not write a closing keyword for any issue.
