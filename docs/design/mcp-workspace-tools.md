# The MCP workspace tools: as built

`ridl mcp` serves nine read-only tools over a workspace on disk. This record
describes them as built: the common rules, the tools, where the code lives, the
errors, the compatibility with `ridl check`, and the tests. The code and its
tests are the source of truth. The binding choices are in
[ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md) (and
[ADR-0005](../decisions/ADR-0005-agent-enablement.md) §3 and §7 for the tool set
and the contract), and the inputs and outputs of each tool are in
[the `ridl-mcp` README](../../crates/ridl-mcp/README.md), which this record does
not repeat. Built by driftsys/ridl#668 and #677; roadmap stories E8.6 and E8.7.

## 1. The tools in one table

| Tool                     | Input besides `path` and `overlays`                                | Result                                                                       |
| ------------------------ | ------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `ridl_check`             | none; or, instead of `path`, `{source, profile}`                   | `{diagnostics, workspace}`; in source mode `{diagnostics}`                   |
| `ridl_explain`           | `code`: a diagnostic code or a diff category word; no path         | a diagnostic entry or a diff category entry                                  |
| `ridl_resolve`           | `name`; optional `from`                                            | package, kind, visibility, location, and `alias` when one was used           |
| `ridl_describe_type`     | `name`; optional `from`                                            | the IR `Decl` of a declaration that is not an interface                      |
| `ridl_list_interactions` | `interface`; optional `from`                                       | the interface header and its interactions in source order                    |
| `ridl_references`        | `name`; optional `from`                                            | each declaration, interaction or component that uses the target              |
| `ridl_dependencies`      | optional `package`                                                 | each package's imports, dependencies and dependents                          |
| `ridl_metrics`           | none; takes `path` only                                            | package fan-in, fan-out, instability and interface cohesion groups           |
| `ridl_diff`              | `old`, `new`: an `.ir.json` file, a snapshot directory or a source | what `ridl diff --format json` prints; overlays apply to a source `new` only |

## 2. Common rules

- **`path`.** Every tool except `ridl_explain` and `ridl_diff` takes `path`: a
  workspace root directory, a package directory or a source file. A relative
  path resolves against the server's working directory. The tool descriptions
  and the server's `instructions` string tell the agent to pass the workspace
  root.
- **`overlays`.** Every path tool except `ridl_metrics`, and `ridl_diff`, takes
  optional `overlays: [{path, source}]`, the unsaved text of a file (§4.1).
  `ridl_metrics` takes only `path` and reads saved source.
- **Output.** Every tool returns MCP structured content with an output schema,
  and the same JSON as text content for hosts that ignore structured content.
- **Workspace status.** A result built from a workspace carries
  `workspace: {root, errors, warnings, notes}`. `errors` and `warnings` count
  the diagnostics of the loaded workspace; `notes` is a list of strings (§5.3).
- **Names.** A `name` is a canonical name `pkg.Name`, or a bare `Name` that is
  searched in the declarations of every workspace package and of `ridl.std`.
  With `from` (a package name) a bare name resolves as it does inside that
  package, through `Resolution.symbols`, so an import alias resolves too.
- **Locations.** A location is `{path, start, end}` with `start` and `end` as
  1-based `{line, column}`, end exclusive, the shape of `span` in
  `ridl check --format json`. Its range is the declared name (`Symbol.range`). A
  declaration that is not a symbol (an interaction, a field) reports its
  enclosing package-level declaration. A service, a component and a standard
  declaration have a null location.

## 3. The tools in detail

The behaviour that is not in the README:

- **`ridl_describe_type`** serialises the `Decl` with the IR's canonical
  protobuf JSON form (the pbjson serde impls in `ridl-ir`, the form `.ir.json`
  snapshots use), so a field the IR gains later appears with no change to the
  tool.
- **`ridl_list_interactions`** returns each interaction as the IR `Decl`, whose
  kind is one of `signal`, `event`, `command`, `query`, `fixed`, or a `reserved`
  tombstone.
