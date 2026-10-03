# Workspace-aware MCP tools — design for spec 1a

Status: design spec for piece 1a of
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md),
written 2026-10-03 against `main` at 29b221a. Sebastien agreed the approach in
the brainstorming session of 2026-10-03 (decisions D-1 to D-6, §2). Nothing here
is implemented. It is archived with its plan when the work lands.

Satisfies: the brief's piece 1a; ROADMAP E8.6 (verify and evolve tools) and E8.7
(grounding and IR-query tools); ADR-0005 §3 (Layer B, the MCP server).

## 1. The problem

`ridl mcp` exposes one tool, `ridl_check(source, profile)`
(`crates/ridl-mcp/src/lib.rs`). It checks one pasted source string against
`ridl.std` only. An import of any other package does not resolve, and every span
names a synthetic file (`input.ridl`). An assistant that works on a real
workspace therefore cannot:

- check the workspace on disk, or check an edit before writing it;
- look up a declaration, a type or an interface that exists in the workspace, so
  it guesses names instead of citing them;
- see which declarations and packages depend on a given one;
- see what a change breaks, which `ridl diff` answers on the command line;
- read what a diagnostic code means.

ADR-0005 §3 names the minimum tool set (`ridl_check`, `ridl_explain`,
`ridl_diff`, `ridl_describe_type`, `ridl_list_interactions`, `ridl_resolve`).
Only the first exists, and only in its pasted-source form.

## 2. Decisions taken in the brainstorming session

- **D-1. Stateless calls behind one seam.** Each tool call loads and checks the
  workspace from disk, applies the overlays, and answers. No cache. Every tool
  that takes `path` goes through one function (`snapshot`, §5.3), so a cache can
  be added there later without a schema change; `ridl_diff` loads its two sides
  through `ridlc::load_diff_side` (§5.2). Measured on 2026-10-03:
  `ridl check examples/cabin` takes less than 10 ms. The condition that reopens
  a cache is a real workspace whose snapshot takes more than 500 ms.
- **D-2. Root discovery is unchanged.** A `path` resolves to a workspace exactly
  as `ridl check <path>` resolves it (`ridl_core::load_workspace`, nearest
  `ridl.toml`). The gap that a file inside a workspace member loads that member
  alone is #529, and it is fixed there for the CLI, the language server and the
  MCP together. 1a only reports it (§6.3).
- **D-3. The tools read the checked IR, not the language server.** The lookup
  logic is a `query` module in `ridl-mcp` over `ridlc`'s `WorkspaceOutput`, and
  it returns structured JSON. `ridl-lsp` is not changed and is not a dependency.
- **D-4. `ridl_explain` answers from the existing catalogues.** It explains a
  diagnostic code from `ridl_core::diag::ALL_CATALOGS` and a diff category from
  `ridl_diff::explain`. The long-form error index (E4.2) is not written here.
- **D-5. One `ridl_check`, two modes, additive.** Today's `{source, profile}`
  input stays unchanged. `{path, overlays}` is added beside it.
- **D-6. Read-only and offline.** No tool writes a file, and no tool fetches a
  remote import.

## 3. Scope

In scope:

- eight tools: `ridl_check` (extended), `ridl_explain`, `ridl_resolve`,
  `ridl_describe_type`, `ridl_list_interactions`, `ridl_references`,
  `ridl_dependencies`, `ridl_diff`;
- an overlay-aware loader in `ridl-core` and two public functions in `ridlc`:
  compiling a workspace with overlays, and loading one side of a diff (§5.1,
  §5.2);
- a `.mcp.json` at the repository root;
- the documentation listed in §9.

Out of scope, and why:

- a workspace cache — D-1;
- the #529 root-discovery change — D-2;
- the long-form error index (E4.2) — D-4;
- lints, the lint registry and SARIF — spec 0 and piece 1b;
- any change to `ridl-lsp`;
- the skill and its evals — piece 1c;
- a CLI subcommand for each tool — E8.8, parked;
- documentation extracted into the IR (spec 2b). The lookup tools return the
  `doc` strings the IR already carries, and return more when 2a and 2b land.

## 4. The tools

### 4.1 Common rules

- **`path`.** Every tool except `ridl_explain` takes `path`: a workspace root
  directory, a package directory, or a source file. A relative path resolves
  against the server's working directory, which the host sets to the project
  directory. The tool descriptions tell the agent to pass the workspace root.
- **`overlays`.** Every tool that takes `path` also takes an optional
  `overlays: [{path, source}]`: unsaved text for a file. §5.1 gives the rules.
- **Output.** Every tool returns its result as MCP structured content with an
  output schema (`CallToolResult::structured`, with the schema set on the
  `#[tool]` attribute), and rmcp also writes the same JSON as the text content
  for hosts that ignore structured content.
- **Workspace status.** Every result built from a workspace carries
  `workspace: {root, errors, warnings, notes}`. `root` is the manifest directory
  that was loaded (or the file, in single-file mode). `errors` and `warnings`
  count the diagnostics of the loaded workspace. `notes` is a list of strings,
  defined in §6.3.
- **Names.** A `name` input is either a canonical name, `pkg.Name` (the package
  name is everything before the last dot), or a bare `Name`. A bare name is
  searched for in the declarations of every workspace package and of `ridl.std`.
  With the optional `from` input (a package name), a bare name resolves the way
  it resolves inside that package, through `Resolution.symbols`, so an import
  alias resolves too. §6.2 covers no match and several matches.
- **Locations.** A location is `{path, start, end}` with `start` and `end` as
  `{line, column}`, 1-based, end exclusive: the same shape as `span` in
  `ridl check --format json`. Its range is the range of the declared name, as
  `Resolution.symbols` records it (`Symbol.range`). A declaration that is not a
  symbol (an interaction, a field) reports the location of its enclosing
  package-level declaration. A service has no symbol, so a reference held by a
  service has `location: null`.

### 4.2 The tool table