- **`ridl_references`** resolves the target as `ridl_resolve` does, to a
  canonical `pkg.Name`. It walks the canonical JSON of every declaration, every
  interface and every service of every workspace package, and collects each
  string that is a type reference. The IR writes a reference in the same package
  as a bare `Name` and a reference across packages as `pkg.Name`, so a bare
  reference is qualified with its own package before the comparison. Results are
  reported once for each (declaration, interaction) pair. `ridl.std` is not
  walked: a standard type cannot refer to a workspace type. `kind` is
  `declaration` (a package-level declaration other than an interface),
  `interface`, `service` or `component`. A reference inside a service reports
  the service's name as its declaration.
- **rsdl component uses.** `ridl_references` and `ridl_dependencies` also read
  the lowered system, `WorkspaceOutput.system`, because components exist only
  there. For each component whose `package` is not empty, each `Require` whose
  interface is present and not inline is a reference to `{catalog}.{name}`,
  reported once per component, with `package` the component's package,
  `declaration` its name, and a null interaction and location. Inline requires
  are skipped because their service is walked in its package; offers, links and
  routes are skipped because they derive from the requires; an implicit
  component (empty `package`) is skipped because its service is walked. In
  `ridl_dependencies`, a component's package depends on the `catalog` of each
  such interface, and the system's package depends on the package of each
  component its member lines name. The exclusions are the same as for an IR
  reference: the package itself and `ridl.std`.
- **`ridl_dependencies`** takes `imports` from `WorkspaceOutput.imports`, the
  member manifest's `[imports]` map (not the workspace root's shared
  `[imports]`), and `depends_on` from the package qualifiers of the reference
  walk plus the rsdl edges above. A qualifier that names no workspace package
  stays as written. `dependents` is computed over the whole workspace before a
  `package` filter is applied. The tool reports the graph; a package import
  cycle is already an error (TYPL-004) and an unused import is the
  `unused-import` lint; neither is a tool result.
- **`ridl_metrics`** applies `ridlc::deps::workspace_package_edges` to the
  complete `package_edges` graph. Its `dependsOn`, fan-in, fan-out and
  instability use workspace targets only; the dependency tool retains external
  qualifiers. Instability is fan-out divided by the sum of fan-in and fan-out,
  or null when both are zero. Each declared interface reports its total member
  count and `ridlc::cohesion_groups`, without a threshold. Standard types do not
  link members; members with no remaining named type are omitted from groups.
  Packages and interfaces are sorted by canonical name. Members within groups
  are sorted, and groups follow their earliest member in source order. Metrics
  are independent of lint levels and return the common workspace status.