| Tool                     | Input besides `path` and `overlays`                                                         | Result                                                                                                                                                                      |
| ------------------------ | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl_check`             | none; or, instead of `path`, today's `{source, profile}`                                    | `{diagnostics, workspace}`; each diagnostic is exactly what `ridl_core::diag::to_json` writes. In source mode the result is `{diagnostics}`, unchanged from today.          |
| `ridl_explain`           | `code`: a diagnostic code (`TYPL-002`) or a diff category word (`payload_changed`); no path | `{kind: "diagnostic", code, severity, summary}` or `{kind: "diff_category", category, text}`                                                                                |
| `ridl_resolve`           | `name`; optional `from`                                                                     | `{name, package, kind, visibility, location, workspace}`, plus `alias` when the input was an import alias                                                                   |
| `ridl_describe_type`     | `name`; optional `from`                                                                     | `{declaration, package, location, workspace}`; `declaration` is the IR `Decl` in the IR's own JSON form                                                                     |
| `ridl_list_interactions` | `interface`; optional `from`                                                                | `{interface: {name, package, doc, labels, deprecated, number, provisional}, interactions, location, workspace}`; `interactions` is the interface's IR `Decl` list, in order |
| `ridl_references`        | `name`; optional `from`                                                                     | `{target, references: [{package, declaration, interaction, location}], workspace}`                                                                                          |
| `ridl_dependencies`      | optional `package`                                                                          | `{packages: [{name, imports, depends_on, dependents}], workspace}`                                                                                                          |
| `ridl_diff`              | `old`, `new`: an `.ir.json` file, a snapshot directory, or a source path                    | the object `ridl diff --format json` prints: `{verdict, changes, …}`; overlays apply to `new` only                                                                          |

### 4.3 Details per tool

**`ridl_check`.** Exactly one of `source` and `path` is given; both or neither
is a tool error. `profile` is required with `source` and refused with `path`. In
path mode the diagnostics are the ones `ridlc::compile_workspace` produces for
the overlaid workspace. They differ from `ridl check <path>` in the ways §7.1
lists, and in no other way.

**`ridl_explain`.** A diagnostic code is looked up in
`ridl_core::diag::ALL_CATALOGS`, and the result carries only what the binary
owns: the code, its severity and its one-line summary. It carries no link to a
document. A URL breaks when a chapter moves, and an agent often cannot fetch
one. Where to read more about each namespace is stated in the skill (piece 1c,
[`skill-ridl-authoring-outline.md`](skill-ridl-authoring-outline.md) §7), which
is prose and changes together with the documents. When the long-form error index
(E4.2) lands, it is added as an `explanation` field. A word that is not a code
is looked up with `ridl_diff::category_from_word`, and `text` is
`ridl_diff::explain(category)`. An input that is neither is a tool error that
lists the five namespaces and says that diff category words are also accepted.

**`ridl_resolve`.** `kind` is the `SymbolKind` in lower case (`type`, `const`,
`struct`, `enum`, `enumset`, `union`, `interface`). `visibility` is `public` or
`internal`.

**`ridl_describe_type`.** It accepts any package-level declaration that is not
an interface. For an interface it returns a tool error that names
`ridl_list_interactions`. `declaration` is serialised with the IR's canonical
protobuf JSON form (the pbjson serde impls in `ridl-ir`, the form `.ir.json`
snapshots use), so the agent reads the same field names that a snapshot carries,
and every field the IR gains later (spec 2b) appears with no change to this
tool.

**`ridl_list_interactions`.** Each interaction is the IR `Decl` of the
interaction. Its kind is one of the IR's five interaction kinds: `signal`,
`event`, `command`, `query`, `fixed`, or a `reserved` tombstone. `ordinal`,
`timing`, `params`, `contracts` and the return or payload type are the IR's
fields, unchanged.

**`ridl_references`.** The target is resolved as `ridl_resolve` resolves it, to
a canonical `pkg.Name`. The tool then walks every declaration of every workspace
package and reports each one that holds a type reference equal to the target.
The IR writes a reference in the same package as a bare `Name` and a reference
across packages as `pkg.Name` (`ir.proto`), so a bare reference is qualified
with its own package before the comparison. The walk covers every field that
holds a type reference: `FieldType.named` (recursively through arrays, maps,
tuples, streams and inline scalars), `UnionArm.type_ref`, `ConstDef.type_ref`,
`EnumSetDef.backing_enum`, `Constraint.pattern_const`, `SignalDef.payload`,
`EventDef.payload`, `StreamType.named`, `FallibleType.ok` and `.err`, parameter
types, return types, `FixedDef.payload`, and in each service of
`Package.services` every `ServiceShape.interface_ref` and every reference inside
an inline `ServiceShape.inline` interface. A reference inside a service reports
the service's name as its declaration. Each reference reports the package, the
package-level declaration that holds it, the interaction name when the reference
sits inside an interface's interaction (otherwise `null`), and the location. A
declaration that refers to the target more than once is reported once.
`ridl.std` is not walked: a standard type cannot refer to a workspace type.

**`ridl_dependencies`.** For each workspace package:

- `imports` is the import names its manifest declares (`Package.imports`);
- `depends_on` is the sorted set of other packages its IR refers to, taken from
  the `pkg.Name` references the `ridl_references` walk finds, excluding
  `ridl.std`, which every package imports implicitly;
- `dependents` is the sorted set of workspace packages whose `depends_on`
  contains it.

With `package`, the result holds that one package; an unknown package is a tool
error that lists the workspace's packages. The tool reports the graph. It does
not judge it: cycles and unused imports are lints, piece 1b.

**`ridl_diff`.** `old` and `new` accept what `ridl diff <old> <new>` accepts,
through the same loader (§5.2), and the result is what `ridl_diff::render_json`
writes for the same inputs, parsed back into a JSON value. `std_ir` is passed as
context, as the CLI does (#598). A comparison against the published baseline is
`old: "<root>/.ridl/baseline"`. A source side that does not compile is a tool
error whose structured content carries the diagnostics (§6.2).

## 5. Where the code lives

### 5.1 `ridl-core` and `ridlc`: compiling with overlays

Overlays are applied **inside the loader**, not after it. The loader enforces
the package-and-directory law (TYPL-001, TYPL-002, TYPL-010) and interns each
file's text for diagnostic spans while it reads the file, and it makes every
subdirectory its own package. An overlay applied after the load would skip those
checks and leave spans pointing at the text on disk.

`ridl-core` gains an overlay-aware entry point; `load_workspace` keeps its
signature and calls it with no overlays:

```rust
pub struct Overlay {
    pub path: PathBuf,
    pub text: String,
}

pub fn load_workspace_with(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[Overlay],
) -> Result<LoadedWorkspace, LoadError>;

pub enum LoadError {
    Io(io::Error),
    OverlayNotSource(PathBuf),
    OverlayOutsideWorkspace { path: PathBuf, missing_directory: bool },
}
```

`ridlc` gains the matching compile entry point; `compile_workspace` keeps its
signature and calls it with no overlays. It runs the same passes as today
(`check_loaded`, the `ridl.std` check, `lower_system`) and adds no checking
logic:

```rust
pub fn compile_workspace_with(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[ridl_core::Overlay],
) -> Result<WorkspaceOutput, ridl_core::LoadError>;
```

`WorkspaceOutput` gains one public field,
`imports: Vec<BTreeMap<String,
String>>`: each checked package's manifest
`[imports]` map (`Package::imports`), positional like `resolutions`. It is what
`ridl_dependencies` reports as `imports`, and nothing else in `WorkspaceOutput`
carries it.

The overlay rules:

1. Paths are compared by their **key**: the canonicalised parent directory
   joined with the file name. The parent must exist; the file need not. An
   overlay whose parent directory does not exist is `OverlayOutsideWorkspace`,
   and its message says to create the directory first.
2. An overlay path must end in `.typl`, `.ridl` or `.rsdl`; otherwise
   `OverlayNotSource`.
3. When the loader reads a source file whose key matches an overlay, it uses the
   overlay text instead of the disk text. The overlay text is what is parsed,
   checked and interned for spans.
4. When the loader lists a package directory, it adds every overlay whose parent
   key is that directory and whose file is not on disk. The added paths are
   sorted together with the files on disk, as the loader sorts them, so a new
   file joins the package of its directory exactly as the same file saved to
   disk would.
5. In single-file mode the entry file is the only file read; an overlay for it
   replaces its text.
6. Every overlay the loader did not consume in rules 3 to 5 is
   `OverlayOutsideWorkspace`: its directory is not a package directory of the
   loaded workspace (outside the root, hidden, a separate package root with its
   own `ridl.toml`, or not loaded in single-file mode).

### 5.2 `ridlc`: loading a diff side

The function that turns a `ridl diff` argument into IR is moved out of
`crates/ridl/src/main.rs` (`load_diff_side` and its private helpers) into
`ridlc`:

```rust
pub struct DiffSide {
    pub packages: Vec<ridl_ir::v2::Package>,
    pub system: Option<ridl_ir::v2::System>,
}

pub fn load_diff_side(
    db: &mut RidlDatabase,
    path: &Path,
    overlays: &[Overlay],
) -> Result<DiffSide, DiffSideError>;
```

`DiffSideError` has one variant for each refusal the CLI makes today (a missing
path, a non-JSON IR file, a nested snapshot directory, an artifact directory,
and the others in `main.rs`), a variant for an overlay error, and a variant that
carries the diagnostics and the `SourceMap` of a source side that did not
compile. Its `Display` writes the exact message the CLI prints today. The CLI's
`run_diff` calls the moved function and prints the error, and its output and
exit codes are byte-identical to today's (the existing `ridl diff` tests are the
proof). Overlays are accepted only when the side is a source path; otherwise
they are refused with an error. The snapshot parse uses
`ridl_diff::load_ir_json` today, so `ridlc` may gain a dependency on
`ridl-diff`; no cycle is created, because `ridl-diff` depends only on `ridl-ir`,
`serde` and `serde_json`.

### 5.3 `ridl-mcp`

- `snapshot(path, overlays) -> Result<Snapshot, ToolError>` builds a
  `RidlDatabase`, calls `ridlc::compile_workspace_with`, and returns
  `Snapshot { db: RidlDatabase, output: WorkspaceOutput, root, notes }`. The
  database stays in the snapshot because a `Symbol`'s file path and text are
  salsa inputs read through it. `snapshot` is the only place a tool that takes
  `path` reads the disk (D-1).
- `query` module: one pure function per lookup tool, from `&Snapshot` and the
  tool's input to the tool's result type. No MCP types appear in it, so each
  function is unit-tested directly.
- `lib.rs`: one `#[tool]` method per tool. Each runs `snapshot` and the query in
  `tokio::task::spawn_blocking`, as `ridl_check` does today, and converts a
  `ToolError` into an MCP tool error.
- New dependencies: `ridl-diff`, `ridl-ir`, `ridl-sem` (for `Symbol`) and
  `rowan` (for `TextRange`). `ridl-lsp` is not a dependency.