- **`ridl_diff`** accepts what `ridl diff <old> <new>` accepts, through
  `ridlc::load_diff_side`, and returns what `ridl_diff::render_json` writes,
  with the standard IR passed as context (driftsys/ridl#598). A comparison
  against the published baseline is `old: "<root>/.ridl/baseline"`.
- **`ridl_explain`** reports, for a diagnostic code, `lint` and `default_level`
  when the code is a lint (ADR-0024 decision 15 and its Consequences). It never
  carries a link to a document.

## 4. Where the code lives

### 4.1 `ridl-core` and `ridlc`: compiling with overlays

`ridl_core::load_workspace_with(db, entry, overlays)` takes
`Overlay { path, text }` values and returns a `LoadedWorkspace`;
`load_workspace` calls it with none. `LoadError` has three variants: `Io`,
`OverlayNotSource(path)` and
`OverlayOutsideWorkspace { path, missing_directory }`. The rules:

1. Paths compare by key: the canonicalised parent directory joined with the file
   name. The parent must exist; the file need not. A parent that does not exist
   is `OverlayOutsideWorkspace` with `missing_directory`, and its message says
   to create the directory first. The canonicalisation runs only when there is
   an overlay.
2. An overlay path must end in `.typl`, `.ridl` or `.rsdl`.
3. A source file whose key matches an overlay is read from the overlay text,
   which is what is parsed, checked and interned for spans.
4. A package directory listing adds every overlay whose parent is that directory
   and whose file is not on disk, sorted together with the disk files, so a new
   file joins the package of its directory as if it had been saved.
5. In single-file mode the entry file is the only file read, and an overlay for
   it replaces its text.
6. An overlay that rules 3 to 5 did not consume is `OverlayOutsideWorkspace`:
   its directory is outside the root, hidden, a separate package root, or not
   loaded in single-file mode.

`ridlc::compile_workspace_with(db, entry, overlays)` runs the passes of
`compile_workspace`, which calls it with none. `WorkspaceOutput.imports` holds
each checked package's manifest `[imports]` map, positional like `resolutions`.

`ridlc::load_diff_side(db, path, overlays)` turns a `ridl diff` argument into a
`DiffSide { packages, system }`. `DiffSideError` has a variant for each refusal
the command line makes, one for an overlay error, and one that carries the
diagnostics and the source map of a source side that did not compile; its
`Display` is the message `ridl diff` prints, and `run_diff` in
`crates/ridl/src/main.rs` calls the function, so its output and exit codes did
not change. Overlays are accepted only when the side is a source path. The
helpers that the baseline commands in `main.rs` also need
(`first_nested_snapshot_dir`, `first_non_json_ir_in`, `ir_json_files`,
`is_non_json_ir`, `is_source_dir` and `snapshot_files`) exist once, in
`ridlc::diff_side`, with the others that module makes public. The helpers whose
message depends on the caller (`refuse_nested_snapshot_directory`,
`refuse_artifact_directory` and the parse remedy of `load_snapshots`) stay in
`main.rs`. `ridlc` depends on `ridl-diff` for the snapshot parse; `ridl-diff`
depends only on `ridl-ir`, `serde` and `serde_json`, so there is no cycle.

### 4.2 `ridl-mcp`

- `snapshot(path, overlays)` in `snapshot.rs` builds the `RidlDatabase`, calls
  `compile_workspace_with`, and returns `Snapshot { db, output, root, notes }`.
  The database stays in the snapshot because a `Symbol`'s file and text are
  salsa inputs read through it. It does not apply `[lints]` levels, so the
  lookup tools keep the emitted severities; `ridl_check` in path mode applies
  them (ADR-0024 decision 8).
- `query.rs` holds the lookups (`find`, `resolve`, `describe_type`,
  `list_interactions`), `refs.rs` the reference and dependency walks,
  `metrics.rs` the package and interface metrics, and `explain.rs`, `diff.rs`
  and `types.rs` the rest. These hold no MCP type and are unit-tested directly.
  The lookups and walks are functions from a `&Snapshot` and the tool input to
  the tool's result type. `explain()` and `diff()` take no `Snapshot`, and
  `diff()` reads the disk through `load_diff_side`.
- `lib.rs` has one `#[tool]` method per tool. Each runs its work (`snapshot` and
  the query, or for `ridl_explain`, `ridl_diff` and source-mode `ridl_check` the
  function without a snapshot) in `tokio::task::spawn_blocking`, and converts a
  `ToolError` into an MCP tool error. A compiler panic stays an MCP internal
  error.
- `ridl-lsp` is not a dependency. The crate depends on `ridl-core`, `ridlc`,
  `ridl-diff`, `ridl-ir`, `ridl-sem` (for `Symbol`), `rowan` (for `TextRange`),
  `rmcp`, `serde`, `serde_json` and `tokio`.
- `.mcp.json` at the repository root registers the server for Claude Code:
  `command: "ridl"`, `args: ["mcp"]`. It assumes `ridl` is on `PATH`.

## 5. Errors

### 5.1 A workspace with problems is not a tool error

`ridl_check` reports the diagnostics. A lookup tool answers from the IR that was
produced, and `workspace.errors` and `workspace.warnings` tell the agent the
tree is not clean. When the workspace has errors and nothing was checked, the
lookup returns a tool error that tells the agent to run `ridl_check` on the same
`path`, and it names no package. When a `from` name resolves to a package with
no checked IR, the error names that package and gives the same advice. A
canonical `pkg.Name` whose package has no IR gets the unknown-name error, with
no `ridl_check` advice.

### 5.2 A wrong request is a tool error

A tool error is an MCP result with `isError: true` and a message that says what
to change. The cases: a `path` that does not exist; a directory with no
`ridl.toml` at or above it (the message says a workspace root holds one);
`ridl_check` given both modes, neither, a source without a profile, a path with
a profile, or a source with overlays (the message gives the two accepted input
forms); an overlay outside the workspace or not a source file; a name with no
match (the message lists up to 10 declared canonical names that contain it,
ignoring case, sorted, and ends after the name when there are none); a bare name
with several matches (every canonical name, and that `pkg.Name` or `from`
selects one); `ridl_describe_type` given an interface; `ridl_list_interactions`
given a declaration that is not an interface (the message points to
`ridl_describe_type`); a `from` or `ridl_dependencies` `package` that names no
workspace package (the message lists the packages); a declaration with no kind
(the message advises `ridl_check`); an unknown code or category (the five
namespaces, and that diff category words are accepted); a `ridl_diff` side that
cannot be loaded (the `DiffSideError` message, the text the CLI prints); and a
`ridl_diff` source side that does not compile (the message identifies the old or
new side, and the diagnostics are in the error's structured content).

### 5.3 Notes

`workspace.notes` carries two notes. The first: when `path` loaded a package
manifest and a `ridl.toml` with a `[workspace]` table exists above it, the note
says the member was loaded alone, that its sibling members do not resolve
(driftsys/ridl#529), and names the workspace root to pass instead. The second:
when the workspace has `.rsdl` files and no system was lowered, the note says
that rsdl uses were not counted, because the workspace declares no `system` or
an error in its closure blocked the lowering, and tells the agent to run
`ridl_check`. The field is a list of strings, so a later note is not a schema
change.

## 6. Compatibility with `ridl check`

For the same tree, path-mode `ridl_check` and `ridl check --format json` give
the same diagnostics, each equal field for field, except for what only the
command line reports because it needs the network, the lockfile round trip or
the baseline: MANI-101 to MANI-104 and the lockfile read and write diagnostics
of `materialize_and_lock`, and the desk check against `.ridl/baseline` (RIDL-407
and the rename hint on RIDL-409). `ridl_diff` with
`old: "<root>/.ridl/baseline"` and `new: "<root>"` gives the same information. A
remote import resolves in neither, and both report the same Info diagnostic. The
crate README states this list.

## 7. The tool surface as a contract

Tool names, input schemas and output schemas change only by addition
([ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md) decision 9).
Source-mode `ridl_check` keeps its input fields and returns the same JSON value;
its schema makes `source` and `profile` optional and adds `path` and `overlays`,
and every call that was valid stays valid. `crates/ridl-mcp/tests/tools.json` is
the pinned `tools/list` response.

## 8. Tests

All are Rust tests, so `just test` runs them.

- `ridl-core` unit tests for `load_workspace_with`: replace a file, add a file
  to a package and to an existing subdirectory, a new file whose package name
  does not match its directory (TYPL-002), a path outside the workspace, a
  directory that does not exist, a non-source extension, single-file mode, the
  sort order of an added file, and that an overlay replaces rather than adds.
- `ridlc` unit tests for `compile_workspace_with` and for each `DiffSideError`
  variant, including overlays refused on a snapshot side.
- `ridl-mcp` unit tests for each lookup, over the fixtures in
  `crates/ridl-mcp/tests/fixtures/` (`ws`, `ws-diag`, `ws-lints`, `ws-rsdl`,
  `ws-v2`), with two member packages, a cross-package reference, an import
  alias, an interface with every interaction kind, and the declaration kinds the
  lookups report. Outputs that no other test checks are pinned: visibility,
  every kind word, the interface header, complete locations, references through
  backing enums and pattern constants, and the root of a file path.
- End-to-end tests in `crates/ridl/tests/servers.rs`, through the built binary
  and the rmcp client: path-mode `ridl_check` agrees with
  `ridl check --format json` on a fixture with one error and one warning;
  `ridl_diff` agrees with `ridl diff --format json`; an overlay that introduces
  an error is reported and the file on disk is unchanged; after one call of each
  tool the fixture's files, sizes and modification times are unchanged; and
  `tools/list` names the nine tools. The unit test `the_tool_list_is_pinned` in
  `crates/ridl-mcp/src/lib.rs` pins the response to `tools.json`.
- Inline tests in `metrics.rs` pin workspace-only edges against the dependency
  tool, null instability for a disconnected package, cohesion group order, empty
  interfaces, and unchanged file contents, sizes and modification times after a
  handler call. The tool-list unit test pins the additive ninth tool.
- The `ridl diff` tests of `crates/ridl/tests/` pass unchanged after the move of
  the diff loader.