- The server's `instructions` string is rewritten to name the tools and to say
  "pass the workspace root as `path`".

## 6. Errors

### 6.1 A workspace with problems is not a tool error

`ridl_check` reports the diagnostics. A lookup tool answers from the IR that was
produced, and `workspace.errors` and `workspace.warnings` tell the agent the
tree is not clean. If the package a lookup needs produced no IR, the tool
returns a tool error that names the package and tells the agent to run
`ridl_check` on the same `path`.

### 6.2 A wrong request is a tool error

A tool error is an MCP result with `isError: true` and a message that says what
to change. The cases:

| Case                                                  | Message content                                                                        |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `path` does not exist                                 | the path                                                                               |
| no `ridl.toml` at or above a directory `path`         | the path, and that a workspace root holds a `ridl.toml`                                |
| `ridl_check` given both or neither mode               | the two accepted input forms                                                           |
| an overlay outside the workspace or not a source file | the overlay path and the rule (§5.1)                                                   |
| a name with no match                                  | the name, and up to 10 declared canonical names that contain it, ignoring case, sorted |
| a bare name with several matches                      | every matching canonical name, and that `pkg.Name` or `from` selects one               |
| `describe_type` given an interface                    | that `ridl_list_interactions` describes interfaces                                     |
| an unknown code or category                           | the five namespaces, and that diff category words are accepted                         |
| a `ridl_diff` side that cannot be loaded              | the `DiffSideError` message, the same text the CLI prints                              |
| a `ridl_diff` source side that does not compile       | the message, with the diagnostics in the error's structured content                    |

A compiler panic inside `spawn_blocking` stays an MCP internal error, as today.

### 6.3 Notes

`workspace.notes` carries one note in 1a. When `path` loaded a package manifest
and a `ridl.toml` that declares a `[workspace]` exists in a directory above it,
the note says that the member was loaded alone, that its sibling members do not
resolve (#529), and names the workspace root to pass instead. Other notes may be
added later; the field is a list of strings, so an addition is not a schema
change.

## 7. Compatibility and the contract

### 7.1 Path-mode `ridl_check` against `ridl check`

For the same tree, the two give the same diagnostics, each one equal field for
field, except for these, which only `ridl check` reports, because they need the
network, the lockfile round trip or the baseline:

- MANI-101 to MANI-104 (fetch and lock failures) and the lockfile read and write
  diagnostics of `materialize_and_lock`;
- the desk check against `.ridl/baseline`: RIDL-407 and the rename hint on
  RIDL-409. The agent gets the same information from
  `ridl_diff(old: "<root>/.ridl/baseline", new: "<root>")`.

A remote import resolves in neither: `compile_workspace` reports the same `Info`
diagnostic ("remote import … is not yet available") that `ridl check` reports.

The `ridl-mcp` README states this list.

### 7.2 The tool surface is an external contract

Tool names, input schemas and output schemas are an external contract. ADR-0005
§7 names three contract surfaces (the coded diagnostics, the `ridl diff`
categories and the IR); this design adds the MCP tool surface as a fourth, and
the ADR-0005 amendment (§9) records it. Later changes may only add to them: a
new optional input, a new output field, a new tool. A test pins the whole
`tools/list` response (§8), so any schema change appears as a diff in review.
Source-mode `ridl_check` keeps today's input fields and returns the same JSON
value. Its input schema changes in two ways: `source` and `profile` become
optional, because `path` mode omits them, and `path` and `overlays` are added.
Every call that was valid stays valid.

## 8. Testing

All tests are Rust tests, so `just test` runs them.

- **`ridl-core` unit tests** for `load_workspace_with`: replace a file's text,
  and the diagnostic spans point into the overlay text; add a new file in a
  package; add a new file in an existing subdirectory, which joins that
  subdirectory's package; an added file whose package name does not match its
  directory draws TYPL-002; refuse a path outside the workspace; refuse a path
  in a directory that does not exist; refuse a non-source extension; single-file
  mode.

- **`ridlc` unit tests** for `compile_workspace_with` (an overlay reaches the
  checked IR) and for `load_diff_side`: each `DiffSideError` variant, and
  overlays refused on a snapshot side.
- **`ridl-mcp` unit tests** for each `query` function, over a fixture workspace
  under `crates/ridl-mcp/tests/fixtures/` that has two member packages, one
  cross-package reference, one import alias, one interface with at least one
  interaction of each kind the grammar allows, and a struct, an enum, a union
  and a constrained scalar with a unit. The cases include each row of §6.2's
  table that a query function decides.
- **End-to-end tests** through the real binary, in
  `crates/ridl/tests/servers.rs` (the rmcp child-process client it already
  uses):
  - path-mode `ridl_check` on the fixture gives the same diagnostics as
    `ridl check --format json` on the same path, compared field for field, on a
    fixture variant that has at least one error and one warning;
  - `ridl_diff` on two fixture variants gives the same JSON value as
    `ridl diff --format json`;
  - an overlay that introduces an error is reported, and the file on disk is
    unchanged;
  - read-only: after one call of each tool, the fixture tree's list of files,
    their sizes and their modification times are unchanged;
  - the `tools/list` response is compared with a checked-in JSON file; the test
    tells the reader to update that file deliberately when the contract changes
    additively;
  - the existing `ridl_check` source-mode tests keep their assertions on the
    returned JSON; tests that build `CheckParams` or `CheckOutput` in Rust, or
    assert the input schema's required fields are updated to the new types; the
    tests that assert a one-tool list are replaced by the pinned list of eight
    tools.
- **The CLI `ridl diff` tests** pass unchanged after §5.2's move.

## 9. Documentation and registration

- `.mcp.json` at the repository root:
  `{"mcpServers": {"ridl": {"command": "ridl", "args": ["mcp"]}}}`. It assumes
  `ridl` is on `PATH`, as `crates/ridl-mcp/README.md` already assumes.
- `crates/ridl-mcp/README.md`: every tool, its input and its result; the §7.1
  list; the note about the workspace root.
- `docs/book/cli-reference.md`, section `ridl mcp`: the tool list replaces the
  sentence that names one tool. The command still takes no flag.
- ADR-0005: the minimum tool table names the inputs as paths, adds
  `ridl_references` and `ridl_dependencies`, names the IR's five interaction
  kinds, and §7 adds the MCP tool surface as a fourth contract surface. The
  amendment is recorded in its `## Status`.
- ROADMAP E8.6 and E8.7: marked as delivered by this work when it lands (by the
  gardening pass, not by the implementation).

## 10. Alternatives considered

- **A cached session per workspace root (the language server's model).**
  Rejected for 1a (D-1). The language server's session logic is private to
  `ServerState`, and `load_workspace` creates new salsa inputs on each call, so
  a cache means a second copy of that bookkeeping (changed, new and deleted
  files, manifest edits) before a measurement shows it is needed. The seam in
  §5.3 keeps the option open.
- **An explicit `ridl_open` session handle.** Rejected: it adds a step that
  models often get wrong, and its only benefit is the cache D-1 defers.
- **`ridl-mcp` depends on `ridl-lsp`** and calls `nav` and `hover`. Rejected
  (D-3): those functions are keyed by cursor offset and return Markdown, most of
  the rendering is private, and the dependency brings `lsp-server` and
  `lsp-types` into the MCP crate.
- **A new shared crate (`ridl-query`) for the LSP and the MCP.** Deferred: it
  changes `ridl-lsp` with no user benefit in 1a. It becomes the right move when
  the language server needs lookups by name.
- **Write the long-form error index inside 1a.** Rejected (D-4): it is writing
  across the 147 catalogued codes, not wrapping compiler functions.
- **A `reference` URL for each namespace in the `ridl_explain` result.**
  Rejected: a chapter that moves breaks the URL, which is a tool contract under
  §7.2, and the FORM and MANI codes have no book chapter at all. The skill
  carries the mapping instead.
- **Explain with an excerpt of the reference prose.** Rejected: the excerpt
  boundaries are guesses, and the build would depend on the Markdown in `docs/`.
- **Path mode only for `ridl_check`.** Rejected (D-5): it breaks the documented
  contract and removes checking a draft before a workspace exists.
- **A separate `ridl_check_path` tool.** Rejected (D-5): two tools that do the
  same work, which models choose between badly.
- **Walk up to the enclosing workspace in the MCP only.** Rejected (D-2): the
  MCP and the CLI would then disagree about the same path.
